#!/usr/bin/env python3
"""
migrate_legacy_data.py — CLI for the safe, copy-only legacy data migration.

Moves the legacy source-relative ``data/`` tree into the platform user-data
directory (%LOCALAPPDATA%\\ContractForge on Windows). The migration is
COPY-ONLY: the legacy source is NEVER deleted. The destination is verified
(byte-identical DB, matching row counts, integrity check) before reporting
success.

The active data root is NOT switched automatically — that is a separate,
explicit decision (see OPEN_SOURCE_DESKTOP_PLAN.md §11). This tool only copies
and verifies. To switch the running application to the new location afterwards,
use the in-app migration review / settings, or set CONTRACTFORGE_DATA_ROOT.

Usage
-----
    python scripts/migrate_legacy_data.py              # plan the migration (dry run)
    python scripts/migrate_legacy_data.py --apply      # actually copy + verify
    python scripts/migrate_legacy_data.py --apply --force  # re-copy even if dest has DB
    python scripts/migrate_legacy_data.py --source PATH --destination PATH

Exit codes
----------
    0 = migration applied successfully (or dry-run reported cleanly)
    1 = migration failed verification or was refused
    2 = usage error / no legacy data found
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def _ensure_app_importable() -> None:
    """Make the ContractForge app package importable from the script location."""
    repo_root = Path(__file__).resolve().parent.parent
    app_source = repo_root / "ContractForge"
    if str(app_source) not in sys.path:
        sys.path.insert(0, str(app_source))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Safely migrate legacy ContractForge data to the user-data directory."
    )
    parser.add_argument(
        "--source", default=None,
        help="legacy data root (default: <repo>/data)",
    )
    parser.add_argument(
        "--destination", default=None,
        help="destination data root (default: %%LOCALAPPDATA%%\\ContractForge)",
    )
    parser.add_argument(
        "--apply", action="store_true",
        help="actually perform the copy (default is a dry-run plan).",
    )
    parser.add_argument(
        "--force", action="store_true",
        help="re-copy even if the destination already has a database "
             "(existing destination files are backed up, never deleted).",
    )
    parser.add_argument(
        "--json", action="store_true",
        help="emit machine-readable JSON result.",
    )
    args = parser.parse_args(argv)

    _ensure_app_importable()
    from app.services.migration_service import (
        migrate_legacy_data, is_legacy_data_present,
    )
    from app.paths import legacy_data_root, user_data_default

    source = Path(args.source) if args.source else legacy_data_root()
    dest = Path(args.destination) if args.destination else user_data_default()

    if not is_legacy_data_present(source):
        msg = (
            f"No legacy database found at {source}. Nothing to migrate."
        )
        if args.json:
            print(json.dumps({"ok": False, "message": msg, "source": str(source)}, ensure_ascii=False))
        else:
            print(msg)
        return 2

    if not args.apply:
        plan = {
            "ok": True,
            "dry_run": True,
            "source": str(source),
            "destination": str(dest),
            "message": (
                "Dry run: no files were copied. Re-run with --apply to perform "
                "the copy-only migration (legacy source is never deleted)."
            ),
        }
        if args.json:
            print(json.dumps(plan, ensure_ascii=False, indent=2))
        else:
            print("Migration plan (dry run):")
            print(f"  source      : {source}")
            print(f"  destination : {dest}")
            print(f"  {plan['message']}")
        return 0

    result = migrate_legacy_data(source, dest, force=args.force)
    payload = result.to_dict()
    payload["dry_run"] = False

    if args.json:
        print(json.dumps(payload, ensure_ascii=False, indent=2, default=str))
    else:
        print(f"Migration {'succeeded' if result.ok else 'FAILED'}.")
        print(f"  source       : {result.source}")
        print(f"  destination  : {result.destination}")
        print(f"  backup dir   : {result.backup_dir or '(none)'}")
        print(f"  copied files : {result.copied_files}")
        print(f"  skipped files: {result.skipped_files}")
        print(f"  db sha (src) : {result.source_db_sha[:16]}...")
        print(f"  db sha (dst) : {result.destination_db_sha[:16]}...")
        print(f"  integrity OK : {result.db_integrity_ok}")
        print(f"  already migrated: {result.already_migrated}")
        print(f"  message      : {result.message}")

    return 0 if result.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
