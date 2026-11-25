use std::io::{Cursor, Write};

use zip::{ZipWriter, write::SimpleFileOptions};

/// Tạo gói DOCX tối giản với các quan hệ và kiểu nội dung cơ bản.
pub fn make_docx(document_body_xml: &str) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let entries = [
        ("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_owned()),
        ("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_owned()),
        ("word/document.xml", format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{document_body_xml}</w:body></w:document>"#)),
        ("word/_rels/document.xml.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#.to_owned()),
    ];
    for (name, contents) in entries {
        if writer
            .start_file(name, SimpleFileOptions::default())
            .is_err()
        {
            return Vec::new();
        }
        if writer.write_all(contents.as_bytes()).is_err() {
            return Vec::new();
        }
    }
    writer
        .finish()
        .map_or_else(|_| Vec::new(), |cursor| cursor.into_inner())
}
