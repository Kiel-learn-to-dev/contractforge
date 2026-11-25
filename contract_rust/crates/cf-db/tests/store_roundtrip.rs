use std::collections::BTreeMap;

use cf_db::{
    change::{Change, ChangeValue},
    store::Store,
};
use rusqlite::{Connection, types::Value};

fn sql_fields(values: &[(&str, Value)]) -> BTreeMap<String, ChangeValue> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), ChangeValue::Sql(value.clone())))
        .collect()
}

fn temporary_store() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let database = cf_testkit::seeded_db(directory.path()).expect("seed fictional database");
    let store = Store::load(database).expect("load seeded store");
    (directory, store)
}

#[test]
fn seeded_store_loads_all_fake_entities_and_indexes() {
    let (_directory, store) = temporary_store();
    assert_eq!(store.customers.len(), 3);
    assert_eq!(store.units.len(), 2);
    assert_eq!(store.products.len(), 3);
    assert_eq!(store.templates.len(), 4);
    assert_eq!(store.contracts.len(), 5);
    assert_eq!(store.indexes.contract_by_number.len(), 5);
    assert_eq!(
        store.indexes.units_by_customer.get(&1).map(Vec::len),
        Some(2)
    );
}

#[test]
fn apply_inserts_referenced_rows_and_rolls_back_whole_batch_on_error() {
    let (_directory, mut store) = temporary_store();
    let timestamp = "2026-03-24 04:40:37.560192";
    let mut contract = sql_fields(&[
        ("contract_number", Value::Text("06/G1/2026".into())),
        ("status", Value::Text("Draft".into())),
        ("pricing_model", Value::Text("items".into())),
        ("doc_kind", Value::Text("contract".into())),
        ("created_at", Value::Text(timestamp.into())),
        ("updated_at", Value::Text(timestamp.into())),
    ]);
    contract.insert("customer_id".into(), ChangeValue::Sql(Value::Integer(1)));
    let mut item = sql_fields(&[
        ("sort_order", Value::Integer(0)),
        ("description", Value::Text("Hàng kiểm tra".into())),
        ("quantity", Value::Text("1".into())),
        ("period_count", Value::Text("1".into())),
        ("vat_rate", Value::Text("10".into())),
        ("amount_pre_vat", Value::Integer(100)),
        ("vat_amount", Value::Integer(10)),
        ("amount_total", Value::Integer(110)),
    ]);
    item.insert("contract_id".into(), ChangeValue::Ref(0));
    let mut event = sql_fields(&[
        ("event_type", Value::Text("created".into())),
        ("occurred_at", Value::Text(timestamp.into())),
    ]);
    event.insert("contract_id".into(), ChangeValue::Ref(0));

    let result = store
        .apply(vec![
            Change::insert("contracts", contract),
            Change::insert("contract_items", item),
            Change::insert("contract_events", event),
        ])
        .expect("insert related rows in one transaction");
    assert_eq!(result.inserted_ids.len(), 3);
    let contract_id = result.inserted_ids[0];
    assert!(store.contracts.contains_key(&contract_id));
    assert!(
        store
            .items
            .values()
            .any(|row| row.contract_id == contract_id)
    );
    assert!(
        store
            .events
            .values()
            .any(|row| row.contract_id == contract_id)
    );

    let customers_before = store.customers.len();
    let new_customer = sql_fields(&[
        ("code", Value::Text("KH-ROLLBACK".into())),
        ("legal_name", Value::Text("Khách rollback".into())),
        ("customer_type", Value::Text("Khác".into())),
        ("is_active", Value::Integer(1)),
        ("is_deleted", Value::Integer(0)),
        ("bill_main_station", Value::Integer(1)),
        ("created_at", Value::Text(timestamp.into())),
        ("updated_at", Value::Text(timestamp.into())),
    ]);
    let duplicate_contract = sql_fields(&[
        ("contract_number", Value::Text("01/G1/2026".into())),
        ("status", Value::Text("Draft".into())),
        ("customer_id", Value::Integer(1)),
        ("pricing_model", Value::Text("items".into())),
        ("doc_kind", Value::Text("contract".into())),
        ("created_at", Value::Text(timestamp.into())),
        ("updated_at", Value::Text(timestamp.into())),
    ]);
    assert!(
        store
            .apply(vec![
                Change::insert("customers", new_customer),
                Change::insert("contracts", duplicate_contract),
            ])
            .is_err()
    );
    assert_eq!(store.customers.len(), customers_before);
    assert!(
        !store
            .customers
            .values()
            .any(|row| row.code == "KH-ROLLBACK")
    );
}

#[test]
fn apply_detects_external_connection_and_reloads_store() {
    let (directory, mut store) = temporary_store();
    let database = directory.path().join("contract_manager.db");
    let other = Connection::open(database).expect("open second connection");
    other
        .execute(
            "INSERT INTO customers (code, legal_name, customer_type, is_active, is_deleted, bill_main_station, created_at, updated_at) VALUES ('KH-EXTERNAL', 'Khách ghi ngoài', 'Khác', 1, 0, 1, '2026-03-24 04:40:37', '2026-03-24 04:40:37')",
            [],
        )
        .expect("external insert");

    let result = store.apply(Vec::new()).expect("detect external write");
    assert!(result.reloaded);
    assert!(
        store
            .customers
            .values()
            .any(|row| row.code == "KH-EXTERNAL")
    );
}
