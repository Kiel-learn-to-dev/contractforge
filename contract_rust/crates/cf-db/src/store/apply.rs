use rusqlite::{Transaction, params_from_iter, types::Value};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    DbError,
    change::{Change, resolve_values},
    store::Store,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyResult {
    pub inserted_ids: Vec<i64>,
    pub reloaded: bool,
}

#[derive(Deserialize)]
struct SchemaColumns {
    required_columns: BTreeMap<String, Vec<String>>,
}

fn validate_columns(table: &str, columns: impl Iterator<Item = String>) -> Result<(), DbError> {
    let schema: SchemaColumns =
        serde_json::from_str(include_str!("../../../../fixtures/schema.json"))
            .map_err(|error| DbError::Schema(error.to_string()))?;
    let allowed = schema
        .required_columns
        .get(table)
        .ok_or_else(|| DbError::Schema(format!("Bảng '{table}' không được phép ghi.")))?;
    let allowed: BTreeSet<_> = allowed.iter().map(String::as_str).collect();
    for column in columns {
        if !allowed.contains(column.as_str()) {
            return Err(DbError::Schema(format!(
                "Cột '{table}.{column}' không được phép ghi."
            )));
        }
    }
    Ok(())
}

fn quoted(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn apply_change(
    transaction: &Transaction<'_>,
    change: &Change,
    inserted_ids: &mut Vec<i64>,
) -> Result<(), DbError> {
    match change {
        Change::Insert { table, values } => {
            validate_columns(table, values.keys().cloned())?;
            let values = resolve_values(values, inserted_ids)?;
            if values.is_empty() {
                transaction
                    .execute(&format!("INSERT INTO {} DEFAULT VALUES", quoted(table)), [])?;
            } else {
                let names = values
                    .iter()
                    .map(|(column, _)| quoted(column))
                    .collect::<Vec<_>>()
                    .join(", ");
                let placeholders = (1..=values.len())
                    .map(|index| format!("?{index}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!(
                    "INSERT INTO {} ({names}) VALUES ({placeholders})",
                    quoted(table)
                );
                let params = values
                    .into_iter()
                    .map(|(_, value)| value)
                    .collect::<Vec<Value>>();
                transaction.execute(&sql, params_from_iter(params))?;
            }
            inserted_ids.push(transaction.last_insert_rowid());
        }
        Change::Update { table, id, values } => {
            if values.is_empty() {
                return Ok(());
            }
            if table == "app_settings" {
                return Err(DbError::Schema("app_settings dùng SetSetting.".into()));
            }
            if values.contains_key("id") {
                return Err(DbError::Schema("Không thể sửa cột id.".into()));
            }
            validate_columns(table, values.keys().cloned())?;
            let values = resolve_values(values, inserted_ids)?;
            let assignments = values
                .iter()
                .enumerate()
                .map(|(index, (column, _))| format!("{} = ?{}", quoted(column), index + 1))
                .collect::<Vec<_>>()
                .join(", ");
            let mut params = values
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>();
            params.push(Value::Integer(*id));
            let sql = format!(
                "UPDATE {} SET {assignments} WHERE {} = ?{}",
                quoted(table),
                quoted("id"),
                params.len()
            );
            transaction.execute(&sql, params_from_iter(params))?;
        }
        Change::Delete { table, id } => {
            validate_columns(table, std::iter::empty())?;
            if table == "app_settings" {
                return Err(DbError::Schema("app_settings phải xóa theo key.".into()));
            }
            transaction.execute(
                &format!("DELETE FROM {} WHERE {} = ?1", quoted(table), quoted("id")),
                [id],
            )?;
        }
        Change::SetSetting { key, value } => {
            transaction.execute("INSERT INTO app_settings (\"key\", value) VALUES (?1, ?2) ON CONFLICT(\"key\") DO UPDATE SET value = excluded.value", [key, value])?;
        }
    }
    Ok(())
}

impl Store {
    pub fn apply(&mut self, changes: Vec<Change>) -> Result<ApplyResult, DbError> {
        let current_version = self
            .conn
            .pragma_query_value(None, "data_version", |row| row.get::<_, i64>(0))?;
        let reloaded = current_version != self.data_version;
        if reloaded {
            self.reload()?;
        }
        if changes.is_empty() {
            return Ok(ApplyResult {
                inserted_ids: Vec::new(),
                reloaded,
            });
        }
        let transaction = self.conn.transaction()?;
        let mut inserted_ids = Vec::new();
        for change in &changes {
            apply_change(&transaction, change, &mut inserted_ids)?;
        }
        transaction.commit()?;
        self.reload()?;
        Ok(ApplyResult {
            inserted_ids,
            reloaded,
        })
    }
}
