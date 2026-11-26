use std::collections::BTreeMap;

use cf_db::store::Store;
use cf_services::customers::edit::{create_customer, generate_next_code, update_customer};
use chrono::NaiveDateTime;
use serde_json::Value as Json;

fn seeded_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed");
    (dir, Store::load(db).expect("load"))
}

fn now() -> NaiveDateTime {
    NaiveDateTime::parse_from_str("2026-03-24 10:00:00", "%Y-%m-%d %H:%M:%S").expect("dt")
}

#[test]
fn generate_next_code_ignores_non_numeric_suffixes() {
    let (_dir, mut store) = seeded_store();
    for code in ["KH-001", "KH-002", "KH-TEST"] {
        let data = BTreeMap::from([
            ("code".into(), Json::String(code.into())),
            ("legal_name".into(), Json::String(format!("Khách {code}"))),
            ("customer_type".into(), Json::String("Trạm y tế".into())),
        ]);
        create_customer(&mut store, &data, now()).expect("create customer");
    }
    assert_eq!(generate_next_code(&store), "KH-003");
}

#[test]
fn create_rejects_duplicate_code_and_update_preserves_code_when_blank() {
    let (_dir, mut store) = seeded_store();
    let dup = BTreeMap::from([
        ("code".into(), Json::String("KH-FAKE-001".into())),
        ("legal_name".into(), Json::String("Trùng mã".into())),
        ("customer_type".into(), Json::String("Trường học".into())),
    ]);
    let err = create_customer(&mut store, &dup, now()).expect_err("duplicate code");
    assert_eq!(
        err.user_message(),
        "Mã khách hàng 'KH-FAKE-001' đã tồn tại."
    );

    let data = BTreeMap::from([
        ("code".into(), Json::String("   ".into())),
        ("legal_name".into(), Json::String("  Tên mới  ".into())),
        ("short_name".into(), Json::String("   ".into())),
        (
            "establishment_date".into(),
            Json::String("15/02/2020".into()),
        ),
        ("unknown_field".into(), Json::String("ignored".into())),
    ]);
    update_customer(&mut store, 1, &data, now()).expect("update customer");

    let updated = store.customers.get(&1).expect("customer 1");
    assert_eq!(updated.code, "KH-FAKE-001");
    assert_eq!(updated.legal_name, "Tên mới");
    assert_eq!(updated.short_name, None);
    assert_eq!(
        updated.establishment_date,
        chrono::NaiveDate::from_ymd_opt(2020, 2, 15)
    );
}
