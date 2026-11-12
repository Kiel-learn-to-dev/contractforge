"""Task 4.3 — gia hạn: phụ lục gia hạn hoặc hợp đồng mới, cho người dùng chọn.

Dữ liệu hoàn toàn hư cấu. Hợp đồng gốc là hợp đồng cũ (legacy), được chuyển dữ liệu
bằng run_migrations như trên máy thật.
"""

import json
from datetime import date
from decimal import Decimal

import pytest


@pytest.fixture()
def legacy_parent(db_session, make_customer, make_product, make_contract):
    from app.models.contract import ContractStatus
    from app.models.seed import run_migrations

    customer = make_customer()
    old = make_product(code="RN-OLD", name="Gói cũ gia hạn", kind="main", group_code="RN-G",
                       default_price=400_000, default_vat_rate=10, is_active=False,
                       default_duration_months=12)
    new = make_product(code="RN-NEW", name="Gói mới gia hạn", kind="main", group_code="RN-G",
                       default_price=800_000, default_vat_rate=10, default_duration_months=12)
    old.replaced_by_id = new.id
    db_session.commit()
    parent = make_contract(customer=customer, product=old, status=ContractStatus.Invoiced,
                           contract_number="HD-RN-GOC", start_date=date(2025, 1, 1),
                           end_date=date(2025, 12, 31), month_count=10, unit_count=1,
                           total_amount=Decimal("5000000"), vat_rate=10)
    run_migrations(db_session)  # hợp đồng cũ nhận đúng một dòng chép từ số đã lưu
    db_session.expire_all()
    return {"customer": customer, "old": old, "new": new, "parent": parent}


def test_defaults_suggest_the_replacement_with_current_price_and_compare_per_month(
        db_session, legacy_parent):
    from app.services.addendum_service import renewal_defaults

    p = legacy_parent
    d = renewal_defaults(db_session, p["parent"].id)

    assert d["start_date"] == date(2026, 1, 1)                 # ngày sau hết hạn gốc
    assert d["month_count"] == 12                              # mặc định của gói hiện tại
    assert d["lines"][0]["product"].id == p["new"].id           # gói cũ ngừng → gói thay thế
    assert d["comparison"]["old_per_month"] == Decimal("500000")  # 5.000.000 / 10 tháng
    assert d["comparison"]["new_per_month"] == Decimal("880000")  # 800.000 + 10% thuế, 1 tháng
    assert d["comparison"]["diff"] == Decimal("380000")


def test_renewal_appendix_uses_the_current_price(db_session, legacy_parent):
    from app.services.addendum_service import create_renewal_appendix

    p = legacy_parent
    child = create_renewal_appendix(db_session, p["parent"].id, {
        "start_date": "2026-01-01", "month_count": "12",
        "items_json": json.dumps([{"product_id": p["new"].id}]),
    })

    db_session.expire_all()
    assert child.doc_kind == "addendum_renewal"
    assert child.parent_contract_id == p["parent"].id
    assert child.start_date == date(2026, 1, 1)
    # Giá danh mục hiện tại (800.000 + 10%) × 12 tháng, không chép giá cũ.
    assert child.total_amount == Decimal("10560000")


def test_no_replacement_leaves_the_line_empty_and_creation_is_refused_without_a_choice(
        db_session, make_product, make_customer, make_contract):
    from app.models.contract import ContractStatus
    from app.models.seed import run_migrations
    from app.services.addendum_service import AddendumError, create_renewal_appendix, renewal_defaults

    customer = make_customer()
    gone = make_product(code="RN-GONE", name="Gói đã ngừng hẳn", kind="main", group_code="RN-H",
                        default_price=300_000, default_vat_rate=10, is_active=False)
    parent = make_contract(customer=customer, product=gone, status=ContractStatus.Invoiced,
                           contract_number="HD-RN-GONE", start_date=date(2025, 1, 1),
                           end_date=date(2025, 12, 31), month_count=12, unit_count=1,
                           total_amount=Decimal("3300000"), vat_rate=10)
    run_migrations(db_session)
    db_session.expire_all()

    d = renewal_defaults(db_session, parent.id)
    assert d["lines"][0]["product"] is None

    with pytest.raises(AddendumError):
        create_renewal_appendix(db_session, parent.id, {
            "start_date": "2026-01-01", "month_count": "12", "items_json": "[]"})


def test_renewal_must_start_after_the_parent_ends(db_session, legacy_parent):
    from app.services.addendum_service import AddendumError, create_renewal_appendix

    p = legacy_parent
    with pytest.raises(AddendumError):
        create_renewal_appendix(db_session, p["parent"].id, {
            "start_date": "2025-12-31", "month_count": "12",
            "items_json": json.dumps([{"product_id": p["new"].id}])})


def test_renewal_as_new_contract_links_to_the_old_one_and_keeps_it_a_contract(
        db_session, legacy_parent):
    from app.services.addendum_service import create_renewal_contract

    p = legacy_parent
    new = create_renewal_contract(db_session, p["parent"].id, {
        "contract_number": "HD-RN-MOI-1", "start_date": "2026-01-01", "month_count": "12",
        "items_json": json.dumps([{"product_id": p["new"].id}]),
    })

    db_session.expire_all()
    assert new.doc_kind == "contract"
    assert new.renewed_from_id == p["parent"].id
    assert new.parent_contract_id is None
    assert p["parent"].doc_kind == "contract"


def test_renewal_form_shows_comparison_and_both_choices(client, legacy_parent):
    p = legacy_parent
    resp = client.get(f"/contracts/{p['parent'].id}/renewal/new")

    assert resp.status_code == 200
    assert "So sánh giá theo tháng" in resp.text
    assert 'value="addendum"' in resp.text and 'value="contract"' in resp.text
    assert "Gói mới gia hạn" in resp.text


def test_posting_the_renewal_form_in_both_modes(client, db_session, legacy_parent):
    from app.models.contract import Contract

    p = legacy_parent
    appendix = client.post(f"/contracts/{p['parent'].id}/renewal/new", data={
        "mode": "addendum", "start_date": "2026-01-01", "month_count": "12",
        "line_product": [str(p["new"].id)],
    }, follow_redirects=False)
    assert appendix.status_code == 303

    as_contract = client.post(f"/contracts/{p['parent'].id}/renewal/new", data={
        "mode": "contract", "contract_number": "HD-RN-FORM-1", "start_date": "2026-01-01",
        "month_count": "12", "line_product": [str(p["new"].id)],
    }, follow_redirects=False)
    assert as_contract.status_code == 303

    db_session.expire_all()
    assert db_session.query(Contract).filter(Contract.doc_kind == "addendum_renewal").count() == 1
    assert db_session.query(Contract).filter(Contract.renewed_from_id == p["parent"].id).count() == 1


def test_renewal_form_refuses_a_line_without_a_package(client, legacy_parent):
    p = legacy_parent
    resp = client.post(f"/contracts/{p['parent'].id}/renewal/new", data={
        "mode": "addendum", "start_date": "2026-01-01", "month_count": "12",
        "line_product": [""],
    })
    assert resp.status_code == 422
    assert "chưa chọn gói" in resp.text


def test_renewal_appendix_keeps_the_chosen_appendix_template(db_session, legacy_parent, make_template):
    from app.services.addendum_service import create_renewal_appendix

    p = legacy_parent
    tpl = make_template(name="Phụ lục gia hạn CNTT", contract_type="Phụ lục")
    child = create_renewal_appendix(db_session, p["parent"].id, {
        "start_date": "2026-01-01", "month_count": "12", "template_id": str(tpl.id),
        "items_json": json.dumps([{"product_id": p["new"].id}]),
    })

    db_session.expire_all()
    assert child.template_id == tpl.id


def test_renewal_appendix_refuses_a_template_of_another_kind(db_session, legacy_parent, make_template):
    from app.services.addendum_service import AddendumError, create_renewal_appendix

    p = legacy_parent
    wrong = make_template(name="Hợp đồng sai loại", contract_type="Khác")
    with pytest.raises(AddendumError, match="cho phụ lục"):
        create_renewal_appendix(db_session, p["parent"].id, {
            "start_date": "2026-01-01", "month_count": "12", "template_id": str(wrong.id),
            "items_json": json.dumps([{"product_id": p["new"].id}]),
        })


def test_renewal_form_offers_appendix_templates_and_passes_the_pick(
        client, db_session, legacy_parent, make_template):
    from app.models.contract import Contract

    p = legacy_parent
    tpl = make_template(name="Phụ lục gia hạn chọn", contract_type="Phụ lục")
    make_template(name="Mẫu không hiện ở đây", contract_type="Khác")

    page = client.get(f"/contracts/{p['parent'].id}/renewal/new")
    assert "Phụ lục gia hạn chọn" in page.text
    assert "Mẫu không hiện ở đây" not in page.text

    resp = client.post(f"/contracts/{p['parent'].id}/renewal/new", data={
        "mode": "addendum", "start_date": "2026-01-01", "month_count": "12",
        "template_id": str(tpl.id), "line_product": [str(p["new"].id)],
    }, follow_redirects=False)
    assert resp.status_code == 303

    db_session.expire_all()
    child = db_session.query(Contract).filter(Contract.doc_kind == "addendum_renewal").one()
    assert child.template_id == tpl.id


def test_full_year_contract_ends_on_31_december_and_renews_from_1_january(
        db_session, make_customer, make_product, make_contract):
    """Q6: hợp đồng 01/01/2027 đủ 12 tháng hết hạn 31/12/2027; gia hạn từ 01/01/2028."""
    from app.models.contract import ContractStatus
    from app.models.seed import run_migrations
    from app.services.addendum_service import create_renewal_appendix, renewal_defaults

    customer = make_customer()
    prod = make_product(code="RN-Y27", name="Gói cả năm 2027", kind="main", group_code="RN-Y",
                        default_price=500_000, default_vat_rate=10, default_duration_months=12)
    parent = make_contract(customer=customer, product=prod, status=ContractStatus.Invoiced,
                           contract_number="HD-RN-2027", start_date=date(2027, 1, 1),
                           end_date=date(2027, 12, 31), month_count=12, unit_count=1,
                           total_amount=Decimal("6600000"), vat_rate=10)
    run_migrations(db_session)
    db_session.expire_all()

    d = renewal_defaults(db_session, parent.id)
    assert d["start_date"] == date(2028, 1, 1)

    child = create_renewal_appendix(db_session, parent.id, {
        "start_date": "2028-01-01", "month_count": "12",
        "items_json": json.dumps([{"product_id": prod.id}])})
    db_session.expire_all()
    assert child.end_date == date(2028, 12, 31)
