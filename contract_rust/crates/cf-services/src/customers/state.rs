use cf_db::{
    change::{Change, ChangeValue},
    codec,
    store::Store,
};
use rusqlite::types::Value;

use crate::ServiceError;

pub fn toggle_active(
    store: &mut Store,
    id: i64,
    now: chrono::NaiveDateTime,
) -> Result<bool, ServiceError> {
    let customer = store
        .customers
        .get(&id)
        .ok_or_else(|| ServiceError::NotFound(format!("Không tìm thấy khách hàng id={id}")))?;
    let active = !customer.is_active;
    store.apply(vec![Change::update(
        "customers",
        id,
        [
            (
                "is_active".into(),
                ChangeValue::Sql(Value::Integer(i64::from(active))),
            ),
            (
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            ),
        ]
        .into(),
    )])?;
    Ok(active)
}

pub fn delete_customer(
    store: &mut Store,
    id: i64,
    now: chrono::NaiveDateTime,
) -> Result<bool, ServiceError> {
    let Some(customer) = store.customers.get(&id) else {
        return Ok(false);
    };
    if customer.is_deleted {
        return Ok(false);
    }
    store.apply(vec![Change::update(
        "customers",
        id,
        [
            ("is_deleted".into(), ChangeValue::Sql(Value::Integer(1))),
            (
                "deleted_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            ),
            (
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            ),
        ]
        .into(),
    )])?;
    Ok(true)
}

pub fn bulk_delete_customers(
    store: &mut Store,
    ids: &[i64],
    now: chrono::NaiveDateTime,
) -> Result<usize, ServiceError> {
    let changes = ids
        .iter()
        .filter_map(|id| {
            store.customers.get(id).filter(|c| !c.is_deleted).map(|_| {
                Change::update(
                    "customers",
                    *id,
                    [
                        ("is_deleted".into(), ChangeValue::Sql(Value::Integer(1))),
                        (
                            "deleted_at".into(),
                            ChangeValue::Sql(codec::write_datetime(Some(now))),
                        ),
                        (
                            "updated_at".into(),
                            ChangeValue::Sql(codec::write_datetime(Some(now))),
                        ),
                    ]
                    .into(),
                )
            })
        })
        .collect::<Vec<_>>();
    let count = changes.len();
    store.apply(changes)?;
    Ok(count)
}

pub fn restore_customer(
    store: &mut Store,
    id: i64,
    now: chrono::NaiveDateTime,
) -> Result<(), ServiceError> {
    let customer = store.customers.get(&id).ok_or_else(|| {
        ServiceError::NotFound(format!("Không tìm thấy khách hàng đã xóa id={id}"))
    })?;
    if !customer.is_deleted {
        return Err(ServiceError::NotFound(format!(
            "Không tìm thấy khách hàng đã xóa id={id}"
        )));
    }
    store.apply(vec![Change::update(
        "customers",
        id,
        [
            ("is_deleted".into(), ChangeValue::Sql(Value::Integer(0))),
            ("deleted_at".into(), ChangeValue::Sql(Value::Null)),
            (
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            ),
        ]
        .into(),
    )])?;
    Ok(())
}

pub fn list_deleted_customers(store: &Store) -> Vec<cf_db::model::Customer> {
    let mut rows = store
        .customers
        .values()
        .filter(|customer| customer.is_deleted)
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by_key(|customer| std::cmp::Reverse(customer.deleted_at));
    rows
}
