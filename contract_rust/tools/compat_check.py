"""Run Rust/Python SQLite roundtrips against an isolated copy of an explicit DB."""

from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import uuid
from pathlib import Path
from urllib.parse import quote


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def backup_database(source: Path, destination: Path) -> None:
    """Copy a consistent SQLite snapshot using SQLite's backup API."""
    source_uri = f"file:{quote(source.resolve().as_posix(), safe='/:')}?mode=ro"
    with closing(sqlite3.connect(source_uri, uri=True)) as src:
        with closing(sqlite3.connect(destination)) as dst:
            src.backup(dst)


def rewrite_paths(database: Path, copied_root: Path, source_root: Path) -> None:
    """Rewrite stored file paths only in the disposable database copy."""
    with closing(sqlite3.connect(database)) as connection:
        tables = [row[0] for row in connection.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
        )]

        def quoted(identifier: str) -> str:
            return '"' + identifier.replace('"', '""') + '"'

        for table in tables:
            table_sql = quoted(table)
            columns = [row[1] for row in connection.execute(f"PRAGMA table_info({table_sql})")]
            path_columns = [name for name in columns if name.endswith("_path") or name == "file_path"]
            for column in path_columns:
                column_sql = quoted(column)
                rows = connection.execute(
                    f"SELECT rowid, {column_sql} FROM {table_sql} WHERE {column_sql} IS NOT NULL AND {column_sql} != ''"
                ).fetchall()
                for rowid, raw_path in rows:
                    original = Path(raw_path)
                    candidates = [original]
                    if not original.is_absolute():
                        candidates.insert(0, source_root / original)
                    existing = next((candidate for candidate in candidates if candidate.is_file()), None)
                    if existing:
                        safe_name = f"{table}_{column}_{rowid}_{existing.name}"
                        target = copied_root / "compat_files" / safe_name
                        target.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copy2(existing, target)
                        replacement = str(target)
                    else:
                        replacement = str(copied_root / "missing_files" / f"{table}_{column}_{rowid}")
                    connection.execute(
                        f"UPDATE {table_sql} SET {column_sql}=? WHERE rowid=?", (replacement, rowid)
                    )
        connection.commit()


def run_python_roundtrip(
    project_root: Path, database_root: Path, token: str, *, mode: str = "write"
) -> None:
    python = next(
        (
            candidate
            for candidate in (
                project_root / ".venv" / "Scripts" / "python.exe",
                project_root.parent / ".venv" / "Scripts" / "python.exe",
                Path(sys.executable),
            )
            if candidate.is_file()
        ),
        None,
    )
    if python is None:
        raise RuntimeError(f"Không tìm thấy Python môi trường dự án quanh: {project_root}")
    script = r'''
import sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from datetime import datetime
from app.database import SessionLocal
from app.models.customer import Customer
from app.models.contract import Contract, ContractStatus
from app.models.contract_item import ContractItem
from app.models.contract_event import ContractEvent

project_root, mode, token = sys.argv[1:]
number = "CF-COMPAT-" + token
session = SessionLocal()
try:
    if mode == "write":
        customer = Customer(code=number, legal_name="Khách Python kiểm thử", customer_type="Khác")
        session.add(customer)
        session.flush()
        contract = Contract(contract_number=number, status=ContractStatus.Draft, customer_id=customer.id,
                            pricing_model="legacy", doc_kind="contract")
        session.add(contract)
        session.flush()
        session.add(ContractItem(contract_id=contract.id, sort_order=0, description="Python compatibility row",
                                 quantity=1, period_count=1, vat_rate=0, amount_pre_vat=1, vat_amount=0, amount_total=1))
        session.add(ContractEvent(contract_id=contract.id, event_type="python_compatibility_check",
                                  description="Created by Python compatibility check", occurred_at=datetime.utcnow()))
        session.commit()
        print("PYTHON WRITE OK " + token)
    else:
        customer = session.query(Customer).filter_by(code=number).one()
        contract = session.query(Contract).filter_by(contract_number=number).one()
        assert contract.customer_id == customer.id
        item = session.query(ContractItem).filter_by(contract_id=contract.id).one()
        event = session.query(ContractEvent).filter_by(contract_id=contract.id).one()
        if mode == "verify-rust":
            assert customer.legal_name == "Khách kiểm thử " + token
            assert contract.notes == "updated-by-rust"
            assert item.description == "Compatibility row"
            assert event.event_type == "compatibility_check"
        else:
            assert customer.legal_name == "Khách Python kiểm thử"
            assert contract.notes is None
            assert item.description == "Python compatibility row"
            assert event.event_type == "python_compatibility_check"
        print("PYTHON VERIFY OK " + token)
finally:
    session.close()
'''
    env = os.environ.copy()
    env["CONTRACTFORGE_DATA_ROOT"] = str(database_root)
    env["PYTHONPATH"] = str(project_root)
    subprocess.run([str(python), "-I", "-c", script, str(project_root), mode, token], cwd=project_root, env=env, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_db", type=Path, help="Đường dẫn rõ ràng tới DB nguồn; chỉ đọc và băm trước/sau")
    parser.add_argument("--rust-binary", required=True, type=Path, help="Đường dẫn binary cf-db-compat đã build")
    parser.add_argument("--project-root", type=Path, default=Path(__file__).resolve().parents[2] / "ContractForge")
    args = parser.parse_args()
    source = args.source_db.resolve(strict=True)
    if not source.is_file():
        parser.error("source_db phải là file SQLite cụ thể")
    rust_binary = args.rust_binary.resolve(strict=True)
    before = sha256(source)
    token = uuid.uuid4().hex[:16]

    with tempfile.TemporaryDirectory(prefix="contractforge-compat-") as temporary:
        root = Path(temporary).resolve()
        copy = root / "contract_manager.db"
        backup_database(source, copy)
        source_root = source.parent
        rewrite_paths(copy, root, source.parent)
        env = os.environ.copy()
        env["CONTRACTFORGE_DATA_ROOT"] = str(root)
        env["CONTRACTFORGE_COMPAT_SOURCE_ROOT"] = str(source_root)
        # The Rust CLI receives the disposable copy only.
        subprocess.run([str(rust_binary), str(copy), token], env=env, check=True)
        run_python_roundtrip(args.project_root, root, token, mode="verify-rust")
        python_token = token + "-PY"
        run_python_roundtrip(args.project_root, root, python_token)
        subprocess.run([str(rust_binary), str(copy), python_token, "--verify"], env=env, check=True)
        run_python_roundtrip(args.project_root, root, python_token, mode="verify")

    after = sha256(source)
    if before != after:
        raise RuntimeError("DB nguồn đã đổi trong lúc chạy kiểm tương thích.")
    print("COMPAT OK")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError, sqlite3.Error) as error:
        print(f"COMPAT FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
