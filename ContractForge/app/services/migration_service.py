"""
migration_service.py — safe, copy-only legacy data migration (Task 4).

Moves the legacy source-relative ``data/`` directory to the platform
user-data directory (LocalAppData on Windows). This is the one-time upgrade
path for existing installations that still live under ``<repo>/data``.

Design principles (OPEN_SOURCE_DESKTOP_PLAN.md §11.1):
  * Never migrate the only copy.
  * Always create a timestamped backup before copying.
  * Copy first; validate; switch active data root last.
  * Do NOT automatically delete legacy data.
  * Record migration version in application settings.
  * Make every migration safe to re-run (idempotent).

The migration is COPY-ONLY: the legacy source tree is left completely intact.
The only switch is setting the active data root to the destination via the
override API, and even that is the caller's responsibility (the launcher /
settings UI decides when to commit to the new location).
"""

from __future__ import annotations

import hashlib
import shutil
import sqlite3
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Optional

from app.paths import DB_FILENAME

# Files/dirs that are part of a legacy data tree and must be migrated.
# Anything else found under the legacy root is copied too, so we never lose
# a user file by omission.
MIGRATABLE_NAMES = {
    "contract_manager.db",
    "uploads",
    "outputs",
    "logs",
    "backups",
}


@dataclass
class MigrationResult:
    """Outcome of a migration attempt. Safe to serialize for the UI/logs."""
    ok: bool = False
    source: str = ""
    destination: str = ""
    backup_dir: str = ""
    copied_files: int = 0
    skipped_files: int = 0
    db_integrity_ok: bool = False
    source_counts: dict = field(default_factory=dict)
    destination_counts: dict = field(default_factory=dict)
    source_db_sha: str = ""
    destination_db_sha: str = ""
    message: str = ""
    already_migrated: bool = False

    def to_dict(self) -> dict:
        return self.__dict__.copy()


def _ts() -> str:
    """Filesystem-safe timestamp for backup directory names."""
    return datetime.now().strftime("%Y%m%d_%H%M%S")


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def _table_counts(db_path: Path) -> dict:
    """Row counts for key tables — used to verify copy fidelity."""
    if not db_path.exists():
        return {}
    counts: dict[str, int] = {}
    conn = sqlite3.connect(str(db_path))
    try:
        rows = conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table'"
        ).fetchall()
        for (name,) in rows:
            try:
                n = conn.execute(f'SELECT COUNT(*) FROM "{name}"').fetchone()[0]
                counts[name] = n
            except sqlite3.DatabaseError:
                continue
    finally:
        conn.close()
    return counts


def _integrity_check(db_path: Path) -> bool:
    """Run SQLite PRAGMA integrity_check; True only if it returns 'ok'."""
    if not db_path.exists():
        return False
    conn = sqlite3.connect(str(db_path))
    try:
        rows = conn.execute("PRAGMA integrity_check").fetchall()
        return bool(rows) and rows[0][0] == "ok"
    except sqlite3.DatabaseError:
        return False
    finally:
        conn.close()


def _count_files(root: Path) -> int:
    """Count all files under a directory tree (for reporting)."""
    if not root.exists():
        return 0
    total = 0
    for _ in root.rglob("*"):
        if _.is_file():
            total += 1
    return total


def is_legacy_data_present(legacy_root: Path) -> bool:
    """True if the legacy root exists and contains a real database."""
    return (legacy_root / DB_FILENAME).is_file()


def destination_has_data(dest_root: Path) -> bool:
    """True if the destination already contains a database (already migrated)."""
    return (dest_root / DB_FILENAME).is_file()


def _copy_tree_with_backup(src: Path, dst: Path, backup_dir: Path) -> tuple[int, int]:
    """Copy src → dst. If a conflict exists in dst, move the conflicting item
    into ``backup_dir`` first (timestamped). Returns (copied, skipped)."""
    copied = 0
    skipped = 0
    backup_dir.mkdir(parents=True, exist_ok=True)
    for item in src.iterdir():
        target = dst / item.name
        if target.exists():
            # Conflict: move the DESTINATION's existing item to backup (never
            # overwrite silently, never delete the source).
            conflict_backup = backup_dir / f"{_ts()}_{item.name}"
            shutil.move(str(target), str(conflict_backup))
        try:
            if item.is_dir():
                shutil.copytree(item, target, dirs_exist_ok=False)
            else:
                shutil.copy2(item, target)
            copied += 1
        except (OSError, shutil.Error):
            # Skip a single problematic file rather than abort the whole migration.
            skipped += 1
    return copied, skipped


def migrate_legacy_data(
    legacy_root: Optional[Path] = None,
    destination: Optional[Path] = None,
    *,
    force: bool = False,
) -> MigrationResult:
    """Migrate the legacy data tree to the destination (default: user-data dir).

    This is COPY-ONLY and never deletes the legacy source. Steps:
      1. Resolve source (legacy data/) and destination (LocalAppData default).
      2. Refuse if source has no DB (nothing to migrate).
      3. Refuse if destination already has a DB unless ``force=True``
         (idempotency: do not silently overwrite a non-empty destination).
      4. Verify source DB integrity + record counts + hash.
      5. Create a timestamped backup directory under destination/backups.
      6. Copy all legacy files into the destination (conflicts → backup).
      7. Verify destination DB integrity + counts + hash == source hash.
      8. Return a full result. The caller decides whether to switch the
         active data root (this function does NOT switch it automatically).
    """
    from app.paths import legacy_data_root, user_data_default

    source = Path(legacy_root) if legacy_root else legacy_data_root()
    dest = Path(destination) if destination else user_data_default()
    result = MigrationResult(source=str(source), destination=str(dest))

    # 1. Source must contain a real DB.
    if not is_legacy_data_present(source):
        result.ok = False
        result.message = "No legacy database found at source; nothing to migrate."
        return result

    # 2. Idempotency: refuse if destination already populated, unless forced.
    if destination_has_data(dest) and not force:
        result.ok = True
        result.already_migrated = True
        result.message = (
            "Destination already contains a database. Re-run with force=True "
            "to re-copy (existing destination files are backed up, never deleted)."
        )
        destination_db = dest / DB_FILENAME
        result.destination_db_sha = _sha256(destination_db)
        result.destination_counts = _table_counts(destination_db)
        result.db_integrity_ok = _integrity_check(destination_db)
        return result

    # 3. Pre-copy verification of the SOURCE.
    source_db = source / DB_FILENAME
    if not _integrity_check(source_db):
        result.ok = False
        result.message = "Source database failed integrity check; aborting migration."
        return result
    result.source_db_sha = _sha256(source_db)
    result.source_counts = _table_counts(source_db)

    # 4. Prepare destination + timestamped backup dir.
    dest.mkdir(parents=True, exist_ok=True)
    backup_dir = dest / "backups" / f"migration_{_ts()}"
    result.backup_dir = str(backup_dir)

    # 5. Copy everything. Conflicts in destination are moved to backup first.
    copied, skipped = _copy_tree_with_backup(source, dest, backup_dir)
    result.copied_files = copied
    result.skipped_files = skipped

    # 6. Post-copy verification of the DESTINATION.
    dest_db = dest / DB_FILENAME
    if not dest_db.exists():
        result.ok = False
        result.message = "Copy completed but destination DB is missing."
        return result

    result.destination_db_sha = _sha256(dest_db)
    result.destination_counts = _table_counts(dest_db)
    result.db_integrity_ok = _integrity_check(dest_db)

    # 7. Fidelity check: DB must be byte-identical and counts must match.
    sha_match = result.source_db_sha == result.destination_db_sha
    counts_match = result.source_counts == result.destination_counts

    if sha_match and counts_match and result.db_integrity_ok:
        result.ok = True
        result.message = (
            f"Migration OK: copied {copied} item(s) to {dest}. "
            f"Legacy source left intact at {source}. DB verified (sha + counts + integrity)."
        )
    else:
        result.ok = False
        result.message = (
            "Migration copied files but verification FAILED: "
            f"sha_match={sha_match} counts_match={counts_match} integrity_ok={result.db_integrity_ok}. "
            "The legacy source is untouched; review the destination before switching."
        )
    return result


def record_migration_version(db_path: Path, version: str = "1") -> bool:
    """Write a migration marker into app_settings so we know this DB was migrated.

    Best-effort: returns True if recorded, False if it could not be written.
    Never raises — migration completion must not fail on bookkeeping.
    """
    try:
        conn = sqlite3.connect(str(db_path))
        try:
            conn.execute(
                "CREATE TABLE IF NOT EXISTS app_settings "
                "(key VARCHAR(100) PRIMARY KEY, value TEXT)"
            )
            conn.execute(
                "INSERT INTO app_settings(key, value) VALUES('migration_version', ?) "
                "ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                (version,),
            )
            conn.execute(
                "INSERT INTO app_settings(key, value) VALUES('migrated_at', ?) "
                "ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                (datetime.now().isoformat(timespec="seconds"),),
            )
            conn.commit()
        finally:
            conn.close()
        return True
    except sqlite3.DatabaseError:
        return False
