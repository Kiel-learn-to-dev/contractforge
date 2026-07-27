"""
tests/fixtures/factories.py — shared record factories.

These mirror the pytest fixtures in conftest but are importable as plain
functions, so non-fixture test helpers or future scripts can create records.
They require an already-open SQLAlchemy session bound to an isolated engine.
"""

from __future__ import annotations

import uuid
from datetime import date, timedelta
from typing import Optional


def make_customer(db, **overrides):
    from app.models.customer import Customer
    defaults = dict(
        code=f"KH-{uuid.uuid4().hex[:6].upper()}",
        legal_name="Công ty TNHH Mẫu",
        short_name="CTY Mẫu",
        customer_type="Doanh nghiệp",
        address="Số 1 Đường Mẫu",
        province="Hà Nội",
        representative_name="Người Đại Diện",
        representative_title="Giám đốc",
        is_active=True,
    )
    defaults.update(overrides)
    c = Customer(**defaults)
    db.add(c)
    db.commit()
    db.refresh(c)
    return c


def make_product(db, **overrides):
    from app.models.product import Product
    defaults = dict(
        code=f"SW-{uuid.uuid4().hex[:6].upper()}",
        name="Sản phẩm mẫu",
        product_type="Phần mềm",
        default_price=1_000_000,
        default_vat_rate=10,
        default_duration_months=12,
    )
    defaults.update(overrides)
    p = Product(**defaults)
    db.add(p)
    db.commit()
    db.refresh(p)
    return p


def make_contract(db, customer=None, product=None, **overrides):
    from app.models.contract import Contract, ContractStatus
    customer = customer or make_customer(db)
    product = product or make_product(db)
    defaults = dict(
        contract_number=f"HD-{uuid.uuid4().hex[:8].upper()}",
        status=ContractStatus.Draft,
        customer_id=customer.id,
        product_id=product.id,
        sign_date=date.today(),
        start_date=date.today(),
        end_date=date.today() + timedelta(days=365),
        month_count=12,
        unit_count=1,
        vat_rate=10,
    )
    defaults.update(overrides)
    c = Contract(**defaults)
    db.add(c)
    db.commit()
    db.refresh(c)
    return c


def make_event(db, contract=None, **overrides):
    from app.models.contract_event import ContractEvent
    contract = contract or make_contract(db)
    defaults = dict(
        contract_id=contract.id,
        event_type="created",
        description="Tạo bản nháp",
        actor="system",
    )
    defaults.update(overrides)
    ev = ContractEvent(**defaults)
    db.add(ev)
    db.commit()
    return ev


def make_document(db, customer=None, **overrides):
    from app.models.customer_document import CustomerDocument
    customer = customer or make_customer(db)
    defaults = dict(
        customer_id=customer.id,
        doc_type="business_license",
        label="Giấy phép kinh doanh",
        file_path="uploads/customer_docs/sample.pdf",
        file_name="sample.pdf",
        mime_hint="application/pdf",
    )
    defaults.update(overrides)
    doc = CustomerDocument(**defaults)
    db.add(doc)
    db.commit()
    return doc
