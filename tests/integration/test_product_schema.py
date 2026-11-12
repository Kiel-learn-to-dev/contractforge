"""Task 1.2 — cột mới của bảng products và migration cho DB cũ.

Dữ liệu hoàn toàn hư cấu. Test dựng lại bảng products kiểu cũ (chỉ các cột
trước Task 1.2) rồi chạy run_migrations, để chắc DB đang dùng vẫn khởi động được.
"""

from sqlalchemy import text


OLD_PRODUCTS_DDL = """
CREATE TABLE products (
    id INTEGER NOT NULL PRIMARY KEY,
    code VARCHAR(50) NOT NULL UNIQUE,
    name VARCHAR(200) NOT NULL,
    product_type VARCHAR(80) NOT NULL,
    default_price NUMERIC(18, 0),
    default_vat_rate NUMERIC(5, 2),
    default_duration_months INTEGER,
    default_template_id INTEGER REFERENCES contract_templates(id),
    contract_number_format VARCHAR(200),
    is_active BOOLEAN NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL
)
"""


def _recreate_legacy_products_table(db_session):
    db_session.execute(text("DROP TABLE products"))
    db_session.execute(text(OLD_PRODUCTS_DDL))
    db_session.execute(text(
        "INSERT INTO products (id, code, name, product_type, default_price, "
        "default_vat_rate, default_duration_months, is_active, created_at, updated_at) "
        "VALUES (1, 'SW-OLD', 'Sản phẩm cũ', 'Phần mềm', 1000000, 10, 12, 1, "
        "'2025-01-01 00:00:00', '2025-01-01 00:00:00')"
    ))
    db_session.commit()


def test_legacy_product_row_gets_safe_defaults_after_migration(db_session):
    from app.models.product import Product
    from app.models.seed import run_migrations

    _recreate_legacy_products_table(db_session)
    run_migrations(db_session)
    db_session.expire_all()

    product = db_session.get(Product, 1)
    assert product.kind == "main"
    assert product.price_period_months == 1
    assert product.quantity_source == "units"
    assert product.sort_order == 0
    assert product.group_code is None
    assert product.replaced_by_id is None
    assert product.feature_list == []


def test_migration_runs_twice_without_error_or_data_change(db_session):
    from app.models.product import Product
    from app.models.seed import run_migrations

    _recreate_legacy_products_table(db_session)
    run_migrations(db_session)
    run_migrations(db_session)
    db_session.expire_all()

    rows = db_session.query(Product).all()
    assert [(p.id, p.code, p.name, p.kind) for p in rows] == [
        (1, "SW-OLD", "Sản phẩm cũ", "main"),
    ]


def test_feature_list_round_trips_as_list_and_defaults_to_empty(db_session):
    from app.models.product import Product

    product = Product(code="SW-FEAT", name="Gói có chức năng",
                      product_type="Phần mềm")
    assert product.feature_list == []

    product.feature_list = [{"no": 1, "name": "Đăng nhập", "description": "Mô tả"}]
    db_session.add(product)
    db_session.commit()
    db_session.expire_all()

    reloaded = db_session.query(Product).filter_by(code="SW-FEAT").one()
    assert reloaded.feature_list == [
        {"no": 1, "name": "Đăng nhập", "description": "Mô tả"},
    ]


def test_product_kind_and_quantity_source_constants():
    from app.models.product import PRODUCT_KINDS, QUANTITY_SOURCES

    assert PRODUCT_KINDS == ("main", "addon", "storage")
    assert QUANTITY_SOURCES == ("units", "records", "manual")
