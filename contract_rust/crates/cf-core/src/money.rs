//! Chuyển đổi và làm tròn tiền.

use rust_decimal::{Decimal, RoundingStrategy};
use std::str::FromStr;

use crate::error::CoreError;

/// Port của app/services/pricing.py::round_money.
pub fn round_half_up(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
}

/// Port của app/services/docx_renderer.py::_fmt_currency (phần cắt số).
pub fn trunc_int(value: Decimal) -> i128 {
    let truncated = value.trunc();
    truncated.mantissa() / 10_i128.pow(truncated.scale())
}

/// Port của app/services/pricing.py::to_decimal.
pub fn to_decimal(raw: &str, field: &str) -> Result<Decimal, CoreError> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(CoreError::Invalid(format!("Thiếu {field}.")));
    }
    Decimal::from_str(value)
        .map_err(|_| CoreError::Invalid(format!("{field} không phải là số: '{raw}'")))
}
