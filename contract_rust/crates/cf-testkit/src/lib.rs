//! Tiện ích dùng chung cho kiểm thử fixture.

#[cfg(feature = "docx")]
pub mod docx;
#[cfg(feature = "db")]
pub mod seed;
#[cfg(feature = "docx")]
pub use docx::make_docx;
#[cfg(feature = "db")]
pub use seed::seeded_db;

use std::path::PathBuf;

use rust_decimal::Decimal;
use serde_json::Value;

/// Đường dẫn fixture tương đối với thư mục `contract_rust/fixtures`.
pub fn fixture_path(rel: impl AsRef<std::path::Path>) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join(rel)
}

/// Đọc fixture JSON. Fixture lỗi hoặc không tồn tại trả về `Value::Null`.
pub fn fixture(name: &str) -> Value {
    std::fs::read_to_string(fixture_path(format!("{name}.json")))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(Value::Null)
}

/// Đọc Decimal từ chuỗi hoặc số JSON. Giá trị không hợp lệ trả về 0 để test
/// có thể tiếp tục và báo sai khác tại assertion.
pub fn dec(value: &Value) -> Decimal {
    let raw = match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => return Decimal::ZERO,
    };
    raw.parse().unwrap_or(Decimal::ZERO)
}

#[derive(Debug, Clone, PartialEq)]
pub enum PyResult {
    Ok(Value),
    Err { class: String, msg: String },
}

/// Đọc kết quả Python dạng `{"ok": ...}` hoặc `{"err": ..., "msg": ...}`.
pub fn py(value: &Value) -> PyResult {
    if let Some(err) = value.get("err") {
        return PyResult::Err {
            class: err.as_str().unwrap_or_default().to_owned(),
            msg: value
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        };
    }
    PyResult::Ok(value.get("ok").cloned().unwrap_or(Value::Null))
}

/// Lấy khác biệt có chủ ý theo mã, ví dụ `KB-01`.
pub fn kb(code: &str) -> Value {
    let differences = fixture("intentional_differences");
    differences
        .as_object()
        .and_then(|items| items.iter().find(|(key, _)| key.starts_with(code)))
        .map(|(_, value)| value.clone())
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_to_words_fixture_has_at_least_forty_cases() {
        let cases = fixture("amount_to_words")["cases"]
            .as_array()
            .map_or(0, Vec::len);
        assert!(cases >= 40, "expected >= 40 fixture cases, got {cases}");
    }

    #[test]
    fn parses_python_ok_and_error_shapes() {
        assert_eq!(
            py(&serde_json::json!({"ok": "value"})),
            PyResult::Ok(serde_json::json!("value"))
        );
        assert_eq!(
            py(&serde_json::json!({"err": "ValueError", "msg": "bad"})),
            PyResult::Err {
                class: "ValueError".into(),
                msg: "bad".into()
            }
        );
    }
}
