"""numbering.py — khuôn số hợp đồng, cấu hình bằng dữ liệu.

Trước đây hai khuôn số hợp đồng của một tổ chức cụ thể được viết cứng trong
`contract_service.py`, chọn bằng cách dò chuỗi trong mã sản phẩm. Cách đó vừa
khoá mã nguồn vào một tổ chức (không công khai được), vừa buộc bất kỳ ai muốn
đánh số kiểu khác phải sửa Python.

Khuôn số nay là **dữ liệu**: một chuỗi mẫu lưu ở `Product.contract_number_format`,
với ba chỗ điền:

    {seq}    số thứ tự, đệm 0 hai chữ số  → "01"
    {slug}   mã rút gọn của khách hàng     → "TRAMYTEXAA"
    {year}   năm 4 chữ số                  → "2026"

Ví dụ ``"{seq}/{slug}/{year}"`` cho ra ``01/TRAMYTEXAA/2026``.

Bộ đếm số thứ tự riêng cho từng (khách hàng, khuôn, năm): cùng một khách hàng
mua hai sản phẩm đánh số khác khuôn thì mỗi bên đếm từ 01, và sang năm mới reset.

Nâng cấp không mất dữ liệu
--------------------------
`infer_format()` đọc ngược khuôn từ chính các số hợp đồng đã có. Nhờ vậy
migration khôi phục được đúng khuôn mà một bản cài đặt đang dùng, mà **không cần
nhắc tới khuôn đó trong mã nguồn** — điều bắt buộc, vì chính những chuỗi ấy là
thứ cần gỡ khỏi bản công khai.
"""

from __future__ import annotations

import re
from datetime import date
from typing import Optional

#: Khuôn trung tính cho bản cài đặt mới, và là lưới an toàn khi chưa cấu hình gì.
DEFAULT_FORMAT = "{seq}/{slug}/{year}"

#: Các chỗ điền hợp lệ.
PLACEHOLDERS = ("seq", "slug", "year")

_SEQ_WIDTH = 2


class InvalidNumberFormat(ValueError):
    """Khuôn số hợp đồng thiếu chỗ điền bắt buộc, hoặc chứa chỗ điền lạ."""


def validate_format(number_format: str) -> str:
    """Kiểm tra khuôn dùng được. Trả về chính nó nếu hợp lệ.

    Raises:
        InvalidNumberFormat: thiếu ``{seq}`` (không có nó thì mọi hợp đồng của
            một khách hàng trong năm sẽ trùng số), hoặc có chỗ điền không hiểu.
    """
    if not number_format or not number_format.strip():
        raise InvalidNumberFormat("Khuôn số hợp đồng không được để trống.")

    found = set(re.findall(r"\{(\w+)\}", number_format))
    unknown = found - set(PLACEHOLDERS)
    if unknown:
        raise InvalidNumberFormat(
            f"Chỗ điền không hợp lệ: {sorted(unknown)}. "
            f"Chỉ dùng được: {list(PLACEHOLDERS)}"
        )
    if "seq" not in found:
        raise InvalidNumberFormat(
            "Khuôn phải chứa {seq}, nếu không mọi hợp đồng sẽ trùng số."
        )
    return number_format


def build_number(seq: str, slug: str, number_format: Optional[str] = None,
                 year: int = 0) -> str:
    """Ghép một số hợp đồng hoàn chỉnh từ khuôn."""
    fmt = validate_format(number_format or DEFAULT_FORMAT)
    return fmt.format(seq=seq, slug=slug or "", year=year or date.today().year)


def like_pattern(number_format: Optional[str] = None, year: int = 0) -> str:
    """Đổi khuôn thành mẫu SQL LIKE để tìm các số đã dùng trong năm.

    ``{seq}`` và ``{slug}`` thành ``%``; ``{year}`` thành năm cụ thể. Nhờ vậy
    bộ đếm chỉ nhìn những hợp đồng cùng khuôn — sản phẩm khác khuôn đếm riêng.
    """
    fmt = validate_format(number_format or DEFAULT_FORMAT)
    return fmt.format(seq="%", slug="%", year=year or date.today().year)


def next_seq(used: set[int]) -> str:
    """Số thứ tự dương nhỏ nhất chưa dùng, đã đệm 0.

    Lấp khoảng trống: xoá 01 mà còn 02 thì kết quả là 01 — hành vi này có từ
    trước và cố ý giữ, để số hợp đồng không nhảy cóc sau khi xoá bản nháp.
    """
    seq = 1
    while seq in used:
        seq += 1
    return str(seq).zfill(_SEQ_WIDTH)


def seq_of(contract_number: str) -> Optional[int]:
    """Bóc số thứ tự ra khỏi một số hợp đồng, hoặc None nếu không đọc được."""
    if not contract_number:
        return None
    head = contract_number.split("/")[0].strip()
    try:
        return int(head)
    except ValueError:
        return None


def infer_format(contract_number: str, slug: str, year) -> Optional[str]:
    """Suy ngược khuôn từ một số hợp đồng đã có.

    Ví dụ số ``01/ABC-TRAMYTEXAA/XYZ/2026`` với slug ``TRAMYTEXAA`` và năm 2026
    cho ra ``{seq}/ABC-{slug}/XYZ/{year}``.

    Trả về None khi không suy được chắc chắn — khi đó nơi gọi nên dùng
    DEFAULT_FORMAT thay vì đoán bừa.

    Dùng cho migration: khôi phục khuôn của bản cài đặt hiện có mà không cần
    ghi khuôn đó vào mã nguồn.
    """
    if not contract_number or not slug:
        return None

    year_text = str(year)
    remainder = contract_number

    # {seq} — luôn là thành phần đầu tiên, và phải là số.
    head, sep, tail = remainder.partition("/")
    if not sep or not head.strip().isdigit():
        return None
    result = "{seq}" + sep + tail

    # {slug} — chỉ thay lần xuất hiện đầu. Slug là chữ+số viết hoa nên khó
    # đụng nhầm phần cố định của khuôn.
    if slug not in result:
        return None
    result = result.replace(slug, "{slug}", 1)

    # {year} — thay lần xuất hiện cuối: năm gần như luôn ở đuôi, và thay từ
    # cuối lên tránh nuốt nhầm một con số trùng năm nằm giữa chuỗi.
    if year_text not in result:
        return None
    cut = result.rfind(year_text)
    result = result[:cut] + "{year}" + result[cut + len(year_text):]

    try:
        return validate_format(result)
    except InvalidNumberFormat:
        return None
