"""Task 2.2 — create_draft nhận danh sách dòng (items_json).

Dữ liệu hoàn toàn hư cấu.

Phần đầu file là test ĐẶC TẢ hành vi cũ (không có items_json). Chúng được viết và
chạy xanh TRƯỚC khi sửa create_draft, để chứng minh nhánh cũ không đổi.
"""

import json
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
