"""Neutral, idempotent first-run seed data for public installations."""

from sqlalchemy.orm import Session
from .product import Product
from app.utils.safe_console import safe_print


def seed_products(db: Session):
    """Create fictional sample products only when the catalog is empty."""
    existing = db.query(Product).count()
    if existing > 0:
        return  # Không seed lại

    products = [
        Product(
            code="SERVICE-BASIC",
            name="Gói Dịch vụ Cơ bản",
            product_type="Dịch vụ",
            default_price=1_000_000,
            default_vat_rate=10,
            default_duration_months=12,
        ),
        Product(
            code="SERVICE-PRO",
            name="Gói Dịch vụ Nâng cao",
            product_type="Dịch vụ",
            default_price=2_500_000,
            default_vat_rate=10,
            default_duration_months=12,
        ),
    ]
    db.add_all(products)
    db.commit()
    safe_print(f"  [seed] Đã tạo {len(products)} sản phẩm.")


def seed_party_b_defaults(db: Session) -> None:
    """Retained for compatibility; organization setup is intentionally blank."""


def run_migrations(db: Session) -> None:
    """Tự động thêm cột mới vào DB cũ nếu chưa có (SQLite không hỗ trợ ALTER TABLE IF NOT EXISTS)."""
    from sqlalchemy import text
    with db.bind.connect() as conn:
        # Tạo bảng app_settings nếu chưa có
        conn.execute(text("""
            CREATE TABLE IF NOT EXISTS app_settings (
                key   VARCHAR(100) NOT NULL PRIMARY KEY,
                value TEXT
            )
        """))
        conn.commit()
        # Kiểm tra và thêm bill_main_station vào customers
        cols = [row[1] for row in conn.execute(text("PRAGMA table_info(customers)")).fetchall()]
        if "bill_main_station" not in cols:
            conn.execute(text("ALTER TABLE customers ADD COLUMN bill_main_station BOOLEAN NOT NULL DEFAULT 1"))
            conn.commit()

        # Kiểm tra và thêm include_in_billing vào customer_units
        cols2 = [row[1] for row in conn.execute(text("PRAGMA table_info(customer_units)")).fetchall()]
        if "include_in_billing" not in cols2:
            conn.execute(text("ALTER TABLE customer_units ADD COLUMN include_in_billing BOOLEAN NOT NULL DEFAULT 1"))
            conn.commit()

        # Mã quan hệ ngân sách
        if "budget_relation_code" not in cols:
            conn.execute(text("ALTER TABLE customers ADD COLUMN budget_relation_code VARCHAR(50)"))
            conn.commit()

        # ── customers — soft-delete (v47) ─────────────────────────────────────
        cols_c = [row[1] for row in conn.execute(text("PRAGMA table_info(customers)")).fetchall()]
        if "is_deleted" not in cols_c:
            conn.execute(text("ALTER TABLE customers ADD COLUMN is_deleted BOOLEAN NOT NULL DEFAULT 0"))
            conn.commit()
        if "deleted_at" not in cols_c:
            conn.execute(text("ALTER TABLE customers ADD COLUMN deleted_at DATETIME"))
            conn.commit()
        if "kcb_code" not in cols_c:
            conn.execute(text("ALTER TABLE customers ADD COLUMN kcb_code VARCHAR(30)"))
            conn.commit()
        if "establishment_decision" not in cols_c:
            conn.execute(text("ALTER TABLE customers ADD COLUMN establishment_decision VARCHAR(100)"))
            conn.commit()
        if "establishment_date" not in cols_c:
            conn.execute(text("ALTER TABLE customers ADD COLUMN establishment_date DATE"))
            conn.commit()
        if "representative_gender" not in cols_c:
            conn.execute(text("ALTER TABLE customers ADD COLUMN representative_gender VARCHAR(10)"))
            conn.commit()

        # ── customer_documents (v47) ───────────────────────────────────────────
        conn.execute(text("""
            CREATE TABLE IF NOT EXISTS customer_documents (
                id          INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                customer_id INTEGER NOT NULL REFERENCES customers(id),
                doc_type    VARCHAR(50)  NOT NULL,
                label       VARCHAR(200),
                file_path   VARCHAR(500) NOT NULL,
                file_name   VARCHAR(200) NOT NULL,
                mime_hint   VARCHAR(20),
                uploaded_at DATETIME
            )
        """))
        conn.execute(text("""
            CREATE INDEX IF NOT EXISTS ix_customer_documents_customer_id
            ON customer_documents(customer_id)
        """))
        conn.commit()

        # ── contracts — columns added incrementally, never migrated ──────────
        # Đây là nguyên nhân lỗi "File chưa được sinh hoặc không còn tồn tại":
        # SQLAlchemy SELECT tất cả columns trong model; nếu cột thiếu trong DB
        # → OperationalError → generate thất bại, output_file_path không lưu được.
        cols_ct = [row[1] for row in conn.execute(text("PRAGMA table_info(contracts)")).fetchall()]
        _ct_migrations = [
            ("party_b_representative",      "ALTER TABLE contracts ADD COLUMN party_b_representative VARCHAR(150)"),
            ("party_b_authorization_number","ALTER TABLE contracts ADD COLUMN party_b_authorization_number VARCHAR(100)"),
            ("party_b_authorization_date",  "ALTER TABLE contracts ADD COLUMN party_b_authorization_date VARCHAR(50)"),
            ("acceptance_date",             "ALTER TABLE contracts ADD COLUMN acceptance_date DATE"),
            ("liquidation_date",            "ALTER TABLE contracts ADD COLUMN liquidation_date DATE"),
            ("output_file_path",            "ALTER TABLE contracts ADD COLUMN output_file_path VARCHAR(500)"),
            ("output_file_name",            "ALTER TABLE contracts ADD COLUMN output_file_name VARCHAR(200)"),
            ("signed_pdf_path",             "ALTER TABLE contracts ADD COLUMN signed_pdf_path VARCHAR(500)"),
            ("signed_pdf_name",             "ALTER TABLE contracts ADD COLUMN signed_pdf_name VARCHAR(200)"),
            ("signed_pdf_uploaded_at",      "ALTER TABLE contracts ADD COLUMN signed_pdf_uploaded_at DATETIME"),
            ("render_data",                 "ALTER TABLE contracts ADD COLUMN render_data TEXT"),
            ("is_batch",                    "ALTER TABLE contracts ADD COLUMN is_batch BOOLEAN NOT NULL DEFAULT 0"),
            ("batch_job_id",                "ALTER TABLE contracts ADD COLUMN batch_job_id VARCHAR(100)"),
            ("selected_unit_names",         "ALTER TABLE contracts ADD COLUMN selected_unit_names TEXT"),
        ]
        for col_name, sql in _ct_migrations:
            if col_name not in cols_ct:
                conn.execute(text(sql))
                conn.commit()

        # ── contracts — invoice / payment columns (v48) ───────────────────────
        cols_ct = [row[1] for row in conn.execute(text("PRAGMA table_info(contracts)")).fetchall()]
        _invoice_migrations = [
            ("invoice_date",             "ALTER TABLE contracts ADD COLUMN invoice_date DATE"),
            ("invoice_pdf_path",         "ALTER TABLE contracts ADD COLUMN invoice_pdf_path VARCHAR(500)"),
            ("invoice_pdf_name",         "ALTER TABLE contracts ADD COLUMN invoice_pdf_name VARCHAR(200)"),
            ("invoice_pdf_uploaded_at",  "ALTER TABLE contracts ADD COLUMN invoice_pdf_uploaded_at DATETIME"),
            ("payment_date",             "ALTER TABLE contracts ADD COLUMN payment_date DATE"),
            ("payment_slip_path",        "ALTER TABLE contracts ADD COLUMN payment_slip_path VARCHAR(500)"),
            ("payment_slip_name",        "ALTER TABLE contracts ADD COLUMN payment_slip_name VARCHAR(200)"),
            ("payment_slip_uploaded_at", "ALTER TABLE contracts ADD COLUMN payment_slip_uploaded_at DATETIME"),
        ]
        for col_name, sql in _invoice_migrations:
            if col_name not in cols_ct:
                conn.execute(text(sql))
                conn.commit()

        # Migrate dữ liệu cũ: Active → PaidActive
        conn.execute(text("UPDATE contracts SET status='PaidActive' WHERE status='Active'"))
        conn.commit()


def run_all_seeds(db: Session):
    safe_print("[seed] Bắt đầu seed data...")
    seed_products(db)
    seed_party_b_defaults(db)
    safe_print("[seed] Hoàn tất.")
