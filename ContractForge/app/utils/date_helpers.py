"""
date_helpers.py — Format ngày tháng cho hợp đồng tiếng Việt.

Các hàm chính:
  format_date_vn(d)          → "15/01/2025"
  split_date_parts(d)        → {"day":"15","month":"01","year":"2025"}
  parse_date_input(s)        → date object từ nhiều format khác nhau
  add_months(start, months) → date           (start + months tháng, kẹp cuối tháng)
  compute_end_date(start, months) → date     (ngày CUỐI CÙNG của kỳ = add_months − 1 ngày)
  months_between(start, end) → int           (end là ngày cuối kỳ, bao gồm)
"""

from datetime import date, timedelta
from typing import Optional, Union
import re


def format_date_vn(d: Optional[date]) -> str:
    """Trả về 'dd/mm/yyyy', hoặc '' nếu d là None."""
    if d is None:
        return ""
    return d.strftime("%d/%m/%Y")


def split_date_parts(d: Optional[date]) -> dict:
    """
    Trả về dict với day, month, year dạng chuỗi 2/4 chữ số.
    Dùng để điền {{SIGN_DATE_DAY}}, {{SIGN_DATE_MONTH}}, {{SIGN_DATE_YEAR}}.
    """
    if d is None:
        return {"day": "", "month": "", "year": ""}
    return {
        "day":   d.strftime("%d"),    # "05", "15"
        "month": d.strftime("%m"),    # "01", "12"
        "year":  d.strftime("%Y"),    # "2025"
    }


def parse_date_input(s: Union[str, date, None]) -> Optional[date]:
    """
    Parse ngày từ nhiều định dạng phổ biến:
      - date object → trả về nguyên
      - "2025-01-15"  (ISO)
      - "15/01/2025"  (VN)
      - "15-01-2025"
      - "15/1/2025"
      - Rỗng / None → None
    """
    if s is None:
        return None
    if isinstance(s, date):
        return s
    s = str(s).strip()
    if not s:
        return None

    from datetime import datetime
    formats = [
        "%Y-%m-%d",   # ISO
        "%d/%m/%Y",   # VN
        "%d-%m-%Y",
        "%d/%m/%y",   # 2-digit year
        "%d-%m-%y",
    ]
    for fmt in formats:
        try:
            return datetime.strptime(s, fmt).date()
        except ValueError:
            continue

    raise ValueError(f"Không nhận dạng được định dạng ngày: {s!r}")


def add_months(start: date, months: int) -> date:
    """start + months tháng, kẹp ngày cuối tháng. 31/01 + 1 = 28/02."""
    import calendar
    month = start.month - 1 + months
    year = start.year + month // 12
    month = month % 12 + 1
    max_day = calendar.monthrange(year, month)[1]
    return date(year, month, min(start.day, max_day))


def compute_end_date(start: date, months: int) -> date:
    """Ngày cuối cùng của kỳ: 01/01/2027 + 12 tháng → 31/12/2027. 15/01/2027 + 1 tháng → 14/02/2027."""
    return add_months(start, months) - timedelta(days=1)


def months_between(start: date, end: date) -> int:
    """Số tháng nguyên của kỳ [start, end]; end là ngày cuối kỳ (bao gồm).
    01/01/2027 → 31/12/2027 = 12. Dữ liệu cũ kiểu end = start + n tháng vẫn ra n."""
    e = end + timedelta(days=1)
    return (e.year - start.year) * 12 + (e.month - start.month)
