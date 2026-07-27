"""Khuôn số hợp đồng: cấu hình được, mà không đổi hành vi của bản cài đặt cũ.

Hai nhóm test:

1. Khuôn hoạt động đúng như một cơ chế chung — khuôn tuỳ ý, bộ đếm riêng theo
   khuôn, reset theo năm, lấp khoảng trống.
2. Kiểm tra chống hồi quy trên DỮ LIỆU THẬT: nạp `data/golden_contract_numbers.json`
   (chụp trước khi trung tính hoá) và khẳng định migration suy ra đúng khuôn để
   sinh lại y hệt từng số hợp đồng. File này nằm ngoài Git nên test tự bỏ qua
   ở CI công khai — nó tồn tại để bảo vệ máy của người đang dùng thật.
"""

from __future__ import annotations

import json
from datetime import date
from pathlib import Path

import pytest

GOLDEN = (Path(__file__).resolve().parents[2] / "data"
          / "golden_contract_numbers.json")


# ─── Cơ chế khuôn số ─────────────────────────────────────────────────────────

def test_default_format_is_neutral(db_engine):
    from app.services.numbering import DEFAULT_FORMAT, build_number

    assert build_number("01", "TRAMYTEXAA", DEFAULT_FORMAT, 2026) == "01/TRAMYTEXAA/2026"


@pytest.mark.parametrize("fmt,expected", [
    ("{seq}/{slug}/{year}", "07/ACME/2026"),
    ("{seq}/ABC/XY-{slug}/{year}", "07/ABC/XY-ACME/2026"),
    ("HD-{seq}-{slug}-{year}", "HD-07-ACME-2026"),
    ("{seq}/{year}", "07/2026"),
])
def test_arbitrary_formats(db_engine, fmt, expected):
    from app.services.numbering import build_number

    assert build_number("07", "ACME", fmt, 2026) == expected


@pytest.mark.parametrize("bad", ["", "   ", "{slug}/{year}", "{seq}/{nonsense}"])
def test_invalid_formats_are_rejected(db_engine, bad):
    from app.services.numbering import InvalidNumberFormat, validate_format

    with pytest.raises(InvalidNumberFormat):
        validate_format(bad)


def test_like_pattern_narrows_to_one_format(db_engine):
    from app.services.numbering import like_pattern

    assert like_pattern("{seq}/ABC/XY-{slug}/{year}", 2026) == "%/ABC/XY-%/2026"


# ─── Suy ngược khuôn ─────────────────────────────────────────────────────────

@pytest.mark.parametrize("number,slug,year,expected", [
    ("01/ABC/XY-TRAMYTEXAA/2026", "TRAMYTEXAA", 2026, "{seq}/ABC/XY-{slug}/{year}"),
    ("01/CORP-TRAMYTEXAA/PKG/2026", "TRAMYTEXAA", 2026, "{seq}/CORP-{slug}/PKG/{year}"),
    ("12/TRAMYTEXAA/2026", "TRAMYTEXAA", 2026, "{seq}/{slug}/{year}"),
])
def test_infer_format_round_trips(db_engine, number, slug, year, expected):
    from app.services.numbering import build_number, infer_format

    inferred = infer_format(number, slug, year)
    assert inferred == expected
    # Vòng tròn khép kín: khuôn suy ra phải dựng lại đúng số ban đầu.
    assert build_number(number.split("/")[0], slug, inferred, year) == number


@pytest.mark.parametrize("number,slug,year", [
    ("khong-co-so/ACME/2026", "ACME", 2026),   # thành phần đầu không phải số
    ("01/ACME/2026", "KHONGKHOP", 2026),       # slug không xuất hiện
    ("01/ACME/2026", "ACME", 1999),            # năm không xuất hiện
    ("", "ACME", 2026),
])
def test_infer_format_gives_up_cleanly(db_engine, number, slug, year):
    from app.services.numbering import infer_format

    assert infer_format(number, slug, year) is None


# ─── Bộ đếm số thứ tự ────────────────────────────────────────────────────────

def test_counter_is_per_format(db_engine, db_session, make_customer,
                               make_product, make_contract):
    """Cùng khách hàng, hai sản phẩm khác khuôn → mỗi bên đếm từ 01."""
    from app.services.contract_service import get_next_contract_seq

    customer = make_customer()
    fmt_a = "{seq}/AAA/{slug}/{year}"
    fmt_b = "{seq}/BBB/{slug}/{year}"

    make_contract(customer=customer, contract_number=f"01/AAA/X/{date.today().year}")

    assert get_next_contract_seq(db_session, customer.id, number_format=fmt_a) == "02"
    assert get_next_contract_seq(db_session, customer.id, number_format=fmt_b) == "01"


def test_counter_fills_gaps(db_engine, db_session, make_customer, make_contract):
    from app.services.contract_service import get_next_contract_seq

    year = date.today().year
    customer = make_customer()
    fmt = "{seq}/{slug}/{year}"
    make_contract(customer=customer, contract_number=f"02/ACME/{year}")

    assert get_next_contract_seq(db_session, customer.id, number_format=fmt) == "01"


def test_counter_resets_each_year(db_engine, db_session, make_customer, make_contract):
    from app.services.contract_service import get_next_contract_seq

    customer = make_customer()
    fmt = "{seq}/{slug}/{year}"
    make_contract(customer=customer, contract_number="01/ACME/2025")

    assert get_next_contract_seq(db_session, customer.id,
                                 number_format=fmt, year=2026) == "01"


def test_format_comes_from_the_product(db_engine, db_session, make_product):
    from app.services.contract_service import resolve_number_format
    from app.services.numbering import DEFAULT_FORMAT

    configured = make_product(contract_number_format="{seq}/ZZZ/{slug}/{year}")
    plain = make_product()

    assert resolve_number_format(db_session, configured.id) == "{seq}/ZZZ/{slug}/{year}"
    assert resolve_number_format(db_session, plain.id) == DEFAULT_FORMAT
    assert resolve_number_format(db_session, None) == DEFAULT_FORMAT
    assert resolve_number_format(db_session, 999_999) == DEFAULT_FORMAT


# ─── Chống hồi quy trên dữ liệu thật ─────────────────────────────────────────

@pytest.mark.skipif(not GOLDEN.is_file(),
                    reason="không có golden file — bản cài đặt sạch hoặc CI công khai")
def test_real_contract_numbers_are_reproducible(db_engine):
    """Mọi số hợp đồng đã có phải dựng lại được từ khuôn suy ngược.

    Đây là lưới an toàn của việc trung tính hoá: nếu bản refactor làm lệch cách
    đánh số dù chỉ một ký tự, test này đỏ.
    """
    from app.services.contract_service import make_contract_slug
    from app.services.numbering import build_number, infer_format

    golden = json.loads(GOLDEN.read_text(encoding="utf-8"))
    contracts = [c for c in golden["contracts"] if c.get("contract_number")]
    assert contracts, "golden file rỗng"

    # Suy khuôn cho từng sản phẩm từ hợp đồng đầu tiên gặp được — đúng việc
    # mà _backfill_contract_number_formats() làm lúc migration.
    formats: dict = {}
    failures: list[str] = []

    for entry in contracts:
        number = entry["contract_number"]
        slug = make_contract_slug(entry["customer_name"] or "")
        year = int(number.rsplit("/", 1)[-1]) if "/" in number else 0
        fmt = infer_format(number, slug, year)
        if fmt and entry["product_id"] not in formats:
            formats[entry["product_id"]] = fmt

    assert formats, "không suy được khuôn nào từ dữ liệu thật"

    for entry in contracts:
        number = entry["contract_number"]
        fmt = formats.get(entry["product_id"])
        if not fmt:
            failures.append(f"{number}: sản phẩm không có khuôn")
            continue
        slug = make_contract_slug(entry["customer_name"] or "")
        seq = number.split("/")[0]
        year = int(number.rsplit("/", 1)[-1])
        rebuilt = build_number(seq, slug, fmt, year)
        if rebuilt != number:
            failures.append(f"{number} → dựng lại thành {rebuilt}")

    assert not failures, (
        f"{len(failures)}/{len(contracts)} số hợp đồng không dựng lại đúng:\n  "
        + "\n  ".join(failures[:10])
    )


# ─── Giá tự điền từ danh mục ─────────────────────────────────────────────────

TEMPLATE_DIR = (Path(__file__).resolve().parents[2]
                / "ContractForge" / "templates")


@pytest.mark.parametrize("relative", ["contracts/form.html", "batch/form.html"])
def test_price_autofill_reads_the_catalog_not_a_hardcoded_table(relative):
    """Không được có bảng giá viết cứng theo mã sản phẩm trong template.

    Bản trước giữ một map `PRICES = {'<mã>': ...}` ngay trong JS, nên sản phẩm
    mới không tự điền giá, và đổi giá phải sửa hai chỗ.
    """
    source = (TEMPLATE_DIR / relative).read_text(encoding="utf-8")

    assert "var PRICES" not in source, "bảng giá viết cứng đã quay lại"
    assert "data-price" in source, "option phải mang giá từ danh mục"
    assert "data-vat" in source


def test_vat_from_data_attribute_is_not_parsed_as_vietnamese_number():
    """data-vat là số thô ("8.00"), không phải số định dạng Việt Nam.

    parseN() coi dấu chấm là phân cách nghìn, nên parseN("8.00") ra 800 — VAT
    8% biến thành 800% và giá sau thuế sai gấp ~8 lần. Đây là lỗi đã xảy ra
    thật khi chuyển từ bảng giá cứng sang đọc danh mục.
    """
    source = (TEMPLATE_DIR / "contracts" / "form.html").read_text(encoding="utf-8")

    body = source[source.index("function priceOfOption"):]
    body = body[:body.index("\n}")]

    assert "parseN(" not in body, (
        "priceOfOption phải dùng parseFloat cho data-*, parseN sẽ đọc sai VAT"
    )
    assert "parseFloat(" in body


@pytest.mark.skipif(not GOLDEN.is_file(), reason="không có golden file")
def test_golden_file_covers_every_product_shape(db_engine):
    """Chốt lại rằng golden file thật sự bao được các khuôn đang dùng."""
    golden = json.loads(GOLDEN.read_text(encoding="utf-8"))
    product_ids = {c["product_id"] for c in golden["contracts"] if c["product_id"]}
    assert len(product_ids) >= 2, (
        "chỉ có 1 sản phẩm trong golden file — không đủ để bắt lỗi lẫn khuôn"
    )
