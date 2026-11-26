use std::collections::BTreeMap;

use cf_core::lifecycle::{self, ContractStatus};
use cf_db::{
    change::{Change, ChangeValue},
    codec,
};
use rusqlite::types::Value;

use crate::{
    ServiceError,
    ctx::Ctx,
    events::{self, ChangeRef},
};

pub fn expire_due(ctx: &mut Ctx) -> Result<usize, ServiceError> {
    ctx.store_mut().apply(vec![])?;
    let today = ctx.clock().today();
    let now = ctx.clock().now_utc();
    let due = ctx
        .store()
        .contracts
        .values()
        .filter(|contract| lifecycle::ACTIVE_LIFECYCLE.contains(&contract.status))
        .filter(|contract| contract.end_date.is_some_and(|end| end <= today))
        .cloned()
        .collect::<Vec<_>>();
    if due.is_empty() {
        return Ok(0);
    }
    let mut changes = Vec::with_capacity(due.len() * 2);
    for contract in &due {
        let end = contract
            .end_date
            .map(|date| date.to_string())
            .unwrap_or_default();
        changes.push(Change::update(
            "contracts",
            contract.id,
            BTreeMap::from([
                (
                    "status".into(),
                    ChangeValue::Sql(Value::Text(ContractStatus::Expired.to_string())),
                ),
                (
                    "updated_at".into(),
                    ChangeValue::Sql(codec::write_datetime(Some(now))),
                ),
            ]),
        ));
        changes.push(events::event(
            ChangeRef::Existing(contract.id),
            "expired",
            &format!("Tự động cập nhật: hết hạn ngày {end}"),
            "system",
            now,
        ));
    }
    ctx.store_mut().apply(changes)?;
    Ok(due.len())
}
