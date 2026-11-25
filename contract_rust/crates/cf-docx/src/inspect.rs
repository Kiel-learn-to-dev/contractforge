use std::collections::BTreeMap;

use crate::{
    DocxError, Package,
    package::is_text_part,
    xml::{self, Node},
};

fn walk_paragraph(node: &Node, output: &mut String) {
    match node {
        Node::Text(text) => output.push_str(text),
        Node::Element { name, .. } if name == "w:br" || name == "w:cr" => output.push('\n'),
        Node::Element { name, .. } if name == "w:tab" => output.push('\t'),
        Node::Element { name, .. } if name == "w:p" => {
            for child in node.children() {
                walk_paragraph(child, output);
            }
        }
        Node::Element { .. } => {
            for child in node.children() {
                if child.name() != Some("w:p") {
                    walk_paragraph(child, output);
                }
            }
        }
        Node::Raw(_) => {}
    }
}

fn collect_paragraphs(node: &Node, output: &mut Vec<String>) {
    if node.name() == Some("w:p") {
        let mut text = String::new();
        walk_paragraph(node, &mut text);
        output.push(text);
        return;
    }
    for child in node.children() {
        collect_paragraphs(child, output);
    }
}

/// Trích một chuỗi cho mỗi đoạn `w:p`, bỏ văn bản trong đoạn lồng bên trong.
pub fn paragraph_texts(part: &Node) -> Vec<String> {
    let mut paragraphs = Vec::new();
    collect_paragraphs(part, &mut paragraphs);
    paragraphs
}

/// Port của app/services/docx_renderer.py::render_docx (đọc các phần chứa chữ).
pub fn docx_texts(bytes: &[u8]) -> Result<BTreeMap<String, Vec<String>>, DocxError> {
    let package = Package::read(bytes)?;
    let mut result = BTreeMap::new();
    for (index, name) in package.entries().iter().enumerate() {
        if !is_text_part(name) {
            continue;
        }
        let part = xml::parse(&package.read_entry(index)?)?;
        result.insert(name.clone(), paragraph_texts(&part));
    }
    Ok(result)
}
