"""Audit known missing separators in DOCX templates; --fix requires backups.

Usage: python scripts/check_template_spacing.py data/uploads/templates
       python scripts/check_template_spacing.py data/uploads/templates --fix --backup-dir data/backups/spacing

Only repairs observed prose boundaries. Dates, identifiers, percentages and
arbitrary adjacent placeholders are deliberately left to the template author.
"""
from __future__ import annotations

import argparse
import io
import re
import shutil
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from zipfile import ZipFile

from lxml import etree

W = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"
XML_SPACE = "{http://www.w3.org/XML/1998/namespace}space"
RULES = (
    re.compile(r"\{\{PARTY_[AB]_REPRESENTATIVE\}\}(?=Chức vụ:)"),
    re.compile(r"\{\{(?:TOTAL_AMOUNT|UNIT_PRICE_VAT)\}\}(?=đ(?=$|[\s/.,;:)]))"),
    re.compile(r"\{\{TOTAL_AMOUNT\}\}(?=\(Bằng chữ:)"),
)


def repair_docx(source: bytes) -> tuple[bytes, int]:
    """Insert separators without merging runs or changing unrelated ZIP parts."""
    updates = {}
    count = 0
    with ZipFile(io.BytesIO(source)) as archive:
        for name in archive.namelist():
            if not re.fullmatch(r"word/(?:document|header\d+|footer\d+)\.xml", name):
                continue
            root = etree.fromstring(archive.read(name))
            part_count = 0
            for paragraph in root.iter(W + "p"):
                nodes, chunks, offset = [], [], 0
                for element in paragraph.iter():
                    # A text box can contain paragraphs nested inside this one.
                    if next(element.iterancestors(W + "p"), None) is not paragraph:
                        continue
                    if element.tag == W + "t":
                        value = element.text or ""
                        nodes.append((offset, offset + len(value), element))
                    elif element.tag in {W + "tab", W + "br", W + "cr"}:
                        value = "\t"
                    else:
                        continue
                    chunks.append(value)
                    offset += len(value)
                text = "".join(chunks)
                positions = {match.end() for rule in RULES for match in rule.finditer(text)}
                for position in sorted(positions, reverse=True):
                    for start, end, node in nodes:
                        if start < position <= end:
                            local = position - start
                            node.text = node.text[:local] + " " + node.text[local:]
                            node.set(XML_SPACE, "preserve")
                            part_count += 1
                            break
            if part_count:
                updates[name] = etree.tostring(root, encoding="UTF-8", xml_declaration=True, standalone=True)
                count += part_count
        if not updates:
            return source, 0
        output = io.BytesIO()
        with ZipFile(output, "w") as repaired:
            repaired.comment = archive.comment
            for entry in archive.infolist():
                repaired.writestr(entry, updates.get(entry.filename, archive.read(entry.filename)))
    return output.getvalue(), count


def fix_file(path: Path, backup: Path) -> int:
    original = path.read_bytes()
    repaired, count = repair_docx(original)
    if count:
        backup.parent.mkdir(parents=True, exist_ok=True)
        # Never replace a previous backup or overwrite before saving the source.
        with backup.open("xb") as stream:
            stream.write(original)
        shutil.copystat(path, backup)
        temporary = None
        try:
            with tempfile.NamedTemporaryFile(dir=path.parent, suffix=".spacing.tmp", delete=False) as stream:
                temporary = Path(stream.name)
                stream.write(repaired)
            temporary.replace(path)
        finally:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
    return count


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", type=Path)
    parser.add_argument("--fix", action="store_true")
    parser.add_argument("--backup-dir", type=Path)
    args = parser.parse_args()
    if args.fix and not args.backup_dir:
        parser.error("--fix requires --backup-dir")
    files = set()
    for path in args.paths:
        if not path.exists():
            parser.error(f"Path does not exist: {path}")
        files.update(path.rglob("*.docx") if path.is_dir() else [path])
    backup_root = (args.backup_dir / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
                   if args.fix else None)
    total = changed = 0
    for index, path in enumerate(sorted(files)):
        count = (fix_file(path, backup_root / str(index) / path.name) if args.fix
                 else repair_docx(path.read_bytes())[1])
        if count:
            print(f"{path}: {count} missing separator(s){' fixed' if args.fix else ''}")
            changed += 1
            total += count
    print(f"Checked {len(files)} templates; {changed} affected; {total} separators.")
    if args.fix and changed:
        print(f"Backups: {backup_root}")
    return 1 if total and not args.fix else 0


if __name__ == "__main__":
    raise SystemExit(main())
