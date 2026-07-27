"""
amount_to_words_vi.py — Chuyển số nguyên dương sang chữ tiếng Việt.

Quy tắc:
  - Đơn vị: tỷ → triệu → nghìn → đồng
  - Hỗ trợ đến 999,999,999,999,999 (999 nghìn tỷ)
  - Output: "Hai mươi bốn triệu đồng"
  - Số 0 → "Không đồng"
  - Input bị coi là VNĐ (int hoặc Decimal)

Ví dụ từ blueprint:
  24_000_000  → "Hai mươi bốn triệu đồng"
  22_856_736  → "Hai mươi hai triệu, tám trăm năm mươi sáu nghìn,
                  bảy trăm ba mươi sáu đồng"
"""

from decimal import Decimal
from typing import Union

_ONES = [
    "", "một", "hai", "ba", "bốn", "năm", "sáu", "bảy", "tám", "chín",
]

_TEENS = [
    "mười", "mười một", "mười hai", "mười ba", "mười bốn",
    "mười lăm", "mười sáu", "mười bảy", "mười tám", "mười chín",
]


def _say_ones(n: int) -> str:
    """0–9 → tên (rỗng nếu 0)."""
    return _ONES[n]


def _say_two_digits(n: int, is_full_group: bool = True) -> str:
    """
    Đọc số 0–99.
    is_full_group=True khi đây là nhóm con của số lớn hơn
    (thêm "không" cho chục=0 và đơn vị ≠ 0).
    """
    if n == 0:
        return ""
    if n < 10:
        if is_full_group:
            return f"không mươi {_say_ones(n)}"   # "không mươi lẻ" không dùng
        return _say_ones(n)
    if n < 20:
        return _TEENS[n - 10]
    chuc = n // 10
    don_vi = n % 10
    prefix = f"{_say_ones(chuc)} mươi"
    if don_vi == 0:
        return prefix
    if don_vi == 1:
        return f"{prefix} mốt"
    if don_vi == 5 and chuc > 1:
        return f"{prefix} lăm"
    return f"{prefix} {_say_ones(don_vi)}"


def _say_three_digits(n: int) -> str:
    """Đọc 0–999 (nhóm 3 chữ số)."""
    if n == 0:
        return ""
    tram = n // 100
    rem = n % 100
    parts = []
    if tram > 0:
        parts.append(f"{_say_ones(tram)} trăm")
    if rem > 0:
        if rem < 10 and tram > 0:
            parts.append(f"không mươi {_say_ones(rem)}")
        else:
            parts.append(_say_two_digits(rem, is_full_group=False))
    return " ".join(parts)


def amount_to_words(amount: Union[int, float, Decimal, str], suffix: str = "đồng") -> str:
    """
    Chuyển số tiền sang chữ tiếng Việt.

    Args:
        amount: Số tiền (int, float, Decimal hoặc str dạng số).
                Số thập phân sẽ được làm tròn xuống.
        suffix: Đơn vị đứng sau, mặc định "đồng".

    Returns:
        Chuỗi tiếng Việt viết hoa chữ đầu, VD: "Hai mươi bốn triệu đồng".

    Raises:
        ValueError nếu amount âm hoặc không hợp lệ.
    """
    try:
        n = int(Decimal(str(amount)))
    except Exception:
        raise ValueError(f"Không hợp lệ: {amount!r}")

    if n < 0:
        raise ValueError("Không hỗ trợ số âm")

    if n == 0:
        return f"Không {suffix}"

    # Tách thành 4 nhóm: tỷ / triệu / nghìn / đơn vị
    ty       = n // 1_000_000_000
    trieu    = (n % 1_000_000_000) // 1_000_000
    nghin    = (n % 1_000_000) // 1_000
    don_vi_  = n % 1_000

    parts = []
    if ty:
        parts.append(f"{_say_three_digits(ty)} tỷ")
    if trieu:
        parts.append(f"{_say_three_digits(trieu)} triệu")
    if nghin:
        parts.append(f"{_say_three_digits(nghin)} nghìn")
    if don_vi_:
        parts.append(_say_three_digits(don_vi_))

    result = ", ".join(parts) + (f" {suffix}" if suffix else "")
    # Viết hoa chữ đầu tiên
    return result[0].upper() + result[1:]


def amount_to_words_chan(amount: Union[int, float, Decimal, str]) -> str:
    """
    Phiên bản thêm "chẵn" ở cuối, dùng cho biên bản thanh lý.
    VD: "Hai mươi bốn triệu đồng chẵn"
    """
    base = amount_to_words(amount, suffix="đồng")
    return base + " chẵn"
