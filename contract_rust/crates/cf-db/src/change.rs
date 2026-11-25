use std::collections::BTreeMap;

use rusqlite::types::Value;

use crate::DbError;
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeValue {
    Sql(Value),
    /// Index của lệnh Insert trong cùng lô; giá trị được thay bằng last_insert_rowid.
    Ref(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Insert {
        table: String,
        values: BTreeMap<String, ChangeValue>,
    },
    Update {
        table: String,
        id: i64,
        values: BTreeMap<String, ChangeValue>,
    },
    Delete {
        table: String,
        id: i64,
    },
    SetSetting {
        key: String,
        value: String,
    },
}

impl Change {
    pub fn insert(table: impl Into<String>, values: BTreeMap<String, ChangeValue>) -> Self {
        Self::Insert {
            table: table.into(),
            values,
        }
    }

    pub fn update(
        table: impl Into<String>,
        id: i64,
        values: BTreeMap<String, ChangeValue>,
    ) -> Self {
        Self::Update {
            table: table.into(),
            id,
            values,
        }
    }

    pub fn delete(table: impl Into<String>, id: i64) -> Self {
        Self::Delete {
            table: table.into(),
            id,
        }
    }
}

pub fn resolve_values(
    values: &BTreeMap<String, ChangeValue>,
    inserted_ids: &[i64],
) -> Result<Vec<(String, Value)>, DbError> {
    values
        .iter()
        .map(|(column, value)| {
            let value = match value {
                ChangeValue::Sql(value) => value.clone(),
                ChangeValue::Ref(index) => {
                    Value::Integer(*inserted_ids.get(*index).ok_or_else(|| {
                        DbError::Schema(format!(
                            "Tham chiếu tới lệnh Insert #{index} chưa tồn tại."
                        ))
                    })?)
                }
            };
            Ok((column.clone(), value))
        })
        .collect()
}
