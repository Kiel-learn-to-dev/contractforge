use quick_xml::{
    XmlVersion,
    escape::{escape, partial_escape},
    events::{BytesStart, Event},
    reader::Reader,
};

use crate::DocxError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element {
        name: String,
        attrs: Vec<(String, String)>,
        children: Vec<Node>,
    },
    Text(String),
    Raw(String),
}

impl Node {
    pub fn element(name: impl Into<String>) -> Self {
        Self::Element {
            name: name.into(),
            attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn children(&self) -> &[Node] {
        match self {
            Self::Element { children, .. } => children,
            _ => &[],
        }
    }

    pub fn push_child(&mut self, child: Node) -> Result<(), DocxError> {
        match self {
            Self::Element { children, .. } => {
                children.push(child);
                Ok(())
            }
            _ => Err(DocxError::Xml(
                "Chỉ phần tử XML mới có thể chứa node con.".into(),
            )),
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Element { name, .. } => Some(name),
            _ => None,
        }
    }
}

fn attributes(start: &BytesStart<'_>) -> Result<Vec<(String, String)>, DocxError> {
    start
        .attributes()
        .map(|attribute| {
            let attribute = attribute.map_err(|error| DocxError::Xml(error.to_string()))?;
            let name = attribute.key.as_ref().to_owned();
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|error| DocxError::Xml(error.to_string()))?
                .into_owned();
            Ok((name, value))
        })
        .collect()
}

fn push_text(element: &mut Node, value: &str) -> Result<(), DocxError> {
    if value.is_empty() {
        return Ok(());
    }
    if let Node::Element { children, .. } = element
        && let Some(Node::Text(existing)) = children.last_mut()
    {
        existing.push_str(value);
        return Ok(());
    }
    element.push_child(Node::Text(value.to_owned()))
}

fn parse_element(reader: &mut Reader<&[u8]>, start: BytesStart<'_>) -> Result<Node, DocxError> {
    let name = start.name().as_ref().to_owned();
    let attrs = attributes(&start)?;
    let mut element = Node::Element {
        name: name.clone(),
        attrs,
        children: Vec::new(),
    };
    loop {
        let event = reader
            .read_event()
            .map_err(|error| DocxError::Xml(error.to_string()))?;
        match event {
            Event::Start(child) => element.push_child(parse_element(reader, child)?)?,
            Event::Empty(child) => {
                let child_name = child.name().as_ref().to_owned();
                let child_attrs = attributes(&child)?;
                element.push_child(Node::Element {
                    name: child_name,
                    attrs: child_attrs,
                    children: Vec::new(),
                })?;
            }
            Event::End(end) => {
                if end.name().as_ref() != name {
                    return Err(DocxError::Xml(format!(
                        "Thẻ đóng '{}' không khớp với '{}'.",
                        end.name().as_ref(),
                        name
                    )));
                }
                return Ok(element);
            }
            Event::Text(text) => {
                let value = text.xml_content(XmlVersion::Implicit1_0);
                push_text(&mut element, &value)?;
            }
            Event::CData(text) => {
                let value = text.xml_content(XmlVersion::Implicit1_0);
                push_text(&mut element, &value)?;
            }
            Event::GeneralRef(reference) => {
                if let Ok(Some(ch)) = reference.resolve_char_ref() {
                    let mut buf = [0_u8; 4];
                    push_text(&mut element, ch.encode_utf8(&mut buf))?;
                } else {
                    match reference.as_ref() {
                        "amp" => push_text(&mut element, "&")?,
                        "lt" => push_text(&mut element, "<")?,
                        "gt" => push_text(&mut element, ">")?,
                        "quot" => push_text(&mut element, "\"")?,
                        "apos" => push_text(&mut element, "'")?,
                        other => element.push_child(Node::Raw(format!("&{other};")))?,
                    }
                }
            }
            Event::Decl(_) | Event::PI(_) | Event::Comment(_) | Event::DocType(_) => {
                let mut raw = Vec::new();
                let mut writer = quick_xml::writer::Writer::new(&mut raw);
                writer
                    .write_event(event.into_owned())
                    .map_err(|error| DocxError::Xml(error.to_string()))?;
                element.push_child(Node::Raw(String::from_utf8_lossy(&raw).into_owned()))?;
            }
            Event::Eof => return Err(DocxError::Xml(format!("Thẻ '{}' chưa được đóng.", name))),
        }
    }
}

/// Parse XML events into a prefix-preserving tree; attribute order is retained.
/// Source: https://docs.rs/quick-xml/0.42.0/quick_xml/events/enum.Event.html
pub fn parse(bytes: &[u8]) -> Result<Node, DocxError> {
    let source = std::str::from_utf8(bytes).map_err(|error| DocxError::Xml(error.to_string()))?;
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(false);
    let mut document = Node::element("#document");
    let mut root_seen = false;
    let mut root_closed = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| DocxError::Xml(error.to_string()))?;
        match event {
            Event::Start(start) => {
                if root_seen || root_closed {
                    return Err(DocxError::Xml("Tài liệu XML có nhiều phần tử gốc.".into()));
                }
                root_seen = true;
                document.push_child(parse_element(&mut reader, start)?)?;
                root_closed = true;
            }
            Event::Empty(start) => {
                if root_seen || root_closed {
                    return Err(DocxError::Xml("Tài liệu XML có nhiều phần tử gốc.".into()));
                }
                root_seen = true;
                root_closed = true;
                let name = start.name().as_ref().to_owned();
                let attrs = attributes(&start)?;
                document.push_child(Node::Element {
                    name,
                    attrs,
                    children: Vec::new(),
                })?;
            }
            Event::Text(text) => {
                let value = text.xml_content(XmlVersion::Implicit1_0).into_owned();
                if !value.trim().is_empty() {
                    return Err(DocxError::Xml(
                        "Văn bản xuất hiện ngoài phần tử gốc XML.".into(),
                    ));
                }
            }
            Event::CData(_) => {
                return Err(DocxError::Xml(
                    "CDATA xuất hiện ngoài phần tử gốc XML.".into(),
                ));
            }
            Event::GeneralRef(_) => {
                return Err(DocxError::Xml(
                    "Tham chiếu entity xuất hiện ngoài phần tử gốc XML.".into(),
                ));
            }
            Event::Decl(_) | Event::PI(_) | Event::Comment(_) | Event::DocType(_) => {
                let mut raw = Vec::new();
                let mut writer = quick_xml::writer::Writer::new(&mut raw);
                writer
                    .write_event(event.into_owned())
                    .map_err(|error| DocxError::Xml(error.to_string()))?;
                document.push_child(Node::Raw(String::from_utf8_lossy(&raw).into_owned()))?;
            }
            Event::End(end) => {
                return Err(DocxError::Xml(format!(
                    "Thẻ đóng '{}' không có thẻ mở tương ứng.",
                    end.name().as_ref()
                )));
            }
            Event::Eof if root_seen => return Ok(document),
            Event::Eof => return Err(DocxError::Xml("Tài liệu XML không có phần tử gốc.".into())),
        }
    }
}

fn serialize_node(node: &Node, output: &mut String) {
    match node {
        Node::Text(text) => output.push_str(&partial_escape(text)),
        Node::Raw(raw) => output.push_str(raw),
        Node::Element { name, .. } if name == "#document" => {
            for child in node.children() {
                serialize_node(child, output);
            }
        }
        Node::Element {
            name,
            attrs,
            children,
        } => {
            output.push('<');
            output.push_str(name);
            for (key, value) in attrs {
                output.push(' ');
                output.push_str(key);
                output.push_str("=\"");
                output.push_str(&escape(value));
                output.push('"');
            }
            output.push('>');
            for child in children {
                serialize_node(child, output);
            }
            output.push_str("</");
            output.push_str(name);
            output.push('>');
        }
    }
}

pub fn serialize(node: &Node) -> Vec<u8> {
    let mut output = String::new();
    serialize_node(node, &mut output);
    output.into_bytes()
}
