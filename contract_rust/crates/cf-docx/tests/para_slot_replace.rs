use std::collections::BTreeMap;

use cf_docx::{Package, inspect, para, replace, slot, xml};

fn first_paragraph(docx_bytes: &[u8]) -> xml::Node {
    let pkg = Package::read(docx_bytes).unwrap();
    let idx = pkg.entry_index("word/document.xml").unwrap();
    let doc = xml::parse(&pkg.read_entry(idx).unwrap()).unwrap();
    doc.children()
        .iter()
        .find(|n| n.name() == Some("w:document"))
        .unwrap()
        .children()
        .iter()
        .find(|n| n.name() == Some("w:body"))
        .unwrap()
        .children()
        .iter()
        .find(|n| n.name() == Some("w:p"))
        .unwrap()
        .clone()
}

#[test]
fn para_includes_hyperlink_and_skips_nested_textbox_paragraphs() {
    let body = r#"<w:p>
        <w:r><w:t>Trước </w:t></w:r>
        <w:hyperlink r:id="rId1"><w:r><w:t>{{LINK}}</w:t></w:r></w:hyperlink>
        <w:r><w:pict><v:shape><v:textbox><w:txbxContent><w:p><w:r><w:t>{{INNER}}</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r>
    </w:p>"#;
    let bytes = cf_testkit::make_docx(body);
    let p = first_paragraph(&bytes);
    let (joined, _) = para::joined_text(&p);
    assert_eq!(joined, "Trước {{LINK}}");
}

#[test]
fn slot_scans_multiple_optional_invalid_and_split_placeholders() {
    let body = r#"<w:p>
        <w:r><w:t>{{A}}{{A}} {{ A }} {{a}} {{?OPT}} </w:t></w:r>
        <w:r><w:t>{{PA</w:t></w:r>
        <w:r><w:t>RTY_</w:t></w:r>
        <w:r><w:t>A}}</w:t></w:r>
    </w:p>"#;
    let bytes = cf_testkit::make_docx(body);
    let p = first_paragraph(&bytes);
    let slots = slot::scan(&p);
    assert_eq!(slots.len(), 4);
    assert_eq!(slots[0].key, "A");
    assert!(!slots[0].optional);
    assert_eq!(slots[1].key, "A");
    assert_eq!(slots[2].key, "OPT");
    assert!(slots[2].optional);
    assert_eq!(slots[3].key, "PARTY_A");
    assert_eq!(slots[3].start_node, 1);
    assert_eq!(slots[3].end_node, 3);
}

#[test]
fn replace_preserves_bold_run_and_surrounding_text_and_handles_newlines_tabs() {
    let body = r#"<w:p><w:r><w:t xml:space="preserve">Đầu {{</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>PARTY_A</w:t></w:r><w:r><w:t>_NAME}} cuối</w:t></w:r></w:p>"#;
    let bytes = cf_testkit::make_docx(body);
    let mut p = first_paragraph(&bytes);
    let values = BTreeMap::from([("PARTY_A_NAME".into(), "  Dòng 1\nDòng 2\tsau tab  ".into())]);
    replace::replace_values(&mut p, &values).unwrap();

    // Bold run (index 1 among w:r) must still exist with <w:b/> and empty text.
    let runs: Vec<&xml::Node> = p
        .children()
        .iter()
        .filter(|n| n.name() == Some("w:r"))
        .collect();
    assert_eq!(runs.len(), 3);
    assert!(runs[1].children().iter().any(|c| c.name() == Some("w:rPr")));
    assert_eq!(para::joined_text(runs[1]).0, "");

    let texts = inspect::paragraph_texts(&p);
    assert_eq!(texts, vec!["Đầu   Dòng 1\nDòng 2\tsau tab   cuối"]);
}
