use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Write};

use zip::{
    CompressionMethod, ZipArchive,
    write::{SimpleFileOptions, ZipWriter},
};

use crate::{
    DocxError, Package, inspect,
    package::is_text_part,
    replace,
    xml::{self, Node},
};

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct RenderData {
    pub values: BTreeMap<String, String>,
    pub lists: Option<BTreeMap<String, Vec<BTreeMap<String, String>>>>,
    pub omit_empty_tables: BTreeSet<String>,
    pub legacy_party_a_rows: bool,
}

#[derive(Debug, Clone)]
pub struct Template {
    source: Vec<u8>,
    package: Package,
    parts: BTreeMap<usize, Node>,
}

impl Template {
    pub fn compile(bytes: &[u8]) -> Result<Self, DocxError> {
        let package = Package::read(bytes)?;
        let mut parts = BTreeMap::new();
        for (index, name) in package.entries().iter().enumerate() {
            if is_text_part(name) {
                parts.insert(index, xml::parse(&package.read_entry(index)?)?);
            }
        }
        Ok(Self {
            source: bytes.to_vec(),
            package,
            parts,
        })
    }

    pub fn render(&self, data: &RenderData) -> Result<Vec<u8>, DocxError> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let mut source = ZipArchive::new(Cursor::new(self.source.as_slice()))?;
        for (index, name) in self.package.entries().iter().enumerate() {
            if let Some(part) = self.parts.get(&index) {
                let mut rendered = part.clone();
                if name == "word/document.xml" && data.legacy_party_a_rows {
                    crate::legacy_party_a::insert_rows(&mut rendered, &data.values)?;
                }
                render_node(&mut rendered, data)?;
                writer
                    .start_file(
                        name,
                        SimpleFileOptions::default()
                            .compression_method(CompressionMethod::Deflated),
                    )
                    .map_err(|error| DocxError::Open(error.to_string()))?;
                writer.write_all(&xml::serialize(&rendered))?;
            } else {
                let entry = source.by_index_raw(index)?;
                writer.raw_copy_file(entry)?;
            }
        }
        let cursor = writer
            .finish()
            .map_err(|error| DocxError::Open(error.to_string()))?;
        Ok(cursor.into_inner())
    }

    pub fn analysis(&self) -> Analysis {
        let mut placeholders = BTreeSet::new();
        let mut optional_keys = BTreeSet::new();
        let mut repeat_groups = BTreeSet::new();
        let mut row_warnings = BTreeSet::new();
        let mut split_run_warnings = BTreeSet::new();
        let mut source_counts = BTreeMap::from([
            ("para".to_owned(), 0),
            ("table".to_owned(), 0),
            ("header".to_owned(), 0),
            ("footer".to_owned(), 0),
        ]);
        let mut uses_legacy_anchor = false;
        for (index, part) in &self.parts {
            let part_name = self
                .package
                .entries()
                .get(*index)
                .map(String::as_str)
                .unwrap_or_default();
            for paragraph in collect_paragraph_nodes(part) {
                let in_table = ancestor_has_table(part, paragraph);
                let bucket = if part_name.starts_with("word/header") {
                    "header"
                } else if part_name.starts_with("word/footer") {
                    "footer"
                } else if in_table {
                    "table"
                } else {
                    "para"
                };
                *source_counts.entry(bucket.to_owned()).or_default() += 1;
                let (joined, _) = crate::para::joined_text(paragraph);
                let slots = crate::slot::scan(paragraph);
                for slot in &slots {
                    placeholders.insert(slot.key.clone());
                    if slot.optional {
                        optional_keys.insert(slot.key.clone());
                    }
                }
                if slots.iter().any(|slot| slot.start_node != slot.end_node) {
                    let warning = joined.trim().chars().take(120).collect::<String>();
                    if !warning.is_empty() {
                        split_run_warnings.insert(warning);
                    }
                }
                if !in_table
                    && slots.iter().any(|slot| {
                        slot.key.starts_with("ITEM_") || slot.key.starts_with("FEATURE_")
                    })
                {
                    row_warnings.insert("`{{ITEM_X}}` phải nằm trong một dòng bảng".to_owned());
                }
            }
            for row in collect_named_nodes(part, "w:tr") {
                for (key, _) in crate::table::row_keys(row) {
                    if key.starts_with("ITEM_") {
                        repeat_groups.insert("items".to_owned());
                    }
                    if key.starts_with("FEATURE_") {
                        repeat_groups.insert("features".to_owned());
                    }
                }
            }
            if part_name == "word/document.xml" {
                uses_legacy_anchor |= has_legacy_anchor(part);
            }
        }
        let uses_legacy_party_a = uses_legacy_anchor && optional_keys.is_empty();
        Analysis {
            placeholders: placeholders.into_iter().collect(),
            optional_keys: optional_keys.into_iter().collect(),
            repeat_groups: repeat_groups.into_iter().collect(),
            row_warnings: row_warnings.into_iter().collect(),
            split_run_warnings: split_run_warnings.into_iter().collect(),
            source_counts,
            uses_legacy_party_a,
            error: None,
        }
    }

    pub fn source_package(&self) -> &Package {
        &self.package
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Analysis {
    pub placeholders: Vec<String>,
    pub optional_keys: Vec<String>,
    pub repeat_groups: Vec<String>,
    pub row_warnings: Vec<String>,
    pub split_run_warnings: Vec<String>,
    pub source_counts: BTreeMap<String, usize>,
    pub uses_legacy_party_a: bool,
    pub error: Option<String>,
}

fn collect_paragraph_nodes(node: &Node) -> Vec<&Node> {
    fn walk<'a>(node: &'a Node, result: &mut Vec<&'a Node>) {
        if node.name() == Some("w:p") {
            result.push(node);
        }
        for child in node.children() {
            walk(child, result);
        }
    }
    let mut result = Vec::new();
    walk(node, &mut result);
    result
}

fn collect_named_nodes<'a>(node: &'a Node, name: &str) -> Vec<&'a Node> {
    fn walk<'a>(node: &'a Node, name: &str, output: &mut Vec<&'a Node>) {
        if node.name() == Some(name) {
            output.push(node);
        }
        for child in node.children() {
            walk(child, name, output);
        }
    }
    let mut output = Vec::new();
    walk(node, name, &mut output);
    output
}

fn ancestor_has_table(root: &Node, target: &Node) -> bool {
    fn visit(node: &Node, target: *const Node, inside_table: bool) -> Option<bool> {
        if std::ptr::eq(node, target) {
            return Some(inside_table);
        }
        for child in node.children() {
            if let Some(found) = visit(child, target, inside_table || node.name() == Some("w:tbl"))
            {
                return Some(found);
            }
        }
        None
    }
    visit(root, target, false).unwrap_or(false)
}

fn has_legacy_anchor(root: &Node) -> bool {
    collect_named_nodes(root, "w:tbl").iter().any(|table| {
        let Some(row) = table
            .children()
            .iter()
            .find(|child| child.name() == Some("w:tr"))
        else {
            return false;
        };
        let Some(cell) = row
            .children()
            .iter()
            .find(|child| child.name() == Some("w:tc"))
        else {
            return false;
        };
        let text = collect_text(cell);
        text.contains("PARTY_A_NAME")
    })
}

fn collect_text(node: &Node) -> String {
    match node {
        Node::Text(text) => text.clone(),
        Node::Element { children, .. } => children.iter().map(collect_text).collect(),
        Node::Raw(_) => String::new(),
    }
}

fn render_node(node: &mut Node, data: &RenderData) -> Result<(), DocxError> {
    crate::table::render_node(node, data)?;
    let paths = collect_paths(node, |candidate| candidate.name() == Some("w:p"));
    for path in paths.into_iter().rev() {
        if let Some(paragraph) = crate::para::node_at_mut(node, &path) {
            replace::replace_values(paragraph, &data.values)?;
        }
    }
    Ok(())
}

fn collect_paths(node: &Node, predicate: impl Fn(&Node) -> bool) -> Vec<Vec<usize>> {
    fn walk(
        node: &Node,
        path: &mut Vec<usize>,
        predicate: &impl Fn(&Node) -> bool,
        result: &mut Vec<Vec<usize>>,
    ) {
        if predicate(node) {
            result.push(path.clone());
        }
        for (index, child) in node.children().iter().enumerate() {
            path.push(index);
            walk(child, path, predicate, result);
            path.pop();
        }
    }
    let mut result = Vec::new();
    walk(node, &mut Vec::new(), &predicate, &mut result);
    result
}

pub fn paragraph_texts(bytes: &[u8]) -> Result<BTreeMap<String, Vec<String>>, DocxError> {
    inspect::docx_texts(bytes)
}
