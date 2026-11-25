//! Quy tắc tên hợp đồng và tên file.

use chrono::NaiveDateTime;

use crate::{fmt::ascii_fold, text::strip_diacritics};

/// Port của app/utils/file_naming.py::sanitize_for_filename.
pub fn sanitize_for_filename(value: &str, max_len: usize) -> String {
    let folded = ascii_fold(value);
    let mut safe = String::new();
    let mut previous_underscore = false;
    for ch in folded.chars() {
        let valid = ch.is_alphanumeric() || ch == '_' || ch == '-';
        let next = if valid { ch } else { '_' };
        if next == '_' {
            if !previous_underscore {
                safe.push(next);
            }
            previous_underscore = true;
        } else {
            safe.push(next);
            previous_underscore = false;
        }
    }
    safe.trim_matches('_')
        .chars()
        .take(max_len)
        .collect::<String>()
        .to_uppercase()
}

/// Port của app/services/contract_service.py::make_contract_slug.
pub fn make_contract_slug(value: &str) -> String {
    use regex::RegexBuilder;
    let mut cleaned = value.to_owned();
    for (pattern, replacement) in [
        (r"\btrạm\s+y\s+tế\b", "TYT"),
        (r"\btrung\s+tâm\s+y\s+tế\b", "TYT"),
        (r"\bphòng\s+y\s+tế\b", "TYT"),
        (r"\bxã\b", ""),
        (r"\bphường\b", ""),
        (r"\bthị\s*trấn\b", ""),
        (r"\bthị\s*xã\b", ""),
        (r"\bthành\s*phố\b", ""),
        (r"\bquận\b", ""),
        (r"\bhuyện\b", ""),
        (r"\btỉnh\b", ""),
    ] {
        if let Ok(regex) = RegexBuilder::new(pattern).case_insensitive(true).build() {
            cleaned = regex.replace_all(&cleaned, replacement).into_owned();
        }
    }
    let normalized = strip_diacritics(&cleaned).to_uppercase();
    normalized
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

/// Port của app/utils/file_naming.py::make_output_filename.
pub fn make_output_filename(
    contract_number: &str,
    customer_code: &str,
    contract_type: &str,
    ts: NaiveDateTime,
) -> String {
    let type_part = sanitize_for_filename(
        if contract_type.is_empty() {
            "HD"
        } else {
            contract_type
        },
        10,
    );
    let customer = sanitize_for_filename(
        if customer_code.is_empty() {
            "KH"
        } else {
            customer_code
        },
        15,
    );
    let number = sanitize_for_filename(
        if contract_number.is_empty() {
            "UNKNOWN"
        } else {
            contract_number
        },
        30,
    );
    format!(
        "{type_part}_{customer}_{number}_{}.docx",
        ts.format("%Y%m%d_%H%M%S")
    )
}

/// Thay các ký tự không thuộc Unicode `\w` hoặc dấu gạch ngang bằng `_`.
pub fn safe_number_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// Port của app/services/manual_quotation.py (KB-19).
pub fn windows_safe_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| if "\\/:*?\"<>|".contains(ch) { '-' } else { ch })
        .collect()
}
