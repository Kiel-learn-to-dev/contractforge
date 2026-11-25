//! Quy tắc đánh số hợp đồng.

use crate::error::CoreError;
use regex::Regex;
use std::collections::BTreeSet;

pub const DEFAULT_FORMAT: &str = "{seq}/{slug}/{year}";
pub const DEFAULT_ADDENDUM_FORMAT: &str = "PL{seq}/{parent}";
const TOKENS: &[&str] = &["seq", "slug", "year"];

fn placeholders(format: &str) -> Vec<String> {
    Regex::new(r"\{(\w+)\}")
        .ok()
        .map(|regex| {
            regex
                .captures_iter(format)
                .filter_map(|capture| capture.get(1).map(|m| m.as_str().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn format_error(format: &str, addendum: bool) -> Option<String> {
    if format.trim().is_empty() {
        return Some(
            if addendum {
                "Khuôn số phụ lục không được để trống."
            } else {
                "Khuôn số hợp đồng không được để trống."
            }
            .into(),
        );
    }
    let allowed: &[&str] = if addendum { &["seq", "parent"] } else { TOKENS };
    let found = placeholders(format);
    let mut unknown: Vec<String> = found
        .iter()
        .filter(|token| !allowed.contains(&token.as_str()))
        .cloned()
        .collect();
    unknown.sort();
    unknown.dedup();
    if !unknown.is_empty() {
        let list = unknown
            .iter()
            .map(|value| format!("'{value}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let tokens = allowed
            .iter()
            .map(|value| format!("'{value}'"))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(if addendum {
            format!("Chỗ điền không hợp lệ: [{list}]. Khuôn phụ lục chỉ dùng được: [{tokens}]")
        } else {
            format!("Chỗ điền không hợp lệ: [{list}]. Chỉ dùng được: [{tokens}]")
        });
    }
    if addendum
        && (!found.iter().any(|token| token == "seq")
            || !found.iter().any(|token| token == "parent"))
    {
        return Some(
            "Khuôn phụ lục phải chứa {seq} và {parent}, nếu không các phụ lục sẽ trùng số.".into(),
        );
    }
    if !addendum && !found.iter().any(|token| token == "seq") {
        return Some("Khuôn phải chứa {seq}, nếu không mọi hợp đồng sẽ trùng số.".into());
    }
    None
}

/// Port của app/services/numbering.py::validate_format.
pub fn validate_format(format: &str) -> Result<&str, CoreError> {
    format_error(format, false).map_or(Ok(format), |message| Err(CoreError::Invalid(message)))
}

/// Port của app/services/numbering.py::build_number.
pub fn build_number(
    seq: &str,
    slug: &str,
    format: Option<&str>,
    year: i32,
) -> Result<String, CoreError> {
    let format = format
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_FORMAT);
    validate_format(format)?;
    Ok(format
        .replace("{seq}", seq)
        .replace("{slug}", slug)
        .replace("{year}", &year.to_string()))
}

pub fn next_seq(used: &BTreeSet<u32>) -> String {
    let mut seq = 1_u32;
    while used.contains(&seq) {
        seq = seq.saturating_add(1);
    }
    format!("{seq:02}")
}

/// Tạo regex khớp toàn bộ số hợp đồng theo KB-02.
pub fn format_regex(format: &str, year: i32) -> Result<Regex, CoreError> {
    validate_format(format)?;
    let mut pattern = String::from("^");
    let mut rest = format;
    let mut captured_seq = false;
    while let Some(start) = rest.find('{') {
        pattern.push_str(&regex::escape(&rest[..start]));
        let Some(end_rel) = rest[start..].find('}') else {
            pattern.push_str(&regex::escape(&rest[start..]));
            rest = "";
            break;
        };
        let end = start + end_rel;
        let token = &rest[start + 1..end];
        match token {
            "seq" if !captured_seq => {
                pattern.push_str("(?P<seq>\\d+)");
                captured_seq = true;
            }
            "seq" => pattern.push_str("\\d+"),
            "slug" => pattern.push_str("[A-Za-z0-9]*"),
            "year" => pattern.push_str(&regex::escape(&year.to_string())),
            _ => pattern.push_str(&regex::escape(&rest[start..=end])),
        }
        rest = &rest[end + 1..];
    }
    pattern.push_str(&regex::escape(rest));
    pattern.push('$');
    Regex::new(&pattern)
        .map_err(|error| CoreError::Invalid(format!("Regex khuôn số không hợp lệ: {error}")))
}

/// Port của app/services/contract_service.py::get_next_contract_seq (KB-02).
pub fn next_number(
    format: &str,
    slug: &str,
    year: i32,
    existing: &[&str],
) -> Result<String, CoreError> {
    let pattern = format_regex(format, year)?;
    let mut used = BTreeSet::new();
    for value in existing {
        if let Some(captures) = pattern.captures(value) {
            let _ = slug; // Existing numbers are already scoped to the requested customer.
            if let Some(number) = captures
                .name("seq")
                .and_then(|capture| capture.as_str().parse().ok())
            {
                used.insert(number);
            }
        }
    }
    build_number(&next_seq(&used), slug, Some(format), year)
}

/// Port của app/services/numbering.py::validate_addendum_format.
pub fn validate_addendum_format(format: &str) -> Result<&str, CoreError> {
    format_error(format, true).map_or(Ok(format), |message| Err(CoreError::Invalid(message)))
}

/// Port của app/services/numbering.py::build_addendum_number.
pub fn build_addendum_number(
    parent: &str,
    seq: &str,
    format: Option<&str>,
) -> Result<String, CoreError> {
    let format = format
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_ADDENDUM_FORMAT);
    validate_addendum_format(format)?;
    Ok(format.replace("{seq}", seq).replace("{parent}", parent))
}

/// Port của app/services/numbering.py::parse_addendum_seq.
pub fn parse_addendum_seq(number: &str, format: Option<&str>) -> Option<u32> {
    let format = format
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_ADDENDUM_FORMAT);
    let mut pattern = String::new();
    let mut rest = format;
    while let Some(start) = rest.find('{') {
        pattern.push_str(&regex::escape(&rest[..start]));
        let end = start + rest[start..].find('}')?;
        match &rest[start + 1..end] {
            "seq" => pattern.push_str("(\\d+)"),
            "parent" => pattern.push_str(".*"),
            _ => pattern.push_str(&regex::escape(&rest[start..=end])),
        }
        rest = &rest[end + 1..];
    }
    pattern.push_str(&regex::escape(rest));
    let regex = Regex::new(&format!("^{pattern}$")).ok()?;
    regex.captures(number)?.get(1)?.as_str().parse().ok()
}

pub fn next_addendum_seq(used: &BTreeSet<u32>) -> String {
    next_seq(used)
}
