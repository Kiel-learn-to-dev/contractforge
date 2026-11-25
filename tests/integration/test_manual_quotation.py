"""Six quotation choices: five independent issuers and the original product layouts."""
from io import BytesIO
import json
from zipfile import ZipFile

from docx import Document
import pytest


def form(**overrides):
    values = dict(layout="compact", issuer_name="ĐƠN VỊ BÁO GIÁ ĐỘC LẬP",
                  issuer_tax_code="0300000099", issuer_address="Địa chỉ nhập riêng",
                  issuer_representative="Nguyễn Văn Mẫu", issuer_title="Giám đốc",
                  recipient_name="Khách hàng Nhập Riêng", quote_date="2026-11-24",
                  valid_until="2026-12-24", quote_number="BG-001", subject="Dịch vụ quản lý",
                  items_json=json.dumps([dict(name="Hạng mục nhập tay", unit="Đơn vị", quantity="2",
                                             periods="3", price="100.000", vat="8")]))
    values.update(overrides)
    return values


def text(content):
    with ZipFile(BytesIO(content)) as archive:
        return "\n".join(archive.read(name).decode() for name in archive.namelist()
                         if name.startswith("word/") and name.endswith(".xml"))


def test_independent_prices_use_shared_rounding_and_reject_missing_price():
    from app.services.manual_quotation import build_lines
    from app.services.pricing import PricingError
    lines, totals = build_lines(form()["items_json"])
    assert lines[0].unit_price_pre_vat == 100000
    assert totals.pre_vat == 600000 and totals.vat == 48000 and totals.total == 648000
    for raw in ("", "NaN", "Infinity", "-1"):
        items = json.loads(form()["items_json"])
        items[0]["price"] = raw
        with pytest.raises(PricingError):
            build_lines(json.dumps(items))


@pytest.mark.parametrize("layout", ["compact", "formal", "landscape", "features", "terms"])
@pytest.mark.parametrize("group", ["FAMILY-A", "FAMILY-B"])
def test_five_layouts_use_typed_issuer_and_prices_for_either_family(
        client, make_product, layout, group):
    product = make_product(group_code=group, default_price=777123, default_vat_rate=10,
                           feature_list=[{"name": "Chức năng Mẫu", "description": "Mô tả chức năng"}])
    response = client.post("/quotation/manual/generate", data=form(layout=layout, product_id=product.id))
    assert response.status_code == 200, response.text[:300]
    assert "wordprocessingml" in response.headers["content-type"]
    content = text(response.content)
    for expected in ("ĐƠN VỊ BÁO GIÁ ĐỘC LẬP", "Khách hàng Nhập Riêng", "648.000", "BG-001"):
        assert expected in content
    for forbidden in ("777.123", "777123", "{{", "THƯ MỜI", "YÊU CẦU BÁO GIÁ"):
        assert forbidden not in content
    if layout == "features":
        assert "Chức năng Mẫu" in content


@pytest.mark.parametrize("layout", ["compact", "formal", "landscape", "features", "terms"])
def test_raw_templates_keep_placeholders_and_are_real_quotations(client, layout):
    response = client.get(f"/quotation/manual/templates/{layout}")
    assert response.status_code == 200
    content = text(response.content)
    assert "{{QUOTE_ISSUER_NAME}}" in content and "{{ITEM_PRICE}}" in content
    assert "BÁO GIÁ" in content and "THƯ MỜI" not in content and "REQUEST_" not in content


def test_manual_form_preserves_values_after_error_and_next_request_has_no_issuer(client):
    payload = form(issuer_name='Đơn vị <riêng> "A"', valid_until="2026-11-01")
    response = client.post("/quotation/manual/generate", data=payload)
    assert response.status_code == 422
    assert "Đơn vị &lt;riêng&gt;" in response.text
    assert "Hạng mục nhập tay" in response.text
    assert 'value="100.000"' in response.text
    assert "Đơn vị &lt;riêng&gt;" not in client.get("/quotation/manual").text
    payload["valid_until"] = "2026-12-24"
    assert client.post("/quotation/manual/generate", data=payload).status_code == 200


def test_no_catalog_price_fallback_or_settings_issuer(client, make_product):
    product = make_product(default_price=777123)
    payload = form(product_id=product.id)
    payload["items_json"] = json.dumps([dict(name="Dịch vụ", quantity=1, periods=1, price="", vat=0)])
    assert client.post("/quotation/manual/generate", data=payload).status_code == 422
    assert client.post("/quotation/manual/generate", data=form(issuer_name="")).status_code == 422


def test_preview_totals_and_template_allowlist(client):
    response = client.post("/quotation/manual/preview", data=form())
    assert response.status_code == 200
    assert response.json()["total"] == "648.000"
    assert client.get("/quotation/manual/templates/not-a-layout").status_code == 404
    assert client.post("/quotation/manual/generate", data=form(layout="../contract")).status_code == 422


def test_empty_feature_table_is_removed(client):
    response = client.post("/quotation/manual/generate", data=form(layout="features"))
    assert response.status_code == 200
    assert "Yêu cầu chức năng" not in text(response.content)
    doc = Document(BytesIO(response.content))
    assert all(not any("Chức năng" in cell.text for cell in table.rows[0].cells) for table in doc.tables)


def test_primary_navigation_has_six_choices_and_retains_legacy(client, make_product, make_template):
    make_product(group_code="FAMILY-A")
    make_template(contract_type="Báo giá", product_group="FAMILY-A", name="Mẫu cũ theo sản phẩm")
    response = client.get("/quotation", follow_redirects=True)
    assert response.status_code == 200
    for key in ("compact", "formal", "landscape", "features", "terms", "legacy"):
        assert f'value="{key}"' in response.text
    assert 'href="/quotation/request"' not in response.text
    assert client.get("/quotation/request", follow_redirects=False).status_code == 303
    legacy = client.get("/quotation/legacy")
    assert legacy.status_code == 200 and "Mẫu cũ theo sản phẩm" in legacy.text


def test_two_legacy_product_families_keep_original_files_and_group_checks(
        client, make_product, make_template, tmp_path):
    for index, group in enumerate(("FAMILY-A", "FAMILY-B")):
        product = make_product(group_code=group)
        path = tmp_path / f"legacy-{index}.docx"
        doc = Document()
        doc.add_paragraph("BÁO GIÁ CŨ {{PARTY_A_NAME}} {{TOTAL_AMOUNT}}")
        doc.save(path)
        original = path.read_bytes()
        template = make_template(contract_type="Báo giá", product_group=group, file_path=str(path))
        response = client.get("/quotation/legacy/template", params={"product_id": product.id,
                                                                   "template_id": template.id})
        assert response.status_code == 200 and response.content == original
        assert path.read_bytes() == original
        other = make_product(group_code="OTHER")
        refused = client.get("/quotation/legacy/template", params={"product_id": other.id,
                                                                   "template_id": template.id}, follow_redirects=False)
        assert refused.status_code == 303


@pytest.mark.parametrize("key", ["quantity", "periods", "price", "vat"])
@pytest.mark.parametrize("raw", ["NaN", "Infinity", "-Infinity"])
def test_preview_rejects_non_finite_fields(client, key, raw):
    rows = json.loads(form()["items_json"])
    rows[0][key] = raw
    response = client.post("/quotation/manual/preview", data={"items_json":json.dumps(rows)})
    assert response.status_code == 422


def test_manual_generation_does_not_change_catalog_or_other_issuers(client, db_session, make_product):
    product = make_product(default_price=777123, default_vat_rate=10)
    first = client.post("/quotation/manual/generate", data=form(product_id=product.id,issuer_name="BÊN THỨ NHẤT"))
    second = client.post("/quotation/manual/generate", data=form(product_id=product.id,issuer_name="BÊN THỨ HAI"))
    assert first.status_code == second.status_code == 200
    assert "BÊN THỨ NHẤT" not in text(second.content)
    db_session.refresh(product)
    assert product.default_price == 777123 and product.default_vat_rate == 10


def test_fractional_unit_price_remains_visible_and_matches_line_total(client):
    payload = form(items_json=json.dumps([dict(name="Dịch vụ số lẻ", unit="Gói", quantity="10",
                                              periods="1", price="100,5", vat="0")]))
    response = client.post("/quotation/manual/generate", data=payload)
    assert response.status_code == 200
    doc = Document(BytesIO(response.content))
    cells = [cell.text for table in doc.tables for row in table.rows for cell in row.cells]
    assert "100,5" in cells
    assert any("1.005" in cell for cell in cells)
