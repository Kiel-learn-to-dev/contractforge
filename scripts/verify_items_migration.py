"""verify_items_migration.py — kiểm tra CHỈ ĐỌC sau khi chuyển dữ liệu contract_items.

Dùng trên BẢN SAO của cơ sở dữ liệu, không dùng trên DB gốc. Script mở file ở chế
độ read-only, không ghi gì, và từ chối chạy trên thư mục `data/` gốc của repo.

Cách dùng:
    python scripts/verify_items_migration.py <thư mục bản sao> [--snapshot FILE.csv]

- --snapshot: ghi CSV các cột tiền và số hợp đồng của mọi hợp đồng, sắp theo id.
  Chạy một lần TRƯỚC khi migrate và một lần SAU, rồi so hai file. Phải giống hệt
  từng byte.
- Luôn chạy hai câu SQL kiểm tra (mục 9 của PRICING_MODEL_IMPLEMENTATION_TASKS.md).
  Cả hai phải trả về 0; nếu không, script thoát với mã 1.

Script không chứa tên hay số của tổ chức nào.
"""

from __future__ import annotations

import argparse
import csv
import sqlite3
import sys
from contextlib import closing
from pathlib import Path

REPO_DATA_DIR = (Path(__file__).resolve().parent.parent / "data").resolve()

SNAPSHOT_COLUMNS = [
    "id", "contract_number", "status", "total_amount", "amount_text",
    "unit_price_pre_vat", "vat_rate", "unit_price_vat", "end_date",
]

CHECK_CONTRACTS_WITHOUT_ONE_ITEM = """
SELECT COUNT(*) FROM contracts c
WHERE (SELECT COUNT(*) FROM contract_items i WHERE i.contract_id = c.id) <> 1
"""

CHECK_ITEM_TOTALS_MISMATCH = """
SELECT COUNT(*) FROM contracts c JOIN contract_items i ON i.contract_id = c.id
WHERE COALESCE(c.total_amount, 0) <> i.amount_total
   OR i.amount_pre_vat + i.vat_amount <> i.amount_total
"""


def _open_readonly(db_file: Path) -> sqlite3.Connection:
    return sqlite3.connect(f"file:{db_file.as_posix()}?mode=ro", uri=True)


def _write_snapshot(conn: sqlite3.Connection, out: Path) -> int:
    rows = conn.execute(
        f"SELECT {', '.join(SNAPSHOT_COLUMNS)} FROM contracts ORDER BY id"
    ).fetchall()
    with open(out, "w", encoding="utf-8", newline="") as fh:
        writer = csv.writer(fh, lineterminator="\n")
        writer.writerow(SNAPSHOT_COLUMNS)
        for row in rows:
            writer.writerow(["" if v is None else v for v in row])
    return len(rows)


def _has_table(conn: sqlite3.Connection, name: str) -> bool:
    return conn.execute(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?", (name,)
    ).fetchone() is not None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[1])
    parser.add_argument("copy_dir", type=Path, help="thư mục chứa bản sao contract_manager.db")
    parser.add_argument("--snapshot", type=Path, default=None,
                        help="ghi CSV snapshot ra file này")
    args = parser.parse_args(argv)

    copy_dir = args.copy_dir.resolve()
    if copy_dir == REPO_DATA_DIR:
        print("Từ chối: đây là thư mục dữ liệu gốc. Hãy chỉ định thư mục bản sao.")
        return 2

    db_file = copy_dir / "contract_manager.db"
    if not db_file.exists():
        print(f"Không tìm thấy {db_file}")
        return 2

    with closing(_open_readonly(db_file)) as conn:
        if args.snapshot is not None:
            count = _write_snapshot(conn, args.snapshot)
            print(f"Snapshot: {count} hợp đồng -> {args.snapshot}")

        if not _has_table(conn, "contract_items"):
            print("FAIL: chưa có bảng contract_items (DB chưa migrate).")
            return 1

        contracts = conn.execute("SELECT COUNT(*) FROM contracts").fetchone()[0]
        items = conn.execute("SELECT COUNT(*) FROM contract_items").fetchone()[0]
        bad_count = conn.execute(CHECK_CONTRACTS_WITHOUT_ONE_ITEM).fetchone()[0]
        bad_total = conn.execute(CHECK_ITEM_TOTALS_MISMATCH).fetchone()[0]

    print(f"Hợp đồng: {contracts}, dòng: {items}")
    print(f"Hợp đồng không có đúng 1 dòng: {bad_count} (phải là 0)")
    print(f"Dòng lệch tổng tiền: {bad_total} (phải là 0)")
    if bad_count or bad_total:
        print("FAIL")
        return 1
    print("OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
