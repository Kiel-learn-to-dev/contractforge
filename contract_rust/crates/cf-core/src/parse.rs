//! Đọc dữ liệu từ các biểu mẫu.

use rust_decimal::Decimal;
use std::sync::LazyLock;

use crate::{error::CoreError, money::to_decimal};

static MANUAL_THOUSANDS: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^\d{1,3}(?:\.\d{3})+$").expect("regex pattern is a source constant")
});

/// Port của app/services/contract_service.py::_safe_decimal.
pub fn safe_decimal(raw: &str) -> Option<Decimal> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.replace('.', "").replace(',', ".").parse().ok()
}

/// Port của app/services/contract_service.py::_safe_int.
pub fn safe_int(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.replace([',', '.'], "").parse().ok()
}

/// Port của app/services/contract_service.py::_dec_str.
pub fn dec_str(value: Option<Decimal>) -> String {
    value
        .map(|number| number.normalize().to_string())
        .unwrap_or_default()
}

/// Port của app/services/manual_quotation.py::_number.
pub fn manual_number(raw: &str, label: &str, money: bool) -> Result<Decimal, CoreError> {
    let mut value: String = raw.trim().chars().filter(|ch| *ch != ' ').collect();
    if value.contains(',') {
        value = value.replace('.', "").replace(',', ".");
    } else if money && MANUAL_THOUSANDS.is_match(&value) {
        value = value.replace('.', "");
    }
    to_decimal(&value, label)
}
