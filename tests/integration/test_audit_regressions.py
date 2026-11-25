"""Regression coverage for document generation and pricing audit findings."""

from pathlib import Path
from io import BytesIO
import json

import pytest
from docx import Document


def _template_file(path, text):
    doc = Document()
    doc.add_paragraph(text)
    doc.save(path)
    return str(path)


@pytest.mark.parametrize("status", [
    "Draft", "Generated", "Sent", "Signed", "Invoiced", "PaidActive",
    "Active", "ExpiringSoon", "Expired", "Terminated",
])
def test_regenerating_keeps_business_status_and_historical_prices(
    db_session, make_product, make_template, make_contract, tmp_path, monkeypatch, status,
):
    from app.models.contract import ContractStatus
    from app.services import contract_service as cs

    monkeypatch.setattr(cs, "OUTPUT_DIR", str(tmp_path / "outputs"))
    product = make_product(is_active=False)
    template = make_template(
        is_active=False, contract_type="Hợp đồng",
        file_path=_template_file(tmp_path / "old.docx", "{{TOTAL_AMOUNT}}"),
    )
    contract = make_contract(
        product=product, template_id=template.id, status=ContractStatus(status),
        total_amount=123_456, pricing_model="legacy",
    )

    cs.generate_docx(db_session, contract.id)

    assert contract.status.value == ("Generated" if status == "Draft" else status)
    assert contract.total_amount == 123_456
    assert Path(contract.output_file_path).is_file()


@pytest.mark.parametrize("failure_stage", ["commit", "bundle"])
def test_failed_regeneration_preserves_existing_docx_and_bundle(
    db_session, make_template, make_contract, tmp_path, monkeypatch, failure_stage,
):
    from app.services import contract_service as cs

    monkeypatch.setattr(cs, "OUTPUT_DIR", str(tmp_path / "outputs"))
    monkeypatch.setattr(cs, "make_output_filename", lambda **kw: "same.docx")
    template = make_template(
        contract_type="Hợp đồng", file_path=_template_file(tmp_path / "main.docx", "Original"),
    )
    contract = make_contract(template_id=template.id)
    cs.generate_docx(db_session, contract.id, bundle=[])
    old_path = Path(contract.output_file_path)
    old_docx = old_path.read_bytes()
    old_zip = Path(cs.bundle_path_for(contract.id))
    old_bundle = old_zip.read_bytes()
    _template_file(tmp_path / "main.docx", "Replacement")

    def fail(*args):
        raise RuntimeError("simulated generation failure")

    if failure_stage == "commit":
        monkeypatch.setattr(db_session, "commit", fail)
    else:
        monkeypatch.setattr(cs, "_write_bundle", fail)
    with pytest.raises(ValueError, match="simulated generation failure"):
        cs.generate_docx(db_session, contract.id, bundle=[])
    db_session.rollback()

    assert old_path.is_file()
    assert old_path.read_bytes() == old_docx
    assert old_zip.read_bytes() == old_bundle
    assert contract.output_file_path == str(old_path)
    assert list(old_path.parent.glob("*.docx")) == [old_path]
    assert list(old_zip.parent.glob("*.zip")) == [old_zip]


@pytest.mark.parametrize("price", [0, 12_345])
@pytest.mark.parametrize("override", ["", "Theo thỏa thuận"])
def test_items_amount_text_uses_final_total_or_explicit_override(
    db_session, make_customer, make_product, price, override,
):
    from app.services.contract_service import create_draft
    from app.utils.amount_to_words_vi import amount_to_words

    customer = make_customer()
    product = make_product(default_price=price, default_vat_rate=0)
    contract = create_draft(db_session, {
        "contract_number": "AUDIT-TEXT", "customer_id": customer.id,
        "product_id": product.id, "month_count": "12", "total_amount": "1.000",
        "amount_text": override,
        "items_json": json.dumps([{"product_id": product.id, "quantity": "1"}]),
    })

    assert contract.total_amount == price * 12
    expected = override or (amount_to_words(price * 12) if price else "")
    assert (contract.amount_text or "") == expected


@pytest.mark.parametrize("kind", ["main", "addon"])
def test_stopped_products_are_refused_for_new_lines(
    db_session, make_product, kind,
):
    from app.services.contract_service import price_preview

    main = make_product(group_code="AUDIT-G")
    stopped = make_product(group_code="AUDIT-G", kind=kind, is_active=False)
    items = [{"product_id": stopped.id, "quantity": "1"}]
    if kind == "addon":
        items.insert(0, {"product_id": main.id, "quantity": "1"})

    with pytest.raises(ValueError, match="ngừng"):
        price_preview(db_session, items, 12)


def test_stopped_main_has_no_new_contract_options(db_session, make_product):
    from app.services.contract_service import product_options

    product = make_product(is_active=False)
    assert product_options(db_session, product.id, 0, 1)["main"] is None


@pytest.mark.parametrize("same_group", [False, True])
def test_quotation_rejects_main_line_different_from_selected_product(
    db_session, make_customer, make_product, make_template, tmp_path, monkeypatch, same_group,
):
    from starlette.requests import Request
    import app.routers.quotation as quotation

    customer = make_customer()
    selected = make_product(group_code="AUDIT-A")
    other = make_product(group_code="AUDIT-A" if same_group else "AUDIT-B")
    template = make_template(
        contract_type="Báo giá", product_group="AUDIT-A",
        file_path=_template_file(tmp_path / "quote.docx", "{{PRODUCT_NAME}} {{TOTAL_AMOUNT}}"),
    )
    monkeypatch.setattr(quotation, "SessionLocal", lambda: db_session)
    response = quotation.quotation_generate(
        Request({"type": "http", "method": "POST", "path": "/quotation/generate",
                 "query_string": b"", "headers": []}),
        product_id=selected.id, template_id=template.id, customer_id=customer.id,
        quote_date="2026-11-17", month_count=12,
        items_json=json.dumps([{"product_id": other.id, "quantity": "1"}]),
    )

    assert response.status_code == 303
    from urllib.parse import unquote

    assert "không khớp" in unquote(response.headers["location"])


def test_quotation_choices_offer_only_main_products(db_session, make_product):
    from app.routers.quotation import list_quotation_products

    main = make_product(kind="main")
    make_product(kind="addon")
    make_product(kind="storage")
    assert [p.id for p in list_quotation_products(db_session)] == [main.id]


def test_08a_attachment_uses_the_specialized_context_and_custom_mapping(
    db_session, make_customer, make_product, make_template, make_contract, tmp_path,
):
    from app.services.attachment_service import render_attachment
    from app.services.template_analyzer import analyze

    customer = make_customer(legal_name="Đơn vị Mẫu", budget_relation_code="TEST-001")
    product = make_product(name="Dịch vụ Mẫu (TEST)")
    path = _template_file(tmp_path / "08a.docx", (
        "{{BUDGET_UNIT_NAME}} | {{BUDGET_UNIT_CODE}} | {{WORK_CONTENT}} | "
        "{{PARTY_A_TITLE_UPPER}} | {{PARTY_B_TITLE_UPPER}} | {{CUSTOM_WORK}}"
    ))
    template = make_template(contract_type="Nghiệm thu", file_path=path)
    result = analyze(Path(path).read_bytes())
    template.detected_placeholders = result.placeholders
    template.field_mapping = {**result.mapping, "CUSTOM_WORK": "form08a.work_content"}
    db_session.commit()
    contract = make_contract(customer=customer, product=product, template_id=template.id)

    data, _ = render_attachment(db_session, contract.id, template.id)
    text = Document(BytesIO(data)).paragraphs[0].text

    assert "{{" not in text
    assert "ĐƠN VỊ MẪU | TEST-001 | Dịch vụ Mẫu" in text
    assert text.endswith("| Dịch vụ Mẫu")
