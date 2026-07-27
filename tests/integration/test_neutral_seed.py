"""Integration tests for neutral first-run seed data (Task 6)."""

from __future__ import annotations

import hashlib

from docx import Document


def test_clean_install_seeds_only_fictional_products(db_session):
    from app.models.customer import Customer
    from app.models.product import Product
    from app.models.seed import run_all_seeds

    run_all_seeds(db_session)

    products = db_session.query(Product).order_by(Product.code).all()
    assert db_session.query(Customer).count() == 0
    assert [product.code for product in products] == ["SERVICE-BASIC", "SERVICE-PRO"]


def test_seed_is_idempotent_and_preserves_existing_catalog(db_session):
    from app.models.product import Product
    from app.models.seed import run_all_seeds

    existing = Product(
        code="CUSTOM-01",
        name="Gói riêng",
        product_type="Dịch vụ",
        default_price=123_000,
        default_vat_rate=10,
        default_duration_months=6,
    )
    db_session.add(existing)
    db_session.commit()

    run_all_seeds(db_session)
    run_all_seeds(db_session)

    products = db_session.query(Product).all()
    assert [(product.code, product.name) for product in products] == [("CUSTOM-01", "Gói riêng")]


def test_bundled_template_discovery_never_overwrites_existing_private_template(
    db_session, make_template, tmp_path, monkeypatch,
):
    import app.services.template_service as template_service
    from app.services.template_service import _bundled_template_code

    source = tmp_path / "sample.docx"
    document = Document()
    document.add_paragraph("{{QUOTE_DATE}}")
    document.save(source)

    private_file = tmp_path / "private.docx"
    private_file.write_bytes(b"private-template-bytes")
    template = make_template(
        code=_bundled_template_code(source.read_bytes()),
        name="Mẫu riêng đã cấu hình",
        file_path=str(private_file),
        file_name="private.docx",
    )
    before_hash = hashlib.sha256(private_file.read_bytes()).hexdigest()

    monkeypatch.setattr(template_service, "_bundled_template_files", lambda: [source])
    template_service.seed_from_project_files(db_session)
    db_session.refresh(template)

    assert hashlib.sha256(private_file.read_bytes()).hexdigest() == before_hash
    assert template.name == "Mẫu riêng đã cấu hình"
    assert template.file_name == "private.docx"
