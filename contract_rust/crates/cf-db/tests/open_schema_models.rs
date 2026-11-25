use std::collections::BTreeMap;

use cf_core::lifecycle::ContractStatus;
use cf_db::{
    DbError,
    change::{Change, ChangeValue},
    clock::{Clock, FixedClock},
    model::{
        contract::Contract,
        product::Product,
        setting::{AppSetting, get_setting, party_b},
    },
    open, schema,
    store::Store,
};
use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::{Connection, types::Value};

#[test]
fn open_creates_baseline_seeds_products_once_and_checks_schema() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("fresh.db");

    {
        let conn = open::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM products", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        // Extra column is allowed.
        conn.execute("ALTER TABLE app_settings ADD COLUMN extra_col TEXT", [])
            .unwrap();
        assert!(schema::check_schema(&conn).is_ok());
    }

    // Re-opening must not seed duplicate products.
    let conn2 = open::open(&db_path).unwrap();
    let count2: i64 = conn2
        .query_row("SELECT COUNT(*) FROM products", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count2, 2);

    // Missing required column must fail with Schema error naming it.
    let bad = Connection::open_in_memory().unwrap();
    bad.execute("CREATE TABLE customers (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    let err = schema::check_schema(&bad).unwrap_err();
    assert!(matches!(err, DbError::Schema(_)));
    assert!(err.to_string().contains("customers.code"));
}

#[test]
fn models_handle_invalid_json_unknown_status_and_empty_settings() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../../../fixtures/schema_baseline.sql"))
        .unwrap();
    let ts = "2026-03-24 04:40:37.560192";
    conn.execute(
        "INSERT INTO products (code, name, product_type, default_price, default_vat_rate, default_duration_months, kind, price_period_months, quantity_source, feature_list, is_active, sort_order, created_at, updated_at) VALUES ('P1', 'SP 1', 'Dịch vụ', 1000, 10, 12, 'main', 12, 'units', '{bad json', 1, 0, ?1, ?1)",
        [ts],
    )
    .unwrap();
    let product = conn
        .query_row("SELECT * FROM products WHERE code='P1'", [], |row| {
            Ok(Product::from_row(row))
        })
        .unwrap()
        .unwrap();
    assert!(product.feature_list.is_empty());

    conn.execute(
        "INSERT INTO customers (code, legal_name, customer_type, is_active, is_deleted, bill_main_station, created_at, updated_at) VALUES ('C1', 'Khách 1', 'Khác', 1, 0, 1, ?1, ?1)",
        [ts],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO contracts (contract_number, status, customer_id, pricing_model, doc_kind, created_at, updated_at) VALUES ('01/C1/2026', 'Bogus', 1, 'items', 'contract', ?1, ?1)",
        [ts],
    )
    .unwrap();
    let contract = conn
        .query_row(
            "SELECT * FROM contracts WHERE contract_number='01/C1/2026'",
            [],
            |row| Ok(Contract::from_row(row)),
        )
        .unwrap()
        .unwrap();
    assert_eq!(contract.status, ContractStatus::Unknown("Bogus".into()));

    let settings = vec![
        AppSetting {
            key: "empty_key".into(),
            value: Some("   ".into()),
        },
        AppSetting {
            key: "party_b_name".into(),
            value: Some("Công ty B".into()),
        },
    ];
    assert_eq!(get_setting(&settings, "empty_key", "mac_dinh"), "mac_dinh");
    assert_eq!(party_b(&settings).name, "Công ty B");
}

#[test]
fn store_indexes_and_delete_fk_and_clock_work_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let db = cf_testkit::seeded_db(dir.path()).unwrap();
    let mut store = Store::load(db).unwrap();

    // T3.12: lookup helpers and diacritic-insensitive search_key
    assert!(store.indexes.contract_by_number.contains_key("01/G1/2026"));
    assert!(!store.indexes.contracts_of_customer(&store, 1).is_empty());
    assert_eq!(store.indexes.units_of(&store, 1).len(), 2);
    if let Some(item) = store.items.values().next() {
        assert!(!store.indexes.items_of(&store, item.contract_id).is_empty());
    }

    let ts = "2026-03-24 04:40:37.560192";
    let cust = BTreeMap::from([
        ("code".into(), ChangeValue::Sql(Value::Text("KH-DT".into()))),
        (
            "legal_name".into(),
            ChangeValue::Sql(Value::Text("Trạm Y tế xã Đồng Tâm".into())),
        ),
        (
            "customer_type".into(),
            ChangeValue::Sql(Value::Text("TYT".into())),
        ),
        ("is_active".into(), ChangeValue::Sql(Value::Integer(1))),
        ("is_deleted".into(), ChangeValue::Sql(Value::Integer(0))),
        (
            "bill_main_station".into(),
            ChangeValue::Sql(Value::Integer(1)),
        ),
        (
            "created_at".into(),
            ChangeValue::Sql(Value::Text(ts.into())),
        ),
        (
            "updated_at".into(),
            ChangeValue::Sql(Value::Text(ts.into())),
        ),
    ]);
    let res = store
        .apply(vec![Change::insert("customers", cust)])
        .unwrap();
    let new_cid = res.inserted_ids[0];
    assert!(
        store
            .indexes
            .customer_search_key
            .get(&new_cid)
            .is_some_and(|k| k.contains("dong tam"))
    );

    // T3.13: deleting contract without deleting its events fails FK & keeps RAM intact.
    let target_id = store
        .contracts
        .values()
        .find(|c| !store.indexes.events_of(&store, c.id).is_empty())
        .map(|c| c.id);
    if let Some(cid) = target_id {
        let count_before = store.contracts.len();
        assert!(store.apply(vec![Change::delete("contracts", cid)]).is_err());
        assert_eq!(store.contracts.len(), count_before);

        let mut changes: Vec<Change> = store
            .indexes
            .events_of(&store, cid)
            .into_iter()
            .map(|e| Change::delete("contract_events", e.id))
            .collect();
        changes.push(Change::delete("contracts", cid));
        store.apply(changes).unwrap();
        assert!(!store.contracts.contains_key(&cid));
        assert!(store.indexes.items_of(&store, cid).is_empty());
    }

    // T3.15: FixedClock
    let fixed_dt = NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S%.6f").unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 3, 24).unwrap();
    let clock = FixedClock {
        date,
        utc: fixed_dt,
        local: fixed_dt,
    };
    assert_eq!(clock.today(), date);
    assert_eq!(clock.now_utc(), fixed_dt);
    assert_eq!(clock.now_local(), fixed_dt);
}
