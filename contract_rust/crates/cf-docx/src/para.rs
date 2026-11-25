use crate::xml::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodePath {
    pub path: Vec<usize>,
    pub text: String,
}

pub fn node_at<'a>(node: &'a Node, path: &[usize]) -> Option<&'a Node> {
    let mut current = node;
    for index in path {
        current = current.children().get(*index)?;
    }
    Some(current)
}

pub fn node_at_mut<'a>(node: &'a mut Node, path: &[usize]) -> Option<&'a mut Node> {
    let Some((first, rest)) = path.split_first() else {
        return Some(node);
    };
    let Node::Element { children, .. } = node else {
        return None;
    };
    node_at_mut(children.get_mut(*first)?, rest)
}

pub fn paragraph_at<'a>(node: &'a Node, path: &[usize]) -> Option<&'a Node> {
    let candidate = node_at(node, path)?;
    if candidate.name() == Some("w:p") {
        Some(candidate)
    } else {
        None
    }
}

pub fn paragraph_at_mut<'a>(node: &'a mut Node, path: &[usize]) -> Option<&'a mut Node> {
    let candidate = node_at_mut(node, path)?;
    if candidate.name() == Some("w:p") {
        Some(candidate)
    } else {
        None
    }
}

fn visit_paragraph(node: &Node, path: &mut Vec<usize>, output: &mut Vec<NodePath>, is_root: bool) {
    if !is_root && node.name() == Some("w:p") {
        return;
    }
    if node.name() == Some("w:t") {
        let text = node
            .children()
            .iter()
            .filter_map(|child| match child {
                Node::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        output.push(NodePath {
            path: path.clone(),
            text,
        });
        return;
    }
    for (index, child) in node.children().iter().enumerate() {
        path.push(index);
        visit_paragraph(child, path, output, false);
        path.pop();
    }
}

/// Lấy các `w:t` thuộc đoạn hiện tại, bỏ qua `w:p` lồng của textbox.
pub fn text_nodes(paragraph: &Node) -> Vec<NodePath> {
    let mut result = Vec::new();
    visit_paragraph(paragraph, &mut Vec::new(), &mut result, true);
    result
}

/// Nối chữ trong các `w:t` của đoạn và trả về ánh xạ offset theo ký tự.
pub fn joined_text(paragraph: &Node) -> (String, Vec<(usize, usize)>) {
    let nodes = text_nodes(paragraph);
    let mut joined = String::new();
    let mut positions = Vec::new();
    for (node_index, node) in nodes.iter().enumerate() {
        for (offset, character) in node.text.chars().enumerate() {
            joined.push(character);
            positions.push((node_index, offset));
        }
    }
    (joined, positions)
}
