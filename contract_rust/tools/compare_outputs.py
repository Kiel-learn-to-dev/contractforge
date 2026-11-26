"""Compare generated DOCX paragraph text against Python on isolated DB copies."""
from __future__ import annotations

import argparse
from contextlib import closing
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import zipfile
import xml.etree.ElementTree as ET

HERE = Path(__file__).resolve().parent
RUST_ROOT = HERE.parent
REPO_ROOT = RUST_ROOT.parent
sys.path.insert(0, str(RUST_ROOT / "tools"))
from urllib.parse import quote

from compat_check import backup_database, rewrite_paths, sha256

W = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"


def is_text_part(name: str) -> bool:
    return (
        name in {"word/document.xml", "word/footnotes.xml", "word/endnotes.xml"}
        or ((name.startswith("word/header") or name.startswith("word/footer")) and name.endswith(".xml"))
    )


def extract_text(path: Path) -> dict[str, list[str]]:
    result: dict[str, list[str]] = {}
    with zipfile.ZipFile(path) as archive:
        for name in archive.namelist():
            if not is_text_part(name):
                continue
            root = ET.fromstring(archive.read(name))
            parents = {child: parent for parent in root.iter() for child in parent}
            paragraphs: list[str] = []
            for paragraph in root.iter(W + "p"):
                text = ""
                for node in paragraph.iter():
                    if node.tag not in {W + "t", W + "br", W + "cr", W + "tab"}:
                        continue
                    owner = parents.get(node)
                    if node.tag == W + "tab" and (owner is None or owner.tag != W + "r"):
                        continue
                    while owner is not None and owner.tag != W + "p":
                        owner = parents.get(owner)
                    if owner is not paragraph:
                        continue
                    if node.tag == W + "t" and node.text:
                        text += node.text
                    elif node.tag in {W + "br", W + "cr"}:
                        text += "\n"
                    elif node.tag == W + "tab":
                        text += "\t"
                paragraphs.append(text)
            result[name] = paragraphs
    return result


def select_contracts(source: Path, count: int) -> list[tuple[int, Path]]:
    source_uri = f"file:{quote(source.resolve().as_posix(), safe='/:')}?mode=ro"
    with closing(sqlite3.connect(source_uri, uri=True)) as db:
        rows = db.execute(
            "SELECT c.id, t.file_path FROM contracts c "
            "JOIN contract_templates t ON t.id = c.template_id "
            "WHERE c.doc_kind = 'contract' ORDER BY c.id DESC"
        ).fetchall()
    selected = []
    for contract_id, raw_path in rows:
        if not raw_path:
            continue
        path = Path(raw_path)
        if not path.is_absolute():
            path = source.parent / path
        if path.is_file():
            selected.append((contract_id, path.resolve()))
        if len(selected) == count:
            break
    if len(selected) < count:
        raise RuntimeError(f"Cần {count} hợp đồng có mẫu tồn tại, chỉ tìm thấy {len(selected)}.")
    return selected


def verify_paths(database: Path, data_root: Path) -> None:
    """Ensure every rewritten stored path is confined to its test copy."""
    root = data_root.resolve()
    with closing(sqlite3.connect(database)) as db:
        tables = [row[0] for row in db.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
        )]
        for table in tables:
            quoted_table = '"' + table.replace('"', '""') + '"'
            columns = [row[1] for row in db.execute(f"PRAGMA table_info({quoted_table})")]
            for column in (name for name in columns if name.endswith("_path") or name == "file_path"):
                quoted_column = '"' + column.replace('"', '""') + '"'
                for (raw,) in db.execute(
                    f"SELECT {quoted_column} FROM {quoted_table} WHERE {quoted_column} IS NOT NULL AND {quoted_column} != ''"
                ):
                    path = Path(raw)
                    if not path.is_absolute():
                        path = root / path
                    if not path.resolve().is_relative_to(root):
                        raise RuntimeError(f"DB kiểm thử có đường dẫn ra ngoài bản sao: {table}.{column}")


def verify_outputs(outputs: dict[str, dict], data_root: Path) -> None:
    root = data_root.resolve()
    for output in outputs.values():
        path = Path(output["file_path"]).resolve(strict=True)
        if not path.is_relative_to(root):
            raise RuntimeError("File DOCX sinh ra nằm ngoài thư mục DB kiểm thử.")


def source_has_header(template: Path, part: str) -> bool:
    with zipfile.ZipFile(template) as archive:
        return part in archive.namelist()


def run_python(project: Path, root: Path, ids: list[int]) -> dict[str, dict]:
    script = r'''
import json, sys
sys.path.insert(0, sys.argv[1])
from app.database import SessionLocal
from app.services.contract_service import generate_docx
session = SessionLocal()
try:
    outputs = {}
    for raw in sys.argv[2:]:
        contract_id = int(raw)
        contract = generate_docx(session, contract_id)
        outputs[str(contract_id)] = {"file_name": contract.output_file_name, "file_path": contract.output_file_path}
    print(json.dumps(outputs, ensure_ascii=False))
finally:
    session.close()
'''
    env = os.environ.copy()
    env["CONTRACTFORGE_DATA_ROOT"] = str(root)
    python = next(
        (
            candidate
            for candidate in (
                project / ".venv" / "Scripts" / "python.exe",
                project.parent / ".venv" / "Scripts" / "python.exe",
                Path(sys.executable),
            )
            if candidate.is_file()
        ),
        Path(sys.executable),
    )
    completed = subprocess.run(
        [str(python), "-I", "-c", script, str(project), *(str(i) for i in ids)],
        cwd=project,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_db", type=Path, help="SQLite DB nguồn, chỉ đọc")
    parser.add_argument("--count", type=int, default=10)
    parser.add_argument("--rust-binary", type=Path, help="Đường dẫn đến cf-compare-generate")
    args = parser.parse_args()
    source = args.source_db.resolve(strict=True)
    if not source.is_file() or args.count < 1:
        parser.error("source_db phải là file DB và --count phải dương")
    before = sha256(source)
    selected = select_contracts(source, args.count)
    ids = [contract_id for contract_id, _ in selected]
    source_templates = dict(selected)

    temporary = tempfile.TemporaryDirectory(prefix="cf-docx-compare-")
    try:
        base = Path(temporary.name)
        rust_root = base / "rust"
        python_root = base / "python"
        rust_root.mkdir()
        python_root.mkdir()
        for root in (rust_root, python_root):
            backup_database(source, root / "contract_manager.db")
            rewrite_paths(root / "contract_manager.db", root, source.parent)
            verify_paths(root / "contract_manager.db", root)

        python_output = run_python(REPO_ROOT / "ContractForge", python_root, ids)
        rust_binary = args.rust_binary
        if rust_binary is None:
            rust_binary = RUST_ROOT / "target" / "debug" / "cf-compare-generate.exe"
            if not rust_binary.is_file():
                rust_binary = RUST_ROOT / "target" / "release" / "cf-compare-generate.exe"
        rust_binary = rust_binary.resolve()
        if not rust_binary.is_file():
            raise RuntimeError("Chưa build cf-compare-generate; chạy cargo build -p cf-services --bin cf-compare-generate")
        completed = subprocess.run(
            [str(rust_binary), str(rust_root), *(str(i) for i in ids)],
            check=True,
            capture_output=True,
            text=True,
            env={**os.environ, "CONTRACTFORGE_DATA_ROOT": str(rust_root)},
        )
        rust_output = json.loads(completed.stdout)
        verify_outputs(python_output, python_root)
        verify_outputs(rust_output, rust_root)
        differences = []
        structural_differences = []
        for contract_id in ids:
            key = str(contract_id)
            left = extract_text(Path(python_output[key]["file_path"]))
            right = extract_text(Path(rust_output[key]["file_path"]))
            for part in sorted(set(left) | set(right)):
                if left.get(part, []) != right.get(part, []):
                    source_has_part = source_has_header(source_templates[contract_id], part)
                    python_header_only = part == "word/header1.xml" and left.get(part) == [""]
                    if python_header_only and not source_has_part and part not in right:
                        structural_differences.append(contract_id)
                    else:
                        # Report only part and paragraph counts. DOCX text may contain customer data.
                        differences.append((contract_id, part, len(left.get(part, [])), len(right.get(part, []))))
    finally:
        temporary.cleanup()

    if before != sha256(source):
        raise RuntimeError("Mã băm DB nguồn thay đổi trong khi so sánh.")
    if differences:
        print(f"{len(differences)} khác biệt đoạn/văn bản trong {len(ids)} hợp đồng:")
        for contract_id, part, left_count, right_count in differences:
            print(f"  HĐ id={contract_id}, {part}: Python={left_count} đoạn, Rust={right_count} đoạn")
        print("COMPARED WITH DIFFERENCES")
        return 1
    print(
        f"0 khác biệt văn bản ngoài KB-03/KB-05; đã so {len(ids)} hợp đồng. "
        f"{len(structural_differences)} khác biệt cấu trúc được phân loại KB-03: "
        "Python tạo header1.xml mặc định chỉ có một đoạn rỗng, Rust giữ nguyên phần header vắng mặt. "
        "DB nguồn không đổi."
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, sqlite3.Error, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        print(f"COMPARE FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
