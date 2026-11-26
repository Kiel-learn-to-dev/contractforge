use std::{collections::BTreeMap, fs, path::PathBuf};

use cf_docx::{DocxError, Package, RenderData, Template, inspect, package::is_text_part, xml};

fn first_tree_difference(left: &xml::Node, right: &xml::Node, path: &str) -> Option<String> {
    match (left, right) {
        (
            xml::Node::Element {
                name: left_name,
                attrs: left_attrs,
                children: left_children,
            },
            xml::Node::Element {
                name: right_name,
                attrs: right_attrs,
                children: right_children,
            },
        ) => {
            if left_name != right_name || left_attrs != right_attrs {
                return Some(format!(
                    "{path}: element/attribute mismatch {left_name} {left_attrs:?} vs {right_name} {right_attrs:?}"
                ));
            }
            for (index, (left_child, right_child)) in
                left_children.iter().zip(right_children).enumerate()
            {
                if let Some(difference) = first_tree_difference(
                    left_child,
                    right_child,
                    &format!("{path}/{left_name}[{index}]"),
                ) {
                    return Some(difference);
                }
            }
            (left_children.len() != right_children.len()).then(|| {
                format!(
                    "{path}/{left_name}: child count {} vs {}",
                    left_children.len(),
                    right_children.len()
                )
            })
        }
        (xml::Node::Text(left), xml::Node::Text(right)) if left != right => {
            Some(format!("{path}: text {left:?} vs {right:?}"))
        }
        (xml::Node::Raw(left), xml::Node::Raw(right)) if left != right => {
            Some(format!("{path}: raw {left:?} vs {right:?}"))
        }
        (left, right) if std::mem::discriminant(left) != std::mem::discriminant(right) => {
            Some(format!("{path}: node type differs: {left:?} vs {right:?}"))
        }
        _ => None,
    }
}

fn docx_samples() -> Vec<PathBuf> {
    let mut files = fs::read_dir(cf_testkit::fixture_path("docx"))
        .expect("DOCX fixtures")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "docx")
        })
        .collect::<Vec<_>>();
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..");
    let sample_dir = repository.join("contract_rust/assets/default_templates");
    files.extend(
        fs::read_dir(sample_dir)
            .expect("sample templates")
            .map(|entry| entry.expect("sample template entry").path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("sample_") && name.ends_with(".docx"))
            }),
    );
    files
}

#[test]
fn every_sample_text_part_roundtrips_as_an_xml_tree() {
    for path in docx_samples() {
        let bytes = fs::read(&path).expect("read DOCX");
        let package = Package::read(&bytes).expect("read ZIP package");
        for (index, name) in package.entries().iter().enumerate() {
            if !is_text_part(name) {
                continue;
            }
            let original = xml::parse(&package.read_entry(index).expect("read text part"))
                .unwrap_or_else(|error| panic!("parse {name} in {}: {error}", path.display()));
            let serialized = xml::serialize(&original);
            let reparsed = xml::parse(&serialized)
                .unwrap_or_else(|error| panic!("reparse {name} in {}: {error}", path.display()));
            assert_eq!(
                first_tree_difference(&original, &reparsed, "#document"),
                None,
                "{}::{name}",
                path.display()
            );
        }
    }
}

#[test]
fn rendering_keeps_non_text_entries_and_replaces_sample_values() {
    let path = cf_testkit::fixture_path("docx/synthetic_all_traps.docx");
    let bytes = fs::read(path).expect("synthetic DOCX");
    let template = Template::compile(&bytes).expect("compile template");
    let input = Package::read(&bytes).expect("source package");
    let fixture = cf_testkit::fixture("docx_render");
    let scenario = fixture["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scenario| scenario["name"] == "lists_none")
        .unwrap();
    let values = scenario["context"]
        .as_object()
        .unwrap()
        .iter()
        .filter_map(|(key, value)| value.as_str().map(|text| (key.clone(), text.to_owned())))
        .collect::<BTreeMap<_, _>>();
    let rendered = template
        .render(&RenderData {
            values,
            ..RenderData::default()
        })
        .expect("render fixture");
    let output = Package::read(&rendered).expect("rendered ZIP package");
    for (index, name) in input.entries().iter().enumerate() {
        if !is_text_part(name) {
            assert_eq!(
                input.read_entry(index).unwrap(),
                output.read_entry(index).unwrap(),
                "non-text entry {name} must retain its bytes"
            );
        }
    }
    let texts = inspect::docx_texts(&rendered).expect("extract rendered text");
    assert!(
        texts["word/document.xml"]
            .iter()
            .any(|text| text.contains("01/TYTSONGXANH/2026"))
    );
    assert!(texts["word/header1.xml"][0].contains("01/TYTSONGXANH/2026"));
}

#[test]
fn package_reads_entries_and_rejects_garbage_and_make_docx_roundtrips() {
    let bytes = fs::read(cf_testkit::fixture_path("docx/synthetic_all_traps.docx")).unwrap();
    let pkg = Package::read(&bytes).unwrap();
    assert!(pkg.entries().iter().any(|e| e == "word/document.xml"));
    assert!(pkg.entries().iter().any(|e| e == "word/header1.xml"));
    assert!(pkg.entries().iter().any(|e| e == "word/footer1.xml"));
    assert!(pkg.entries().iter().any(|e| e == "word/styles.xml"));
    assert!(is_text_part("word/document.xml"));
    assert!(is_text_part("word/header1.xml"));
    assert!(is_text_part("word/footer1.xml"));
    assert!(!is_text_part("word/styles.xml"));

    let err = Package::read(b"not a zip").unwrap_err();
    assert!(matches!(err, DocxError::Open(_)));
    assert!(err.to_string().starts_with("Không mở được file .docx:"));

    let mini = cf_testkit::make_docx("<w:p><w:r><w:t>Xin chào</w:t></w:r></w:p>");
    let mini_pkg = Package::read(&mini).unwrap();
    assert!(mini_pkg.entries().iter().any(|e| e == "word/document.xml"));
    assert_eq!(
        inspect::docx_texts(&mini).unwrap()["word/document.xml"],
        vec!["Xin chào"]
    );
}

#[test]
fn inspect_matches_expected_docx_texts_in_fixture() {
    let fixture = cf_testkit::fixture("docx_render");
    for scenario in fixture["scenarios"].as_array().unwrap() {
        let name = scenario["name"].as_str().unwrap();
        let path = cf_testkit::fixture_path(format!("docx/expected_{name}.docx"));
        let bytes = fs::read(&path).unwrap();
        let actual = inspect::docx_texts(&bytes).unwrap();
        let expected: BTreeMap<String, Vec<String>> = scenario["expected_texts"]
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
        assert_eq!(actual, expected, "expected_{name}.docx");
    }
}
