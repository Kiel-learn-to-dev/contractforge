use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use chrono::{NaiveDate, NaiveDateTime};

fn context(root: &std::path::Path) -> cf_services::Ctx {
    let db = cf_testkit::seeded_db(root).expect("seed database");
    let store = Store::load(db).expect("load database");
    let clock = FixedClock {
        date: NaiveDate::from_ymd_opt(2026, 3, 24).expect("date"),
        utc: NaiveDateTime::parse_from_str("2026-03-24 04:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("time"),
        local: NaiveDateTime::parse_from_str("2026-03-24 11:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("time"),
    };
    cf_services::Ctx::new(store, DataPaths::new(root), Box::new(clock))
}

#[test]
fn fixed_customer_document_replacement_removes_old_file_and_keeps_one_row() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut ctx = context(dir.path());
    let first = cf_services::customers::docs::upload_doc(
        &mut ctx,
        1,
        "cccd_front",
        "",
        b"%PDF first",
        "front.pdf",
    )
    .expect("upload first");
    let old_path = cf_services::customers::docs::doc_path(ctx.store(), dir.path(), 1, first)
        .expect("resolve old file");
    assert!(old_path.is_file());
    let second = cf_services::customers::docs::upload_doc(
        &mut ctx,
        1,
        "cccd_front",
        "",
        b"%PDF second",
        "front.pdf",
    )
    .expect("replace document");
    let new_path = cf_services::customers::docs::doc_path(ctx.store(), dir.path(), 1, second)
        .expect("resolve replacement");
    assert_eq!(first, second);
    assert!(!old_path.exists());
    assert_eq!(
        std::fs::read(new_path).expect("read replacement"),
        b"%PDF second"
    );
    assert_eq!(
        ctx.store()
            .documents
            .values()
            .filter(|doc| doc.customer_id == 1 && doc.doc_type == "cccd_front")
            .count(),
        1
    );
}

#[test]
fn customer_document_upload_rejects_missing_other_label_and_invalid_file() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut ctx = context(dir.path());
    let missing_label = cf_services::customers::docs::upload_doc(
        &mut ctx,
        1,
        "other",
        "   ",
        b"%PDF test",
        "document.pdf",
    )
    .expect_err("other document needs a label");
    assert_eq!(missing_label.user_message(), "Vui lòng nhập tên giấy tờ");
    let invalid = cf_services::customers::docs::upload_doc(
        &mut ctx,
        1,
        "cccd_front",
        "",
        b"MZ executable",
        "document.exe",
    )
    .expect_err("invalid magic bytes");
    assert_eq!(
        invalid.user_message(),
        "File không hợp lệ — chỉ chấp nhận PDF, JPG hoặc PNG"
    );
    assert!(ctx.store().documents.is_empty());
}
