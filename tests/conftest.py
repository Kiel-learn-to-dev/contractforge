"""
Root pytest configuration — the data-safety guarantee.

Goal (OPEN_SOURCE_DESKTOP_PLAN.md Task 2):
    Every test runs against an isolated, temporary SQLite database and a
    temporary data tree. The real production database at ``data/contract_manager.db``
    MUST NOT change in size, timestamp, or hash as a result of running the suite.

How it works
------------
1. At import time (before any test module imports the application) we point the
   env var ``CONTRACTFORGE_DATA_ROOT`` at a unique temp directory. ``app.paths``
   and ``app.database`` resolve against it, so the engine binds to a temp DB.
2. A session-scoped engine is created once and shared; each test gets a clean
   schema inside a transaction that is rolled back, or a fresh temp DB depending
   on the fixture used. For this single-user SQLite app we use a per-test temp
   file DB (simple, robust against commit/rollback nuances in services).
3. Factory fixtures make it easy to create customers, products, templates,
   contracts, documents, and events without touching real data.

IMPORTANT: do NOT import the application at module top-level in test files that
don't need a DB; prefer ``db_session`` so the harness stays lazy and fast.
"""

from __future__ import annotations

import os
import sys
import tempfile
import uuid
from datetime import date, timedelta
from pathlib import Path

import pytest

# ─── 1. Redirect runtime data BEFORE app import ───────────────────────────────
# A unique temp root per pytest *process*. Created lazily; cleaned at exit.
_TEST_DATA_ROOT = Path(tempfile.gettempdir()) / f"contractforge_tests_{uuid.uuid4().hex[:8]}"
_TEST_DATA_ROOT.mkdir(parents=True, exist_ok=True)

# Set the env var so app.paths._resolve_data_root() picks it up at import time.
os.environ["CONTRACTFORGE_DATA_ROOT"] = str(_TEST_DATA_ROOT)

# Make the app importable: ContractForge/ contains main.py and the app/ package.
_REPO_ROOT = Path(__file__).resolve().parent.parent
_APP_SOURCE = _REPO_ROOT / "ContractForge"
if str(_APP_SOURCE) not in sys.path:
    sys.path.insert(0, str(_APP_SOURCE))


# ─── 2. Database isolation ────────────────────────────────────────────────────
@pytest.fixture()
def data_root(tmp_path: Path) -> Path:
    """A unique per-test data root. All runtime paths resolve under it."""
    root = tmp_path / "cf_data"
    root.mkdir()
    return root


@pytest.fixture()
def db_engine(data_root: Path, monkeypatch):
    """Create a fresh in-memory-per-file SQLite engine bound to the temp root.

    We redirect ``app.paths`` to the temp root via its public override API
    (``cf_set_data_root``), which refreshes DATA_DIR, every derived path, and
    the runtime-dirs tuple together — so services that captured module-level
    path constants at import time (e.g.
    ``OUTPUT_DIR = str(OUTPUT_CONTRACTS_DIR)``) resolve to the temp tree.
    Each test gets an empty schema.
    """
    import app.paths as paths
    import app.database as database
    from sqlalchemy import create_engine, event
    from sqlalchemy.orm import sessionmaker

    # Redirect the data root via the documented override API. This refreshes
    # DATA_DIR and all derived constants in place, including _RUNTIME_DIRS.
    paths.cf_set_data_root(str(data_root))
    paths.ensure_runtime_dirs()

    db_path = data_root / "contract_manager.db"
    engine = create_engine(
        f"sqlite:///{db_path.as_posix()}",
        connect_args={"check_same_thread": False},
    )

    @event.listens_for(engine, "connect")
    def _fk_on(conn, _):  # pragma: no cover - mirrors production pragma
        # Use the DBAPI cursor directly (production database.py uses the same
        # conn.execute("PRAGMA ...") form via SQLAlchemy's legacy layer).
        cur = conn.cursor()
        cur.execute("PRAGMA foreign_keys=ON")
        cur.close()

    # Rebind the shared module engine + sessionmaker. This ensures any code that
    # calls ``SessionLocal()`` directly (e.g. templating_patcher) hits the temp DB.
    monkeypatch.setattr(database, "engine", engine, raising=True)
    TestSession = sessionmaker(autocommit=False, autoflush=False, bind=engine)
    monkeypatch.setattr(database, "SessionLocal", TestSession, raising=True)

    # Create all tables on the temp engine.
    from app.database import Base
    # Ensure EVERY model module is imported so its table is registered on Base.
    # app/models/__init__.py does not re-export AppSetting, so import it
    # explicitly (and the package) to mirror production's full schema.
    import app.models  # noqa: F401
    import app.models.app_setting  # noqa: F401
    Base.metadata.create_all(bind=engine)

    yield engine

    engine.dispose()
    # Restore paths to their pre-test resolution so the override does not leak
    # into later tests. cf_set_data_root mutates module state directly, so we
    # clear it explicitly (the env var is managed by the data_root fixture's
    # own tmp_path scoping and is not relied upon after this).
    paths.cf_clear_data_root_override()


@pytest.fixture()
def db_session(db_engine):
    """A clean DB session for a single test."""
    from app.database import SessionLocal
    session = SessionLocal()
    try:
        yield session
    finally:
        session.close()


# ─── 3. Record factories ──────────────────────────────────────────────────────
@pytest.fixture()
def make_customer(db_session):
    from app.models.customer import Customer

    def _create(**overrides):
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
        db_session.add(c)
        db_session.commit()
        db_session.refresh(c)
        return c

    return _create


@pytest.fixture()
def make_product(db_session):
    from app.models.product import Product

    def _create(**overrides):
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
        db_session.add(p)
        db_session.commit()
        db_session.refresh(p)
        return p

    return _create


@pytest.fixture()
def make_contract(db_session, make_customer, make_product):
    from app.models.contract import Contract, ContractStatus

    def _create(**overrides):
        customer = overrides.pop("customer", None) or make_customer()
        product = overrides.pop("product", None) or make_product()
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
        db_session.add(c)
        db_session.commit()
        db_session.refresh(c)
        return c

    return _create


@pytest.fixture()
def make_event(db_session, make_contract):
    from app.models.contract_event import ContractEvent

    def _create(contract=None, **overrides):
        contract = contract or make_contract()
        defaults = dict(
            contract_id=contract.id,
            event_type="created",
            description="Tạo bản nháp",
            actor="system",
        )
        defaults.update(overrides)
        ev = ContractEvent(**defaults)
        db_session.add(ev)
        db_session.commit()
        return ev

    return _create


@pytest.fixture()
def make_document(db_session, make_customer):
    from app.models.customer_document import CustomerDocument

    def _create(customer=None, **overrides):
        customer = customer or make_customer()
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
        db_session.add(doc)
        db_session.commit()
        return doc

    return _create


@pytest.fixture()
def make_template(db_session):
    """A template record without requiring a real .docx file on disk."""
    from app.models.contract_template import ContractTemplate

    def _create(**overrides):
        defaults = dict(
            code=f"TPL-{uuid.uuid4().hex[:6].upper()}",
            name="Mẫu hợp đồng mẫu",
            contract_type="Khác",
            mode="placeholder",
            is_active=True,
            file_path="uploads/templates/sample.docx",
            file_name="sample.docx",
        )
        defaults.update(overrides)
        t = ContractTemplate(**defaults)
        db_session.add(t)
        db_session.commit()
        db_session.refresh(t)
        return t

    return _create
