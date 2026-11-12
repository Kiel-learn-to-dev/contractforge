"""pricing.py — nguồn duy nhất cho phép tính tiền của hợp đồng theo dòng.

Quy tắc, theo đúng các cột của bảng giá trị trong hợp đồng:

    số kỳ            = số tháng / số tháng của một kỳ giá
    thành tiền trước thuế = làm tròn(đơn giá × số lượng × số kỳ)
    tiền thuế        = làm tròn(thành tiền trước thuế × thuế suất / 100)
    thành tiền sau thuế   = thành tiền trước thuế + tiền thuế

Tổng hợp đồng là tổng các dòng. Không nhân từ đơn giá có thuế đã làm tròn: cách
đó lệch vài đồng so với bảng in ra, và là cách hợp đồng `legacy` đã dùng. Hợp
đồng `legacy` vì thế KHÔNG đi qua module này.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date
from decimal import Decimal, InvalidOperation, ROUND_CEILING, ROUND_HALF_UP
from typing import Optional, Sequence

from app.utils.amount_to_words_vi import amount_to_words
from app.utils.date_helpers import compute_end_date

_ONE = Decimal("1")
_HUNDRED = Decimal("100")


class PricingError(ValueError):
    """Dữ liệu đầu vào không tính được tiền."""


def to_decimal(value, field: str = "giá trị") -> Decimal:
    """Đọc số ở dạng máy ("82.5", 82.5, Decimal). KHÔNG đọc số kiểu "1.000.000"."""
    if isinstance(value, Decimal):
        return value
    if value is None or str(value).strip() == "":
        raise PricingError(f"Thiếu {field}.")
    try:
        return Decimal(str(value).strip())
    except InvalidOperation:
        raise PricingError(f"{field} không phải là số: {value!r}")


def round_money(value) -> Decimal:
    """Làm tròn tới đồng, 0,5 làm tròn lên."""
    return to_decimal(value).quantize(_ONE, rounding=ROUND_HALF_UP)


def normalize_quantity(raw, min_quantity=None, step=None) -> Decimal:
    """Áp mức sàn rồi làm tròn LÊN theo bội số của `step`."""
    qty = to_decimal(raw, "số lượng")
    if qty <= 0:
        raise PricingError("Số lượng phải lớn hơn 0.")
    if min_quantity:
        qty = max(qty, Decimal(min_quantity))
    if step:
        step_d = Decimal(step)
        qty = (qty / step_d).to_integral_value(rounding=ROUND_CEILING) * step_d
    return qty


def period_count(months, price_period_months=1) -> Decimal:
    months_d = to_decimal(months, "số tháng")
    period_d = to_decimal(price_period_months or 1, "kỳ giá")
    if months_d <= 0 or period_d <= 0:
        raise PricingError("Số tháng và kỳ giá phải lớn hơn 0.")
    return months_d / period_d


@dataclass(frozen=True)
class Line:
    description: str
    unit_label: str
    quantity: Decimal
    period_count: Decimal
    unit_price_pre_vat: Decimal
    vat_rate: Decimal
    amount_pre_vat: Decimal
    vat_amount: Decimal
    amount_total: Decimal
    product_id: Optional[int] = None
    note: str = ""


@dataclass(frozen=True)
class Totals:
    pre_vat: Decimal
    vat: Decimal
    total: Decimal
    amount_text: str


def price_line(*, description: str, unit_label: str = "", quantity, months,
               unit_price_pre_vat, vat_rate=0, price_period_months=1,
               amount_pre_vat_override=None, product_id: Optional[int] = None,
               note: str = "") -> Line:
    qty = to_decimal(quantity, "số lượng")
    price = to_decimal(unit_price_pre_vat, "đơn giá")
    vat = to_decimal(vat_rate if vat_rate not in (None, "") else 0, "thuế suất")
    if qty <= 0:
        raise PricingError("Số lượng phải lớn hơn 0.")
    if price < 0:
        raise PricingError("Đơn giá không được âm.")
    if not (0 <= vat <= 100):
        raise PricingError("Thuế suất phải trong khoảng 0–100.")
    periods = period_count(months, price_period_months)

    if amount_pre_vat_override not in (None, ""):
        if not note.strip():
            raise PricingError("Ghi đè thành tiền phải kèm ghi chú lý do.")
        pre = round_money(amount_pre_vat_override)
    else:
        pre = round_money(price * qty * periods)
    vat_amount = round_money(pre * vat / _HUNDRED)
    return Line(description=description, unit_label=unit_label, quantity=qty,
                period_count=periods, unit_price_pre_vat=price, vat_rate=vat,
                amount_pre_vat=pre, vat_amount=vat_amount,
                amount_total=pre + vat_amount, product_id=product_id, note=note)


def unit_price_with_vat(unit_price_pre_vat, vat_rate) -> Decimal:
    """Đơn giá có VAT của gói chính, làm tròn tới đồng. Chỉ để hiển thị và ghi cột đơn giá.

    Tổng hợp đồng không nhân từ giá này: tổng là tổng các dòng (xem module docstring).
    """
    return round_money(to_decimal(unit_price_pre_vat) * (_ONE + to_decimal(vat_rate) / _HUNDRED))


def sum_lines(lines: Sequence[Line]) -> Totals:
    if not lines:
        raise PricingError("Hợp đồng phải có ít nhất một dòng.")
    pre = sum((l.amount_pre_vat for l in lines), Decimal("0"))
    vat = sum((l.vat_amount for l in lines), Decimal("0"))
    total = pre + vat
    return Totals(pre_vat=pre, vat=vat, total=total,
                  amount_text=amount_to_words(total) if total else "")


def addon_months(start: date, main_end: date) -> int:
    """Số tháng còn lại của gói chính tính từ `start`, làm tròn LÊN theo tháng."""
    if start > main_end:
        raise PricingError("Ngày bắt đầu gói bổ sung sau ngày hết hạn gói chính.")
    months = 1
    while compute_end_date(start, months) <= main_end:
        months += 1
        if months > 600:
            raise PricingError("Khoảng thời gian không hợp lệ.")
    return months


def fmt_money(value) -> str:
    """1234567 → '1.234.567'."""
    return f"{int(round_money(value)):,}".replace(",", ".")


def describe(lines: Sequence[Line], totals: Totals) -> str:
    """Mô tả cách tính, điền vào {{PRICING_DESCRIPTION}}."""
    parts = [f"{l.description}: {fmt_money(l.amount_total)} VNĐ" for l in lines]
    return "; ".join(parts) + f". Tổng cộng: {fmt_money(totals.total)} VNĐ"
