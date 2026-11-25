//! Đọc số tiền thành chữ.

use rust_decimal::Decimal;

use crate::error::CoreError;

const ONES: [&str; 10] = [
    "", "một", "hai", "ba", "bốn", "năm", "sáu", "bảy", "tám", "chín",
];
const TEENS: [&str; 10] = [
    "mười",
    "mười một",
    "mười hai",
    "mười ba",
    "mười bốn",
    "mười lăm",
    "mười sáu",
    "mười bảy",
    "mười tám",
    "mười chín",
];

fn two_digits(value: u64, full_group: bool) -> String {
    if value == 0 {
        return String::new();
    }
    if value < 10 {
        return if full_group {
            format!("không mươi {}", ONES[value as usize])
        } else {
            ONES[value as usize].to_owned()
        };
    }
    if value < 20 {
        return TEENS[(value - 10) as usize].to_owned();
    }
    let tens = value / 10;
    let ones = value % 10;
    let prefix = format!("{} mươi", ONES[tens as usize]);
    match ones {
        0 => prefix,
        1 => format!("{prefix} mốt"),
        5 if tens > 1 => format!("{prefix} lăm"),
        _ => format!("{prefix} {}", ONES[ones as usize]),
    }
}

fn three_digits(value: u64) -> String {
    if value == 0 {
        return String::new();
    }
    let hundreds = value / 100;
    let remainder = value % 100;
    let mut parts = Vec::new();
    if hundreds > 0 {
        parts.push(format!("{} trăm", ONES[hundreds as usize]));
    }
    if remainder > 0 {
        if remainder < 10 && hundreds > 0 {
            parts.push(format!("không mươi {}", ONES[remainder as usize]));
        } else {
            parts.push(two_digits(remainder, false));
        }
    }
    parts.join(" ")
}

/// Port của app/utils/amount_to_words_vi.py::amount_to_words.
pub fn amount_to_words(value: u64) -> String {
    amount_to_words_with_suffix(value, "đồng")
}

fn amount_to_words_with_suffix(value: u64, suffix: &str) -> String {
    if value == 0 {
        return format!("Không {suffix}");
    }
    let billions = value / 1_000_000_000;
    let millions = (value % 1_000_000_000) / 1_000_000;
    let thousands = (value % 1_000_000) / 1_000;
    let units = value % 1_000;
    let mut parts = Vec::new();
    if billions > 0 {
        if billions >= 1_000 {
            parts.push(format!("{} tỷ", amount_to_words_with_suffix(billions, "")));
        } else {
            parts.push(format!("{} tỷ", three_digits(billions)));
        }
    }
    if millions > 0 {
        parts.push(format!("{} triệu", three_digits(millions)));
    }
    if thousands > 0 {
        parts.push(format!("{} nghìn", three_digits(thousands)));
    }
    if units > 0 {
        parts.push(three_digits(units));
    }
    let mut result = parts.join(", ");
    if !suffix.is_empty() {
        result.push(' ');
        result.push_str(suffix);
    }
    if let Some(first) = result.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    result
}

/// Port của app/utils/amount_to_words_vi.py::amount_to_words_chan.
pub fn amount_to_words_chan(value: u64) -> String {
    format!("{} chẵn", amount_to_words(value))
}

/// Port của app/utils/amount_to_words_vi.py::amount_to_words (Decimal input).
pub fn amount_to_words_dec(value: Decimal) -> Result<String, CoreError> {
    if value.is_sign_negative() {
        return Err(CoreError::Invalid("Không hỗ trợ số âm".into()));
    }
    let integer = value
        .trunc()
        .to_string()
        .parse::<u64>()
        .map_err(|_| CoreError::Invalid("Số tiền vượt giới hạn hỗ trợ".into()))?;
    Ok(amount_to_words(integer))
}
