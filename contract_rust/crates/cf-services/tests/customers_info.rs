use std::io::{Cursor, Read};

use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    customers::{
        docs::upload_doc,
        info::{download_bundle, export_info},
    },
};
use chrono::{NaiveDate, NaiveDateTime};
use zip::ZipArchive;

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
fn export_info_and_bundle_include_markdown_sections_and_documents() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    upload_doc(&mut ctx, 1, "cccd_front", "", b"%PDF cccd", "cccd.pdf").expect("upload doc");

    let md = export_info(ctx.store(), 1, ctx.clock().now_local()).expect("export info");
    let text = String::from_utf8(md.content).expect("utf8");
    for heading in [
        "## Thông tin cơ bản",
        "## Địa chỉ",
        "## Pháp lý & Mã số",
        "## Ngân hàng",
        "## Liên hệ",
        "## Cơ sở tính phí",
        "## Hợp đồng",
    ] {
        assert!(text.contains(heading), "missing heading: {heading}");
    }

    let bundle = download_bundle(&ctx, 1).expect("download bundle");
    assert!(bundle.file_name.ends_with(".zip"));
    let mut archive = ZipArchive::new(Cursor::new(bundle.content)).expect("zip archive");
    let mut names = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).expect("entry");
        names.push(entry.name().expect("entry name").into_owned());
    }
    assert!(names.contains(&"thong_tin.md".to_string()));
    assert!(names.contains(&"CCCD_mat_truoc.pdf".to_string()));

    let mut doc_bytes = Vec::new();
    archive
        .by_name("CCCD_mat_truoc.pdf")
        .expect("doc in zip")
        .read_to_end(&mut doc_bytes)
        .expect("read");
    assert_eq!(doc_bytes, b"%PDF cccd");
}
