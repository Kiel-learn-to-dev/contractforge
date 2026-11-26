use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    sync::Arc,
};

use cf_docx::{RenderData, Template, TemplateCache, inspect, write_new_file};
use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

fn template_bytes_for(file_name: &str) -> Vec<u8> {
    if file_name == "synthetic_all_traps.docx" {
        fs::read(cf_testkit::fixture_path("docx/synthetic_all_traps.docx")).unwrap()
    } else {
        fs::read(
            repo_root()
                .join("contract_rust/assets/default_templates")
                .join(file_name),
        )
        .unwrap()
    }
}

fn parse_values(obj: &serde_json::Map<String, Value>) -> BTreeMap<String, String> {
    obj.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
        .collect()
}

fn parse_lists(val: &Value) -> Option<BTreeMap<String, Vec<BTreeMap<String, String>>>> {
    val.as_object().map(|obj| {
        obj.iter()
            .map(|(group, rows)| {
                let parsed = rows
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| parse_values(row.as_object().unwrap()))
                    .collect();
                (group.clone(), parsed)
            })
            .collect()
    })
}

fn parse_expected_texts(val: &Value) -> BTreeMap<String, Vec<String>> {
    val.as_object()
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
        .collect()
}

#[test]
fn analysis_matches_template_analysis_fixture() {
    let fixture = cf_testkit::fixture("template_analysis");
    for case in fixture["results"].as_array().unwrap() {
        let file = case["file"].as_str().unwrap();
        let bytes = template_bytes_for(file);
        let template = Template::compile(&bytes).unwrap();
        let actual = template.analysis();

        let expected_placeholders: Vec<String> = case["placeholders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        let expected_groups: Vec<String> = case["repeat_groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        let expected_warnings: Vec<String> = case["row_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        let expected_error = case["error"].as_str().unwrap_or_default();

        assert_eq!(actual.placeholders, expected_placeholders, "{file}");
        assert_eq!(actual.repeat_groups, expected_groups, "{file}");
        assert_eq!(actual.row_warnings, expected_warnings, "{file}");
        assert_eq!(
            actual.error.as_deref().unwrap_or(""),
            expected_error,
            "{file}"
        );
    }
}

#[test]
fn render_samples_and_manual_quotation_match_fixture() {
    let fixture = cf_testkit::fixture("docx_render");
    for sample in fixture["samples"].as_array().unwrap() {
        let rel = sample["template"].as_str().unwrap();
        let bytes = fs::read(repo_root().join(rel)).unwrap();
        let template = Template::compile(&bytes).unwrap();
        let rendered = template
            .render(&RenderData {
                values: parse_values(sample["context"].as_object().unwrap()),
                lists: parse_lists(&sample["lists"]),
                omit_empty_tables: BTreeSet::new(),
                legacy_party_a_rows: true,
            })
            .unwrap();
        Template::compile(&rendered).unwrap();
        let actual = inspect::docx_texts(&rendered).unwrap();
        let mut expected = parse_expected_texts(&sample["expected_texts"]);
        expected.retain(|part, lines| {
            actual.contains_key(part) || !lines.iter().all(|line| line.is_empty())
        });
        assert_eq!(actual, expected, "{rel}");
    }

    for mq in fixture["manual_quotation"].as_array().unwrap() {
        let layout = mq["values"]["layout"].as_str().unwrap();
        let bytes = fs::read(
            repo_root()
                .join("contract_rust/assets/default_templates")
                .join(format!("sample_quote_{layout}.docx")),
        )
        .unwrap();
        let template = Template::compile(&bytes).unwrap();
        let rendered = template
            .render(&RenderData {
                values: parse_values(mq["context"].as_object().unwrap()),
                lists: parse_lists(&mq["lists"]),
                omit_empty_tables: BTreeSet::from(["features".into()]),
                legacy_party_a_rows: false,
            })
            .unwrap();
        Template::compile(&rendered).unwrap();
        let actual = inspect::docx_texts(&rendered).unwrap();
        let mut expected = parse_expected_texts(&mq["expected_texts"]);
        expected.retain(|part, lines| {
            actual.contains_key(part) || !lines.iter().all(|line| line.is_empty())
        });
        assert_eq!(actual, expected, "manual_quotation {layout}");
    }
}

#[test]
fn write_new_file_increments_suffix_and_preserves_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    let (p1, n1) = write_new_file(dir.path(), "a.docx", b"first").unwrap();
    let (p2, n2) = write_new_file(dir.path(), "a.docx", b"second").unwrap();
    let (p3, n3) = write_new_file(dir.path(), "a.docx", b"third").unwrap();
    assert_eq!(n1, "a.docx");
    assert_eq!(n2, "a_2.docx");
    assert_eq!(n3, "a_3.docx");
    assert_eq!(fs::read(p1).unwrap(), b"first");
    assert_eq!(fs::read(p2).unwrap(), b"second");
    assert_eq!(fs::read(p3).unwrap(), b"third");
}

#[test]
fn template_cache_reuses_arc_and_recompiles_on_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.docx");
    let bytes1 = cf_testkit::make_docx("<w:p><w:r><w:t>{{A}}</w:t></w:r></w:p>");
    fs::write(&path, &bytes1).unwrap();

    let mut cache = TemplateCache::default();
    let t1 = cache.get_or_compile(&path).unwrap();
    let t2 = cache.get_or_compile(&path).unwrap();
    assert!(Arc::ptr_eq(&t1, &t2));

    let bytes2 = cf_testkit::make_docx("<w:p><w:r><w:t>{{B_LONGER}}</w:t></w:r></w:p>");
    fs::write(&path, &bytes2).unwrap();
    let t3 = cache.get_or_compile(&path).unwrap();
    assert!(!Arc::ptr_eq(&t1, &t3));
    assert_eq!(t3.analysis().placeholders, vec!["B_LONGER"]);
}
