"""Luật nhóm mẫu được áp ở server (B4): lệch nhóm hoặc sai vai trò thì không ghi gì.

Dữ liệu hoàn toàn hư cấu: nhóm GRP-A, GRP-B. Mẫu Word dựng trong thư mục tạm.
"""

import io
import json
import zipfile
from datetime import date
from decimal import Decimal

import docx
import pytest


def _docx(path, text="Văn bản {{CONTRACT_NUMBER}}"):
    doc = docx.Document()
    doc.add_paragraph(text)
    doc.save(str(path))
    return str(path)


@pytest.fixture()
def world(db_session, make_customer, make_product, make_template, make_contract, tmp_path):
    from app.models.contract import ContractStatus

    customer = make_customer(legal_name="Khách nhóm hư cấu")
    a_main = make_product(code="EG-A-MAIN", name="Chính A", kind="main", group_code="GRP-A",
                          unit_label="cơ sở", default_price=500_000, default_vat_rate=10)
    a_add = make_product(code="EG-A-ADD", name="Bổ sung A", kind="addon", group_code="GRP-A",
                         unit_label="cơ sở", default_price=150_000, default_vat_rate=10)
    b_main = make_product(code="EG-B-MAIN", name="Chính B", kind="main", group_code="GRP-B",
                          unit_label="cơ sở", default_price=700_000, default_vat_rate=10)

    def tpl(name, contract_type, group=None, text="Văn bản {{CONTRACT_NUMBER}}"):
        return make_template(name=name, contract_type=contract_type, product_group=group,
                             file_path=_docx(tmp_path / f"{name.replace(' ', '_')}.docx", text))

    t = {
        "a_contract": tpl("HĐ A", "Hợp đồng", "GRP-A"),
        "b_contract": tpl("HĐ B", "Hợp đồng", "GRP-B"),
        "a_handover": tpl("BB bàn giao A", "Bàn giao", "GRP-A"),
        "b_handover": tpl("BB bàn giao B", "Bàn giao", "GRP-B"),
        "shared_ack": tpl("BB nghiệm thu chung", "Nghiệm thu", None),
        "a_addon": tpl("PL bổ sung A", "Phụ lục bổ sung", "GRP-A"),
        "b_addon": tpl("PL bổ sung B", "Phụ lục bổ sung", "GRP-B"),
        "a_renewal": tpl("PL gia hạn A", "Phụ lục gia hạn", "GRP-A"),
        "shared_appendix": tpl("Phụ lục chung cũ", "Phụ lục", None),
        "b_quote": tpl("Báo giá B", "Báo giá", "GRP-B"),
        "shared_quote": tpl("Báo giá chung", "Báo giá", None),
        "shared_contract": tpl("Hợp đồng chung", "Hợp đồng", None),
    }

    draft = make_contract(customer=customer, product=a_main, status=ContractStatus.Draft,
                          contract_number="HD-EG-DRAFT", template_id=t["a_contract"].id,
                          start_date=date(2027, 1, 1), end_date=date(2027, 12, 31),
                          month_count=12, unit_count=1)
    parent = make_contract(customer=customer, product=a_main, status=ContractStatus.Invoiced,
                           contract_number="HD-EG-P", template_id=t["a_contract"].id,
                           start_date=date(2027, 1, 1), end_date=date(2027, 12, 31),
                           month_count=12, unit_count=1, total_amount=Decimal("6600000"),
                           vat_rate=10)
    return {"customer": customer, "a_main": a_main, "a_add": a_add, "b_main": b_main,
            "t": t, "draft": draft, "parent": parent, "tmp": tmp_path}


# ─── Tạo hợp đồng ─────────────────────────────────────────────────────────────

def _draft_form(w, number, product, template_id):
    return {
        "contract_number": number,
        "customer_id": str(w["customer"].id),
        "product_id": str(product.id),
        "template_id": str(template_id),
        "start_date": "2027-01-01",
        "end_date": "2027-12-31",
        "month_count": "12",
        "unit_count": "1",
        "unit_price_pre_vat": "500000",
        "vat_rate": "10",
    }


def test_create_draft_refuses_a_template_of_another_group(db_session, world):
    from app.models.contract import Contract
    from app.services.contract_service import create_draft

    w = world
    with pytest.raises(ValueError):
        create_draft(db_session, _draft_form(w, "HD-EG-X1", w["a_main"], w["t"]["b_contract"].id))

    db_session.expire_all()
    assert db_session.query(Contract).filter_by(contract_number="HD-EG-X1").count() == 0


def test_create_draft_refuses_a_non_contract_template_as_the_contract_template(db_session, world):
    from app.services.contract_service import create_draft

    w = world
    with pytest.raises(ValueError):
        create_draft(db_session, _draft_form(w, "HD-EG-X2", w["a_main"], w["t"]["a_handover"].id))


def test_product_without_group_accepts_only_shared_templates(db_session, make_product, world):
    from app.services.contract_service import create_draft

    w = world
    plain = make_product(code="EG-NG", name="Gói không nhóm", kind="main", default_price=500_000,
                         default_vat_rate=10)
    created = create_draft(db_session, _draft_form(w, "HD-EG-NG", plain, w["t"]["shared_contract"].id))
    assert created.template_id == w["t"]["shared_contract"].id

    with pytest.raises(ValueError):
        create_draft(db_session, _draft_form(w, "HD-EG-NG2", plain, w["t"]["a_contract"].id))


# ─── Sinh hợp đồng và văn bản kèm theo ────────────────────────────────────────

def test_generate_refuses_an_attachment_of_another_group_and_writes_nothing(
        client, db_session, world, data_root):
    from app.models.contract import ContractStatus
    from app.models.contract import Contract
    from app.services import contract_service

    w = world
    out_dir = contract_service.OUTPUT_DIR
    import os
    before = set(os.listdir(out_dir)) if os.path.isdir(out_dir) else set()

    resp = client.post(f"/contracts/{w['draft'].id}/generate",
                       data={"attachment_ids": [str(w["t"]["b_handover"].id)]},
                       follow_redirects=False)

    assert resp.status_code == 303
    db_session.expire_all()
    contract = db_session.get(Contract, w["draft"].id)
    assert contract.status == ContractStatus.Draft
    after = set(os.listdir(out_dir)) if os.path.isdir(out_dir) else set()
    assert after == before


def test_generate_bundles_own_group_and_shared_attachments(client, db_session, world, data_root):
    w = world
    resp = client.post(f"/contracts/{w['draft'].id}/generate",
                       data={"attachment_ids": [str(w["t"]["a_handover"].id),
                                                str(w["t"]["shared_ack"].id)]})

    assert resp.status_code == 200
    assert resp.headers["content-type"] == "application/zip"
    assert len(zipfile.ZipFile(io.BytesIO(resp.content)).namelist()) == 3


def test_attachment_download_of_another_group_is_400(client, world):
    w = world
    resp = client.get(f"/contracts/{w['draft'].id}/attachment",
                      params={"template_id": w["t"]["b_handover"].id})
    assert resp.status_code == 400


def test_regenerating_still_works_after_the_template_is_switched_off(db_session, world, data_root):
    from app.services.contract_service import generate_docx

    w = world
    generate_docx(db_session, w["draft"].id, [], None)
    w["t"]["a_contract"].is_active = False
    db_session.commit()

    contract = generate_docx(db_session, w["draft"].id, [], None)
    assert contract.output_file_name


# ─── Phụ lục và gia hạn ──────────────────────────────────────────────────────

def test_addendum_refuses_a_template_of_another_group(db_session, world):
    from app.services.addendum_service import AddendumError, create_addendum

    w = world
    with pytest.raises(AddendumError):
        create_addendum(db_session, w["parent"].id, "addon", {
            "start_date": "2027-07-01", "template_id": str(w["t"]["b_addon"].id),
            "items_json": json.dumps([{"product_id": w["a_add"].id}])})


def test_addendum_without_a_pick_takes_the_group_addon_template(db_session, world):
    from app.services.addendum_service import create_addendum

    w = world
    child = create_addendum(db_session, w["parent"].id, "addon", {
        "start_date": "2027-07-01", "items_json": json.dumps([{"product_id": w["a_add"].id}])})
    assert child.template_id == w["t"]["a_addon"].id


def test_addendum_refuses_a_renewal_template(db_session, world):
    from app.services.addendum_service import AddendumError, create_addendum

    w = world
    with pytest.raises(AddendumError):
        create_addendum(db_session, w["parent"].id, "addon", {
            "start_date": "2027-07-01", "template_id": str(w["t"]["a_renewal"].id),
            "items_json": json.dumps([{"product_id": w["a_add"].id}])})


def test_addendum_accepts_the_legacy_shared_appendix_label(db_session, world):
    from app.services.addendum_service import create_addendum

    w = world
    child = create_addendum(db_session, w["parent"].id, "addon", {
        "start_date": "2027-07-01", "template_id": str(w["t"]["shared_appendix"].id),
        "items_json": json.dumps([{"product_id": w["a_add"].id}])})
    assert child.template_id == w["t"]["shared_appendix"].id


def test_renewal_appendix_without_a_pick_takes_the_group_renewal_template(db_session, world):
    from app.services.addendum_service import create_renewal_appendix

    w = world
    child = create_renewal_appendix(db_session, w["parent"].id, {
        "start_date": "2028-01-01", "month_count": "12",
        "items_json": json.dumps([{"product_id": w["a_main"].id}])})
    assert child.template_id == w["t"]["a_renewal"].id


def test_renewal_contract_takes_the_package_default_template(db_session, world, data_root):
    from app.services.addendum_service import create_renewal_contract
    from app.services.contract_service import generate_docx

    w = world
    w["a_main"].default_template_id = w["t"]["a_contract"].id
    db_session.commit()

    new = create_renewal_contract(db_session, w["parent"].id, {
        "contract_number": "HD-EG-NEW", "start_date": "2028-01-01", "month_count": "12",
        "items_json": json.dumps([{"product_id": w["a_main"].id}])})
    assert new.template_id == w["t"]["a_contract"].id

    generated = generate_docx(db_session, new.id, [], None)
    assert generated.output_file_name


# ─── Báo giá và lô ───────────────────────────────────────────────────────────

def test_quotation_refuses_a_template_of_another_group(db_session, world):
    from app.routers.quotation import resolve_quotation_selection

    w = world
    with pytest.raises(ValueError):
        resolve_quotation_selection(db_session, w["a_main"].id, w["t"]["b_quote"].id)


def test_quotation_accepts_a_shared_template(db_session, world):
    from app.routers.quotation import resolve_quotation_selection

    w = world
    product, template = resolve_quotation_selection(db_session, w["a_main"].id, w["t"]["shared_quote"].id)
    assert template.id == w["t"]["shared_quote"].id


def test_batch_row_with_another_group_fails_alone(db_session, make_customer, world, make_template):
    from app.services.batch_generator import run_batch

    w = world
    customers = [make_customer() for _ in range(2)]
    shared = {"month_count": "12", "start_date": "01/01/2027",
              "items_json": json.dumps([{"product_id": w["a_main"].id}])}
    overrides = {c.id: {"unit_count": "1", "contract_number": f"BATCH-EG-{i}"}
                 for i, c in enumerate(customers, start=1)}

    result = run_batch(db=db_session, template_id=w["t"]["b_contract"].id,
                       customer_ids=[c.id for c in customers],
                       shared_params=shared, per_customer_overrides=overrides)

    assert result.failed == 2 and result.succeeded == 0
    assert all("không dùng cho nhóm" in (row.error or "") for row in result.rows)


# ─── Form tạo hợp đồng thật (route) ──────────────────────────────────────────

def _post_new(client, w, template_id, items, product):
    data = {
        "contract_number": f"HD-EG-FORM-{template_id}-{product.id}",
        "customer_id": str(w["customer"].id),
        "product_id": str(product.id),
        "template_id": str(template_id),
        "sign_date": "2027-01-01",
        "start_date": "2027-01-01",
        "end_date": "2027-12-31",
        "month_count": "12",
        "unit_count": "1",
        "items_json": json.dumps(items),
    }
    return client.post("/contracts/new", data=data, follow_redirects=False)


def test_form_post_with_own_group_template_is_accepted(client, world, data_root):
    w = world
    items = [{"product_id": w["a_main"].id, "quantity": 1}]
    resp = _post_new(client, w, w["t"]["a_contract"].id, items, w["a_main"])
    assert resp.status_code == 303


def test_form_post_with_other_group_template_is_refused(client, world, data_root):
    w = world
    items = [{"product_id": w["a_main"].id, "quantity": 1}]
    resp = _post_new(client, w, w["t"]["b_contract"].id, items, w["a_main"])
    assert resp.status_code == 422


def test_group_comes_from_the_main_line_when_it_differs_from_the_picker(client, world, data_root):
    """Ô chọn sản phẩm là A nhưng dòng chính trong items_json là B: nhóm theo dòng chính."""
    w = world
    items = [{"product_id": w["b_main"].id, "quantity": 1}]
    assert _post_new(client, w, w["t"]["b_contract"].id, items, w["a_main"]).status_code == 303
    assert _post_new(client, w, w["t"]["a_contract"].id, items, w["a_main"]).status_code == 422
