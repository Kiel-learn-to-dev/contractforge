use std::{collections::BTreeMap, env, process::ExitCode};

use cf_db::{
    change::{Change, ChangeValue},
    store::Store,
};
use rusqlite::types::Value;

fn fields(values: &[(&str, Value)]) -> BTreeMap<String, ChangeValue> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), ChangeValue::Sql(value.clone())))
        .collect()
}

fn check_token(store: &Store, token: &str, python_written: bool) -> Result<(), String> {
    let customer = store
        .customers
        .values()
        .find(|row| row.code == format!("CF-COMPAT-{token}"))
        .ok_or_else(|| format!("Không tìm thấy khách kiểm tương thích {token}."))?;
    let contract = store
        .contracts
        .values()
        .find(|row| row.contract_number == format!("CF-COMPAT-{token}"))
        .ok_or_else(|| format!("Không tìm thấy hợp đồng kiểm tương thích {token}."))?;
    if contract.customer_id != customer.id {
        return Err("Liên kết khách/hợp đồng không khớp sau khi nạp Store.".into());
    }
    let expected_customer = if python_written {
        "Khách Python kiểm thử".to_owned()
    } else {
        format!("Khách kiểm thử {token}")
    };
    let expected_note = (!python_written).then_some("updated-by-rust");
    if customer.legal_name != expected_customer || contract.notes.as_deref() != expected_note {
        return Err("Tên khách hoặc ghi chú hợp đồng không khớp sau khi nạp Store.".into());
    }
    let item = store
        .items
        .values()
        .find(|row| row.contract_id == contract.id)
        .ok_or("Thiếu dòng hàng sau khi nạp Store.")?;
    let event = store
        .events
        .values()
        .find(|row| row.contract_id == contract.id)
        .ok_or("Thiếu sự kiện sau khi nạp Store.")?;
    let expected_item = if python_written {
        "Python compatibility row"
    } else {
        "Compatibility row"
    };
    let expected_event = if python_written {
        "python_compatibility_check"
    } else {
        "compatibility_check"
    };
    if item.description != expected_item || event.event_type != expected_event {
        return Err("Dòng hàng hoặc sự kiện không khớp sau khi nạp Store.".into());
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let database = args
        .next()
        .ok_or("Cách dùng: cf-db-compat <db-copy> <token> [--verify]")?;
    let token = args.next().ok_or("Thiếu mã phiên kiểm tương thích.")?;
    let verify = args.next().as_deref() == Some("--verify");
    let mut store = Store::load(database).map_err(|error| error.to_string())?;
    if verify {
        check_token(&store, &token, true)?;
        println!("RUST VERIFY OK {token}");
        return Ok(());
    }

    let now = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S%.6f")
        .to_string();
    let number = format!("CF-COMPAT-{token}");
    let ref_value = |index| ChangeValue::Ref(index);
    let changes = vec![
        Change::insert(
            "customers",
            fields(&[
                ("code", Value::Text(number.clone())),
                ("legal_name", Value::Text(format!("Khách kiểm thử {token}"))),
                ("customer_type", Value::Text("Khác".into())),
                ("is_active", Value::Integer(1)),
                ("is_deleted", Value::Integer(0)),
                ("bill_main_station", Value::Integer(1)),
                ("created_at", Value::Text(now.clone())),
                ("updated_at", Value::Text(now.clone())),
            ]),
        ),
        Change::insert("contracts", {
            let mut values = fields(&[
                ("contract_number", Value::Text(number.clone())),
                ("status", Value::Text("Draft".into())),
                ("pricing_model", Value::Text("legacy".into())),
                ("doc_kind", Value::Text("contract".into())),
                ("created_at", Value::Text(now.clone())),
                ("updated_at", Value::Text(now.clone())),
            ]);
            values.insert("customer_id".into(), ref_value(0));
            values
        }),
        Change::insert("contract_items", {
            let mut values = fields(&[
                ("sort_order", Value::Integer(0)),
                ("description", Value::Text("Compatibility row".into())),
                ("quantity", Value::Text("1".into())),
                ("period_count", Value::Text("1".into())),
                ("vat_rate", Value::Text("0".into())),
                ("amount_pre_vat", Value::Integer(1)),
                ("vat_amount", Value::Integer(0)),
                ("amount_total", Value::Integer(1)),
            ]);
            values.insert("contract_id".into(), ref_value(1));
            values
        }),
        Change::insert("contract_events", {
            let mut values = fields(&[
                ("event_type", Value::Text("compatibility_check".into())),
                (
                    "description",
                    Value::Text("Created by Rust compatibility check".into()),
                ),
                ("occurred_at", Value::Text(now.clone())),
            ]);
            values.insert("contract_id".into(), ref_value(1));
            values
        }),
    ];
    let result = store.apply(changes).map_err(|error| error.to_string())?;
    let contract_id = result
        .inserted_ids
        .get(1)
        .copied()
        .ok_or("Thiếu id hợp đồng.")?;
    store
        .apply(vec![Change::update(
            "contracts",
            contract_id,
            fields(&[("notes", Value::Text("updated-by-rust".into()))]),
        )])
        .map_err(|error| error.to_string())?;
    check_token(&store, &token, false)?;
    if store
        .contracts
        .get(&contract_id)
        .and_then(|row| row.notes.as_deref())
        != Some("updated-by-rust")
    {
        return Err("Không đọc lại được trường hợp đồng vừa cập nhật.".into());
    }
    println!("RUST WRITE OK {token}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cf-db-compat: {error}");
            ExitCode::FAILURE
        }
    }
}
