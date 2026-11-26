use std::collections::BTreeMap;

use cf_core::dates::parse_date_input;
use cf_db::{
    change::{Change, ChangeValue},
    codec,
    store::Store,
};
use rusqlite::types::Value;
use serde_json::Value as Json;

use crate::ServiceError;

const FIELDS: &[&str] = &[
    "code",
    "legal_name",
    "short_name",
    "customer_type",
    "address",
    "ward",
    "district",
    "province",
    "tax_code",
    "representative_name",
    "representative_title",
    "bank_account",
    "bank_name",
    "beneficiary_name",
    "contact_name",
    "phone",
    "email",
    "notes",
    "is_active",
    "budget_relation_code",
    "kcb_code",
    "establishment_decision",
    "establishment_date",
    "representative_gender",
];

pub fn generate_next_code(store: &Store) -> String {
    let max = store
        .customers
        .values()
        .filter_map(|customer| customer.code.strip_prefix("KH-")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    format!("KH-{:03}", max + 1)
}

pub(crate) fn clean(
    data: &BTreeMap<String, Json>,
) -> Result<BTreeMap<String, ChangeValue>, ServiceError> {
    let mut values = BTreeMap::new();
    for (key, value) in data {
        if !FIELDS.contains(&key.as_str()) {
            continue;
        }
        let value = match key.as_str() {
            "is_active" => {
                let active = match value {
                    Json::Bool(v) => *v,
                    Json::String(v) => {
                        !["false", "0", "không", "no"].contains(&v.trim().to_lowercase().as_str())
                    }
                    Json::Number(v) => v.as_i64().unwrap_or(0) != 0,
                    Json::Null => false,
                    _ => {
                        return Err(ServiceError::Invalid(
                            "Giá trị trạng thái không hợp lệ.".into(),
                        ));
                    }
                };
                Value::Integer(i64::from(active))
            }
            "establishment_date" => {
                let raw = value.as_str().unwrap_or_default();
                codec::write_date(parse_date_input(Some(raw))?)
            }
            _ => {
                let raw = match value {
                    Json::String(v) => v.clone(),
                    Json::Null => String::new(),
                    Json::Number(v) => v.to_string(),
                    Json::Bool(v) => v.to_string(),
                    _ => {
                        return Err(ServiceError::Invalid(format!(
                            "Trường '{key}' không hợp lệ."
                        )));
                    }
                };
                let raw = raw.trim();
                if raw.is_empty() {
                    Value::Null
                } else {
                    Value::Text(raw.to_owned())
                }
            }
        };
        values.insert(key.clone(), ChangeValue::Sql(value));
    }
    Ok(values)
}

pub fn create_customer(
    store: &mut Store,
    data: &BTreeMap<String, Json>,
    now: chrono::NaiveDateTime,
) -> Result<i64, ServiceError> {
    let mut values = clean(data)?;
    let code = values
        .get("code")
        .and_then(|v| match v {
            ChangeValue::Sql(Value::Text(v)) => Some(v.clone()),
            _ => None,
        })
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| generate_next_code(store));
    if store.indexes.customer_by_code.contains_key(&code) {
        return Err(ServiceError::Invalid(format!(
            "Mã khách hàng '{code}' đã tồn tại."
        )));
    }
    values.insert("code".into(), ChangeValue::Sql(Value::Text(code)));
    for required in ["legal_name", "customer_type"] {
        if !values
            .get(required)
            .is_some_and(|v| matches!(v, ChangeValue::Sql(Value::Text(s)) if !s.is_empty()))
        {
            return Err(ServiceError::Invalid(
                if required == "legal_name" {
                    "Vui lòng nhập tên pháp lý."
                } else {
                    "Vui lòng chọn loại khách hàng."
                }
                .into(),
            ));
        }
    }
    values.insert(
        "is_active".into(),
        values
            .get("is_active")
            .cloned()
            .unwrap_or(ChangeValue::Sql(Value::Integer(1))),
    );
    values.insert("is_deleted".into(), ChangeValue::Sql(Value::Integer(0)));
    values.insert(
        "bill_main_station".into(),
        ChangeValue::Sql(Value::Integer(1)),
    );
    values.insert(
        "created_at".into(),
        ChangeValue::Sql(codec::write_datetime(Some(now))),
    );
    values.insert(
        "updated_at".into(),
        ChangeValue::Sql(codec::write_datetime(Some(now))),
    );
    let result = store.apply(vec![Change::insert("customers", values)])?;
    result
        .inserted_ids
        .first()
        .copied()
        .ok_or_else(|| ServiceError::Invalid("Không tạo được khách hàng.".into()))
}

pub fn update_customer(
    store: &mut Store,
    id: i64,
    data: &BTreeMap<String, Json>,
    now: chrono::NaiveDateTime,
) -> Result<(), ServiceError> {
    let customer = store
        .customers
        .get(&id)
        .ok_or_else(|| ServiceError::NotFound(format!("Không tìm thấy khách hàng id={id}")))?;
    let mut values = clean(data)?;
    match values.get("code") {
        Some(ChangeValue::Sql(Value::Null)) => {
            values.remove("code");
        }
        Some(ChangeValue::Sql(Value::Text(code))) if code.is_empty() => {
            values.remove("code");
        }
        Some(ChangeValue::Sql(Value::Text(code)))
            if store
                .indexes
                .customer_by_code
                .get(code)
                .is_some_and(|existing| *existing != id) =>
        {
            return Err(ServiceError::Invalid(format!(
                "Mã '{code}' đã được dùng bởi khách hàng khác."
            )));
        }
        _ => {}
    }
    if values.is_empty() {
        return Ok(());
    }
    values.insert(
        "updated_at".into(),
        ChangeValue::Sql(codec::write_datetime(Some(now))),
    );
    let _ = customer;
    store.apply(vec![Change::update("customers", id, values)])?;
    Ok(())
}
