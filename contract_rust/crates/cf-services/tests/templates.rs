use std::{collections::BTreeMap, fs, path::PathBuf};

use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    templates::{
        bulk::bulk_upload,
        bundled::seed_from_project_files,
        cleanup::cleanup_orphan_template_files,
        create::{TemplateDraft, create_template},
        edit::{reanalyze, toggle_active, update_mapping, update_metadata},
        mapping::default_mapping,
    },
};
use chrono::{NaiveDate, NaiveDateTime};

fn context(root: &std::path::Path) -> Ctx {
    let db = cf_testkit::seeded_db(root).expect("seed");
    let store = Store::load(db).expect("load");
    let clock = FixedClock {
        date: NaiveDate::from_ymd_opt(2026, 3, 24).expect("date"),
        utc: NaiveDateTime::parse_from_str("2026-03-24 04:40:37", "%Y-%m-%d %H:%M:%S").expect("dt"),
        local: NaiveDateTime::parse_from_str("2026-03-24 11:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("dt"),
    };
    Ctx::new(store, DataPaths::new(root), Box::new(clock))
}

fn default_template_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("ContractForge")
        .join("assets")
        .join("default_templates")
        .join(name)
}

#[test]
fn default_mapping_matches_template_analysis_fixture() {
    let fixture = cf_testkit::fixture("template_analysis");
    let results = fixture["results"].as_array().expect("results array");
    assert!(!results.is_empty());
    for item in results {
        let placeholders: Vec<String> = item["placeholders"]
            .as_array()
            .expect("placeholders")
            .iter()
            .map(|v| v.as_str().expect("str").to_owned())
            .collect();
        let expected: BTreeMap<String, String> = item["mapping"]
            .as_object()
            .expect("mapping object")
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
            .collect();
        assert_eq!(
            default_mapping(&placeholders),
            expected,
            "mapping mismatch for {}",
            item["file"]
        );
    }
}

#[test]
fn create_template_writes_file_analyzes_placeholders_and_cleans_up_on_corrupt_docx() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    let sample_bytes =
        fs::read(default_template_path("sample_service_contract.docx")).expect("read sample");

    let id = create_template(
        &mut ctx,
        TemplateDraft {
            code: "",
            name: "Mẫu thử nghiệm",
            contract_type: "Hợp đồng",
            file_bytes: &sample_bytes,
            original_filename: "sample service.docx",
            description: "Mô tả",
            mode: "placeholder",
            product_group: "G1",
        },
    )
    .expect("create template");

    let tpl = &ctx.store().templates[&id];
    assert_eq!(tpl.code, "TPL-001");
    assert!(!tpl.detected_placeholders.is_empty());
    let stored =
        cf_db::paths::resolve_stored(&ctx.paths().root, tpl.file_path.as_deref().expect("path"));
    assert!(stored.is_file());

    // Corrupt docx -> no record, no file created
    let before_count = ctx.store().templates.len();
    let files_before = fs::read_dir(&ctx.paths().templates).expect("dir").count();
    let err = create_template(
        &mut ctx,
        TemplateDraft {
            code: "",
            name: "Mẫu hỏng",
            contract_type: "Hợp đồng",
            file_bytes: b"not a zip docx",
            original_filename: "broken.docx",
            description: "",
            mode: "placeholder",
            product_group: "",
        },
    )
    .expect_err("corrupt docx must fail");
    assert!(!err.user_message().is_empty());
    assert_eq!(ctx.store().templates.len(), before_count);
    assert_eq!(
        fs::read_dir(&ctx.paths().templates).expect("dir").count(),
        files_before
    );
}

#[test]
fn edit_mapping_drops_unknown_keys_and_reanalyze_preserves_existing_mapping() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    let sample_bytes =
        fs::read(default_template_path("sample_service_contract.docx")).expect("read sample");
    let id = create_template(
        &mut ctx,
        TemplateDraft {
            code: "TPL-EDIT",
            name: "Mẫu sửa",
            contract_type: "Hợp đồng",
            file_bytes: &sample_bytes,
            original_filename: "edit.docx",
            description: "",
            mode: "placeholder",
            product_group: "G1",
        },
    )
    .expect("create");

    let first_placeholder = ctx.store().templates[&id].detected_placeholders[0].clone();
    let custom_mapping = BTreeMap::from([
        (first_placeholder.clone(), "custom.field".to_string()),
        (
            "UNKNOWN_KEY_XYZ".to_string(),
            "should.be.dropped".to_string(),
        ),
    ]);
    let now = ctx.clock().now_utc();
    update_mapping(ctx.store_mut(), id, &custom_mapping, now).expect("update mapping");
    let tpl = &ctx.store().templates[&id];
    assert_eq!(
        tpl.field_mapping
            .get(&first_placeholder)
            .map(String::as_str),
        Some("custom.field")
    );
    assert!(!tpl.field_mapping.contains_key("UNKNOWN_KEY_XYZ"));

    update_metadata(&mut ctx, id, "Mẫu đã đổi tên", "Hợp đồng", "Mới", "G2")
        .expect("update metadata");
    assert_eq!(ctx.store().templates[&id].name, "Mẫu đã đổi tên");
    assert_eq!(
        ctx.store().templates[&id].product_group.as_deref(),
        Some("G2")
    );

    reanalyze(&mut ctx, id).expect("reanalyze");
    assert_eq!(
        ctx.store().templates[&id]
            .field_mapping
            .get(&first_placeholder)
            .map(String::as_str),
        Some("custom.field")
    );

    let active = toggle_active(ctx.store_mut(), id, now).expect("toggle");
    assert!(!active);
}

#[test]
fn bulk_upload_bundled_seed_and_orphan_cleanup_work() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());

    let bundle_dir = dir.path().join("bundled_src");
    fs::create_dir_all(&bundle_dir).expect("mkdir");
    fs::copy(
        default_template_path("sample_service_contract.docx"),
        bundle_dir.join("sample_ignored.docx"),
    )
    .expect("copy sample");
    fs::copy(
        default_template_path("sample_service_contract.docx"),
        bundle_dir.join("Hop_dong_dich_vu.docx"),
    )
    .expect("copy contract");
    fs::copy(
        default_template_path("template_BAOGIA_HSSK.docx"),
        bundle_dir.join("Mau_Bao_Gia_HSSK.docx"),
    )
    .expect("copy baogia");

    let bulk = bulk_upload(
        &mut ctx,
        &[
            bundle_dir.join("Hop_dong_dich_vu.docx"),
            bundle_dir.join("Mau_Bao_Gia_HSSK.docx"),
        ],
    );
    assert_eq!(bulk.len(), 2);
    assert!(
        bulk.iter()
            .all(|r| r.error.is_none() && r.template_id.is_some())
    );
    let t1_id = bulk[0].template_id.unwrap();
    let t2_id = bulk[1].template_id.unwrap();
    assert_eq!(ctx.store().templates[&t1_id].contract_type, "Hợp đồng");
    assert_eq!(ctx.store().templates[&t2_id].contract_type, "Báo giá");
    let t1_path = ctx.store().templates[&t1_id]
        .file_path
        .clone()
        .expect("t1 path");

    // Seed from project files: skips sample_*, imports non-sample once, second run imports 0
    let first_seed = seed_from_project_files(&mut ctx, &bundle_dir).expect("seed 1");
    assert_eq!(first_seed, 2);
    let second_seed = seed_from_project_files(&mut ctx, &bundle_dir).expect("seed 2");
    assert_eq!(second_seed, 0);

    // Orphan cleanup: create an orphan file, ensure referenced files stay and orphan is removed
    let orphan = ctx.paths().templates.join("orphan_unused.docx");
    fs::write(&orphan, b"orphan").expect("write orphan");
    let removed = cleanup_orphan_template_files(&ctx).expect("cleanup");
    assert_eq!(removed, 1);
    assert!(!orphan.exists());
    let kept = cf_db::paths::resolve_stored(&ctx.paths().root, &t1_path);
    assert!(kept.is_file());
}
