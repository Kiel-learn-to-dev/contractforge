"""Task 5.4 — canh gác: tiền chỉ được tính ở một nơi.

Mẫu HTML không được chứa hàm tính giá cũ, và file JS dùng chung không được có công thức tiền.
Các công thức legacy còn lại trong server phải có chú thích "legacy:".
"""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TEMPLATES = ROOT / "ContractForge" / "templates"
SHARED_JS = ROOT / "ContractForge" / "static" / "js" / "contract_items.js"
APP = ROOT / "ContractForge" / "app"

FORBIDDEN_IN_TEMPLATES = ("recalcPrice", "priceOfOption")
FORMULA_PATTERNS = ("* (1 +", "/ 100")


def test_no_template_defines_or_calls_the_old_price_functions():
    for path in TEMPLATES.rglob("*.html"):
        text = path.read_text(encoding="utf-8")
        for name in FORBIDDEN_IN_TEMPLATES:
            assert name not in text, f"{path.relative_to(ROOT)} còn {name}"


def test_shared_line_script_has_no_price_formula():
    text = SHARED_JS.read_text(encoding="utf-8")
    for pattern in FORMULA_PATTERNS:
        assert pattern not in text, f"contract_items.js còn công thức {pattern!r}"


def test_no_contract_form_template_has_a_price_formula():
    for name in ("contracts/form.html", "quotation/form.html", "batch/form.html"):
        text = (TEMPLATES / name).read_text(encoding="utf-8")
        for pattern in FORMULA_PATTERNS:
            assert pattern not in text, f"{name} còn công thức {pattern!r}"


def test_remaining_price_formulas_in_the_server_are_marked_legacy():
    """Mọi dòng công thức VAT còn lại trong app/ phải có chú thích legacy phía trên (hoặc
    là chính module pricing, hoặc là hàm hiển thị không tính tiền)."""
    allowed_files = {"pricing.py", "amount_to_words_vi.py"}
    needle = "(1 + "
    for path in APP.rglob("*.py"):
        if path.name in allowed_files:
            continue
        lines = path.read_text(encoding="utf-8").splitlines()
        for i, line in enumerate(lines):
            if needle in line and "vat" in line.lower():
                context = "\n".join(lines[max(0, i - 2):i + 1]).lower()
                assert "legacy" in context, (
                    f"{path.relative_to(ROOT)}:{i + 1} công thức VAT chưa được đánh dấu legacy")
