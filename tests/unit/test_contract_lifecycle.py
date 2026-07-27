"""Luật vòng đời hợp đồng — app/services/lifecycle.py.

Bao phủ mọi cặp chuyển trạng thái (hợp lệ lẫn bị từ chối) và các chứng từ
bắt buộc, để không ai lặng lẽ nới lỏng bảng luật mà không có test nào đỏ.
"""

from __future__ import annotations

from datetime import date, timedelta

import pytest


def _lifecycle():
    from app.services import lifecycle
    return lifecycle


def _status():
    from app.models.contract import ContractStatus
    return ContractStatus


# ─── Bảng chuyển trạng thái ──────────────────────────────────────────────────

def test_every_status_has_an_entry_in_the_transition_table(db_engine):
    """Trạng thái thiếu trong bảng sẽ im lặng thành ngõ cụt."""
    lifecycle, ContractStatus = _lifecycle(), _status()
    for st in ContractStatus:
        assert st in lifecycle.VALID_TRANSITIONS, f"{st.value} không có trong VALID_TRANSITIONS"


def test_no_transition_targets_a_deprecated_status(db_engine):
    """ExpiringSoon không còn được ghi — không bước chuyển nào được trỏ tới nó."""
    lifecycle = _lifecycle()
    for source, targets in lifecycle.VALID_TRANSITIONS.items():
        overlap = targets & lifecycle.DEPRECATED_STATUSES
        assert not overlap, f"{source.value} vẫn còn chuyển tới trạng thái đã bỏ: {overlap}"


def test_terminated_is_terminal(db_engine):
    lifecycle, ContractStatus = _lifecycle(), _status()
    assert lifecycle.VALID_TRANSITIONS[ContractStatus.Terminated] == set()


ALLOWED = [
    ("Draft", "Generated"),
    ("Generated", "Sent"),
    ("Generated", "Signed"),
    ("Sent", "Signed"),
    ("Signed", "Invoiced"),
    ("Signed", "Terminated"),
    ("Invoiced", "PaidActive"),
    ("Invoiced", "Terminated"),
    ("PaidActive", "Expired"),
    ("PaidActive", "Terminated"),
    ("Expired", "Terminated"),
]

REJECTED = [
    # Nhảy cóc qua các bước phê duyệt
    ("Draft", "Signed"),
    ("Draft", "PaidActive"),
    ("Generated", "PaidActive"),
    ("Generated", "Invoiced"),
    ("Sent", "PaidActive"),
    ("Signed", "PaidActive"),      # phải qua Invoiced trước
    # Đi lùi
    ("Signed", "Sent"),
    ("Invoiced", "Signed"),
    ("PaidActive", "Invoiced"),
    ("Expired", "PaidActive"),
    # Từ trạng thái kết thúc
    ("Terminated", "Signed"),
    ("Terminated", "Expired"),
]


@pytest.mark.parametrize("source,target", ALLOWED)
def test_allowed_transitions_pass_validation(db_engine, make_contract, source, target):
    lifecycle, ContractStatus = _lifecycle(), _status()
    contract = make_contract(status=ContractStatus(source))
    # Cấp sẵn chứng từ để test này chỉ kiểm bảng luật, không kiểm chứng từ.
    contract.invoice_pdf_path = "uploads/invoice_docs/x.pdf"
    contract.payment_slip_path = "uploads/payment_slips/x.pdf"

    assert lifecycle.validate_transition(contract, target) == ContractStatus(target)


@pytest.mark.parametrize("source,target", REJECTED)
def test_rejected_transitions_raise(db_engine, make_contract, source, target):
    lifecycle, ContractStatus = _lifecycle(), _status()
    contract = make_contract(status=ContractStatus(source))
    contract.invoice_pdf_path = "uploads/invoice_docs/x.pdf"
    contract.payment_slip_path = "uploads/payment_slips/x.pdf"

    with pytest.raises(ValueError, match="Không thể chuyển"):
        lifecycle.validate_transition(contract, target)


def test_unknown_status_string_is_rejected(db_engine, make_contract):
    lifecycle, ContractStatus = _lifecycle(), _status()
    contract = make_contract(status=ContractStatus.Draft)
    with pytest.raises(ValueError, match="Trạng thái không hợp lệ"):
        lifecycle.validate_transition(contract, "Nonsense")


# ─── Chứng từ bắt buộc ───────────────────────────────────────────────────────

def test_invoiced_requires_an_invoice_file(db_engine, make_contract):
    lifecycle, ContractStatus = _lifecycle(), _status()
    contract = make_contract(status=ContractStatus.Signed)
    assert not contract.invoice_pdf_path

    with pytest.raises(ValueError, match="hóa đơn"):
        lifecycle.validate_transition(contract, ContractStatus.Invoiced)

    contract.invoice_pdf_path = "uploads/invoice_docs/hd.pdf"
    assert lifecycle.validate_transition(contract, ContractStatus.Invoiced)


def test_paid_active_requires_a_payment_slip(db_engine, make_contract):
    lifecycle, ContractStatus = _lifecycle(), _status()
    contract = make_contract(status=ContractStatus.Invoiced)
    assert not contract.payment_slip_path

    with pytest.raises(ValueError, match="ủy nhiệm chi"):
        lifecycle.validate_transition(contract, ContractStatus.PaidActive)

    contract.payment_slip_path = "uploads/payment_slips/unc.pdf"
    assert lifecycle.validate_transition(contract, ContractStatus.PaidActive)


def test_update_status_records_an_event(db_engine, db_session, make_contract):
    """Mọi lần đổi trạng thái thành công đều phải để lại dấu vết."""
    from app.models.contract_event import ContractEvent
    from app.services.contract_service import update_status
    ContractStatus = _status()

    contract = make_contract(status=ContractStatus.Draft)
    update_status(db_session, contract.id, "Generated", actor="tester", note="ghi chú")

    events = (
        db_session.query(ContractEvent)
        .filter(ContractEvent.contract_id == contract.id,
                ContractEvent.event_type == "status_changed")
        .all()
    )
    assert len(events) == 1
    assert "Draft" in events[0].description and "Generated" in events[0].description
    assert "ghi chú" in events[0].description
    assert events[0].actor == "tester"


def test_update_status_rejects_missing_contract(db_engine, db_session):
    from app.services.contract_service import update_status

    with pytest.raises(LookupError):
        update_status(db_session, 999_999, "Generated")


# ─── Hết hạn: dẫn xuất, không phải trạng thái ────────────────────────────────

@pytest.mark.parametrize("days_out,expected", [
    (-10, "expired"),
    (0, "expired"),      # hết hạn hôm nay = đã hết hiệu lực
    (1, "critical"),
    (7, "critical"),
    (8, "warning"),
    (14, "warning"),
    (15, "info"),
    (30, "info"),
    (31, None),
    (60, None),
    (365, None),
])
def test_expiry_bucket_boundaries(db_engine, days_out, expected):
    lifecycle = _lifecycle()
    today = date(2026, 7, 27)
    assert lifecycle.expiry_bucket(today + timedelta(days=days_out), today) == expected


def test_expiry_helpers_tolerate_a_missing_end_date(db_engine):
    lifecycle = _lifecycle()
    assert lifecycle.expiry_bucket(None) is None
    assert lifecycle.days_to_expiry(None) is None
    assert lifecycle.is_expiring_soon(None) is False


def test_contract_expiry_properties_match_the_helpers(db_engine, make_contract):
    """Template đọc c.expiry_bucket / c.is_expiring_soon — phải khớp lifecycle."""
    contract = make_contract(end_date=date.today() + timedelta(days=5))
    assert contract.days_to_expiry == 5
    assert contract.expiry_bucket == "critical"
    assert contract.is_expiring_soon is True

    far = make_contract(end_date=date.today() + timedelta(days=200))
    assert far.expiry_bucket is None
    assert far.is_expiring_soon is False
