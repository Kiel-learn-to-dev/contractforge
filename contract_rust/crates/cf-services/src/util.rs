use std::collections::BTreeMap;

use cf_db::change::{Change, ChangeValue};
use rusqlite::types::Value;

use crate::{Ctx, ServiceError};

/// Bọc giá trị SQL thường thành `ChangeValue::Sql`.
pub(crate) fn sql(values: BTreeMap<String, Value>) -> BTreeMap<String, ChangeValue> {
    values
        .into_iter()
        .map(|(column, value)| (column, ChangeValue::Sql(value)))
        .collect()
}

/// Áp một lô thay đổi lên Store và trả về id các dòng vừa Insert.
pub(crate) fn apply(ctx: &mut Ctx, changes: Vec<Change>) -> Result<Vec<i64>, ServiceError> {
    Ok(ctx.store_mut().apply(changes)?.inserted_ids)
}
