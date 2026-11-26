use cf_db::{
    change::{Change, ChangeValue},
    codec,
    store::Store,
};
use rusqlite::types::Value;

use crate::ServiceError;

#[derive(Debug, Clone, Default)]
pub struct UnitInput {
    pub id: Option<i64>,
    pub name: String,
    pub unit_type: String,
    pub address: String,
    pub notes: String,
    pub include_in_billing: Option<bool>,
}

pub fn save_sub_units(
    store: &mut Store,
    customer_id: i64,
    input: &[UnitInput],
    now: chrono::NaiveDateTime,
) -> Result<(), ServiceError> {
    if !store.customers.contains_key(&customer_id) {
        return Err(ServiceError::NotFound(format!(
            "Không tìm thấy khách hàng id={customer_id}"
        )));
    }
    let existing = store
        .indexes
        .units_by_customer
        .get(&customer_id)
        .cloned()
        .unwrap_or_default();
    let submitted = input
        .iter()
        .filter_map(|unit| unit.id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut changes = existing
        .iter()
        .filter(|id| !submitted.contains(id))
        .map(|id| Change::delete("customer_units", *id))
        .collect::<Vec<_>>();
    for (order, unit) in input.iter().enumerate() {
        let name = unit.name.trim();
        if name.is_empty() {
            continue;
        }
        let id = unit.id.filter(|id| {
            store
                .units
                .get(id)
                .is_some_and(|existing| existing.customer_id == customer_id)
        });
        let mut values: std::collections::BTreeMap<String, ChangeValue> = [
            (
                "customer_id".to_owned(),
                ChangeValue::Sql(Value::Integer(customer_id)),
            ),
            (
                "name".to_owned(),
                ChangeValue::Sql(Value::Text(name.to_owned())),
            ),
            (
                "unit_type".to_owned(),
                ChangeValue::Sql(Value::Text(if unit.unit_type.trim().is_empty() {
                    "Điểm trạm".into()
                } else {
                    unit.unit_type.trim().to_owned()
                })),
            ),
            (
                "address".to_owned(),
                ChangeValue::Sql(codec::opt_text(Some(unit.address.trim()))),
            ),
            (
                "notes".to_owned(),
                ChangeValue::Sql(codec::opt_text(Some(unit.notes.trim()))),
            ),
            ("is_active".to_owned(), ChangeValue::Sql(Value::Integer(1))),
            (
                "sort_order".to_owned(),
                ChangeValue::Sql(Value::Integer(order as i64)),
            ),
            (
                "updated_at".to_owned(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            ),
        ]
        .into();
        if let Some(id) = id {
            values.remove("customer_id");
            if let Some(included) = unit.include_in_billing {
                values.insert(
                    "include_in_billing".into(),
                    ChangeValue::Sql(Value::Integer(i64::from(included))),
                );
            }
            changes.push(Change::update("customer_units", id, values));
        } else {
            values.insert(
                "include_in_billing".into(),
                ChangeValue::Sql(Value::Integer(i64::from(
                    unit.include_in_billing.unwrap_or(true),
                ))),
            );
            values.insert(
                "created_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            );
            changes.push(Change::insert("customer_units", values));
        }
    }
    store.apply(changes)?;
    Ok(())
}

pub fn save_billing(
    store: &mut Store,
    customer_id: i64,
    bill_main: bool,
    included_unit_ids: &[i64],
    now: chrono::NaiveDateTime,
) -> Result<(), ServiceError> {
    if !store.customers.contains_key(&customer_id) {
        return Err(ServiceError::NotFound(format!(
            "Không tìm thấy khách hàng id={customer_id}"
        )));
    }
    let included = included_unit_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut changes = vec![Change::update(
        "customers",
        customer_id,
        [
            (
                "bill_main_station".into(),
                ChangeValue::Sql(Value::Integer(i64::from(bill_main))),
            ),
            (
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            ),
        ]
        .into(),
    )];
    if let Some(ids) = store.indexes.units_by_customer.get(&customer_id) {
        for id in ids {
            changes.push(Change::update(
                "customer_units",
                *id,
                [
                    (
                        "include_in_billing".into(),
                        ChangeValue::Sql(Value::Integer(i64::from(included.contains(id)))),
                    ),
                    (
                        "updated_at".into(),
                        ChangeValue::Sql(codec::write_datetime(Some(now))),
                    ),
                ]
                .into(),
            ));
        }
    }
    store.apply(changes)?;
    Ok(())
}
