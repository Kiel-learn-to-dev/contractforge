use cf_db::store::Store;
use cf_services::customers::units::{UnitInput, save_billing, save_sub_units};
use chrono::NaiveDateTime;

fn seeded_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed");
    (dir, Store::load(db).expect("load"))
}

fn now() -> NaiveDateTime {
    NaiveDateTime::parse_from_str("2026-03-24 10:00:00", "%Y-%m-%d %H:%M:%S").expect("dt")
}

#[test]
fn save_sub_units_and_billing_updates_total_units_and_names() {
    let (_dir, mut store) = seeded_store();
    // Seed có id=1 ("Điểm mẫu A") và id=2 ("Điểm mẫu B") cho customer_id=1.
    // Giữ lại id=1 (đổi tên), bỏ id=2, thêm 1 đơn vị mới.
    save_sub_units(
        &mut store,
        1,
        &[
            UnitInput {
                id: Some(1),
                name: "Điểm A mới".into(),
                unit_type: "Điểm trạm".into(),
                address: "Số 1".into(),
                notes: "".into(),
                include_in_billing: Some(true),
            },
            UnitInput {
                id: None,
                name: "Điểm C".into(),
                unit_type: "".into(),
                address: "".into(),
                notes: "".into(),
                include_in_billing: None,
            },
        ],
        now(),
    )
    .expect("save sub units");

    let units: Vec<_> = store
        .units
        .values()
        .filter(|u| u.customer_id == 1)
        .collect();
    assert_eq!(units.len(), 2);
    assert!(units.iter().any(|u| u.id == 1 && u.name == "Điểm A mới"));
    assert!(
        units
            .iter()
            .any(|u| u.name == "Điểm C" && u.unit_type == "Điểm trạm")
    );
    assert!(!units.iter().any(|u| u.name == "Điểm mẫu B"));

    // Tắt bill_main_station, chỉ giữ id=1 trong billing
    save_billing(&mut store, 1, false, &[1], now()).expect("save billing");
    let customer = &store.customers[&1];
    assert!(!customer.bill_main_station);
    let billed_units: Vec<_> = store
        .units
        .values()
        .filter(|u| u.customer_id == 1 && u.is_active && u.include_in_billing)
        .map(|u| u.name.clone())
        .collect();
    let total_units = usize::from(customer.bill_main_station) + billed_units.len();
    assert_eq!(total_units, 1);
    assert_eq!(billed_units, vec!["Điểm A mới".to_string()]);
}
