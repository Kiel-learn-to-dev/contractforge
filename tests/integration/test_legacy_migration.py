"""
Integration tests: legacy-data migration (Task 4).

Covers the migration safety properties required by the plan (§11.1) and the
task's verification steps:
  * empty destination → full copy, fidelity verified,
  * existing destination → idempotent refusal unless forced,
  * interrupted copy (partial destination) → re-run completes safely,
  * repeated migration → safe to re-run,
  * source NEVER deleted,
  * DB integrity + row counts + sha match after migration,
  * uploads/outputs remain reachable in destination.
"""

from __future__ import annotations

import sqlite3
from pathlib import Path

import pytest


# ─── Fixture: a realistic legacy data tree ───────────────────────────────────


def _make_legacy_db(db_path: Path) -> None:
    """Create a small but realistic legacy SQLite DB with sample rows."""
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path))
    conn.executescript(
        """
        CREATE TABLE customers (id INTEGER PRIMARY KEY, legal_name TEXT);
        INSERT INTO customers VALUES (1, 'Khách hàng A');
        INSERT INTO customers VALUES (2, 'Khách hàng B');
        CREATE TABLE contracts (id INTEGER PRIMARY KEY, status TEXT);
        INSERT INTO contracts VALUES (1, 'Signed');
        CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT);
        """
    )
    conn.commit()
    conn.close()


@pytest.fixture()
def legacy_tree(tmp_path: Path) -> Path:
    """A legacy data/ tree with a DB, an upload, and a generated output."""
    root = tmp_path / "legacy_data"
    _make_legacy_db(root / "contract_manager.db")
    uploads = root / "uploads" / "templates"
    uploads.mkdir(parents=True)
    (uploads / "tpl_alpha.docx").write_bytes(b"PK\x03\x04alpha")
    outputs = root / "outputs" / "contracts"
    outputs.mkdir(parents=True)
    (outputs / "HD-001.docx").write_bytes(b"PK\x03\x04contract-001")
    (root / "logs").mkdir()
    (root / "logs" / "app.log").write_text("old log line\n", encoding="utf-8")
    return root


@pytest.fixture()
def dest_root(tmp_path: Path) -> Path:
    return tmp_path / "destination_data"


# ─── Tests ────────────────────────────────────────────────────────────────────


def test_migrate_empty_destination(legacy_tree, dest_root):
    from app.services.migration_service import migrate_legacy_data
    result = migrate_legacy_data(legacy_tree, dest_root, force=False)
    assert result.ok, result.message
    assert not result.already_migrated
    # DB copied and byte-identical.
    assert (dest_root / "contract_manager.db").is_file()
    assert result.source_db_sha == result.destination_db_sha
    assert result.db_integrity_ok
    # Row counts preserved.
    assert result.source_counts == result.destination_counts
    assert result.source_counts.get("customers") == 2
    assert result.source_counts.get("contracts") == 1
    # Uploads + outputs reachable in destination.
    assert (dest_root / "uploads" / "templates" / "tpl_alpha.docx").is_file()
    assert (dest_root / "outputs" / "contracts" / "HD-001.docx").is_file()
    # Backup dir created.
    assert Path(result.backup_dir).is_dir()


def test_migrate_never_deletes_source(legacy_tree, dest_root):
    from app.services.migration_service import migrate_legacy_data
    migrate_legacy_data(legacy_tree, dest_root, force=False)
    # Source tree must be completely intact.
    assert (legacy_tree / "contract_manager.db").is_file()
    assert (legacy_tree / "uploads" / "templates" / "tpl_alpha.docx").is_file()
    assert (legacy_tree / "outputs" / "contracts" / "HD-001.docx").is_file()
    assert (legacy_tree / "logs" / "app.log").is_file()


def test_migrate_idempotent_refuses_populated_dest(legacy_tree, dest_root):
    from app.services.migration_service import migrate_legacy_data
    # First migration.
    first = migrate_legacy_data(legacy_tree, dest_root, force=False)
    assert first.ok
    # Second migration to the same populated destination must refuse (not overwrite).
    second = migrate_legacy_data(legacy_tree, dest_root, force=False)
    assert second.ok
    assert second.already_migrated
    # Dest DB unchanged (still byte-identical to source).
    assert second.destination_db_sha == first.source_db_sha


def test_migrate_force_re_copies_with_backup(legacy_tree, dest_root):
    from app.services.migration_service import migrate_legacy_data
    migrate_legacy_data(legacy_tree, dest_root, force=False)
    # Mutate the destination DB to simulate a divergent destination.
    conn = sqlite3.connect(str(dest_root / "contract_manager.db"))
    conn.execute("INSERT INTO customers VALUES (99, 'divergent')")
    conn.commit()
    conn.close()
    # Force re-copy: the divergent destination DB must be backed up, not deleted.
    result = migrate_legacy_data(legacy_tree, dest_root, force=True)
    assert result.ok
    # Destination now matches source again.
    assert result.destination_db_sha == result.source_db_sha
    assert result.destination_counts.get("customers") == 2  # original, not 3
    # The divergent DB was backed up (find a .db under backups).
    backup_dbs = list(Path(result.backup_dir).rglob("*.db"))
    assert backup_dbs, "conflicting destination DB must be backed up, not overwritten"


def test_migrate_interrupted_copy_completes(legacy_tree, dest_root):
    """Simulate an interrupted first copy: destination has uploads but no DB."""
    from app.services.migration_service import migrate_legacy_data
    # Partial state: some files copied, DB missing (interrupted).
    partial_uploads = dest_root / "uploads" / "templates"
    partial_uploads.mkdir(parents=True)
    (partial_uploads / "tpl_alpha.docx").write_bytes(b"PK\x03\x04partial-stale")
    # Re-run: should complete the copy and bring DB over.
    result = migrate_legacy_data(legacy_tree, dest_root, force=True)
    assert result.ok
    assert (dest_root / "contract_manager.db").is_file()
    assert result.destination_db_sha == result.source_db_sha
    # The stale partial upload was backed up and the fresh one copied in.
    assert (dest_root / "uploads" / "templates" / "tpl_alpha.docx").read_bytes() == b"PK\x03\x04alpha"


def test_migrate_no_legacy_db_is_noop(dest_root):
    from app.services.migration_service import migrate_legacy_data
    empty_source = dest_root.parent / "empty_source"
    empty_source.mkdir()
    result = migrate_legacy_data(empty_source, dest_root, force=False)
    assert not result.ok
    assert "nothing to migrate" in result.message.lower()


def test_migrate_corrupt_source_aborts(tmp_path, dest_root):
    from app.services.migration_service import migrate_legacy_data
    source = tmp_path / "bad_legacy"
    source.mkdir()
    # A file that is NOT a valid SQLite DB.
    (source / "contract_manager.db").write_bytes(b"NOT A DATABASE")
    result = migrate_legacy_data(source, dest_root, force=False)
    assert not result.ok
    assert "integrity" in result.message.lower() or "failed" in result.message.lower()
    # Destination must not receive a corrupt DB.
    assert not (dest_root / "contract_manager.db").exists()


def test_record_migration_version(dest_root):
    from app.services.migration_service import record_migration_version, DB_FILENAME
    dest_root.mkdir(parents=True)
    db = dest_root / DB_FILENAME
    # Create a minimal DB to mark.
    conn = sqlite3.connect(str(db))
    conn.execute("CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT)")
    conn.commit()
    conn.close()
    assert record_migration_version(db, version="1")
    conn = sqlite3.connect(str(db))
    row = conn.execute(
        "SELECT value FROM app_settings WHERE key='migration_version'"
    ).fetchone()
    conn.close()
    assert row is not None and row[0] == "1"


def test_cli_dry_run_does_not_copy(legacy_tree, dest_root, monkeypatch):
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        "migrate_cli",
        str(Path(__file__).resolve().parent.parent.parent / "scripts" / "migrate_legacy_data.py"),
    )
    cli = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(cli)
    rc = cli.main([
        "--source", str(legacy_tree),
        "--destination", str(dest_root),
    ])
    assert rc == 0
    # Dry run must NOT have copied anything.
    assert not dest_root.exists() or not any(dest_root.iterdir())


def test_cli_apply_migrates(legacy_tree, dest_root):
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        "migrate_cli_apply",
        str(Path(__file__).resolve().parent.parent.parent / "scripts" / "migrate_legacy_data.py"),
    )
    cli = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(cli)
    rc = cli.main([
        "--apply",
        "--source", str(legacy_tree),
        "--destination", str(dest_root),
    ])
    assert rc == 0
    assert (dest_root / "contract_manager.db").is_file()
