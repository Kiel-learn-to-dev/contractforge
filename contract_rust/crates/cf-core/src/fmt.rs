//! Định dạng số và văn bản.

use rust_decimal::Decimal;

use crate::{
    money::{round_half_up, trunc_int},
    text::strip_diacritics,
};

fn grouped_integer(value: &str) -> String {
    let (sign, digits) = value.strip_prefix('-').map_or(("", value), |d| ("-", d));
    let mut result = String::with_capacity(value.len() + value.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.chars().count() - index).is_multiple_of(3) {
            result.push('.');
        }
        result.push(ch);
    }
    format!("{sign}{result}")
}

/// Port của app/services/docx_renderer.py::_fmt_currency.
pub fn fmt_currency(value: Option<Decimal>) -> String {
    value
        .map(|number| grouped_integer(&trunc_int(number).to_string()))
        .unwrap_or_default()
}

/// Port của app/services/docx_renderer.py::_fmt_number.
pub fn fmt_number(value: Option<Decimal>) -> String {
    let Some(number) = value else {
        return String::new();
    };
    if number.fract().is_zero() {
        number.trunc().to_string()
    } else {
        number.to_string()
    }
}

/// Port của app/services/docx_renderer.py::_fmt_vat.
pub fn fmt_vat(value: Option<Decimal>) -> String {
    value
        .map(|number| {
            if number.fract().is_zero() {
                format!("{}%", number.trunc())
            } else {
                format!("{number}%")
            }
        })
        .unwrap_or_default()
}

/// Port của app/services/pricing.py::fmt_money.
pub fn fmt_money(value: Decimal) -> String {
    grouped_integer(&round_half_up(value).to_string())
}

/// Port của app/services/manual_quotation.py::_unit_price_text.
pub fn unit_price_text(value: Decimal) -> String {
    let mut raw = value.to_string();
    if raw.contains('.') {
        while raw.ends_with('0') {
            raw.pop();
        }
        if raw.ends_with('.') {
            raw.pop();
        }
    }
    let (integer, fraction) = raw.split_once('.').unwrap_or((&raw, ""));
    let integer = grouped_integer(integer);
    if fraction.is_empty() {
        integer
    } else {
        format!("{integer},{fraction}")
    }
}

/// Bỏ dấu trong tên phần tử đã làm sạch để chia sẻ logic thay thế ký tự.
pub(crate) fn ascii_fold(value: &str) -> String {
    strip_diacritics(value)
        .replace('Ơ', "O")
        .replace('ơ', "o")
        .replace('Ư', "U")
        .replace('ư', "u")
}
