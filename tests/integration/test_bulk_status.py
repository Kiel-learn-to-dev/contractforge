"""Đổi trạng thái hàng loạt phải theo đúng luật của đổi từng cái.

Lỗi gốc: `bulk_update_status()` gán thẳng `c.status`. Chọn nhiều hợp đồng ở màn
hình danh sách rồi đổi trạng thái là nhảy được từ 'Đã sinh file' sang 'Đã thanh
toán' — không hoá đơn, không uỷ nhiệm chi, không dòng lịch sử nào. Trong khi đổi
từng cái thì chặn đúng.
"""

from __future__ import annotations

import pytest


def _status():
    from app.models.contract import ContractStatus
    return ContractStatus


def _with_evidence(contract, db_session):
    contract.invoice_pdf_path = "uploads/invoice_docs/hd.pdf"
    contract.payment_slip_path = "uploads/payment_slips/unc.pdf"
    db_session.commit()
    return contract


# ─── Lỗ hổng gốc ─────────────────────────────────────────────────────────────

def test_bulk_cannot_jump_generated_to_paid_active(db_engine, db_session, make_contract):
    """Đúng kịch bản bypass cũ — nay phải bị từ chối."""
    from app.services.contract_service import bulk_update_status
    ContractStatus = _status()

    contracts = [make_contract(status=ContractStatus.Generated) for _ in range(3)]
    ids = [c.id for c in contracts]

    with pytest.raises(ValueError, match="Không cập nhật hợp đồng nào"):
        bulk_update_status(db_session, ids, "PaidActive")

    for c in contracts:
        db_session.refresh(c)
        assert c.status == ContractStatus.Generated


def test_bulk_enforces_required_evidence(db_engine, db_session, make_contract):
    """Signed → Invoiced hợp lệ về bảng luật, nhưng vẫn cần file hoá đơn."""
    from app.services.contract_service import bulk_update_status
    ContractStatus = _status()

    contract = make_contract(status=ContractStatus.Signed)
    assert not contract.invoice_pdf_path

    with pytest.raises(ValueError, match="hóa đơn"):
        bulk_update_status(db_session, [contract.id], "Invoiced")

    db_session.refresh(contract)
    assert contract.status == ContractStatus.Signed


def test_bulk_records_an_event_per_contract(db_engine, db_session, make_contract):
    from app.models.contract_event import ContractEvent
    from app.services.contract_service import bulk_update_status
    ContractStatus = _status()

    contracts = [make_contract(status=ContractStatus.Draft) for _ in range(3)]
    ids = [c.id for c in contracts]

    assert bulk_update_status(db_session, ids, "Generated", actor="tester") == 3

    events = (
        db_session.query(ContractEvent)
        .filter(ContractEvent.contract_id.in_(ids),
                ContractEvent.event_type == "status_changed")
        .all()
    )
    assert len(events) == 3
    assert {e.actor for e in events} == {"tester"}


# ─── Chính sách giao dịch: tất cả hoặc không có gì ───────────────────────────

def test_mixed_batch_changes_nothing(db_engine, db_session, make_contract):
    """Một hợp đồng hỏng trong lô thì không hợp đồng nào bị đổi."""
    from app.services.contract_service import bulk_update_status
    ContractStatus = _status()

    ok_one = make_contract(status=ContractStatus.Draft)
    ok_two = make_contract(status=ContractStatus.Draft)
    bad = make_contract(status=ContractStatus.Terminated)   # ngõ cụt

    with pytest.raises(ValueError) as exc:
        bulk_update_status(db_session, [ok_one.id, ok_two.id, bad.id], "Generated")

    # Thông báo phải chỉ đúng hợp đồng có lỗi, để người dùng bỏ chọn được nó.
    assert bad.contract_number in str(exc.value)

    for c in (ok_one, ok_two, bad):
        db_session.refresh(c)
    assert ok_one.status == ContractStatus.Draft
    assert ok_two.status == ContractStatus.Draft
    assert bad.status == ContractStatus.Terminated


def test_missing_ids_abort_the_batch(db_engine, db_session, make_contract):
    from app.services.contract_service import bulk_update_status
    ContractStatus = _status()

    ok = make_contract(status=ContractStatus.Draft)

    with pytest.raises(ValueError, match="không tìm thấy"):
        bulk_update_status(db_session, [ok.id, 999_999], "Generated")

    db_session.refresh(ok)
    assert ok.status == ContractStatus.Draft


def test_bulk_rejects_an_unknown_status(db_engine, db_session, make_contract):
    from app.services.contract_service import bulk_update_status
    ContractStatus = _status()

    contract = make_contract(status=ContractStatus.Draft)
    with pytest.raises(ValueError, match="Trạng thái không hợp lệ"):
        bulk_update_status(db_session, [contract.id], "Nonsense")

    db_session.refresh(contract)
    assert contract.status == ContractStatus.Draft


def test_empty_selection_is_a_no_op(db_engine, db_session):
    from app.services.contract_service import bulk_update_status

    assert bulk_update_status(db_session, [], "Generated") == 0


# ─── Đường đi hạnh phúc, đủ vòng đời ─────────────────────────────────────────

def test_full_lifecycle_in_bulk(db_engine, db_session, make_contract):
    from app.services.contract_service import bulk_update_status
    ContractStatus = _status()

    contracts = [make_contract(status=ContractStatus.Draft) for _ in range(2)]
    ids = [c.id for c in contracts]

    for step in ("Generated", "Sent", "Signed"):
        assert bulk_update_status(db_session, ids, step) == 2

    for c in contracts:
        _with_evidence(c, db_session)

    assert bulk_update_status(db_session, ids, "Invoiced") == 2
    assert bulk_update_status(db_session, ids, "PaidActive") == 2

    for c in contracts:
        db_session.refresh(c)
        assert c.status == ContractStatus.PaidActive


def test_bulk_and_single_agree_on_every_rejection(db_engine, db_session, make_contract):
    """Hai lối đi phải cho cùng một câu trả lời — đó là điểm mấu chốt của bản sửa."""
    from app.services.contract_service import bulk_update_status, update_status
    ContractStatus = _status()

    for target in ("PaidActive", "Invoiced", "Expired"):
        single = make_contract(status=ContractStatus.Generated)
        batch = make_contract(status=ContractStatus.Generated)

        with pytest.raises(ValueError):
            update_status(db_session, single.id, target)
        with pytest.raises(ValueError):
            bulk_update_status(db_session, [batch.id], target)

        db_session.refresh(single)
        db_session.refresh(batch)
        assert single.status == batch.status == ContractStatus.Generated
