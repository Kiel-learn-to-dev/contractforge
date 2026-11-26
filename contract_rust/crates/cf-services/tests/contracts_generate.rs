use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{Ctx, ServiceError, contracts::generate::generate_docx};
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

#[test]
fn generate_docx_transitions_draft_regenerates_cleanly_creates_bundle_and_fails_on_missing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());

    // 1. Draft -> Generated + file contains contract number
    let first = generate_docx(&mut ctx, 1, &[]).expect("generate first");
    assert!(first.file_path.is_file());
    assert_eq!(ctx.store().contracts[&1].status.as_str(), "Generated");
    assert!(
        ctx.store()
            .events
            .values()
            .any(|e| e.contract_id == 1 && e.event_type == "generated")
    );
    let bytes = std::fs::read(&first.file_path).expect("read docx");
    let texts = cf_docx::inspect::docx_texts(&bytes)
        .expect("inspect docx")
        .into_values()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
    assert!(texts.contains("01/G1/2026"));

    // 2. Regenerate with attachment -> old file removed if distinct name (advance clock or check bundle), status stays Generated
    // Move clock forward 1 minute so make_output_filename produces a distinct filename or verify same/replaced file
    let second = generate_docx(&mut ctx, 1, &[2]).expect("regenerate with attachment");
    assert!(second.file_path.is_file());
    assert_eq!(second.attachments_count, 1);
    let bundle = second.bundle_path.expect("bundle zip");
    assert!(bundle.is_file());
    assert_eq!(ctx.store().contracts[&1].status.as_str(), "Generated");
    if first.file_path != second.file_path {
        assert!(!first.file_path.exists());
    }

    // 3. Missing template file -> FileMissing and leaves contract 2 untouched
    ctx.store_mut()
        .templates
        .get_mut(&1)
        .expect("tpl 1")
        .file_path = Some("assets/does_not_exist.docx".into());
    let before_status = ctx.store().contracts[&2].status.clone();
    let err = generate_docx(&mut ctx, 2, &[]).expect_err("missing template file");
    assert!(matches!(err, ServiceError::FileMissing(_)));
    assert_eq!(ctx.store().contracts[&2].status, before_status);
    assert!(ctx.store().contracts[&2].output_file_path.is_none());
}
