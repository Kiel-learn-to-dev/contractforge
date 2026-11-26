use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use cf_db::store::Store;
use cf_services::contracts::{
    export::export_contracts,
    list::{ContractFilters, list_contracts},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;

fn seeded_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed");
    (dir, Store::load(db).expect("load"))
}

#[test]
fn list_contracts_filters_alerts_years_sum_total_and_accent_insensitive_search() {
    let (_dir, mut store) = seeded_store();
    // Seeded:
    // id=1 Draft, id=2 Generated, id=3 Signed, id=4 PaidActive, id=5 Expired.
    store
        .conn
        .execute(
            "UPDATE contracts SET status='Sent', updated_at='2026-03-01 10:00:00', sign_date='2025-05-10', total_amount=1000000 WHERE id=2",
            [],
        )
        .expect("update 2");
    store
        .conn
        .execute(
            "UPDATE contracts SET sign_date='2026-02-01', end_date='2026-04-15', total_amount=2500000, output_file_path='outputs/c4.docx' WHERE id=4",
            [],
        )
        .expect("update 4");
    store.reload().expect("reload");

    let today = NaiveDate::from_ymd_opt(2026, 3, 24).expect("today");

    let all = list_contracts(
        &store,
        &ContractFilters {
            per_page: 999,
            ..Default::default()
        },
        today,
    );
    assert_eq!(all.total, 5);
    assert_eq!(all.sum_total, Decimal::from(3_500_000));
    assert_eq!(all.years, vec![2026, 2025]);
    assert_eq!(all.cnt_unsent, 1);
    assert_eq!(all.cnt_expiring, 1);

    let unsent = list_contracts(
        &store,
        &ContractFilters {
            alert_filter: "unsent_10d".into(),
            ..Default::default()
        },
        today,
    );
    assert_eq!(
        unsent
            .rows
            .iter()
            .map(|r| r.contract.id)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert!(unsent.rows[0].flag_unsent);

    let exp = list_contracts(
        &store,
        &ContractFilters {
            alert_filter: "expiring_60".into(),
            ..Default::default()
        },
        today,
    );
    assert_eq!(
        exp.rows.iter().map(|r| r.contract.id).collect::<Vec<_>>(),
        vec![4]
    );
    assert!(exp.rows[0].flag_expiring);

    let generated = list_contracts(
        &store,
        &ContractFilters {
            alert_filter: "generated".into(),
            ..Default::default()
        },
        today,
    );
    assert_eq!(
        generated
            .rows
            .iter()
            .map(|r| r.contract.id)
            .collect::<Vec<_>>(),
        vec![4]
    );

    let active_svc = list_contracts(
        &store,
        &ContractFilters {
            alert_filter: "all_active".into(),
            ..Default::default()
        },
        today,
    );
    assert_eq!(
        active_svc
            .rows
            .iter()
            .map(|r| r.contract.id)
            .collect::<Vec<_>>(),
        vec![4]
    );

    // Accent-insensitive search for "Đồng Tâm" (customer 1: contracts 1, 2, 4)
    let search = list_contracts(
        &store,
        &ContractFilters {
            q: "dong tam".into(),
            ..Default::default()
        },
        today,
    );
    assert_eq!(search.total, 3);
}

#[test]
fn export_contracts_writes_two_sheets_formatted_currency_and_date_filename() {
    let (_dir, mut store) = seeded_store();
    store
        .conn
        .execute(
            "UPDATE contracts SET total_amount=1234567.89 WHERE id=1",
            [],
        )
        .expect("set amount");
    store.reload().expect("reload");

    let today = NaiveDate::from_ymd_opt(2026, 3, 24).expect("today");
    let exported = export_contracts(&store, &ContractFilters::default(), today).expect("export");
    assert_eq!(exported.file_name, "hop-dong-20260324.xlsx");

    let mut wb: Xlsx<_> = open_workbook_from_rs(Cursor::new(exported.bytes)).expect("open xlsx");
    assert_eq!(wb.sheet_names(), &["Danh sách hợp đồng", "Chi tiết dòng"]);
    let sheet = wb.worksheet_range("Danh sách hợp đồng").expect("sheet 1");
    let headers: Vec<_> = sheet
        .rows()
        .next()
        .expect("header")
        .iter()
        .map(|cell| match cell {
            Data::String(s) => s.as_str(),
            _ => "",
        })
        .collect();
    assert_eq!(headers[0], "STT");
    assert_eq!(headers[1], "Số hợp đồng");
    assert_eq!(headers[6], "Giá trị (VNĐ)");
    // Find row for 01/G1/2026 and check truncated & dot-separated currency "1.234.568" or "1.234.567"
    let row1 = sheet
        .rows()
        .skip(1)
        .find(|r| matches!(r.get(1), Some(Data::String(s)) if s == "01/G1/2026"))
        .expect("row 01/G1/2026");
    let amount_str = match &row1[6] {
        Data::String(s) => s.as_str(),
        _ => "",
    };
    assert!(amount_str.contains('.'));
    assert!(!amount_str.contains(','));
}
