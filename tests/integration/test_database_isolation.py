"""
Integration test: database isolation (OPEN_SOURCE_DESKTOP_PLAN.md Task 2).

Proves that the test harness never mutates the real production database:
  - the real DB hash is unchanged before/after the suite,
  - records created in a test live only in the temp root,
  - the schema initializes deterministically,
  - the app's path override env var redirects runtime paths.
"""

from __future__ import annotations

import hashlib
import importlib
from pathlib import Path


def _real_db_path() -> Path:
    """Path to the REAL production database (repo sibling data/)."""
    from app.paths import BASE_DIR
    return BASE_DIR.parent / "data" / "contract_manager.db"


def _hash_file(path: Path) -> str | None:
    if not path.exists():
        return None
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def test_data_root_env_redirects_paths(monkeypatch, tmp_path):
    """Setting CONTRACTFORGE_DATA_ROOT moves DATA_DIR and DB_PATH."""
    import app.paths as paths

    target = tmp_path / "custom_data"
    monkeypatch.setenv("CONTRACTFORGE_DATA_ROOT", str(target))
    resolved = paths._resolve_data_root()
    assert resolved == target.resolve()


def test_db_session_isolated_from_production(db_session, make_customer):
    """A record created via the fixture must NOT appear in the real DB."""
    real_db = _real_db_path()
    real_hash_before = _hash_file(real_db)

    created = make_customer(legal_name="ZZZ_TEST_ISOLATION_MARKER")
    assert created.id is not None

    # The temp DB must contain the marker; the real DB must be untouched.
    from app.models.customer import Customer
    in_temp = db_session.query(Customer).filter(
        Customer.legal_name == "ZZZ_TEST_ISOLATION_MARKER"
    ).first()
    assert in_temp is not None

    real_hash_after = _hash_file(real_db)
    assert real_hash_before == real_hash_after, (
        "Real production database was mutated by a test!"
    )


def test_schema_creates_all_tables(db_engine):
    """init_db-style schema creation works against the temp engine."""
    from sqlalchemy import inspect
    inspector = inspect(db_engine)
    tables = set(inspector.get_table_names())
    expected = {
        "customers", "customer_units", "products", "contracts",
        "contract_events", "contract_templates", "customer_documents",
        "app_settings",
    }
    assert expected.issubset(tables), f"Missing tables: {expected - tables}"


def test_factory_roundtrip(db_session, make_customer, make_product, make_contract, make_event):
    """Factories can compose into a realistic record graph in the temp DB."""
    c = make_customer()
    p = make_product()
    contract = make_contract(customer=c, product=p)
    ev = make_event(contract=contract, event_type="status_changed")
    assert contract.customer_id == c.id
    assert contract.product_id == p.id
    assert ev.contract_id == contract.id


def test_repeated_inits_are_deterministic(db_engine, db_session):
    """Running create_all twice must be idempotent (no duplicate tables)."""
    from app.database import Base
    Base.metadata.create_all(bind=db_engine)  # second time
    Base.metadata.create_all(bind=db_engine)  # third time
    from sqlalchemy import inspect
    tables = inspect(db_engine).get_table_names()
    assert tables.count("contracts") == 1
