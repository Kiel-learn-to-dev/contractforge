use crate::{DocxError, xml::Node};

fn direct_children(node: &Node, name: &str) -> Vec<usize> {
    node.children()
        .iter()
        .enumerate()
        .filter_map(|(index, child)| (child.name() == Some(name)).then_some(index))
        .collect()
}

fn collect_text(node: &Node) -> String {
    match node {
        Node::Text(text) => text.clone(),
        Node::Element { name, .. } if name == "w:tbl" => String::new(),
        Node::Element { children, .. } => children.iter().map(collect_text).collect(),
        Node::Raw(_) => String::new(),
    }
}

fn has_anchor(table: &Node) -> bool {
    let Some(row_index) = direct_children(table, "w:tr").first().copied() else {
        return false;
    };
    let Some(row) = table.children().get(row_index) else {
        return false;
    };
    let Some(cell_index) = direct_children(row, "w:tc").first().copied() else {
        return false;
    };
    row.children()
        .get(cell_index)
        .is_some_and(|cell| collect_text(cell).contains("PARTY_A_NAME"))
}

fn top_level_targets(node: &Node) -> Vec<Vec<usize>> {
    fn collect(node: &Node, in_table: bool, path: &mut Vec<usize>, targets: &mut Vec<Vec<usize>>) {
        for (index, child) in node.children().iter().enumerate() {
            path.push(index);
            if child.name() == Some("w:tbl") && !in_table {
                if has_anchor(child) {
                    targets.push(path.clone());
                }
            } else {
                collect(
                    child,
                    in_table || child.name() == Some("w:tbl"),
                    path,
                    targets,
                );
            }
            path.pop();
        }
    }

    let mut targets = Vec::new();
    collect(node, false, &mut Vec::new(), &mut targets);
    targets
}

fn node_at_mut<'a>(node: &'a mut Node, path: &[usize]) -> Option<&'a mut Node> {
    let Some((index, rest)) = path.split_first() else {
        return Some(node);
    };
    let Node::Element { children, .. } = node else {
        return None;
    };
    node_at_mut(children.get_mut(*index)?, rest)
}

fn child_mut(node: &mut Node, index: usize) -> Result<&mut Node, DocxError> {
    let Node::Element { children, .. } = node else {
        return Err(DocxError::Xml("Dòng bảng không hợp lệ.".into()));
    };
    children
        .get_mut(index)
        .ok_or_else(|| DocxError::Xml("Ô bảng không tồn tại.".into()))
}

fn first_run_properties(node: &Node) -> Option<Node> {
    if node.name() == Some("w:r") {
        return node
            .children()
            .iter()
            .find(|child| child.name() == Some("w:rPr"))
            .cloned();
    }
    node.children().iter().find_map(first_run_properties)
}

fn set_cell_text(
    cell: &mut Node,
    text: &str,
    run_properties: Option<Node>,
) -> Result<(), DocxError> {
    let Node::Element { children, .. } = cell else {
        return Err(DocxError::Xml("Ô bảng không hợp lệ.".into()));
    };
    let paragraph_index = children
        .iter()
        .position(|child| child.name() == Some("w:p"));
    if paragraph_index.is_none() {
        children.push(Node::element("w:p"));
    }
    let paragraph_index = paragraph_index.unwrap_or(children.len() - 1);
    let paragraph = children
        .get_mut(paragraph_index)
        .ok_or_else(|| DocxError::Xml("Đoạn văn trong ô không tồn tại.".into()))?;
    let Node::Element {
        children: paragraph_children,
        ..
    } = paragraph
    else {
        return Err(DocxError::Xml("Đoạn văn trong ô không hợp lệ.".into()));
    };
    paragraph_children.retain(|child| child.name() != Some("w:r"));
    let mut run_children = Vec::new();
    if let Some(properties) = run_properties {
        run_children.push(properties);
    }
    let attrs = if text.chars().next().is_some_and(char::is_whitespace)
        || text.chars().next_back().is_some_and(char::is_whitespace)
    {
        vec![("xml:space".into(), "preserve".into())]
    } else {
        Vec::new()
    };
    run_children.push(Node::Element {
        name: "w:t".into(),
        attrs,
        children: vec![Node::Text(text.to_owned())],
    });
    paragraph_children.push(Node::Element {
        name: "w:r".into(),
        attrs: Vec::new(),
        children: run_children,
    });
    Ok(())
}

/// Thêm các dòng thông tin Bên A vào bảng legacy cấp một của thân.
pub fn insert_rows(
    document: &mut Node,
    values: &std::collections::BTreeMap<String, String>,
) -> Result<(), DocxError> {
    let fields = [
        ("Mã số thuế", "PARTY_A_TAX_CODE"),
        ("Số tài khoản", "PARTY_A_BANK_ACCOUNT"),
        ("Ngân hàng", "PARTY_A_BANK_NAME"),
        ("Tên thụ hưởng", "PARTY_A_BENEFICIARY"),
        ("Mã QHNS", "PARTY_A_BUDGET_CODE"),
    ];
    let fields: Vec<_> = fields
        .into_iter()
        .filter_map(|(label, key)| {
            values
                .get(key)
                .filter(|value| !value.trim().is_empty())
                .map(|value| (label, value.as_str()))
        })
        .collect();
    if fields.is_empty() {
        return Ok(());
    }
    let mut targets = top_level_targets(document);
    targets.reverse();
    for path in targets {
        if let Some(table) = node_at_mut(document, &path) {
            insert_rows_in_table(table, &fields)?;
        }
    }
    Ok(())
}

fn insert_rows_in_table(table: &mut Node, fields: &[(&str, &str)]) -> Result<(), DocxError> {
    let row_indices = direct_children(table, "w:tr");
    if row_indices.is_empty() {
        return Ok(());
    }
    let Node::Element { children, .. } = table else {
        return Ok(());
    };
    let anchor_index = row_indices.iter().copied().find(|index| {
        let row = &children[*index];
        let Some(cell_index) = direct_children(row, "w:tc").first().copied() else {
            return false;
        };
        let label = collect_text(&row.children()[cell_index]).trim().to_owned();
        label == "Chức vụ" || label == "Chức danh"
    });
    let insertion_index = anchor_index.unwrap_or(*row_indices.last().unwrap_or(&0));
    let template_index = anchor_index
        .and_then(|anchor| {
            row_indices
                .iter()
                .copied()
                .take_while(|index| *index < anchor)
                .last()
        })
        .or_else(|| row_indices.last().copied())
        .unwrap_or(insertion_index);
    let template = children
        .get(template_index)
        .cloned()
        .ok_or_else(|| DocxError::Xml("Không tìm thấy dòng khuôn.".into()))?;
    let run_properties = first_run_properties(&template);
    let cell_count = direct_children(&template, "w:tc").len();
    let mut rows = Vec::new();
    for (label, value) in fields {
        let mut row = template.clone();
        let cells = direct_children(&row, "w:tc");
        if cells.len() >= 3 {
            set_cell_text(
                child_mut(&mut row, cells[0])?,
                label,
                run_properties.clone(),
            )?;
            set_cell_text(child_mut(&mut row, cells[1])?, ":", run_properties.clone())?;
            set_cell_text(
                child_mut(&mut row, cells[2])?,
                value,
                run_properties.clone(),
            )?;
        } else if cell_count == 2 {
            set_cell_text(
                child_mut(&mut row, cells[0])?,
                label,
                run_properties.clone(),
            )?;
            set_cell_text(
                child_mut(&mut row, cells[1])?,
                value,
                run_properties.clone(),
            )?;
        }
        rows.push(row);
    }
    let insert_after = anchor_index.unwrap_or(insertion_index);
    children.splice(insert_after + 1..insert_after + 1, rows);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::insert_rows;
    use crate::{inspect::paragraph_texts, xml};
    use std::collections::BTreeMap;

    #[test]
    fn inserts_optional_party_a_fields_in_each_top_level_party_table() {
        let source = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl>
                    <w:tr><w:tc><w:p><w:r><w:t>{{PARTY_A_NAME}}</w:t></w:r></w:p></w:tc></w:tr>
                    <w:tr><w:tc><w:p><w:r><w:t>Đại diện</w:t></w:r></w:p></w:tc><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
                    <w:tr><w:tc><w:p><w:r><w:t>Chức vụ</w:t></w:r></w:p></w:tc><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
                </w:tbl>
                <w:tbl>
                    <w:tr><w:tc><w:p><w:r><w:t>{{PARTY_A_NAME}}</w:t></w:r></w:p></w:tc></w:tr>
                    <w:tr><w:tc><w:p><w:r><w:t>Đại diện</w:t></w:r></w:p></w:tc><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
                    <w:tr><w:tc><w:p><w:r><w:t>Chức vụ</w:t></w:r></w:p></w:tc><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
                </w:tbl>
            </w:body></w:document>"#;
        let mut document = xml::parse(source.as_bytes()).expect("parse synthetic document");
        let values = BTreeMap::from([("PARTY_A_TAX_CODE".into(), "12345".into())]);

        insert_rows(&mut document, &values).expect("insert optional rows");

        let paragraphs = paragraph_texts(&document);
        assert_eq!(
            paragraphs
                .iter()
                .filter(|text| text.as_str() == "Mã số thuế")
                .count(),
            2
        );
        assert_eq!(
            paragraphs
                .iter()
                .filter(|text| text.as_str() == "12345")
                .count(),
            2
        );
    }
}
