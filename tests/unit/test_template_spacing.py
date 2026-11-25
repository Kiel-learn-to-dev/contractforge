"""Regression checks for repairs to templates, not automatic prose rendering."""
import importlib.util
import io
from pathlib import Path
from zipfile import ZipFile

import pytest
from docx import Document

from app.services.docx_renderer import render_docx

spec = importlib.util.spec_from_file_location(
    "check_template_spacing", Path(__file__).resolve().parents[2] / "scripts/check_template_spacing.py"
)
spacing = importlib.util.module_from_spec(spec)
spec.loader.exec_module(spacing)


def sample():
    doc = Document()
    p = doc.add_paragraph()
    p.add_run("{{PARTY_A_").bold = True
    p.add_run("REPRESENTATIVE}}")
    p.add_run("Chức vụ: {{PARTY_A_TITLE}}")
    doc.add_table(rows=1, cols=1).cell(0, 0).text = "{{TOTAL_AMOUNT}}(Bằng chữ: {{AMOUNT_TEXT}})"
    doc.sections[0].header.paragraphs[0].text = "{{UNIT_PRICE_VAT}}đ/tháng"
    doc.sections[0].footer.paragraphs[0].text = "{{TOTAL_AMOUNT}}đ"
    doc.add_paragraph("PL{{SEQ}} {{DAY}}/{{MONTH}}/{{YEAR}} {{VAT_RATE}}% {{TOTAL_AMOUNT}} đồng")
    p = doc.add_paragraph("{{PARTY_B_REPRESENTATIVE}}")
    p.add_run().add_tab()
    p.add_run("Chức vụ: Giám đốc")
    p = doc.add_paragraph("{{PARTY_A_REPRESENTATIVE}}")
    p.add_run().add_break()
    p.add_run("Chức vụ: Giám đốc")
    output = io.BytesIO()
    doc.save(output)
    return output.getvalue()


def test_repair_split_runs_tables_headers_and_footers_then_render():
    source = sample()
    before = Document(io.BytesIO(render_docx(source, {"PARTY_A_REPRESENTATIVE": "Nguyễn Văn A"})))
    assert "AChức vụ" in before.paragraphs[0].text  # Reproduces the reported defect.
    repaired, count = spacing.repair_docx(source)
    assert count == 4
    doc = Document(io.BytesIO(repaired))
    assert doc.paragraphs[0].text == "{{PARTY_A_REPRESENTATIVE}} Chức vụ: {{PARTY_A_TITLE}}"
    assert doc.paragraphs[0].runs[0].bold is True
    assert len(doc.paragraphs[0].runs) == 3
    rendered = Document(io.BytesIO(render_docx(repaired, {
        "PARTY_A_REPRESENTATIVE": "Nguyễn Văn A", "TOTAL_AMOUNT": "1.000.000",
        "UNIT_PRICE_VAT": "100.000", "AMOUNT_TEXT": "Một triệu đồng",
    })))
    assert "Nguyễn Văn A Chức vụ:" in rendered.paragraphs[0].text
    assert rendered.tables[0].cell(0, 0).text == "1.000.000 (Bằng chữ: Một triệu đồng)"
    assert rendered.sections[0].header.paragraphs[0].text == "100.000 đ/tháng"
    assert rendered.sections[0].footer.paragraphs[0].text == "1.000.000 đ"
    assert doc.paragraphs[1].text == Document(io.BytesIO(source)).paragraphs[1].text
    assert "\tChức vụ" in doc.paragraphs[2].text
    assert "\nChức vụ" in doc.paragraphs[3].text
    assert spacing.repair_docx(repaired) == (repaired, 0)
    with ZipFile(io.BytesIO(source)) as old, ZipFile(io.BytesIO(repaired)) as new:
        assert old.namelist() == new.namelist()
        for name in old.namelist():
            if name not in {"word/document.xml", "word/header1.xml", "word/footer1.xml"}:
                assert old.read(name) == new.read(name)


def test_backup_exact_original_and_refuse_overwrite(tmp_path):
    path, backup = tmp_path / "template.docx", tmp_path / "backup.docx"
    original = sample()
    path.write_bytes(original)
    backup.write_bytes(b"existing backup")
    with pytest.raises(FileExistsError):
        spacing.fix_file(path, backup)
    assert path.read_bytes() == original
    backup.unlink()
    assert spacing.fix_file(path, backup) == 4
    assert backup.read_bytes() == original
    fixed = path.read_bytes()
    assert spacing.fix_file(path, backup) == 0
    assert path.read_bytes() == fixed


def test_does_not_remove_an_existing_temporary_file(tmp_path):
    path = tmp_path / "template.docx"
    existing = tmp_path / "template.docx.spacing.tmp"
    existing.write_bytes(b"unrelated work")
    path.write_bytes(sample())
    assert spacing.fix_file(path, tmp_path / "backup.docx") == 4
    assert existing.read_bytes() == b"unrelated work"
