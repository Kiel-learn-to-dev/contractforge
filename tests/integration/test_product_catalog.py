"""Task 1.3 — thêm và sửa sản phẩm trên giao diện /products.

Dữ liệu hoàn toàn hư cấu. Các test POST đi qua fixture `client` (TestClient nối
vào DB tạm).
"""

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
