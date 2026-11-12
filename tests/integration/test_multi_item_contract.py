"""Task 2.2 — create_draft nhận danh sách dòng (items_json).

Dữ liệu hoàn toàn hư cấu.

Phần đầu file là test ĐẶC TẢ hành vi cũ (không có items_json). Chúng được viết và
chạy xanh TRƯỚC khi sửa create_draft, để chứng minh nhánh cũ không đổi.
"""

import json
import pathlib
from decimal import Decimal

import pytest


def _legacy_form(customer, product, **overrides):
    base = {
        "contract_number": "HD-LEG-001",
        "customer_id": str(customer.id),
        "product_id": str(product.id),
        "start_date": "01/01/2026",
        "month_count": "12",
        "unit_count": "2",
        "unit_price_pre_vat": "500000",
        "vat_rate": "10",
    }
    base.update(overrides)
    return base


# ─── Đặc tả hành vi cũ (không có items_json) — viết TRƯỚC khi sửa code ──────

def test_legacy_form_without_items_json_keeps_its_stored_numbers(make_customer, make_product,
                                                                 db_session):
    from app.services.contract_service import create_draft

    customer = make_customer()
    product = make_product(default_price=1_000_000, default_vat_rate=10)
    contract = create_draft(db_session, _legacy_form(customer, product))

    assert contract.pricing_model == "legacy"
    assert contract.unit_price_pre_vat == Decimal("500000")
    assert contract.vat_rate == Decimal("10")
    assert contract.unit_price_vat == Decimal("550000")
    assert contract.quantity == Decimal("24")
    assert contract.total_amount == Decimal("13200000")
    assert contract.amount_text
    assert contract.items == []


def test_legacy_form_without_items_json_is_not_changed_by_a_price_with_decimal_comma(
        make_customer, make_product, db_session):
    from app.services.contract_service import create_draft

    customer = make_customer()
    product = make_product(default_price=1_000_000, default_vat_rate=10)
    contract = create_draft(db_session, _legacy_form(
        customer, product, contract_number="HD-LEG-002",
        unit_price_pre_vat="500.000", vat_rate="8"))

    assert contract.unit_price_pre_vat == Decimal("500000")
    assert contract.vat_rate == Decimal("8")
    # Cách tính cũ: đơn giá có VAT × số lượng = 540.000 × 24
    assert contract.total_amount == Decimal("12960000")


# ─── Đường mới: items_json ───────────────────────────────────────────────────

def _items_form(customer, main, items, contract_number="HD-ITEMS-001", **overrides):
    base = {
        "contract_number": contract_number,
        "customer_id": str(customer.id),
        "product_id": str(main.id),
        "start_date": "01/01/2026",
        "month_count": "12",
        "items_json": json.dumps(items),
    }
    base.update(overrides)
    return base


def _product_row(product_id, quantity, **extra):
    row = {"product_id": product_id, "quantity": str(quantity)}
    row.update(extra)
    return row


def test_two_lines_main_and_addon_are_priced_per_line(make_customer, make_product, db_session):
    from app.models.contract_item import ContractItem
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="G-MAIN", name="Gói chính hư cấu", kind="main",
                        group_code="NHOM-1", unit_label="cơ sở",
                        default_price=500_000, default_vat_rate=10)
    addon = make_product(code="G-ADD", name="Gói kèm hư cấu", kind="addon",
                         group_code="NHOM-1", default_price=150_000, default_vat_rate=10)

    contract = create_draft(db_session, _items_form(customer, main, [
        _product_row(main.id, 2),
        _product_row(addon.id, 1),
    ]))

    # Dòng 1: 500.000 × 2 × 12 = 12.000.000 + 10%. Dòng 2: 150.000 × 1 × 12 = 1.800.000 + 10%.
    assert contract.pricing_model == "items"
    assert contract.total_pre_vat == Decimal("13800000")
    assert contract.total_vat == Decimal("1380000")
    assert contract.total_amount == Decimal("15180000")
    assert contract.product_id == main.id
    assert contract.unit_count == 2
    assert contract.quantity == Decimal("24")
    assert contract.unit_price_pre_vat == Decimal("500000")
    assert contract.vat_rate == Decimal("10")
    assert contract.amount_text

    db_session.expire_all()
    lines = (db_session.query(ContractItem)
             .filter(ContractItem.contract_id == contract.id)
             .order_by(ContractItem.sort_order).all())
    assert [(l.product_id, l.sort_order) for l in lines] == [(main.id, 0), (addon.id, 1)]
    assert [l.amount_total for l in lines] == [Decimal("13200000"), Decimal("1980000")]
    assert "Tổng cộng: 15.180.000 VNĐ" in contract.pricing_description


def test_quantity_is_raised_to_the_minimum_and_rounded_up_to_the_step(make_customer, make_product,
                                                                     db_session):
    from app.models.contract_item import ContractItem
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="G-REC", name="Gói theo hồ sơ", kind="main", group_code="NHOM-2",
                        default_price=1_000, default_vat_rate=0,
                        min_quantity=2000, quantity_step=500, quantity_source="records")

    contract = create_draft(db_session, _items_form(customer, main,
                                                    [_product_row(main.id, 1200)],
                                                    contract_number="HD-ITEMS-002"))
    db_session.expire_all()
    line = db_session.query(ContractItem).filter_by(contract_id=contract.id).one()
    assert line.quantity == Decimal("2000")


def test_period_count_is_one_when_the_price_period_matches_the_contract(make_customer,
                                                                        make_product, db_session):
    from app.models.contract_item import ContractItem
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="G-YR", name="Gói năm", kind="main", group_code="NHOM-3",
                        default_price=16_320, default_vat_rate=10, price_period_months=12)

    contract = create_draft(db_session, _items_form(customer, main,
                                                    [_product_row(main.id, 5)],
                                                    contract_number="HD-ITEMS-003"))
    db_session.expire_all()
    line = db_session.query(ContractItem).filter_by(contract_id=contract.id).one()
    assert line.period_count == Decimal("1")
    assert line.amount_pre_vat == Decimal("81600")
    assert line.vat_amount == Decimal("8160")


def test_first_line_must_be_a_main_package(make_customer, make_product, db_session):
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="G-M1", name="Gói chính", kind="main", group_code="NHOM-4")
    addon = make_product(code="G-A1", name="Gói kèm", kind="addon", group_code="NHOM-4")

    with pytest.raises(ValueError, match="gói chính"):
        create_draft(db_session, _items_form(customer, main,
                                             [_product_row(addon.id, 1),
                                              _product_row(main.id, 1)],
                                             contract_number="HD-ITEMS-004"))


def test_addon_from_another_group_is_refused(make_customer, make_product, db_session):
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="G-M2", name="Gói chính", kind="main", group_code="NHOM-5")
    other = make_product(code="G-A2", name="Gói kèm nhóm khác", kind="addon",
                         group_code="NHOM-6")

    with pytest.raises(ValueError, match="nhóm"):
        create_draft(db_session, _items_form(customer, main,
                                             [_product_row(main.id, 1),
                                              _product_row(other.id, 1)],
                                             contract_number="HD-ITEMS-005"))


def test_unknown_product_is_refused_with_its_line_number(make_customer, make_product, db_session):
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="G-M3", name="Gói chính", kind="main", group_code="NHOM-7")

    with pytest.raises(ValueError, match="Dòng 1"):
        create_draft(db_session, _items_form(customer, main,
                                             [_product_row(999999, 1)],
                                             contract_number="HD-ITEMS-006"))


def test_price_in_items_json_is_a_machine_number(make_customer, make_product, db_session):
    from app.models.contract_item import ContractItem
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="G-DEC", name="Gói thập phân", kind="main", group_code="NHOM-8",
                        default_price=1, default_vat_rate=0)

    contract = create_draft(db_session, _items_form(customer, main, [
        _product_row(main.id, 1, unit_price_pre_vat="82.5", vat_rate="0"),
    ], contract_number="HD-ITEMS-007", month_count="1"))
    db_session.expire_all()
    line = db_session.query(ContractItem).filter_by(contract_id=contract.id).one()
    assert line.unit_price_pre_vat == Decimal("82.5")
    assert line.amount_pre_vat == Decimal("83")  # 82,5 làm tròn lên thành đồng


def test_items_json_without_months_is_refused(make_customer, make_product, db_session):
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="G-M4", name="Gói chính", kind="main", group_code="NHOM-9")

    with pytest.raises(ValueError, match="số tháng"):
        create_draft(db_session, _items_form(customer, main, [_product_row(main.id, 1)],
                                             contract_number="HD-ITEMS-008", month_count=""))


def test_items_json_that_is_not_a_list_is_refused(make_customer, make_product, db_session):
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="G-M5", name="Gói chính", kind="main", group_code="NHOM-10")
    form = _items_form(customer, main, [], contract_number="HD-ITEMS-009")
    form["items_json"] = '{"product_id": 1}'

    with pytest.raises(ValueError):
        create_draft(db_session, form)


# ─── Task 2.3: API xem trước giá và danh sách gói bổ sung ────────────────────

def test_product_options_lists_main_and_only_active_addons_of_the_same_group(
        client, make_product, make_customer):
    main = make_product(code="PO-MAIN", name="Gói chính PO", kind="main", group_code="PO-G",
                        default_price=500_000, default_vat_rate=10, unit_label="cơ sở")
    make_product(code="PO-ADD-B", name="Kèm B", kind="addon", group_code="PO-G", sort_order=2)
    make_product(code="PO-ADD-A", name="Kèm A", kind="addon", group_code="PO-G", sort_order=1)
    make_product(code="PO-OFF", name="Kèm đã ngừng", kind="addon", group_code="PO-G",
                 is_active=False)
    make_product(code="PO-OTHER", name="Kèm nhóm khác", kind="addon", group_code="PO-X")

    resp = client.get(f"/contracts/product-options?product_id={main.id}&unit_count=3")

    assert resp.status_code == 200
    body = resp.json()
    assert body["main"]["code"] == "PO-MAIN"
    assert body["main"]["price"] == "500000"
    assert body["main"]["vat"] == "10"
    assert body["main"]["default_quantity"] == "3"
    assert [a["code"] for a in body["addons"]] == ["PO-ADD-A", "PO-ADD-B"]


def test_product_options_default_quantity_follows_the_quantity_source(
        client, make_product, make_customer):
    customer = make_customer(record_count=1200)
    records = make_product(code="PO-REC", name="Theo hồ sơ", kind="main", group_code="PO-R",
                           quantity_source="records", min_quantity=2000, quantity_step=500)
    manual = make_product(code="PO-MAN", name="Nhập tay", kind="main", group_code="PO-M",
                          quantity_source="manual")

    rec = client.get(f"/contracts/product-options?product_id={records.id}"
                     f"&customer_id={customer.id}").json()["main"]
    man = client.get(f"/contracts/product-options?product_id={manual.id}").json()["main"]
    no_count = make_customer(record_count=None)
    empty = client.get(f"/contracts/product-options?product_id={records.id}"
                       f"&customer_id={no_count.id}").json()["main"]

    assert rec["default_quantity"] == "2000"
    assert man["default_quantity"] == "1"
    assert empty["default_quantity"] == ""


def test_product_options_for_a_missing_product_returns_no_main(client):
    assert client.get("/contracts/product-options?product_id=999999").json() == {
        "main": None, "addons": [],
        "templates": {"contract": [], "attachments": [], "default_id": None}}


def test_price_preview_returns_lines_totals_and_description_as_strings(client, make_product):
    main = make_product(code="PP-MAIN", name="Gói chính PP", kind="main", group_code="PP-G",
                        default_price=500_000, default_vat_rate=10, unit_label="cơ sở")
    addon = make_product(code="PP-ADD", name="Gói kèm PP", kind="addon", group_code="PP-G",
                         default_price=150_000, default_vat_rate=10)

    resp = client.post("/contracts/price-preview", json={"month_count": 12, "items": [
        {"product_id": main.id, "quantity": "2"},
        {"product_id": addon.id, "quantity": "1"},
    ]})

    assert resp.status_code == 200
    body = resp.json()
    assert [l["amount_total"] for l in body["lines"]] == ["13200000", "1980000"]
    assert body["totals"] == {"pre_vat": "13800000", "vat": "1380000", "total": "15180000",
                              "amount_text": body["totals"]["amount_text"]}
    assert body["totals"]["amount_text"]
    assert "Tổng cộng: 15.180.000 VNĐ" in body["description"]
    for line in body["lines"]:
        for key, value in line.items():
            assert isinstance(value, str) or key == "description", key


def test_price_preview_with_no_lines_is_422(client):
    resp = client.post("/contracts/price-preview", json={"month_count": 12, "items": []})
    assert resp.status_code == 422
    assert "error" in resp.json()


def test_price_preview_without_months_or_with_bad_json_is_422(client, make_product):
    main = make_product(code="PP-BAD", name="Gói", kind="main", group_code="PP-B")
    no_months = client.post("/contracts/price-preview",
                            json={"items": [{"product_id": main.id, "quantity": "1"}]})
    assert no_months.status_code == 422

    bad_json = client.post("/contracts/price-preview", content=b"{not json",
                           headers={"Content-Type": "application/json"})
    assert bad_json.status_code == 422


def test_preview_matches_the_numbers_create_draft_saves(client, db_session, make_customer,
                                                        make_product):
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="PM-MAIN", name="Gói chính PM", kind="main", group_code="PM-G",
                        default_price=440_909, default_vat_rate=8, unit_label="cơ sở")
    addon = make_product(code="PM-ADD", name="Gói kèm PM", kind="addon", group_code="PM-G",
                         default_price=75, default_vat_rate=10)
    items = [{"product_id": main.id, "quantity": "3"},
             {"product_id": addon.id, "quantity": "2000"}]

    preview = client.post("/contracts/price-preview",
                          json={"month_count": 12, "items": items}).json()
    contract = create_draft(db_session, _items_form(customer, main, items,
                                                    contract_number="HD-PM-001"))

    assert preview["totals"]["total"] == str(int(contract.total_amount))
    assert preview["totals"]["pre_vat"] == str(int(contract.total_pre_vat))
    assert preview["totals"]["vat"] == str(int(contract.total_vat))


# ─── Task 2.4: trang chi tiết hiện các dòng ──────────────────────────────────

def test_detail_of_an_items_contract_shows_both_lines_and_formatted_totals(
        client, db_session, make_customer, make_product):
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="DT-MAIN", name="Gói chính chi tiết", kind="main",
                        group_code="DT-G", unit_label="cơ sở",
                        default_price=500_000, default_vat_rate=10)
    addon = make_product(code="DT-ADD", name="Gói kèm chi tiết", kind="addon",
                         group_code="DT-G", default_price=150_000, default_vat_rate=10)
    contract = create_draft(db_session, _items_form(
        customer, main, [_product_row(main.id, 2), _product_row(addon.id, 1)],
        contract_number="HD-DT-001", payment_mode="prepaid", payment_cycle_months="6"))

    resp = client.get(f"/contracts/{contract.id}")

    assert resp.status_code == 200
    assert "Gói chính chi tiết" in resp.text
    assert "Gói kèm chi tiết" in resp.text
    assert "15.180.000" in resp.text
    assert "13.800.000" in resp.text
    assert "Đơn giá chưa VAT" not in resp.text
    assert "trả trước" in resp.text
    assert "chu kỳ 6 tháng" in resp.text


def test_detail_of_a_legacy_contract_keeps_the_stored_price_block(client, make_contract):
    contract = make_contract(unit_price_pre_vat=Decimal("500000"), vat_rate=10,
                             unit_price_vat=Decimal("550000"), total_amount=Decimal("13200000"))

    resp = client.get(f"/contracts/{contract.id}")

    assert resp.status_code == 200
    assert "Đơn giá chưa VAT" in resp.text
    assert "13.200.000" in resp.text


# ─── Task 2.5: form lập hợp đồng nhiều dòng ──────────────────────────────────

FORM_PATH = pathlib.Path(__file__).resolve().parents[2] / "ContractForge" / "templates" / "contracts" / "form.html"


def test_new_form_has_no_price_inputs_and_posts_items_json(client):
    resp = client.get("/contracts/new")

    assert resp.status_code == 200
    assert 'name="unit_price_pre_vat"' not in resp.text
    assert 'name="total_amount"' not in resp.text
    assert 'name="quantity"' not in resp.text
    assert 'name="items_json"' in resp.text
    assert 'id="tbl-items"' in resp.text
    assert 'name="payment_mode"' in resp.text


def test_new_form_dropdown_lists_main_packages_only(client, make_product):
    make_product(code="FORM-MAIN-HK", name="Gói chính hư cấu", kind="main", group_code="FG-1")
    make_product(code="FORM-ADDON-HK", name="Gói kèm hư cấu", kind="addon", group_code="FG-1")

    resp = client.get("/contracts/new")

    assert 'data-code="FORM-MAIN-HK"' in resp.text
    assert 'data-code="FORM-ADDON-HK"' not in resp.text


def test_form_script_has_no_price_formula_left():
    text = FORM_PATH.read_text(encoding="utf-8")
    assert "* (1 +" not in text
    assert "/ 100" not in text
    assert "recalcPrice" not in text
    assert "priceOfOption" not in text


def test_posting_items_json_creates_a_multi_line_contract(client, db_session, make_customer,
                                                          make_product):
    from app.models.contract import Contract
    from app.models.contract_item import ContractItem

    customer = make_customer()
    main = make_product(code="FP-MAIN", name="Gói chính POST", kind="main", group_code="FP-G",
                        default_price=500_000, default_vat_rate=10)
    addon = make_product(code="FP-ADD", name="Gói kèm POST", kind="addon", group_code="FP-G",
                         default_price=150_000, default_vat_rate=10)
    items = [_product_row(main.id, 1), _product_row(addon.id, 1)]

    resp = client.post("/contracts/new", data=_items_form(
        customer, main, items, contract_number="HD-POST-001"), follow_redirects=False)

    assert resp.status_code == 303
    location = resp.headers["location"]
    contract_id = int(location.split("/contracts/")[1].split("?")[0])
    db_session.expire_all()
    contract = db_session.get(Contract, contract_id)
    assert contract.pricing_model == "items"
    # Trước thuế 6.000.000 + 1.800.000 = 7.800.000; thuế 10% = 780.000; tổng 8.580.000.
    assert contract.total_amount == Decimal("8580000")
    assert db_session.query(ContractItem).filter_by(contract_id=contract_id).count() == 2


def test_posting_bad_items_json_returns_422_and_keeps_the_rows(client, make_customer,
                                                              make_product):
    customer = make_customer()
    main = make_product(code="FQ-MAIN", name="Gói chính lỗi", kind="main", group_code="FQ-G")
    addon = make_product(code="FQ-ADD", name="Gói kèm lỗi", kind="addon", group_code="FQ-G")
    items = [_product_row(addon.id, 1), _product_row(main.id, 1)]

    resp = client.post("/contracts/new", data=_items_form(
        customer, main, items, contract_number="HD-POST-002"))

    assert resp.status_code == 422
    assert "Dòng 1 phải là gói chính" in resp.text
    # Trường ẩn items_json phải giữ đúng các dòng đã gửi, để người dùng không phải nhập lại.
    import html
    import re
    match = re.search(r'id="inp-items-json" value="([^"]*)"', resp.text)
    assert match is not None
    kept = json.loads(html.unescape(match.group(1)))
    assert [row["product_id"] for row in kept] == [addon.id, main.id]


# ─── Task 5.3: xuất Excel có chi tiết dòng ───────────────────────────────────

def _read_export(content):
    import io

    import openpyxl

    wb = openpyxl.load_workbook(io.BytesIO(content), read_only=True, data_only=True)
    try:
        return {ws.title: [list(r) for r in ws.iter_rows(values_only=True)] for ws in wb.worksheets}
    finally:
        wb.close()


def test_export_keeps_old_columns_and_adds_the_new_ones_at_the_end(client, db_session,
                                                                   make_customer, make_product,
                                                                   make_contract):
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="EX5-MAIN", name="Gói chính xuất", kind="main", group_code="EX5-G",
                        default_price=500_000, default_vat_rate=10)
    addon = make_product(code="EX5-ADD", name="Gói kèm xuất", kind="addon", group_code="EX5-G",
                         default_price=150_000, default_vat_rate=10)
    create_draft(db_session, _items_form(customer, main, [
        _product_row(main.id, 2), _product_row(addon.id, 1)], contract_number="HD-EX5-001"))
    parent = make_contract(customer=customer, contract_number="HD-EX5-GOC")
    make_contract(customer=customer, contract_number="HD-EX5-PL", parent_contract_id=parent.id,
                  doc_kind="addendum_addon")

    resp = client.get("/contracts/export")
    assert resp.status_code == 200
    sheets = _read_export(resp.content)

    assert list(sheets) == ["Danh sách hợp đồng", "Chi tiết dòng"]
    header = sheets["Danh sách hợp đồng"][0]
    assert header[:9] == ["STT", "Số hợp đồng", "Khách hàng", "Sản phẩm", "Ngày ký",
                          "Ngày hết hạn", "Giá trị (VNĐ)", "Trạng thái", "Ghi chú"]
    assert header[9:] == ["Trước thuế", "Thuế", "Loại văn bản", "Hợp đồng gốc"]

    by_number = {row[1]: row for row in sheets["Danh sách hợp đồng"][1:]}
    # Trước thuế 12.000.000 + 1.800.000; thuế 10% = 1.380.000.
    assert by_number["HD-EX5-001"][9:] == ["13.800.000", "1.380.000", "Hợp đồng", None]
    assert by_number["HD-EX5-PL"][11] == "Phụ lục bổ sung"
    assert by_number["HD-EX5-PL"][12] == "HD-EX5-GOC"


def test_detail_sheet_has_one_row_per_line_with_the_money_columns(client, db_session,
                                                                  make_customer, make_product):
    from app.services.contract_service import create_draft

    customer = make_customer()
    main = make_product(code="DS5-MAIN", name="Gói chính chi tiết xuất", kind="main",
                        group_code="DS5-G", unit_label="cơ sở",
                        default_price=500_000, default_vat_rate=10)
    addon = make_product(code="DS5-ADD", name="Gói kèm chi tiết xuất", kind="addon",
                         group_code="DS5-G", unit_label="cơ sở",
                         default_price=150_000, default_vat_rate=10)
    create_draft(db_session, _items_form(customer, main, [
        _product_row(main.id, 2), _product_row(addon.id, 1)], contract_number="HD-DS5-001"))

    sheets = _read_export(client.get("/contracts/export").content)
    detail = sheets["Chi tiết dòng"]

    assert detail[0] == ["Số hợp đồng", "TT", "Dịch vụ", "ĐVT", "Số lượng", "Số kỳ",
                         "Đơn giá", "Trước thuế", "Thuế", "Sau thuế"]
    lines = [r for r in detail[1:] if r[0] == "HD-DS5-001"]
    assert len(lines) == 2
    assert [r[1] for r in lines] == [1, 2]
    assert lines[0][2] == "Gói chính chi tiết xuất"
    assert lines[0][4] == 2 and lines[0][5] == 12
    assert lines[0][7] == "12.000.000"
    assert lines[1][9] == "1.980.000"


# ─── Task 5.1: sinh hàng loạt theo dòng ──────────────────────────────────────

def _batch_template(path):
    import docx

    doc = docx.Document()
    doc.add_paragraph("Hợp đồng {{CONTRACT_NUMBER}} cho {{PARTY_A_NAME}}")
    t = doc.add_table(rows=3, cols=3)
    for i, h in enumerate(["TT", "Dịch vụ", "Thành tiền"]):
        t.rows[0].cells[i].text = h
    for i, v in enumerate(["{{ITEM_NO}}", "{{ITEM_NAME}}", "{{ITEM_AMOUNT_TOTAL}}"]):
        t.rows[1].cells[i].text = v
    t.rows[2].cells[0].text = "Tổng"
    t.rows[2].cells[2].text = "{{TOTAL_AMOUNT}}"
    doc.save(str(path))
    return path


def test_batch_prices_each_customer_by_its_own_units(db_session, make_customer, make_product,
                                                     make_template, tmp_path):
    from app.models.contract import Contract
    from app.services.batch_generator import run_batch

    main = make_product(code="BT-MAIN", name="Gói chính lô", kind="main", group_code="BT-G",
                        unit_label="cơ sở", default_price=500_000, default_vat_rate=10)
    addon = make_product(code="BT-ADD", name="Gói kèm lô", kind="addon", group_code="BT-G",
                         unit_label="cơ sở", default_price=150_000, default_vat_rate=10)
    template = make_template(file_path=str(_batch_template(tmp_path / "batch_items.docx")),
                             contract_type="Hợp đồng")
    customers = [make_customer() for _ in range(3)]
    shared = {"product_id": str(main.id), "month_count": "12", "start_date": "01/01/2026",
              "items_json": json.dumps([{"product_id": main.id}, {"product_id": addon.id}])}
    overrides = {c.id: {"unit_count": str(n), "contract_number": f"BATCH-U-{n}"}
                 for n, c in zip((1, 2, 3), customers)}

    result = run_batch(db=db_session, template_id=template.id,
                       customer_ids=[c.id for c in customers],
                       shared_params=shared, per_customer_overrides=overrides)

    assert result.succeeded == 3 and result.failed == 0
    db_session.expire_all()
    by_number = {c.contract_number: c for c in db_session.query(Contract).all()}
    # Mỗi KH: (500.000 + 150.000) × số cơ sở × 12 tháng, cộng 10% thuế.
    assert by_number["BATCH-U-1"].total_amount == Decimal("8580000")
    assert by_number["BATCH-U-2"].total_amount == Decimal("17160000")
    assert by_number["BATCH-U-3"].total_amount == Decimal("25740000")
    assert by_number["BATCH-U-2"].pricing_model == "items"
    assert len(by_number["BATCH-U-2"].items) == 2


def test_row_without_record_count_fails_alone(db_session, make_customer, make_product,
                                              make_template, tmp_path):
    from app.services.batch_generator import run_batch

    rec = make_product(code="BT-REC", name="Gói theo hồ sơ", kind="main", group_code="BT-R",
                       quantity_source="records", default_price=1000, default_vat_rate=0)
    template = make_template(file_path=str(_batch_template(tmp_path / "batch_records.docx")),
                             contract_type="Hợp đồng")
    with_count = make_customer(record_count=1200)
    without = make_customer(record_count=None)
    shared = {"product_id": str(rec.id), "month_count": "12", "start_date": "01/01/2026",
              "items_json": json.dumps([{"product_id": rec.id}])}
    overrides = {with_count.id: {"contract_number": "BATCH-R-1"},
                 without.id: {"contract_number": "BATCH-R-2"}}

    result = run_batch(db=db_session, template_id=template.id,
                       customer_ids=[with_count.id, without.id],
                       shared_params=shared, per_customer_overrides=overrides)

    assert result.succeeded == 1 and result.failed == 1
    failed = [r for r in result.rows if not r.success]
    assert "thiếu số hồ sơ" in failed[0].error


# ─── Bổ sung: đường Excel và vòng đi về qua trang xem trước ──────────────────

def _excel_bytes(rows):
    import io

    import openpyxl

    wb = openpyxl.Workbook()
    try:
        ws = wb.active
        ws.append(["Mã khách hàng", "Số hợp đồng", "Ngày bắt đầu", "Số tháng",
                   "Số đơn vị", "Mã gói chính", "Mã gói bổ sung"])
        for r in rows:
            ws.append(r)
        buf = io.BytesIO()
        wb.save(buf)
        return buf.getvalue()
    finally:
        wb.close()


def test_excel_rows_with_package_codes_price_items_and_a_bad_code_fails_alone(
        db_session, make_customer, make_product, make_template, tmp_path):
    from app.services.batch_generator import import_excel_mapping, run_batch
    from app.models.contract import Contract

    c1 = make_customer(code="EXL-C1")
    c2 = make_customer(code="EXL-C2")
    main = make_product(code="EXL-MAIN", name="Gói chính Excel", kind="main", group_code="EXL-G",
                        default_price=500_000, default_vat_rate=10)
    make_product(code="EXL-ADD1", name="Kèm 1 Excel", kind="addon", group_code="EXL-G",
                 default_price=150_000, default_vat_rate=10)
    make_product(code="EXL-ADD2", name="Kèm 2 Excel", kind="addon", group_code="EXL-G",
                 default_price=150_000, default_vat_rate=10)
    template = make_template(file_path=str(_batch_template(tmp_path / "excel_items.docx")),
                             contract_type="Hợp đồng")

    content = _excel_bytes([
        ["EXL-C1", "EXL-001", "01/01/2026", 12, 1, "EXL-MAIN", "EXL-ADD1; EXL-ADD2"],
        ["EXL-C2", "EXL-002", "01/01/2026", 12, 1, "EXL-NOPE", ""],
    ])
    parsed = import_excel_mapping(content, {"EXL-C1": c1.id, "EXL-C2": c2.id})
    assert parsed.errors == []
    overrides = {row["customer_id"]: {k: v for k, v in row.items() if k != "customer_id"}
                 for row in parsed.rows}

    result = run_batch(db=db_session, template_id=template.id,
                       customer_ids=[row["customer_id"] for row in parsed.rows],
                       shared_params={}, per_customer_overrides=overrides)

    assert result.succeeded == 1 and result.failed == 1
    db_session.expire_all()
    good = db_session.query(Contract).filter(Contract.contract_number == "EXL-001").one()
    assert good.pricing_model == "items"
    assert len(good.items) == 3              # gói chính + hai gói bổ sung
    assert good.product_id == main.id
    # 500.000 + 150.000 + 150.000, mỗi gói × 1 cơ sở × 12 tháng, cộng 10% thuế.
    assert good.total_amount == Decimal("10560000")
    bad = [r for r in result.rows if not r.success]
    assert len(bad) == 1 and "EXL-NOPE" in bad[0].error


def _named_inputs(page: str) -> dict:
    """Mọi ô <input> có thuộc tính name, kèm giá trị đã giải mã (ô ẩn và ô hiển thị)."""
    import html
    import re

    out = {}
    for tag in re.findall(r"<input[^>]*>", page):
        name = re.search(r'name="([^"]+)"', tag)
        if name is None:
            continue
        value = re.search(r'value="([^"]*)"', tag)
        out[name.group(1)] = html.unescape(value.group(1)) if value else ""
    return out


def test_batch_preview_round_trip_keeps_the_lines(client, db_session, make_customer,
                                                  make_product, make_template, tmp_path):
    import html
    import re

    from app.models.contract import Contract

    main = make_product(code="RT5-MAIN", name="Gói chính vòng đi", kind="main", group_code="RT5-G",
                        default_price=500_000, default_vat_rate=10)
    addon = make_product(code="RT5-ADD", name="Gói kèm vòng đi", kind="addon", group_code="RT5-G",
                         default_price=150_000, default_vat_rate=10)
    template = make_template(file_path=str(_batch_template(tmp_path / "round_trip.docx")),
                             contract_type="Hợp đồng")
    # Tên pháp nhân khác nhau: cùng tên thì số hợp đồng trùng nhau, đúng như nghiệp vụ.
    c1 = make_customer(legal_name="Công ty xem trước A")
    c2 = make_customer(legal_name="Công ty xem trước B")
    items = json.dumps([{"product_id": main.id}, {"product_id": addon.id}])

    preview = client.post("/batch/preview", data={
        "template_id": str(template.id),
        "customer_ids": [str(c1.id), str(c2.id)],
        "sign_date": "",
        "start_date": "01/01/2026",
        "month_count": "12",
        "product_id": str(main.id),
        "items_json": items,
    })
    assert preview.status_code == 200
    assert "unit_price_vat" not in preview.text

    posted = _named_inputs(preview.text)
    assert posted["items_json"] == items
    assert posted["row_contract_number_0"] and posted["row_contract_number_1"]
    posted["selected_rows"] = "0,1"

    run = client.post("/batch", data=posted, follow_redirects=False)

    assert run.status_code == 303
    assert "s=2&f=0" in run.headers["location"]
    db_session.expire_all()
    made = db_session.query(Contract).filter(Contract.is_batch == True).all()  # noqa: E712
    assert len(made) == 2
    for contract in made:
        assert contract.pricing_model == "items"
        assert len(contract.items) == 2
        assert contract.total_amount == Decimal("8580000")
