"""Cảnh báo hết hạn không được đụng vào trạng thái thanh toán.

Lỗi gốc: bản quét tự động đổi `Signed → ExpiringSoon` khi hợp đồng còn ≤30 ngày.
`ExpiringSoon` không có lối đi tới `Invoiced`, nên hợp đồng đã ký bước vào 30 ngày
cuối bị khoá — không bao giờ xuất hoá đơn được nữa. Bug này im lặng: nó chỉ nổ ra
khi ai đó thử xuất hoá đơn cho một hợp đồng sắp hết hạn.
"""

from __future__ import annotations

from datetime import date, timedelta

import pytest


def _status():
    from app.models.contract import ContractStatus
    return ContractStatus


def _sweep(db_session):
    from app.services.dashboard_service import auto_update_contract_statuses
    return auto_update_contract_statuses(db_session)


# ─── Lỗi gốc ─────────────────────────────────────────────────────────────────

def test_signed_contract_near_expiry_stays_signed(db_engine, db_session, make_contract):
    ContractStatus = _status()
    contract = make_contract(
        status=ContractStatus.Signed,
        end_date=date.today() + timedelta(days=5),
    )

    _sweep(db_session)

    db_session.refresh(contract)
    assert contract.status == ContractStatus.Signed
    # Vẫn được đánh dấu sắp hết hạn — cảnh báo là thông tin dẫn xuất.
    assert contract.is_expiring_soon is True
    assert contract.expiry_bucket == "critical"


def test_signed_contract_near_expiry_can_still_be_invoiced(db_engine, db_session, make_contract):
    """Đây chính là thao tác từng bị chặn."""
    from app.services.contract_service import update_status
    ContractStatus = _status()

    contract = make_contract(
        status=ContractStatus.Signed,
        end_date=date.today() + timedelta(days=5),
    )
    _sweep(db_session)

    contract.invoice_pdf_path = "uploads/invoice_docs/hd.pdf"
    db_session.commit()
    update_status(db_session, contract.id, "Invoiced")

    contract.payment_slip_path = "uploads/payment_slips/unc.pdf"
    db_session.commit()
    update_status(db_session, contract.id, "PaidActive")

    db_session.refresh(contract)
    assert contract.status == ContractStatus.PaidActive


def test_sweep_never_writes_the_deprecated_status(db_engine, db_session, make_contract):
    from app.services import lifecycle
    ContractStatus = _status()

    for days in (1, 5, 15, 29, 30, 45):
        make_contract(status=ContractStatus.Signed,
                      end_date=date.today() + timedelta(days=days))
        make_contract(status=ContractStatus.PaidActive,
                      end_date=date.today() + timedelta(days=days))

    _sweep(db_session)

    from app.models.contract import Contract
    statuses = {c.status for c in db_session.query(Contract).all()}
    assert not (statuses & lifecycle.DEPRECATED_STATUSES)


# ─── Chuyển sang Expired vẫn phải chạy ───────────────────────────────────────

@pytest.mark.parametrize("days_out,should_expire", [
    (-30, True),
    (-1, True),
    (0, True),       # hết hạn hôm nay
    (1, False),
    (30, False),
])
def test_sweep_expires_only_past_end_dates(db_engine, db_session, make_contract,
                                           days_out, should_expire):
    ContractStatus = _status()
    contract = make_contract(
        status=ContractStatus.PaidActive,
        end_date=date.today() + timedelta(days=days_out),
    )

    counts = _sweep(db_session)

    db_session.refresh(contract)
    if should_expire:
        assert contract.status == ContractStatus.Expired
        assert counts["newly_expired"] == 1
    else:
        assert contract.status == ContractStatus.PaidActive
        assert counts["newly_expired"] == 0


def test_sweep_leaves_pre_signature_contracts_alone(db_engine, db_session, make_contract):
    """Bản nháp/đã gửi quá hạn không phải việc của bản quét hết hạn."""
    ContractStatus = _status()
    draft = make_contract(status=ContractStatus.Draft,
                          end_date=date.today() - timedelta(days=10))
    sent = make_contract(status=ContractStatus.Sent,
                         end_date=date.today() - timedelta(days=10))

    _sweep(db_session)

    db_session.refresh(draft)
    db_session.refresh(sent)
    assert draft.status == ContractStatus.Draft
    assert sent.status == ContractStatus.Sent


def test_sweep_is_idempotent(db_engine, db_session, make_contract):
    ContractStatus = _status()
    make_contract(status=ContractStatus.PaidActive,
                  end_date=date.today() - timedelta(days=1))

    assert _sweep(db_session)["newly_expired"] == 1
    assert _sweep(db_session)["newly_expired"] == 0


# ─── Dashboard, danh sách và báo cáo phải khớp nhau ──────────────────────────

def test_expiring_list_covers_every_live_status(db_engine, db_session, make_contract):
    """Bản cũ bỏ sót Invoiced ở chỗ này và bỏ sót PaidActive ở chỗ kia."""
    from app.services.dashboard_service import get_expiring_contracts
    ContractStatus = _status()

    live = [ContractStatus.Signed, ContractStatus.Invoiced, ContractStatus.PaidActive]
    for st in live:
        make_contract(status=st, end_date=date.today() + timedelta(days=20))

    rows = get_expiring_contracts(db_session, days=30)

    assert {r["status"] for r in rows} == {st.value for st in live}


def test_dashboard_in_force_matches_the_expiring_source(db_engine, db_session, make_contract):
    from app.services.dashboard_service import get_dashboard_data
    ContractStatus = _status()

    for st in (ContractStatus.Signed, ContractStatus.Invoiced, ContractStatus.PaidActive):
        make_contract(status=st, end_date=date.today() + timedelta(days=200))
    # Hai cái sắp hết hạn, nằm TRONG tập đang hiệu lực, không phải nhóm riêng.
    for _ in range(2):
        make_contract(status=ContractStatus.PaidActive,
                      end_date=date.today() + timedelta(days=10))
    # Nhiễu: chưa ký và đã kết thúc — không được tính vào đâu cả.
    make_contract(status=ContractStatus.Draft)
    make_contract(status=ContractStatus.Terminated)

    data = get_dashboard_data(db_session)
    summary = data["summary"]

    assert summary["in_force"] == 5
    assert summary["expiring_soon"] == 2
    assert summary["expiring_soon"] == len(data["expiring_30"])
    # in_service loại Signed (đã ký, chờ xuất hoá đơn).
    assert summary["in_service"] == 4


def test_customer_list_counts_agree_with_the_dashboard(db_engine, db_session,
                                                       make_customer, make_contract):
    """Danh sách khách hàng từng thiếu Invoiced nên đếm lệch dashboard."""
    from app.services.customer_service import CustomerFilters, list_customers
    from app.services.dashboard_service import get_dashboard_data
    ContractStatus = _status()

    customer = make_customer()
    for st in (ContractStatus.Signed, ContractStatus.Invoiced, ContractStatus.PaidActive):
        make_contract(customer=customer, status=st,
                      end_date=date.today() + timedelta(days=200))
    make_contract(customer=customer, status=ContractStatus.PaidActive,
                  end_date=date.today() + timedelta(days=10))

    rows, _ = list_customers(db_session, CustomerFilters())
    active_from_list = sum(int(r[2] or 0) for r in rows)
    expiring_from_list = sum(int(r[3] or 0) for r in rows)

    summary = get_dashboard_data(db_session)["summary"]

    assert active_from_list == summary["in_force"] == 4
    assert expiring_from_list == summary["expiring_soon"] == 1


def test_expiring_page_classifies_every_row_it_shows(db_engine, db_session, make_contract):
    """Trang 'sắp hết hạn' đếm nhóm theo `urgency`; None sẽ rơi khỏi mọi nhóm.

    Cửa sổ xem lên tới 365 ngày, rộng hơn ngưỡng 30 ngày của expiry_bucket, nên
    mọi dòng vẫn phải có nhãn — nếu không, các bộ đếm trên đầu trang đếm thiếu.
    """
    from app.services.dashboard_service import get_expiring_contracts
    ContractStatus = _status()

    for days in (3, 10, 25, 90, 300):
        make_contract(status=ContractStatus.PaidActive,
                      end_date=date.today() + timedelta(days=days))

    rows = get_expiring_contracts(db_session, days=365)

    assert len(rows) == 5
    assert all(r["urgency"] in {"critical", "warning", "info"} for r in rows)
    # Tổng ba nhóm phải bằng tổng số dòng — đây chính là phép cộng của template.
    buckets = [r["urgency"] for r in rows]
    assert sum(buckets.count(b) for b in ("critical", "warning", "info")) == len(rows)


def test_contract_list_expiring_filter_matches_the_dashboard(db_engine, db_session,
                                                             make_contract):
    from app.services.contract_service import list_contracts
    from app.services.dashboard_service import get_dashboard_data
    ContractStatus = _status()

    for st in (ContractStatus.Signed, ContractStatus.Invoiced, ContractStatus.PaidActive):
        make_contract(status=st, end_date=date.today() + timedelta(days=45))

    _, filtered = list_contracts(db_session, alert_filter="expiring_60", per_page=1)
    data = get_dashboard_data(db_session)

    assert filtered == 3
    assert len(data["expiring_30"]) + len(data["expiring_31_60"]) == 3
