"""Task 4.2 — phụ lục bổ sung gói giữa kỳ.

Dữ liệu hoàn toàn hư cấu. Hợp đồng gốc ở trạng thái Invoiced (đang phục vụ).
"""

import json
from datetime import date
from decimal import Decimal

import pytest


@pytest.fixture()
def parent_setup(db_session, make_customer, make_product, make_contract):
    customer = make_customer()
    main = make_product(code="AD-MAIN", name="Gói chính phụ lục", kind="main",
                        group_code="AD-G", unit_label="cơ sở",
                        default_price=500_000, default_vat_rate=10)
    addon = make_product(code="AD-ADD", name="Gói bổ sung phụ lục", kind="addon",
                         group_code="AD-G", unit_label="cơ sở",
                         default_price=150_000, default_vat_rate=10)
    other = make_product(code="AD-OTH", name="Gói khác nhóm", kind="addon",
                         group_code="AD-X", default_price=99_000, default_vat_rate=10)
    from app.models.contract import ContractStatus
    parent = make_contract(customer=customer, product=main, status=ContractStatus.Invoiced,
                           contract_number="HD-AD-GOC", start_date=date(2026, 1, 1),
                           end_date=date(2026, 12, 31), month_count=12, unit_count=2,
                           sign_location="Địa điểm hư cấu")
    return {"customer": customer, "main": main, "addon": addon, "other": other, "parent": parent}


def test_addon_addendum_is_priced_for_the_remaining_months_and_ends_with_the_parent(
        db_session, parent_setup):
    from app.models.contract import Contract
    from app.models.contract_event import ContractEvent
    from app.services.addendum_service import create_addendum

    p = parent_setup
    child = create_addendum(db_session, p["parent"].id, "addon", {
        "start_date": "2026-10-01",
        "items_json": json.dumps([{"product_id": p["addon"].id}]),
    })

    db_session.expire_all()
    assert child.doc_kind == "addendum_addon"
    assert child.parent_contract_id == p["parent"].id
    assert child.end_date == date(2026, 12, 31)           # bằng ngày hết hạn của gốc
    assert child.month_count == 3                          # 1/10 đến 31/12: làm tròn lên 3 tháng
    assert child.contract_number == "PL01/HD-AD-GOC"
    # Số lượng mặc định = số cơ sở của gốc (2). 150.000 × 2 × 3 = 900.000, cộng 10% thuế.
    assert child.total_amount == Decimal("990000")
    assert child.pricing_model == "items"
    assert child in db_session.get(Contract, p["parent"].id).addenda

    events = db_session.query(ContractEvent).filter(ContractEvent.event_type == "addendum").all()
    assert {e.contract_id for e in events} == {p["parent"].id, child.id}


def test_second_addendum_takes_the_next_number(db_session, parent_setup):
    from app.services.addendum_service import create_addendum

    p = parent_setup
    for start in ("2026-10-01", "2026-11-01"):
        create_addendum(db_session, p["parent"].id, "addon", {
            "start_date": start, "items_json": json.dumps([{"product_id": p["addon"].id}])})
    numbers = sorted(c.contract_number for c in p["parent"].addenda)
    assert numbers == ["PL01/HD-AD-GOC", "PL02/HD-AD-GOC"]


def test_parent_that_is_not_in_service_refuses_an_addendum(db_session, make_contract, parent_setup):
    from app.models.contract import ContractStatus
    from app.services.addendum_service import AddendumError, create_addendum

    p = parent_setup
    draft = make_contract(customer=p["customer"], product=p["main"], status=ContractStatus.Draft,
                          contract_number="HD-AD-DRAFT", start_date=date(2026, 1, 1),
                          end_date=date(2026, 12, 31))
    with pytest.raises(AddendumError):
        create_addendum(db_session, draft.id, "addon", {
            "start_date": "2026-10-01", "items_json": json.dumps([{"product_id": p["addon"].id}])})


def test_start_after_the_parent_end_is_refused(db_session, parent_setup):
    from app.services.addendum_service import AddendumError, create_addendum

    p = parent_setup
    with pytest.raises(AddendumError):
        create_addendum(db_session, p["parent"].id, "addon", {
            "start_date": "2027-01-05", "items_json": json.dumps([{"product_id": p["addon"].id}])})


def test_main_package_cannot_be_the_addendum_line(db_session, parent_setup):
    from app.services.addendum_service import AddendumError, create_addendum

    p = parent_setup
    with pytest.raises(AddendumError):
        create_addendum(db_session, p["parent"].id, "addon", {
            "start_date": "2026-10-01", "items_json": json.dumps([{"product_id": p["main"].id}])})


def test_add_on_from_another_group_is_refused(db_session, parent_setup):
    from app.services.addendum_service import AddendumError, create_addendum

    p = parent_setup
    with pytest.raises(AddendumError):
        create_addendum(db_session, p["parent"].id, "addon", {
            "start_date": "2026-10-01", "items_json": json.dumps([{"product_id": p["other"].id}])})


def test_addendum_shares_the_contract_lifecycle(db_session, parent_setup):
    from app.services.addendum_service import create_addendum
    from app.services.contract_service import update_status

    p = parent_setup
    child = create_addendum(db_session, p["parent"].id, "addon", {
        "start_date": "2026-10-01", "items_json": json.dumps([{"product_id": p["addon"].id}])})
    for step in ("Generated", "Sent", "Signed"):
        child = update_status(db_session, child.id, step)
    assert child.status.value == "Signed"


def test_parent_with_addenda_cannot_be_deleted_alone_or_in_bulk(db_session, parent_setup):
    from app.services.addendum_service import create_addendum
    from app.services.contract_service import bulk_delete, delete_contract

    p = parent_setup
    child = create_addendum(db_session, p["parent"].id, "addon", {
        "start_date": "2026-10-01", "items_json": json.dumps([{"product_id": p["addon"].id}])})
    parent_id = p["parent"].id

    with pytest.raises(ValueError, match="phụ lục"):
        delete_contract(db_session, parent_id)
    with pytest.raises(ValueError, match="phụ lục"):
        bulk_delete(db_session, [parent_id])

    assert delete_contract(db_session, child.id) is True
    assert delete_contract(db_session, parent_id) is True


def test_addendum_form_lists_only_same_group_add_ons(client, parent_setup):
    p = parent_setup
    resp = client.get(f"/contracts/{p['parent'].id}/addendum/new?kind=addon&start=2026-10-01")

    assert resp.status_code == 200
    assert "Gói bổ sung phụ lục" in resp.text
    assert "Gói khác nhóm" not in resp.text
    assert 'name="addon_id"' in resp.text


def test_posting_the_addendum_form_creates_it_and_bad_start_is_shown(client, db_session,
                                                                    parent_setup):
    from app.models.contract import Contract

    p = parent_setup
    ok = client.post(f"/contracts/{p['parent'].id}/addendum/new", data={
        "kind": "addon", "start_date": "2026-10-01", "addon_id": str(p["addon"].id),
    }, follow_redirects=False)
    assert ok.status_code == 303
    db_session.expire_all()
    assert db_session.query(Contract).filter(Contract.doc_kind == "addendum_addon").count() == 1

    bad = client.post(f"/contracts/{p['parent'].id}/addendum/new", data={
        "kind": "addon", "start_date": "2027-02-01", "addon_id": str(p["addon"].id),
    })
    assert bad.status_code == 422
    assert "sau ngày hết hạn" in bad.text


def test_detail_page_links_the_parent_and_the_addendum(client, parent_setup):
    from app.services.addendum_service import create_addendum

    p = parent_setup
    child = create_addendum(_session_of(p), p["parent"].id, "addon", {
        "start_date": "2026-10-01", "items_json": json.dumps([{"product_id": p["addon"].id}])})
    parent_page = client.get(f"/contracts/{p['parent'].id}").text
    child_page = client.get(f"/contracts/{child.id}").text
    assert f"/contracts/{child.id}" in parent_page
    assert "PL01/HD-AD-GOC" in parent_page
    assert f"/contracts/{p['parent'].id}" in child_page


def _session_of(p):
    from sqlalchemy.orm import object_session
    return object_session(p["parent"])


def test_render_context_gives_the_appendix_sequence_and_empty_for_the_parent(
        db_session, parent_setup):
    from app.services.addendum_service import create_addendum
    from app.services.docx_renderer import build_render_context

    p = parent_setup
    first = create_addendum(db_session, p["parent"].id, "addon", {
        "start_date": "2026-10-01", "items_json": json.dumps([{"product_id": p["addon"].id}])})
    second = create_addendum(db_session, p["parent"].id, "addon", {
        "start_date": "2026-11-01", "items_json": json.dumps([{"product_id": p["addon"].id}])})
    db_session.expire_all()

    parent_ctx = build_render_context(p["parent"], p["customer"], p["main"], None, db=db_session)
    first_ctx = build_render_context(first, p["customer"], p["addon"], None, db=db_session)
    second_ctx = build_render_context(second, p["customer"], p["addon"], None, db=db_session)

    assert parent_ctx["ADDENDUM_SEQ"] == ""
    assert first_ctx["ADDENDUM_SEQ"] == "01"
    assert second_ctx["ADDENDUM_SEQ"] == "02"
