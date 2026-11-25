use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use cf_docx::{Package, RenderData, Template, inspect, table, xml};

#[test]
fn row_keys_ignores_nested_tables() {
    let body = r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>{{OUTER}}</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>{{ITEM_NAME}}</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl>"#;
    let bytes = cf_testkit::make_docx(body);
    let pkg = Package::read(&bytes).unwrap();
    let idx = pkg.entry_index("word/document.xml").unwrap();
    let doc = xml::parse(&pkg.read_entry(idx).unwrap()).unwrap();
    let row = doc
        .children()
        .iter()
        .find(|n| n.name() == Some("w:document"))
        .unwrap()
        .children()
        .iter()
        .find(|n| n.name() == Some("w:body"))
        .unwrap()
        .children()
        .iter()
        .find(|n| n.name() == Some("w:tbl"))
        .unwrap()
        .children()
        .iter()
        .find(|n| n.name() == Some("w:tr"))
        .unwrap();
    let keys = table::keys(row);
    assert!(keys.contains(&("OUTER".to_owned(), false)));
    assert!(!keys.contains(&("ITEM_NAME".to_owned(), false)));
}

#[test]
fn all_docx_render_scenarios_match_expected_texts_with_kb() {
    let template_bytes =
        fs::read(cf_testkit::fixture_path("docx/synthetic_all_traps.docx")).unwrap();
    let template = Template::compile(&template_bytes).unwrap();
    let fixture = cf_testkit::fixture("docx_render");
    let kb03 = cf_testkit::kb("KB-03_docx_nested_and_all_parts");
    let kb03_from = kb03["replace_paragraph"]["from"].as_str().unwrap();
    let kb03_to = kb03["replace_paragraph"]["to"].as_str().unwrap();

    for scenario in fixture["scenarios"].as_array().unwrap() {
        let name = scenario["name"].as_str().unwrap();
        let values: BTreeMap<String, String> = scenario["context"]
            .as_object()
            .unwrap()
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
            .collect();
        let lists = scenario["lists"].as_object().map(|obj| {
            obj.iter()
                .map(|(group, rows)| {
                    let parsed = rows
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|row| {
                            row.as_object()
                                .unwrap()
                                .iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                                .collect()
                        })
                        .collect();
                    (group.clone(), parsed)
                })
                .collect()
        });
        let omit_empty_tables: BTreeSet<String> = scenario["omit_empty_tables"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        let legacy_party_a_rows = scenario["legacy_party_a_rows"].as_bool().unwrap_or(true);

        let rendered = template
            .render(&RenderData {
                values,
                lists,
                omit_empty_tables,
                legacy_party_a_rows,
            })
            .unwrap();
        // Rendered output must be re-compilable.
        Template::compile(&rendered).unwrap();

        let actual = inspect::docx_texts(&rendered).unwrap();
        let mut expected: BTreeMap<String, Vec<String>> = scenario["expected_texts"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(part, lines)| {
                (
                    part.clone(),
                    lines
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().to_owned())
                        .collect(),
                )
            })
            .collect();
        // Apply KB-03: Rust renders nested paragraphs too.
        if let Some(doc_lines) = expected.get_mut("word/document.xml") {
            for line in doc_lines.iter_mut() {
                if line == kb03_from {
                    *line = kb03_to.to_owned();
                }
            }
        }
        assert_eq!(actual, expected, "scenario {name}");
    }
}

#[test]
fn optional_rows_and_paragraphs_follow_kb06() {
    let body = concat!(
        r#"<w:p><w:r><w:t>{{?OPT_PARA}}</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>Giữ {{?KEEP_PARA}}</w:t></w:r></w:p>"#,
        r#"<w:tbl>"#,
        r#"<w:tr><w:tc><w:p><w:r><w:t>{{?A}}</w:t></w:r></w:p></w:tc></w:tr>"#,
        r#"<w:tr><w:tc><w:p><w:r><w:t>{{?B}}</w:t></w:r></w:p></w:tc></w:tr>"#,
        r#"<w:tr><w:tc><w:p><w:r><w:t>{{?C}}</w:t></w:r></w:p></w:tc></w:tr>"#,
        r#"<w:tr><w:tc><w:p><w:r><w:t>{{?B}} và {{?D}}</w:t></w:r></w:p></w:tc></w:tr>"#,
        r#"</w:tbl>"#,
    );
    let bytes = cf_testkit::make_docx(body);
    let template = Template::compile(&bytes).unwrap();
    let rendered = template
        .render(&RenderData {
            values: BTreeMap::from([
                ("OPT_PARA".into(), "".into()),
                ("KEEP_PARA".into(), "ok".into()),
                ("A".into(), "x".into()),
                ("B".into(), "".into()),
                ("D".into(), "y".into()),
            ]),
            ..RenderData::default()
        })
        .unwrap();
    let texts = inspect::docx_texts(&rendered).unwrap();
    assert_eq!(texts["word/document.xml"], vec!["Giữ ok", "x", " và y"]);
}
