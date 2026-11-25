use std::sync::LazyLock;

use regex::Regex;

use crate::{para, xml::Node};

static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{(\??)([A-Z][A-Z0-9_]*)\}\}")
        .expect("placeholder regex is a compile-time constant")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub key: String,
    pub optional: bool,
    pub start: usize,
    pub end: usize,
    pub start_node: usize,
    pub start_offset: usize,
    pub end_node: usize,
    pub end_offset: usize,
}

/// Quét placeholder trong một đoạn; offset start/end tính theo ký tự Unicode.
pub fn scan(paragraph: &Node) -> Vec<Slot> {
    let (text, positions) = para::joined_text(paragraph);
    PLACEHOLDER
        .captures_iter(&text)
        .filter_map(|captures| {
            let whole = captures.get(0)?;
            let start_char = text[..whole.start()].chars().count();
            let end_char = text[..whole.end()].chars().count();
            let (start_node, start_offset) = *positions.get(start_char)?;
            let (last_node, last_offset) = *positions.get(end_char.checked_sub(1)?)?;
            Some(Slot {
                key: captures.get(2)?.as_str().to_owned(),
                optional: captures
                    .get(1)
                    .is_some_and(|capture| !capture.as_str().is_empty()),
                start: start_char,
                end: end_char,
                start_node,
                start_offset,
                end_node: last_node,
                end_offset: last_offset + 1,
            })
        })
        .collect()
}
