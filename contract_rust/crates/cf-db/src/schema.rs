use rusqlite::Connection;
use serde::Deserialize;
use std::collections::BTreeMap;

use crate::DbError;

#[derive(Deserialize)]
struct SchemaFixture {
    required_columns: BTreeMap<String, Vec<String>>,
}

/// Kiểm tra bảng/cột bắt buộc mà không tự thay đổi schema hiện có.
pub fn check_schema(connection: &Connection) -> Result<(), DbError> {
    let fixture: SchemaFixture =
        serde_json::from_str(include_str!("../../../fixtures/schema.json"))
            .map_err(|error| DbError::Schema(error.to_string()))?;
    let mut missing = Vec::new();
    for (table, required) in fixture.required_columns {
        let mut statement = connection.prepare(&format!(
            "PRAGMA table_info(\"{}\")",
            table.replace('"', "")
        ))?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>("name"))?
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        if columns.is_empty() {
            missing.push(format!("{table} (thiếu bảng)"));
            continue;
        }
        for column in required {
            if !columns.contains(&column) {
                missing.push(format!("{table}.{column}"));
            }
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(DbError::Schema(format!(
            "Cơ sở dữ liệu thuộc phiên bản cũ. Hãy mở bản ContractForge (Python) 0.10 trở lên một lần để nâng cấp, rồi mở lại. Thiếu: {}",
            missing.join(", ")
        )))
    }
}
