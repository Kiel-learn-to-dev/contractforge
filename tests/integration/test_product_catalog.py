"""Task 1.3 — thêm và sửa sản phẩm trên giao diện /products.

Dữ liệu hoàn toàn hư cấu. Các test POST đi qua fixture `client` (TestClient nối
vào DB tạm).
"""

import os

from decimal import Decimal


def _form(**overrides):
    base = {
        "code": "SW-NEW",
        "name": "Gói mới",
        "product_type": "Dịch vụ",
        "group_code": "",
        "kind": "main",
        "unit_label": "",
        "default_price": "500.000",
        "default_vat_rate": "8.5",
        "price_period_months": "1",
        "quantity_source": "units",
        "min_quantity": "",
        "quantity_step": "",
        "account_limit": "",
        "default_duration_months": "12",
        "default_template_id": "",
        "contract_number_format": "",
        "replaced_by_id": "",
        "sort_order": "0",
    }
    base.update(overrides)
    return base


def test_new_form_renders_every_field(client):
    resp = client.get("/products/new")
    assert resp.status_code == 200
    for field in ("code", "name", "product_type", "group_code", "kind", "unit_label",
                  "default_price", "default_vat_rate", "price_period_months",
                  "quantity_source", "min_quantity", "quantity_step", "account_limit",
                  "default_duration_months", "default_template_id",
                  "contract_number_format", "replaced_by_id", "sort_order"):
        assert f'name="{field}"' in resp.text, field


def test_post_new_product_saves_every_field(client, db_session, make_template):
    from app.models.product import Product

    template = make_template()
    resp = client.post("/products/new", data=_form(
        code="sw-new",
        group_code="NHOM-A",
        unit_label="cơ sở",
        price_period_months="12",
        quantity_source="records",
        min_quantity="2000",
        quantity_step="500",
        account_limit="3",
        default_template_id=str(template.id),
        contract_number_format="{seq}/{slug}/{year}",
        sort_order="2",
    ), follow_redirects=False)

    assert resp.status_code == 303
    db_session.expire_all()
    product = db_session.query(Product).filter_by(code="SW-NEW").one()
    assert product.name == "Gói mới"
    assert product.kind == "main"
    assert product.group_code == "NHOM-A"
    assert product.unit_label == "cơ sở"
    assert product.default_price == Decimal("500000")
    assert product.default_vat_rate == Decimal("8.5")
    assert product.price_period_months == 12
    assert product.quantity_source == "records"
    assert product.min_quantity == 2000
    assert product.quantity_step == 500
    assert product.account_limit == 3
    assert product.default_duration_months == 12
    assert product.default_template_id == template.id
    assert product.contract_number_format == "{seq}/{slug}/{year}"
    assert product.sort_order == 2
    assert product.is_active is True


def test_duplicate_code_is_refused_and_typed_values_are_kept(client, db_session, make_product):
    from app.models.product import Product

    make_product(code="SW-DUP")
    resp = client.post("/products/new", data=_form(code="sw-dup", name="Tên giữ lại"))

    assert resp.status_code == 422
    assert "đã tồn tại" in resp.text
    assert 'value="Tên giữ lại"' in resp.text
    db_session.expire_all()
    assert db_session.query(Product).filter_by(code="SW-DUP").count() == 1


def test_addon_without_group_is_refused(client, db_session):
    from app.models.product import Product

    resp = client.post("/products/new", data=_form(code="SW-ADD", kind="addon", group_code=""))

    assert resp.status_code == 422
    assert "phải thuộc một nhóm" in resp.text
    db_session.expire_all()
    assert db_session.query(Product).filter_by(code="SW-ADD").count() == 0


def test_contract_number_format_without_seq_is_refused(client, db_session):
    from app.models.product import Product

    resp = client.post("/products/new", data=_form(
        code="SW-FMT", contract_number_format="{slug}/{year}"))

    assert resp.status_code == 422
    assert "phải chứa" in resp.text
    db_session.expire_all()
    assert db_session.query(Product).filter_by(code="SW-FMT").count() == 0


def test_vat_above_one_hundred_is_refused(client):
    resp = client.post("/products/new", data=_form(code="SW-VAT", default_vat_rate="150"))
    assert resp.status_code == 422
    assert "trong khoảng 0 đến 100" in resp.text


def test_product_cannot_replace_itself(client, db_session, make_product):
    product = make_product(code="SW-SELF")
    resp = client.post(f"/products/{product.id}/edit", data=_form(
        code="SW-SELF", replaced_by_id=str(product.id)))

    assert resp.status_code == 422
    assert "không được là chính sản phẩm này" in resp.text


def test_edit_form_prefills_existing_values(client, make_product):
    product = make_product(code="SW-EDIT", name="Tên cũ", group_code="NHOM-B",
                           default_price=1_500_000, default_vat_rate=10)
    resp = client.get(f"/products/{product.id}/edit")

    assert resp.status_code == 200
    assert 'value="Tên cũ"' in resp.text
    assert 'value="NHOM-B"' in resp.text
    assert 'value="1500000"' in resp.text


def test_edit_missing_product_returns_404(client):
    assert client.get("/products/999999/edit").status_code == 404


def test_edit_price_changes_catalog_but_not_an_existing_contract(client, db_session,
                                                                 make_product, make_contract):
    from app.models.contract import Contract
    from app.models.product import Product

    product = make_product(code="SW-PRICE", default_price=1_000_000)
    contract = make_contract(product=product, unit_price_pre_vat=1_000_000,
                             vat_rate=10, total_amount=1_100_000)

    resp = client.post(f"/products/{product.id}/edit", data=_form(
        code="SW-PRICE", name=product.name, default_price="2.000.000"),
        follow_redirects=False)

    assert resp.status_code == 303
    db_session.expire_all()
    assert db_session.get(Product, product.id).default_price == Decimal("2000000")
    reloaded = db_session.get(Contract, contract.id)
    assert reloaded.unit_price_pre_vat == Decimal("1000000")
    assert reloaded.total_amount == Decimal("1100000")


def test_list_groups_products_under_a_heading(client, make_product):
    make_product(code="SW-MAIN", name="Gói chính", group_code="NHOM-X", kind="main")
    make_product(code="SW-ADDON", name="Gói kèm", group_code="NHOM-X", kind="addon")
    resp = client.get("/products")

    assert resp.status_code == 200
    assert "NHOM-X" in resp.text
    assert "Thêm sản phẩm" in resp.text
    assert "Gói bổ sung" in resp.text


def test_list_orders_main_before_addon_and_ungrouped_last(db_session, make_product):
    from app.services.product_service import list_products

    make_product(code="A-UNGROUPED", name="Không nhóm", group_code=None, kind="main")
    make_product(code="B-ADDON", name="Kèm", group_code="NHOM-Y", kind="addon", sort_order=0)
    make_product(code="C-MAIN", name="Chính", group_code="NHOM-Y", kind="main", sort_order=5)

    codes = [p.code for p in list_products(db_session)]
    assert codes == ["C-MAIN", "B-ADDON", "A-UNGROUPED"]


# ─── Task 1.4: ngừng bán và bật lại ──────────────────────────────────────────

def test_stopped_product_disappears_from_contract_and_quotation_forms(client, make_product):
    product = make_product(code="SW-OFF-HK", name="Gói ngừng hư cấu")

    assert "SW-OFF-HK" in client.get("/contracts/new").text  # đối chứng: đang hiện

    resp = client.post(f"/products/{product.id}/toggle-active",
                       data={"show": "active"}, follow_redirects=False)
    assert resp.status_code == 303

    assert "SW-OFF-HK" not in client.get("/contracts/new").text
    assert "SW-OFF-HK" not in client.get("/quotation").text


def test_stopped_product_keeps_existing_contract_readable_and_numbered(client, make_product,
                                                                      make_contract):
    from datetime import date

    product = make_product(code="SW-OLD-HK", name="Gói cũ hư cấu",
                           contract_number_format="{seq}/{slug}/{year}")
    contract = make_contract(product=product)

    client.post(f"/products/{product.id}/toggle-active", data={"show": "active"},
                follow_redirects=False)

    detail = client.get(f"/contracts/{contract.id}")
    assert detail.status_code == 200
    assert contract.contract_number in detail.text
    assert "Gói cũ hư cấu" in detail.text  # tên sản phẩm hiện trên trang chi tiết

    suggest = client.get(
        f"/contracts/suggest-number?customer_id={contract.customer_id}&product_id={product.id}")
    assert suggest.status_code == 200
    number = suggest.json()["number"]
    assert number.endswith(f"/{date.today().year}")
    assert "{" not in number


def test_count_in_service_counts_only_in_service_statuses(db_session, make_product,
                                                          make_contract):
    from app.models.contract import ContractStatus
    from app.services.product_service import count_in_service

    product = make_product(code="SW-CNT")
    # Theo lifecycle.IN_SERVICE_STATUSES, Signed chưa được tính là đang phục vụ.
    make_contract(product=product, status=ContractStatus.Signed)
    make_contract(product=product, status=ContractStatus.Signed)
    make_contract(product=product, status=ContractStatus.Draft)
    assert count_in_service(db_session, product.id) == 0

    make_contract(product=product, status=ContractStatus.Invoiced)
    assert count_in_service(db_session, product.id) == 1


def test_toggle_flashes_message_and_restores_active(client, db_session, make_product):
    from urllib.parse import unquote

    from app.models.product import Product

    product = make_product(code="SW-TOG", name="Gói bật tắt")

    stop = client.post(f"/products/{product.id}/toggle-active", data={"show": "active"},
                       follow_redirects=False)
    assert stop.status_code == 303
    assert "Đã ngừng bán Gói bật tắt" in unquote(stop.headers["location"])
    db_session.expire_all()
    assert db_session.get(Product, product.id).is_active is False

    resume = client.post(f"/products/{product.id}/toggle-active", data={"show": "inactive"},
                         follow_redirects=False)
    assert resume.status_code == 303
    assert resume.headers["location"].startswith("/products?show=inactive")
    assert "Đã bán lại Gói bật tắt" in unquote(resume.headers["location"])
    db_session.expire_all()
    assert db_session.get(Product, product.id).is_active is True


def test_toggle_on_missing_product_returns_404(client):
    resp = client.post("/products/999999/toggle-active", data={"show": "active"},
                       follow_redirects=False)
    assert resp.status_code == 404


def test_list_hides_stopped_products_by_default_and_filters_by_show(client, make_product):
    make_product(code="SW-SHOW-ON", name="Đang bán hư cấu")
    make_product(code="SW-SHOW-OFF", name="Đã ngừng hư cấu", is_active=False)

    default = client.get("/products").text
    assert "SW-SHOW-ON" in default
    assert "SW-SHOW-OFF" not in default

    inactive = client.get("/products?show=inactive").text
    assert "SW-SHOW-OFF" in inactive
    assert "SW-SHOW-ON" not in inactive

    everything = client.get("/products?show=all").text
    assert "SW-SHOW-ON" in everything
    assert "SW-SHOW-OFF" in everything


# ─── Task 1.5: nhập và xuất danh mục bằng Excel ──────────────────────────────

XLSX_MIME = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"


def _columns():
    from app.services.product_service import IMPORT_COLUMNS
    return IMPORT_COLUMNS


def _xlsx(rows, header=None):
    from openpyxl import Workbook

    wb = Workbook()
    try:
        ws = wb.active
        ws.append(header or _columns())
        for r in rows:
            ws.append(r)
        import io
        buf = io.BytesIO()
        wb.save(buf)
        return buf.getvalue()
    finally:
        wb.close()


def _read_xlsx(content):
    import io

    from openpyxl import load_workbook

    wb = load_workbook(io.BytesIO(content), read_only=True, data_only=True)
    try:
        return [list(r) for r in wb.active.iter_rows(values_only=True)]
    finally:
        wb.close()


def _row(**overrides):
    base = {col: None for col in _columns()}
    base.update(code="IM-NEW", name="Gói nhập", product_type="Dịch vụ", kind="main",
                default_price=500000, default_vat_rate=10, price_period_months=1,
                quantity_source="units", default_duration_months=12, is_active=1,
                sort_order=0)
    base.update(overrides)
    return [base[c] for c in _columns()]


def test_template_download_has_header_and_two_fictional_examples(client):
    resp = client.get("/products/template.xlsx")
    assert resp.status_code == 200
    rows = _read_xlsx(resp.content)
    assert rows[0] == _columns()
    assert [r[0] for r in rows[1:]] == ["EX-MAIN", "EX-ADDON"]


def test_preview_reports_create_update_and_error_by_row(client, db_session, make_product):
    from app.models.product import Product

    make_product(code="IM-OLD", name="Tên cũ")
    content = _xlsx([
        _row(code="IM-NEW"),
        _row(code="IM-OLD", name="Tên mới"),
        _row(code="IM-BAD", default_price="abc"),
    ])
    resp = client.post("/products/import",
                       files={"file": ("cat.xlsx", content, XLSX_MIME)})

    assert resp.status_code == 200
    assert "Tạo mới" in resp.text
    assert "Cập nhật" in resp.text
    assert "Đơn giá không phải là số" in resp.text
    assert "Ghi 3 dòng" not in resp.text  # có lỗi thì không cho ghi
    db_session.expire_all()
    assert db_session.query(Product).filter_by(code="IM-NEW").count() == 0


def test_price_cell_is_a_machine_number_not_vietnamese_thousands(db_session):
    from decimal import Decimal

    from app.services.product_service import parse_catalog_xlsx

    rows = parse_catalog_xlsx(db_session, _xlsx([_row(code="IM-DEC", default_price=82.5)]))
    assert rows[0]["action"] == "create"
    assert rows[0]["data"]["default_price"] == Decimal("82.5")


def test_apply_refuses_the_whole_file_when_one_row_is_wrong(db_session):
    from app.models.product import Product
    from app.services.product_service import (ProductError, apply_catalog_rows,
                                              parse_catalog_xlsx)
    import pytest

    rows = parse_catalog_xlsx(db_session, _xlsx([
        _row(code="IM-A"),
        _row(code="IM-B", default_price="abc"),
    ]))
    with pytest.raises(ProductError):
        apply_catalog_rows(db_session, rows)
    db_session.expire_all()
    assert db_session.query(Product).filter_by(code="IM-A").count() == 0


def test_apply_counts_created_and_updated_rows(db_session, make_product):
    from decimal import Decimal

    from app.models.product import Product
    from app.services.product_service import apply_catalog_rows, parse_catalog_xlsx

    make_product(code="IM-OLD", name="Tên cũ")
    rows = parse_catalog_xlsx(db_session, _xlsx([
        _row(code="IM-NEW"),
        _row(code="IM-OLD", name="Tên mới"),
    ]))
    assert apply_catalog_rows(db_session, rows) == (1, 1)

    db_session.expire_all()
    assert db_session.query(Product).filter_by(code="IM-OLD").one().name == "Tên mới"
    created = db_session.query(Product).filter_by(code="IM-NEW").one()
    assert created.default_price == Decimal("500000")
    assert created.is_active is True


def test_import_never_changes_default_template_or_replacement(db_session, make_product,
                                                              make_template):
    from app.models.product import Product
    from app.services.product_service import apply_catalog_rows, parse_catalog_xlsx

    template = make_template()
    successor = make_product(code="RT-NEW", name="Gói thay thế")
    make_product(code="RT-KEEP", name="Cũ", default_template_id=template.id,
                 replaced_by_id=successor.id)

    rows = parse_catalog_xlsx(db_session, _xlsx([_row(code="RT-KEEP", name="Mới")]))
    apply_catalog_rows(db_session, rows)

    db_session.expire_all()
    kept = db_session.query(Product).filter_by(code="RT-KEEP").one()
    assert kept.name == "Mới"
    assert kept.default_template_id == template.id
    assert kept.replaced_by_id == successor.id


def test_duplicate_code_inside_the_file_is_an_error(db_session):
    from app.services.product_service import parse_catalog_xlsx

    rows = parse_catalog_xlsx(db_session, _xlsx([_row(code="IM-DUP"), _row(code="im-dup")]))
    assert [r["action"] for r in rows] == ["create", "error"]
    assert "xuất hiện nhiều lần" in rows[1]["error"]


def test_missing_column_is_refused_on_upload(client):
    content = _xlsx([], header=_columns()[:-1])
    resp = client.post("/products/import",
                       files={"file": ("cat.xlsx", content, XLSX_MIME)})
    assert resp.status_code == 422
    assert "thiếu cột" in resp.text


def test_export_then_import_the_same_file_changes_nothing(client, db_session, make_product):
    from decimal import Decimal

    from app.models.product import Product
    from app.services.product_service import (apply_catalog_rows, form_values,
                                              parse_catalog_xlsx)

    make_product(code="RT-A", name="Gói A", group_code="NHOM-RT", kind="main",
                 unit_label="cơ sở", default_price=Decimal("500000"),
                 default_vat_rate=Decimal("8.5"), min_quantity=2000, quantity_step=500,
                 contract_number_format="{seq}/{slug}/{year}", sort_order=3)
    make_product(code="RT-B", name="Gói B", group_code="NHOM-RT", kind="addon",
                 default_price=Decimal("150000"), default_vat_rate=Decimal("10"),
                 account_limit=3, is_active=False)

    def snapshot():
        db_session.expire_all()
        return {p.code: {**form_values(p), "is_active": p.is_active}
                for p in db_session.query(Product).all()}

    before = snapshot()
    exported = client.get("/products/export.xlsx")
    assert exported.status_code == 200
    rows = parse_catalog_xlsx(db_session, exported.content)
    assert len(rows) == 2
    assert all(r["action"] == "update" for r in rows)
    assert apply_catalog_rows(db_session, rows) == (0, 2)
    assert snapshot() == before


def test_apply_route_writes_the_rows_from_the_preview(client, db_session):
    import json
    from decimal import Decimal
    from urllib.parse import unquote

    from app.models.product import Product
    from app.services.product_service import parse_catalog_xlsx

    rows = parse_catalog_xlsx(db_session, _xlsx([_row(code="IM-VIA", default_vat_rate=8.5)]))
    resp = client.post("/products/import/apply",
                       data={"rows_json": json.dumps(rows, ensure_ascii=False, default=str)},
                       follow_redirects=False)

    assert resp.status_code == 303
    assert "Đã tạo 1 sản phẩm" in unquote(resp.headers["location"])
    db_session.expire_all()
    product = db_session.query(Product).filter_by(code="IM-VIA").one()
    assert product.default_vat_rate == Decimal("8.5")


def test_apply_route_refuses_preview_that_still_has_an_error(client, db_session):
    import json
    from urllib.parse import unquote

    from app.models.product import Product
    from app.services.product_service import parse_catalog_xlsx

    rows = parse_catalog_xlsx(db_session, _xlsx([
        _row(code="IM-HALF"), _row(code="IM-BROKEN", default_price="abc")]))
    resp = client.post("/products/import/apply",
                       data={"rows_json": json.dumps(rows, ensure_ascii=False, default=str)},
                       follow_redirects=False)

    assert resp.status_code == 303
    assert "Chưa ghi file nào" in unquote(resp.headers["location"])
    db_session.expire_all()
    assert db_session.query(Product).filter_by(code="IM-HALF").count() == 0


def test_export_lists_every_product_with_header(client, make_product):
    make_product(code="EXP-1", name="Một")
    make_product(code="EXP-2", name="Hai")
    rows = _read_xlsx(client.get("/products/export.xlsx").content)
    assert rows[0] == _columns()
    assert sorted(r[0] for r in rows[1:]) == ["EXP-1", "EXP-2"]


def test_stopped_product_contract_still_regenerates_its_file(db_session, make_product,
                                                            make_contract, make_template,
                                                            tmp_path):
    import docx

    from app.services.contract_service import generate_docx

    product = make_product(code="SW-REGEN-HK", name="Gói ngừng sinh lại", is_active=False)
    doc = docx.Document()
    doc.add_paragraph("Hợp đồng {{CONTRACT_NUMBER}}")
    template_path = tmp_path / "regen_template.docx"
    doc.save(str(template_path))
    template = make_template(file_path=str(template_path), contract_type="Hợp đồng")
    contract = make_contract(product=product, template_id=template.id,
                             contract_number="HD-REGEN-STOP-1")

    result = generate_docx(db_session, contract.id)

    assert result.status.value == "Generated"
    assert result.output_file_path and os.path.exists(result.output_file_path)
    assert product.is_active is False
