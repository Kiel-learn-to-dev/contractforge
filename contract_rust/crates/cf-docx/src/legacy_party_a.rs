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

fn top_level_target(node: &Node, in_table: bool) -> Option<Vec<usize>> {
    for (index, child) in node.children().iter().enumerate() {
        if child.name() == Some("w:tbl") && !in_table && has_anchor(child) {
            return Some(vec![index]);
        }
        let mut path = top_level_target(child, in_table || child.name() == Some("w:tbl"));
        if let Some(path) = path.as_mut() {
            path.insert(0, index);
            return path.clone().into();
        }
    }
    None
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
    let Some(path) = top_level_target(document, false) else {
        return Ok(());
    };
    let Some(table) = node_at_mut(document, &path) else {
        return Ok(());
    };
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
