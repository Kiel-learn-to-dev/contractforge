//! Xử lý văn bản tiếng Việt.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// Port của app/utils/file_naming.py::sanitize_for_filename (bỏ dấu tiếng Việt).
pub fn strip_diacritics(value: &str) -> String {
    value
        .replace('Đ', "D")
        .replace('đ', "d")
        .nfd()
        .filter(|ch| !is_combining_mark(*ch))
        .collect()
}

/// Khóa tìm kiếm không phân biệt dấu và chữ hoa thường.
pub fn search_key(value: &str) -> String {
    strip_diacritics(value).to_lowercase()
}
