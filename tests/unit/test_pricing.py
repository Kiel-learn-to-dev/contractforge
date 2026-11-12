"""Unit tests for app.services.pricing — pure functions, no database.

Mỗi test khớp một dòng của bảng ở Task 1.1 trong PRICING_MODEL_IMPLEMENTATION_TASKS.md.
Dữ liệu hoàn toàn hư cấu.
"""

from datetime import date
from decimal import Decimal

import pytest

from app.services import pricing
from app.services.pricing import PricingError


def _line(**overrides):
    base = dict(description="Dịch vụ mẫu", quantity=1, months=12,
                unit_price_pre_vat=1000, vat_rate=10)
    base.update(overrides)
    return pricing.price_line(**base)


def test_price_line_three_units_twelve_months_ten_percent_vat():
    line = pricing.price_line(description="Gói A", quantity=3, months=12,
                              unit_price_pre_vat=500000, vat_rate=10)
    assert line.amount_pre_vat == 18_000_000
    assert line.vat_amount == 1_800_000
    assert line.amount_total == 19_800_000


def test_price_line_quantity_is_multiplied_before_rounding():
    line = pricing.price_line(description="Gói B", quantity=2000, months=12,
                              unit_price_pre_vat=75, vat_rate=10)
    assert line.amount_pre_vat == 1_800_000
    assert line.vat_amount == 180_000
    assert line.amount_total == 1_980_000


def test_price_line_yearly_price_period_gives_one_period():
    line = pricing.price_line(description="Gói năm", quantity=5, months=12,
                              unit_price_pre_vat=16320, vat_rate=10,
                              price_period_months=12)
    assert line.period_count == 1
    assert line.amount_pre_vat == 81_600
    assert line.vat_amount == 8_160
    assert line.amount_total == 89_760


def test_price_line_eight_percent_vat_rounds_vat_half_up():
    line = pricing.price_line(description="Gói C", quantity=3, months=12,
                              unit_price_pre_vat=440909, vat_rate=8)
    assert line.amount_pre_vat == 15_872_724
    assert line.vat_amount == 1_269_818
    assert line.amount_total == 17_142_542


def test_line_by_line_total_differs_from_legacy_vat_inclusive_multiplication():
    # Cách tính cũ: làm tròn đơn giá có VAT rồi nhân. Hợp đồng cũ đã lưu theo
    # cách đó, nên hợp đồng mới KHÔNG được tính lại bằng cách này.
    legacy_total = round(440909 * 1.08) * 36
    line = pricing.price_line(description="Gói C", quantity=3, months=12,
                              unit_price_pre_vat=440909, vat_rate=8)
    assert legacy_total == 17_142_552
    assert line.amount_total == 17_142_542
    assert line.amount_total != legacy_total


def test_normalize_quantity_raises_to_minimum():
    assert pricing.normalize_quantity(1200, 2000, 500) == Decimal("2000")


def test_normalize_quantity_rounds_up_to_step():
    assert pricing.normalize_quantity(2300, 2000, 500) == Decimal("2500")


def test_normalize_quantity_on_step_is_unchanged():
    assert pricing.normalize_quantity(2500, 2000, 500) == Decimal("2500")


def test_normalize_quantity_zero_is_rejected():
    with pytest.raises(PricingError):
        pricing.normalize_quantity(0)


def test_amount_override_without_note_is_rejected():
    with pytest.raises(PricingError):
        _line(amount_pre_vat_override=100000, note="")


def test_amount_override_with_note_replaces_pre_vat_amount():
    line = _line(amount_pre_vat_override=100000, note="lẻ tháng", vat_rate=10)
    assert line.amount_pre_vat == 100_000
    assert line.vat_amount == 10_000
    assert line.amount_total == 110_000


def test_sum_lines_adds_each_column_across_mixed_vat_rates():
    a = pricing.price_line(description="A", quantity=1, months=12,
                           unit_price_pre_vat=1_000_000, vat_rate=10)
    b = pricing.price_line(description="B", quantity=1, months=12,
                           unit_price_pre_vat=500_000, vat_rate=0)
    totals = pricing.sum_lines([a, b])
    assert totals.pre_vat == 12_000_000 + 6_000_000
    assert totals.vat == 1_200_000 + 0
    assert totals.total == totals.pre_vat + totals.vat
    assert totals.amount_text != ""


def test_sum_lines_empty_is_rejected():
    with pytest.raises(PricingError):
        pricing.sum_lines([])


def test_addon_months_counts_remaining_months_rounded_up():
    assert pricing.addon_months(date(2026, 10, 1), date(2027, 3, 30)) == 6


def test_addon_months_partial_first_month_counts_as_one():
    assert pricing.addon_months(date(2026, 10, 15), date(2026, 10, 20)) == 1


def test_addon_months_start_after_main_end_is_rejected():
    with pytest.raises(PricingError):
        pricing.addon_months(date(2027, 4, 1), date(2027, 3, 30))


def test_to_decimal_reads_machine_decimal_not_vietnamese_thousands():
    assert pricing.to_decimal("82.5") == Decimal("82.5")


def test_fmt_money_uses_dot_thousands_separator():
    assert pricing.fmt_money(1234567) == "1.234.567"


def test_describe_lists_each_line_and_total():
    a = pricing.price_line(description="Gói A", quantity=1, months=12,
                           unit_price_pre_vat=1000, vat_rate=10)
    totals = pricing.sum_lines([a])
    text = pricing.describe([a], totals)
    assert "Gói A: 13.200 VNĐ" in text  # 1.000 × 12 tháng = 12.000, + 10% VAT
    assert "Tổng cộng: 13.200 VNĐ" in text
