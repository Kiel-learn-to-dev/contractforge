use std::collections::BTreeMap;

use crate::{DocxError, para, slot, slot::Slot, xml::Node};

fn text_at(node: &Node) -> String {
    match node {
        Node::Text(text) => text.clone(),
        Node::Element { children, .. } => children.iter().map(text_at).collect(),
        Node::Raw(_) => String::new(),
    }
}

fn set_text(node: &mut Node, text: String) -> Result<(), DocxError> {
    let Node::Element { children, .. } = node else {
        return Err(DocxError::Xml("w:t không phải phần tử.".into()));
    };
    *children = vec![Node::Text(text)];
    Ok(())
}

fn replace_in_run(
    paragraph: &mut Node,
    path: &[usize],
    prefix: String,
    value: &str,
    suffix: String,
) -> Result<(), DocxError> {
    let run_path = path
        .iter()
        .enumerate()
        .rev()
        .find_map(|(position, _)| {
            let candidate = &path[..=position];
            (para::node_at(paragraph, candidate)?.name() == Some("w:r")).then(|| candidate.to_vec())
        })
        .ok_or_else(|| DocxError::Xml("Không tìm thấy run chứa w:t.".into()))?;
    let child_position = run_path.len();
    let child_index = *path
        .get(child_position)
        .ok_or_else(|| DocxError::Xml("Đường dẫn w:t không hợp lệ.".into()))?;
    let run = para::node_at_mut(paragraph, &run_path)
        .ok_or_else(|| DocxError::Xml("Run không tồn tại.".into()))?;
    let Node::Element { children, .. } = run else {
        return Err(DocxError::Xml("Run không hợp lệ.".into()));
    };
    let old = children
        .get(child_index)
        .ok_or_else(|| DocxError::Xml("w:t không tồn tại.".into()))?;
    let Node::Element { attrs, .. } = old else {
        return Err(DocxError::Xml("w:t không hợp lệ.".into()));
    };
    let original_attrs = attrs.clone();
    let mut parts = Vec::new();
    let mut current = String::new();
    current.push_str(&prefix);
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\r' if chars.peek() == Some(&'\n') => {
                chars.next();
                parts.push(text_node(std::mem::take(&mut current), &original_attrs));
                parts.push(Node::Element {
                    name: "w:br".into(),
                    attrs: Vec::new(),
                    children: Vec::new(),
                });
            }
            '\n' => {
                parts.push(text_node(std::mem::take(&mut current), &original_attrs));
                parts.push(Node::Element {
                    name: "w:br".into(),
                    attrs: Vec::new(),
                    children: Vec::new(),
                });
            }
            '\t' => {
                parts.push(text_node(std::mem::take(&mut current), &original_attrs));
                parts.push(Node::Element {
                    name: "w:tab".into(),
                    attrs: Vec::new(),
                    children: Vec::new(),
                });
            }
            _ => current.push(character),
        }
    }
    current.push_str(&suffix);
    parts.push(text_node(current, &original_attrs));
    children.splice(child_index..=child_index, parts);
    Ok(())
}

fn text_node(text: String, attrs: &[(String, String)]) -> Node {
    let mut attrs = attrs.to_vec();
    if text.chars().next().is_some_and(char::is_whitespace)
        || text.chars().next_back().is_some_and(char::is_whitespace)
    {
        if let Some((_, value)) = attrs.iter_mut().find(|(key, _)| key == "xml:space") {
            *value = "preserve".into();
        } else {
            attrs.push(("xml:space".into(), "preserve".into()));
        }
    }
    Node::Element {
        name: "w:t".into(),
        attrs,
        children: vec![Node::Text(text)],
    }
}

/// Thay một placeholder trong đoạn. Giá trị có xuống dòng/tab được xử lý ở bước T2.08.
pub fn replace_slot(paragraph: &mut Node, slot: &Slot, value: &str) -> Result<(), DocxError> {
    let nodes = para::text_nodes(paragraph);
    let first = nodes
        .get(slot.start_node)
        .ok_or_else(|| DocxError::Xml("Node đầu placeholder không tồn tại.".into()))?;
    let last = nodes
        .get(slot.end_node)
        .ok_or_else(|| DocxError::Xml("Node cuối placeholder không tồn tại.".into()))?;
    let start_byte = first
        .text
        .char_indices()
        .nth(slot.start_offset)
        .map_or(first.text.len(), |(offset, _)| offset);
    let end_byte = last
        .text
        .char_indices()
        .nth(slot.end_offset)
        .map_or(last.text.len(), |(offset, _)| offset);
    let prefix = first.text[..start_byte].to_owned();
    let suffix = last.text[end_byte..].to_owned();
    if slot.start_node == slot.end_node {
        return replace_in_run(paragraph, &first.path, prefix, value, suffix);
    }
    for node in nodes
        .iter()
        .skip(slot.start_node + 1)
        .take(slot.end_node - slot.start_node - 1)
    {
        if let Some(target) = para::node_at_mut(paragraph, &node.path) {
            set_text(target, String::new())?;
        }
    }
    if slot.start_node != slot.end_node {
        let last = para::node_at_mut(paragraph, &last.path)
            .ok_or_else(|| DocxError::Xml("w:t cuối không tồn tại.".into()))?;
        set_text(last, suffix)?;
    }
    replace_in_run(paragraph, &first.path, prefix, value, String::new())
}

/// Thay mọi slot đã biết trong một đoạn từ cuối về đầu để giữ vị trí ổn định.
pub fn replace_values(
    paragraph: &mut Node,
    values: &BTreeMap<String, String>,
) -> Result<(), DocxError> {
    for slot in slot::scan(paragraph).iter().rev() {
        if let Some(value) = values.get(&slot.key) {
            replace_slot(paragraph, slot, value)?;
        }
    }
    Ok(())
}

pub fn plain_text(node: &Node) -> String {
    text_at(node)
}
