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
    # Ghi chú: trang chi tiết hiện chưa in tên sản phẩm (biến `product` được
    # truyền vào template nhưng không dùng). Việc đó nằm ngoài Task 1.4.

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
