"""Ngày hết hạn là ngày cuối cùng của kỳ (Q6): 01/01/2027 + 12 tháng → 31/12/2027."""
from datetime import date

from app.services import pricing
from app.utils.date_helpers import add_months, compute_end_date, months_between


def test_compute_end_date_is_last_day_of_term():
    assert compute_end_date(date(2027, 1, 1), 12) == date(2027, 12, 31)


def test_compute_end_date_mid_month_start():
    assert compute_end_date(date(2027, 1, 15), 1) == date(2027, 2, 14)


def test_compute_end_date_month_end_start_clamps_then_subtracts_one_day():
    # 31/01 + 1 tháng = 28/02 (kẹp), trừ 1 ngày = 27/02
    assert compute_end_date(date(2027, 1, 31), 1) == date(2027, 2, 27)


def test_add_months_is_exclusive_boundary_and_clamps():
    assert add_months(date(2027, 1, 1), 12) == date(2028, 1, 1)
    assert add_months(date(2027, 1, 31), 1) == date(2027, 2, 28)


def test_months_between_counts_inclusive_end():
    assert months_between(date(2027, 1, 1), date(2027, 12, 31)) == 12


def test_months_between_mid_month_inclusive_end():
    assert months_between(date(2027, 1, 15), date(2028, 1, 14)) == 12


def test_months_between_old_style_exclusive_end_still_gives_twelve():
    # Dữ liệu cũ kiểu cộng tháng (end = start + 12) vẫn ra 12
    assert months_between(date(2027, 1, 15), date(2028, 1, 15)) == 12


def test_addon_months_unchanged_by_end_date_rule():
    assert pricing.addon_months(date(2027, 7, 1), date(2027, 12, 31)) == 6
