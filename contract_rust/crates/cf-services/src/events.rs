use std::collections::BTreeMap;

use cf_db::{
    change::{Change, ChangeValue},
    codec,
};
use chrono::NaiveDateTime;
use rusqlite::types::Value;

/// Tham chiếu tới hợp đồng cho sự kiện: id đã có, hoặc vị trí lệnh Insert trong cùng lô.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeRef {
    Existing(i64),
    New(usize),
}

/// Port của app/services/contract_service.py::_add_event.
/// Một chỗ duy nhất tạo sự kiện hợp đồng; mọi nơi khác gọi hàm này.
pub fn event(
    contract_id: ChangeRef,
    kind: &str,
    description: &str,
    actor: &str,
    now_utc: NaiveDateTime,
) -> Change {
    let contract_value = match contract_id {
        ChangeRef::Existing(id) => ChangeValue::Sql(Value::Integer(id)),
        ChangeRef::New(index) => ChangeValue::Ref(index),
    };
    let values = BTreeMap::from([
        ("contract_id".to_owned(), contract_value),
        (
            "event_type".to_owned(),
            ChangeValue::Sql(Value::Text(kind.to_owned())),
        ),
        (
            "description".to_owned(),
            ChangeValue::Sql(Value::Text(description.to_owned())),
        ),
        (
            "actor".to_owned(),
            ChangeValue::Sql(Value::Text(actor.to_owned())),
        ),
        ("meta_json".to_owned(), ChangeValue::Sql(Value::Null)),
        (
            "occurred_at".to_owned(),
            ChangeValue::Sql(codec::write_datetime(Some(now_utc))),
        ),
    ]);
    Change::insert("contract_events", values)
}
