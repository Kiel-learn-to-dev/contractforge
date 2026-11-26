use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::atomic::{AtomicBool, Ordering},
};

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    batch::{
        excel::{import_excel_mapping, make_batch_template_xlsx},
        row::{batch_row, form_from_params},
        run::{BatchJob, run_batch},
    },
    contracts::create::ContractForm,
};
use chrono::{NaiveDate, NaiveDateTime};
use rust_xlsxwriter::Workbook;
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
fn batch_template_xlsx_has_expected_headers_and_import_maps_customers() {
    let bytes = make_batch_template_xlsx().expect("make template");
    let mut wb: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).expect("open");
    let range = wb.worksheet_range("Batch Import").expect("sheet");
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
    assert_eq!(headers[0], "Mã khách hàng");
    assert_eq!(headers[1], "Số hợp đồng");
    assert_eq!(headers[12], "Mã gói chính");

    // Build Excel with 1 valid row, 1 empty row (skipped), 1 missing customer row
    let mut custom = Workbook::new();
    let ws = custom.add_worksheet();
    for (col, title) in ["Mã khách hàng", "Số hợp đồng", "Số tháng", "Mã gói chính"]
        .iter()
        .enumerate()
    {
        ws.write_string(0, col as u16, *title).expect("header");
    }
    ws.write_string(1, 0, "KH-FAKE-001").expect("code 1");
    ws.write_string(1, 1, "B-01/2026").expect("num 1");
    ws.write_string(1, 2, "12").expect("months 1");
    ws.write_string(1, 3, "G1-MAIN").expect("prod 1");
    // Row 2 (Excel row 3) blank
    // Row 3 (Excel row 4) unknown customer
    ws.write_string(3, 0, "KH-KHONG-TON-TAI").expect("bad code");
    ws.write_string(3, 1, "B-02/2026").expect("num 2");

    let lookup = BTreeMap::from([("KH-FAKE-001".to_string(), 1_i64)]);
    let imported = import_excel_mapping(&custom.save_to_buffer().expect("save"), &lookup);
    assert_eq!(imported.total_rows, 2);
    assert_eq!(imported.rows.len(), 1);
    assert_eq!(
        imported.rows[0].get("customer_id").map(String::as_str),
        Some("1")
    );
    assert_eq!(imported.errors.len(), 1);
    assert_eq!(imported.errors[0].0, 4);
}

#[test]
fn batch_row_and_run_batch_isolate_row_errors_build_zip_and_support_cancel() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());

    // form_from_params with nonexistent main_code fails
    let err = form_from_params(
        ctx.store(),
        1,
        1,
        &BTreeMap::from([("main_code".into(), "NO-SUCH-PRODUCT".into())]),
    )
    .expect_err("missing main_code");
    assert!(
        err.user_message()
            .contains("Không tìm thấy gói có mã NO-SUCH-PRODUCT")
    );

    // Compile template directly to test batch_row duplicate filename suffix _2
    let tpl_path = ctx
        .paths()
        .resolve_stored(ctx.store().templates[&1].file_path.as_deref().unwrap());
    let compiled =
        cf_docx::Template::compile(&std::fs::read(&tpl_path).expect("read tpl")).expect("compile");
    let r1 = batch_row(
        &mut ctx,
        "J0",
        ContractForm {
            contract_number: "BATCH-01/2026".into(),
            customer_id: 1,
            product_id: Some(1),
            template_id: Some(1),
            month_count: Some(12),
            ..ContractForm::default()
        },
        2026,
        &compiled,
        true,
    )
    .expect("row 1");
    let r2 = batch_row(
        &mut ctx,
        "J0",
        ContractForm {
            contract_number: "BATCH-01-B/2026".into(),
            customer_id: 1,
            product_id: Some(1),
            template_id: Some(1),
            month_count: Some(12),
            ..ContractForm::default()
        },
        2026,
        &compiled,
        true,
    )
    .expect("row 2");
    assert!(r1.file_path.is_file());
    assert!(r2.file_path.is_file());
    assert_ne!(r1.file_path, r2.file_path);

    // run_batch with 3 rows (1 fails due to missing customer 999) -> 2 succeed + ZIP with 2 files
    let job = BatchJob {
        id: "JOB1".into(),
        template_id: 1,
        year: 2026,
        rows: vec![
            ContractForm {
                contract_number: "B1/2026".into(),
                customer_id: 1,
                product_id: Some(1),
                template_id: Some(1),
                month_count: Some(12),
                ..ContractForm::default()
            },
            ContractForm {
                contract_number: "B-FAIL/2026".into(),
                customer_id: 999,
                product_id: Some(1),
                template_id: Some(1),
                month_count: Some(12),
                ..ContractForm::default()
            },
            ContractForm {
                contract_number: "B2/2026".into(),
                customer_id: 2,
                product_id: Some(1),
                template_id: Some(1),
                month_count: Some(12),
                ..ContractForm::default()
            },
        ],
    };
    let cancel = AtomicBool::new(false);
    let mut progress_steps = Vec::new();
    let res = run_batch(
        &mut ctx,
        job,
        |done, total| progress_steps.push((done, total)),
        &cancel,
    )
    .expect("run_batch");
    assert_eq!(res.succeeded, 2);
    assert_eq!(res.failed, 1);
    assert!(!res.cancelled);
    assert_eq!(progress_steps, vec![(1, 3), (2, 3), (3, 3)]);
    let zip_path = res.zip_path.expect("zip path");
    assert!(zip_path.is_file());
    let archive = ZipArchive::new(std::fs::File::open(zip_path).expect("open zip")).expect("zip");
    assert_eq!(archive.len(), 2);

    // Cancellation after row 1 -> 1 succeeded, cancelled=true
    let cancel_after_one = AtomicBool::new(false);
    let job2 = BatchJob {
        id: "JOB2".into(),
        template_id: 1,
        year: 2026,
        rows: vec![
            ContractForm {
                contract_number: "C1/2026".into(),
                customer_id: 1,
                product_id: Some(1),
                template_id: Some(1),
                month_count: Some(12),
                ..ContractForm::default()
            },
            ContractForm {
                contract_number: "C2/2026".into(),
                customer_id: 2,
                product_id: Some(1),
                template_id: Some(1),
                month_count: Some(12),
                ..ContractForm::default()
            },
        ],
    };
    let res2 = run_batch(
        &mut ctx,
        job2,
        |done, _| {
            if done >= 1 {
                cancel_after_one.store(true, Ordering::Relaxed);
            }
        },
        &cancel_after_one,
    )
    .expect("run_batch cancel");
    assert!(res2.cancelled);
    assert_eq!(res2.succeeded, 1);
}
