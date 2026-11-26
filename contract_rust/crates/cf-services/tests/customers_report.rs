use std::io::Cursor;

use calamine::{DataType, Reader, Xlsx, open_workbook_from_rs};
use cf_core::lifecycle::ContractStatus;
use cf_db::store::Store;
use cf_services::customers::report::export_report;
use chrono::NaiveDate;
use rust_decimal::Decimal;

#[test]
fn export_report_builds_four_sheets_and_matches_non_deleted_customer_count() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed");
    let store = Store::load(db).expect("load");
    let expected_rows = store.customers.values().filter(|c| !c.is_deleted).count();
    let today = NaiveDate::from_ymd_opt(2026, 3, 24).expect("today");

    let bytes = export_report(&store, today).expect("export report");
    let mut wb: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).expect("open workbook");
    assert_eq!(
        wb.sheet_names(),
        &[
            "Ưu tiên hành động",
            "Hồ sơ đầy đủ",
            "Lịch sử hợp đồng",
            "Tóm tắt",
        ]
    );
    let priority = wb
        .worksheet_range("Ưu tiên hành động")
        .expect("priority sheet");
    // 1 header row + expected_rows data rows
    assert_eq!(priority.height(), expected_rows + 1);
}

#[test]
fn export_report_counts_and_labels_modern_active_lifecycle_statuses() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed");
    let mut store = Store::load(db).expect("load");
    let mut contracts = store.contracts.values_mut();
    let invoiced = contracts.next().expect("seeded contract");
    invoiced.status = ContractStatus::Invoiced;
    invoiced.contract_number = "CF-KB09-INVOICED".into();
    invoiced.total_amount = Some(Decimal::from(123));
    let paid_active = contracts.next().expect("second seeded contract");
    paid_active.status = ContractStatus::PaidActive;
    paid_active.contract_number = "CF-KB09-PAID".into();
    paid_active.total_amount = Some(Decimal::from(456));
    for contract in contracts {
        contract.status = ContractStatus::Terminated;
        contract.total_amount = None;
    }

    let bytes = export_report(&store, NaiveDate::from_ymd_opt(2026, 3, 24).expect("today"))
        .expect("export");
    let mut workbook: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).expect("open workbook");
    let history = workbook
        .worksheet_range("Lịch sử hợp đồng")
        .expect("history sheet");
    let invoiced_row = history
        .rows()
        .find(|row| {
            row.get(3)
                .is_some_and(|cell| cell.get_string() == Some("CF-KB09-INVOICED"))
        })
        .expect("invoiced contract row");
    assert_eq!(invoiced_row[5].to_string(), "Đã xuất hóa đơn");
    let paid_row = history
        .rows()
        .find(|row| {
            row.get(3)
                .is_some_and(|cell| cell.get_string() == Some("CF-KB09-PAID"))
        })
        .expect("paid contract row");
    assert_eq!(paid_row[5].to_string(), "Đã thanh toán");

    let summary = workbook.worksheet_range("Tóm tắt").expect("summary sheet");
    let active_total = summary
        .rows()
        .find(|row| {
            row.first().is_some_and(|cell| {
                cell.get_string() == Some("Tổng giá trị HĐ đang hiệu lực (VNĐ)")
            })
        })
        .and_then(|row| row.get(1))
        .expect("active value summary");
    assert_eq!(active_total.get_float(), Some(579.0));
}
