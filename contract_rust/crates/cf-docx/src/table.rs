use std::collections::{BTreeMap, BTreeSet};

use crate::{DocxError, replace, slot, xml::Node};

fn own_text(node: &Node, output: &mut String) {
    if node.name() == Some("w:tbl") {
        return;
    }
    match node {
        Node::Text(text) => output.push_str(text),
        Node::Element { .. } => {
            for child in node.children() {
                own_text(child, output);
            }
        }
        Node::Raw(_) => {}
    }
}

fn collect_row_keys(row: &Node) -> BTreeSet<(String, bool)> {
    let mut text = String::new();
    own_text(row, &mut text);
    let text_node = Node::Element {
        name: "w:t".into(),
        attrs: Vec::new(),
        children: vec![Node::Text(text)],
    };
    let run = Node::Element {
        name: "w:r".into(),
        attrs: Vec::new(),
        children: vec![text_node],
    };
    let paragraph = Node::Element {
        name: "w:p".into(),
        attrs: Vec::new(),
        children: vec![run],
    };
    slot::scan(&paragraph)
        .into_iter()
        .map(|found| (found.key, found.optional))
        .collect()
}

/// Khóa placeholder thuộc riêng dòng, không tính bảng lồng trong ô.
pub fn keys(row: &Node) -> BTreeSet<(String, bool)> {
    collect_row_keys(row)
}

fn replace_row(row: &mut Node, values: &BTreeMap<String, String>) -> Result<(), DocxError> {
    if row.name() == Some("w:tbl") {
        return Ok(());
    }
    if row.name() == Some("w:p") {
        return replace::replace_values(row, values);
    }
    if let Node::Element { children, .. } = row {
        for child in children {
            replace_row(child, values)?;
        }
    }
    Ok(())
}

fn optional_empty(node: &Node, values: &BTreeMap<String, String>) -> bool {
    fn paragraphs<'a>(node: &'a Node, result: &mut Vec<&'a Node>) {
        if node.name() == Some("w:tbl") {
            return;
        }
        if node.name() == Some("w:p") {
            result.push(node);
            return;
        }
        for child in node.children() {
            paragraphs(child, result);
        }
    }
    let mut found = Vec::new();
    paragraphs(node, &mut found);
    let optional: Vec<_> = found
        .into_iter()
        .flat_map(crate::slot::scan)
        .filter(|slot| slot.optional)
        .collect();
    !optional.is_empty()
        && optional.iter().all(|slot| {
            values
                .get(&slot.key)
                .is_none_or(|value| value.trim().is_empty())
        })
}

fn render_table(table: &mut Node, data: &crate::template::RenderData) -> Result<bool, DocxError> {
    let Node::Element { children, .. } = table else {
        return Ok(true);
    };
    let Some(lists) = &data.lists else {
        let mut output = Vec::with_capacity(children.len());
        for mut child in std::mem::take(children) {
            if child.name() == Some("w:tr") && optional_empty(&child, &data.values) {
                continue;
            }
            render_node(&mut child, data)?;
            output.push(child);
        }
        if output.iter().all(|child| child.name() != Some("w:tr")) {
            return Ok(false);
        }
        *children = output;
        return Ok(true);
    };
    for group in ["items", "features"] {
        if !data.omit_empty_tables.contains(group)
            || lists.get(group).is_some_and(|rows| !rows.is_empty())
        {
            continue;
        }
        let table_keys: Vec<_> = children
            .iter()
            .filter(|child| child.name() == Some("w:tr"))
            .flat_map(collect_row_keys)
            .collect();
        let has_group_key = table_keys.iter().any(|(key, _)| match group {
            "items" => key.starts_with("ITEM_"),
            _ => key.starts_with("FEATURE_"),
        });
        let only_group_keys = table_keys.iter().all(|(key, _)| match group {
            "items" => key.starts_with("ITEM_"),
            _ => key.starts_with("FEATURE_"),
        });
        if has_group_key && only_group_keys {
            return Ok(false);
        }
    }
    let mut output = Vec::with_capacity(children.len());
    let mut source_group: Option<(usize, String)> = None;
    for (index, child) in children.iter().enumerate() {
        if child.name() != Some("w:tr") {
            output.push(child.clone());
            continue;
        }
        let keys = collect_row_keys(child);
        let group = if keys.iter().any(|(key, _)| key.starts_with("ITEM_")) {
            Some("items")
        } else if keys.iter().any(|(key, _)| key.starts_with("FEATURE_")) {
            Some("features")
        } else {
            None
        };
        if let Some(group) = group {
            let rows = lists.get(group).cloned().unwrap_or_default();
            if rows.is_empty() {
                source_group = Some((index, group.to_owned()));
                continue;
            }
            for row_values in rows {
                if optional_empty(child, &row_values) {
                    continue;
                }
                let mut row = child.clone();
                replace_row(&mut row, &row_values)?;
                output.push(row);
            }
            source_group = Some((index, group.to_owned()));
        } else if !optional_empty(child, &data.values) {
            output.push(child.clone());
        }
    }
    let all_rows_gone = output.iter().all(|child| child.name() != Some("w:tr"));
    if let Some((_, group)) = source_group
        && data.omit_empty_tables.contains(&group)
        && lists.get(&group).is_none_or(Vec::is_empty)
    {
        return Ok(false);
    }
    if all_rows_gone {
        return Ok(false);
    }
    *children = output;
    for child in children {
        render_node(child, data)?;
    }
    Ok(true)
}

pub fn row_keys(row: &Node) -> BTreeSet<(String, bool)> {
    collect_row_keys(row)
}

pub fn render_node(node: &mut Node, data: &crate::template::RenderData) -> Result<(), DocxError> {
    let Node::Element { children, .. } = node else {
        return Ok(());
    };
    let mut kept = Vec::with_capacity(children.len());
    for mut child in std::mem::take(children) {
        if child.name() == Some("w:tbl") {
            if render_table(&mut child, data)? {
                kept.push(child);
            }
        } else {
            if child.name() == Some("w:p") && optional_empty(&child, &data.values) {
                continue;
            }
            render_node(&mut child, data)?;
            kept.push(child);
        }
    }
    *children = kept;
    Ok(())
}
