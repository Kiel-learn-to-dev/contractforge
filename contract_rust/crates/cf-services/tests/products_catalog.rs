use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    products::{
        catalog_export::{IMPORT_COLUMNS, make_catalog_xlsx},
        catalog_import::{CatalogAction, apply_catalog_rows, parse_catalog_xlsx},
    },
};
use chrono::{NaiveDate, NaiveDateTime};
use rust_xlsxwriter::Workbook;

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
fn export_and_reimport_catalog_updates_without_errors_or_changes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    let bytes = make_catalog_xlsx(Some(ctx.store()), true).expect("export catalog");

    let mut wb: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes.clone())).expect("open xlsx");
    let range = wb.worksheet_range("Danh mục").expect("sheet");
    let headers: Vec<_> = range
        .rows()
        .next()
        .expect("header")
        .iter()
        .map(|cell| match cell {
            Data::String(s) => s.as_str(),
            _ => "",
        })
        .collect();
    assert_eq!(headers, IMPORT_COLUMNS);
    assert_eq!(range.height(), ctx.store().products.len() + 1);

    let preview = parse_catalog_xlsx(ctx.store(), &bytes).expect("parse preview");
    assert_eq!(preview.len(), ctx.store().products.len());
    assert!(
        preview
            .iter()
            .all(|row| row.action == CatalogAction::Update)
    );

    let before_count = ctx.store().products.len();
    let (created, updated) = apply_catalog_rows(&mut ctx, &preview).expect("apply");
    assert_eq!(created, 0);
    assert_eq!(updated, before_count);
    assert_eq!(ctx.store().products.len(), before_count);
}

#[test]
fn apply_rejects_when_any_row_has_error_leaving_db_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (col, name) in IMPORT_COLUMNS.iter().enumerate() {
        ws.write_string(0, col as u16, *name).expect("header");
    }
    // Row 2: valid create
    ws.write_string(1, 0, "NEW-1").expect("code");
    ws.write_string(1, 1, "Sản phẩm mới").expect("name");
    ws.write_string(1, 2, "Dịch vụ").expect("type");
    ws.write_string(1, 4, "main").expect("kind");
    ws.write_string(1, 6, "300000").expect("price");
    // Row 3: invalid vat rate
    ws.write_string(2, 0, "BAD-1").expect("code");
    ws.write_string(2, 1, "Sản phẩm lỗi").expect("name");
    ws.write_string(2, 2, "Dịch vụ").expect("type");
    ws.write_string(2, 4, "main").expect("kind");
    ws.write_string(2, 7, "abc").expect("vat");

    let bytes = wb.save_to_buffer().expect("save");
    let rows = parse_catalog_xlsx(ctx.store(), &bytes).expect("parse");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].action, CatalogAction::Create);
    assert_eq!(rows[1].action, CatalogAction::Error);

    let before = ctx.store().products.len();
    let err = apply_catalog_rows(&mut ctx, &rows).expect_err("must reject");
    assert!(err.user_message().contains("Chưa ghi"));
    assert_eq!(ctx.store().products.len(), before);
    assert!(!ctx.store().products.values().any(|p| p.code == "NEW-1"));
}
