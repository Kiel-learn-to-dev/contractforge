use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    customers::{
        export_list::make_customer_import_template_xlsx, import_flat::import_flat,
        import_grouped::import_grouped_from_excel,
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
fn export_template_has_expected_headers_in_order() {
    let bytes = make_customer_import_template_xlsx().expect("make template");
    let mut wb: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).expect("open template");
    let range = wb.worksheet_range_at(0).expect("sheet 0").expect("range");
    let headers = range
        .rows()
        .next()
        .expect("header row")
        .iter()
        .map(|cell| match cell {
            Data::String(s) => s.as_str(),
            _ => "",
        })
        .collect::<Vec<_>>();
    assert_eq!(
        headers,
        vec![
            "Mã khách hàng",
            "Tên pháp lý",
            "Tên ngắn",
            "Loại đơn vị",
            "Địa chỉ",
            "Xã / Phường / Đặc khu",
            "Tỉnh / Thành phố",
            "Mã số thuế",
            "Mã cơ sở KCB",
            "Số QĐ thành lập",
            "Ngày thành lập",
            "Đại diện pháp lý",
            "Giới tính đại diện",
            "Chức vụ",
            "Số tài khoản",
            "Ngân hàng",
            "Tên thụ hưởng",
            "Đầu mối liên hệ",
            "Điện thoại",
            "Email",
            "Ghi chú",
        ]
    );
}

#[test]
fn import_flat_creates_updates_and_reports_row_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (col, title) in ["Mã khách hàng", "Tên pháp lý", "Loại đơn vị"]
        .iter()
        .enumerate()
    {
        ws.write_string(0, col as u16, *title).expect("header");
    }
    // Dòng 2: tạo mới (không mã -> tự sinh)
    ws.write_string(1, 1, "Khách hàng Mới").expect("r2 name");
    ws.write_string(1, 2, "Bệnh viện").expect("r2 type");
    // Dòng 3: cập nhật KH-FAKE-001
    ws.write_string(2, 0, "KH-FAKE-001").expect("r3 code");
    ws.write_string(2, 1, "Đồng Tâm Cập Nhật").expect("r3 name");
    ws.write_string(2, 2, "Trường học").expect("r3 type");
    // Dòng 4: lỗi thiếu tên pháp lý
    ws.write_string(3, 0, "KH-ERR").expect("r4 code");
    ws.write_string(3, 2, "Trạm y tế").expect("r4 type");

    let bytes = wb.save_to_buffer().expect("save xlsx");
    let result = import_flat(&mut ctx, &bytes).expect("import flat");
    assert_eq!(result.created, 1);
    assert_eq!(result.updated, 1);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].0, 4);
    assert_eq!(ctx.store().customers[&1].legal_name, "Đồng Tâm Cập Nhật");
    assert!(
        ctx.store()
            .customers
            .values()
            .any(|c| c.legal_name == "Khách hàng Mới")
    );
}

#[test]
fn import_grouped_creates_stations_and_deactivates_missing_sub_unit_on_reimport() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());

    let build_grouped_xlsx = |include_point_b2: bool| -> Vec<u8> {
        let mut wb = Workbook::new();
        let ws = wb.add_worksheet();
        for (col, title) in ["STT", "Đơn vị", "Loại"].iter().enumerate() {
            ws.write_string(0, col as u16, *title).expect("header");
        }
        let mut rows = vec![
            ("1", "Trạm y tế Xã A", "Trạm y tế"),
            ("1.1", "Điểm trạm A1", "Điểm trạm"),
            ("2", "Trạm y tế Xã B", "Trạm y tế"),
            ("2.1", "Điểm trạm B1", "Điểm trạm"),
        ];
        if include_point_b2 {
            rows.push(("2.2", "Điểm trạm B2", "Điểm trạm"));
        }
        for (idx, (stt, name, kind)) in rows.into_iter().enumerate() {
            let r = (idx + 1) as u32;
            ws.write_string(r, 0, stt).expect("stt");
            ws.write_string(r, 1, name).expect("name");
            ws.write_string(r, 2, kind).expect("kind");
        }
        wb.save_to_buffer().expect("save grouped xlsx")
    };

    let first = import_grouped_from_excel(&mut ctx, &build_grouped_xlsx(true), None)
        .expect("first grouped import");
    assert_eq!(first.total_customers, 2);
    assert_eq!(first.created, 2);
    assert_eq!(first.total_units, 3);

    let station_b = ctx
        .store()
        .customers
        .values()
        .find(|c| c.legal_name == "Trạm y tế Xã B")
        .expect("station B")
        .id;
    let b2_before = ctx
        .store()
        .units
        .values()
        .find(|u| u.customer_id == station_b && u.name == "Điểm trạm B2")
        .expect("point B2");
    assert!(b2_before.is_active);

    // Nhập lại bỏ Điểm trạm B2 -> is_active=false
    let second = import_grouped_from_excel(&mut ctx, &build_grouped_xlsx(false), None)
        .expect("second grouped import");
    assert_eq!(second.updated, 2);
    let b2_after = ctx
        .store()
        .units
        .values()
        .find(|u| u.customer_id == station_b && u.name == "Điểm trạm B2")
        .expect("point B2 after");
    assert!(!b2_after.is_active);
}
