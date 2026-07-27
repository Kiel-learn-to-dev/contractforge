"""Bản clone sạch phải chạy được, không phụ thuộc file riêng của ai.

Mẫu hợp đồng là do người dùng tải lên, nhưng biểu 08a thì được render on-the-fly
từ một file đóng gói sẵn. Repo loại trừ mọi .docx trong assets vì chúng mang tiêu
đề thư của tổ chức cụ thể — nên nếu không có mẫu trung tính đi kèm, chức năng
08a chết ngay trên bản clone.
"""

from __future__ import annotations

import re
import zipfile
from pathlib import Path

import pytest

ASSETS = (Path(__file__).resolve().parents[2]
          / "ContractForge" / "assets" / "default_templates")
SAMPLE = ASSETS / "sample_form_08a.docx"


def _docx_text(path: Path) -> str:
    with zipfile.ZipFile(path) as archive:
        return archive.read("word/document.xml").decode("utf-8", errors="ignore")


def test_sample_form_is_committed():
    assert SAMPLE.is_file(), (
        "thiếu sample_form_08a.docx — chạy scripts/make_sample_templates.py"
    )


def test_sample_form_carries_every_placeholder_the_service_fills():
    """Mẫu phải nhận đủ các trường mà build_08a_context() sinh ra."""
    text = _docx_text(SAMPLE)
    # Placeholder có thể bị Word cắt thành nhiều run, nên bỏ hết thẻ XML trước.
    plain = re.sub(r"<[^>]+>", "", text)

    required = {
        "BUDGET_UNIT_NAME", "BUDGET_UNIT_CODE", "WORK_CONTENT",
        "CONTRACT_NUMBER", "PARTY_A_NAME", "PARTY_A_REPRESENTATIVE",
        "PARTY_A_TITLE_UPPER", "PARTY_B_REPRESENTATIVE", "PARTY_B_TITLE_UPPER",
        "QUANTITY", "UNIT_PRICE_VAT", "TOTAL_AMOUNT", "SIGN_DATE",
        "ACCEPTANCE_DATE_DAY", "ACCEPTANCE_DATE_MONTH", "ACCEPTANCE_DATE_YEAR",
    }
    missing = {name for name in required if "{{" + name + "}}" not in plain}
    assert not missing, f"mẫu thiếu placeholder: {sorted(missing)}"


def test_sample_form_is_fictional(db_engine):
    """Mẫu công khai không được chứa dữ liệu thật của tổ chức nào."""
    import sys

    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
    from check_public_repo import load_private_denylist, scan_docx

    root = Path(__file__).resolve().parents[2]
    terms = load_private_denylist(root)
    if not terms:
        pytest.skip("không có denylist cục bộ")

    _, hits = scan_docx(SAMPLE, "sample_form_08a.docx", terms)
    assert not hits, f"mẫu chứa dữ liệu riêng: {[h.reason for h in hits]}"


def test_private_template_wins_when_present(db_engine, tmp_path):
    """Cơ quan đã có mẫu riêng thì nâng cấp không được đổi giấy tờ đang phát hành."""
    from app.paths import resolve_form_08a_template

    assets = tmp_path / "assets"
    (assets / "default_templates").mkdir(parents=True)
    private = assets / "default_templates" / "template_08A.docx"
    sample = assets / "default_templates" / "sample_form_08a.docx"
    private.write_bytes(b"private")
    sample.write_bytes(b"sample")

    assert resolve_form_08a_template(assets) == private


def test_sample_is_used_when_no_private_template(db_engine, tmp_path):
    from app.paths import resolve_form_08a_template

    assets = tmp_path / "assets"
    (assets / "default_templates").mkdir(parents=True)
    sample = assets / "default_templates" / "sample_form_08a.docx"
    sample.write_bytes(b"sample")

    assert resolve_form_08a_template(assets) == sample


def test_form_08a_template_actually_exists(db_engine):
    """Đường dẫn app đang dùng phải trỏ tới một file có thật."""
    from app.paths import FORM_08A_TEMPLATE

    assert FORM_08A_TEMPLATE.is_file(), (
        f"không tìm thấy mẫu 08a tại {FORM_08A_TEMPLATE}"
    )
