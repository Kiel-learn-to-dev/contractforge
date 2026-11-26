use std::collections::BTreeMap;

use cf_core::lifecycle::{self, ContractStatus, Evidence};
use cf_db::{
    change::{Change, ChangeValue},
    codec,
    model::Contract,
    store::Store,
};
use rusqlite::types::Value;

use crate::{
    ServiceError,
    ctx::Ctx,
    events::{self, ChangeRef},
};

fn transition(
    contract: &Contract,
    target: &str,
    evidence: Evidence,
) -> Result<ContractStatus, ServiceError> {
    lifecycle::validate_transition(&contract.status, target, evidence).map_err(ServiceError::from)
}

pub fn change_status_changes(
    store: &Store,
    contract_id: i64,
    target: &str,
    actor: &str,
    note: &str,
    now_utc: chrono::NaiveDateTime,
) -> Result<Vec<Change>, ServiceError> {
    let contract = store.contracts.get(&contract_id).ok_or_else(|| {
        ServiceError::NotFound(format!("Không tìm thấy hợp đồng id={contract_id}"))
    })?;
    change_status_changes_with_evidence(
        store,
        contract_id,
        target,
        actor,
        note,
        now_utc,
        Evidence {
            has_invoice: contract
                .invoice_pdf_path
                .as_ref()
                .is_some_and(|path| !path.trim().is_empty()),
            has_payment_slip: contract
                .payment_slip_path
                .as_ref()
                .is_some_and(|path| !path.trim().is_empty()),
        },
    )
}

/// Tạo thay đổi trạng thái với chứng cứ sẽ được ghi trong cùng transaction.
pub fn change_status_changes_with_evidence(
    store: &Store,
    contract_id: i64,
    target: &str,
    actor: &str,
    note: &str,
    now_utc: chrono::NaiveDateTime,
    evidence: Evidence,
) -> Result<Vec<Change>, ServiceError> {
    let contract = store.contracts.get(&contract_id).ok_or_else(|| {
        ServiceError::NotFound(format!("Không tìm thấy hợp đồng id={contract_id}"))
    })?;
    let new_status = transition(contract, target, evidence)?;
    let description = if note.is_empty() {
        format!("Chuyển trạng thái {} → {}", contract.status, new_status)
    } else {
        format!(
            "Chuyển trạng thái {} → {}: {note}",
            contract.status, new_status
        )
    };
    Ok(vec![
        Change::update(
            "contracts",
            contract_id,
            BTreeMap::from([
                (
                    "status".into(),
                    ChangeValue::Sql(Value::Text(new_status.to_string())),
                ),
                (
                    "updated_at".into(),
                    ChangeValue::Sql(codec::write_datetime(Some(now_utc))),
                ),
            ]),
        ),
        events::event(
            ChangeRef::Existing(contract_id),
            "status_changed",
            &description,
            actor,
            now_utc,
        ),
    ])
}

pub fn change_status(
    ctx: &mut Ctx,
    contract_id: i64,
    target: &str,
    actor: &str,
    note: &str,
) -> Result<(), ServiceError> {
    let now = ctx.clock().now_utc();
    ctx.store_mut().apply(vec![])?;
    let changes = change_status_changes(ctx.store(), contract_id, target, actor, note, now)?;
    ctx.store_mut().apply(changes)?;
    Ok(())
}

pub fn bulk_change_status(ctx: &mut Ctx, ids: &[i64], target: &str) -> Result<usize, ServiceError> {
    ctx.store_mut().apply(vec![])?;
    let _ = lifecycle::parse_status(target)?;
    let mut seen = std::collections::BTreeSet::new();
    let ids = ids
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect::<Vec<_>>();
    let store = ctx.store();
    let missing = ids
        .iter()
        .filter(|id| !store.contracts.contains_key(id))
        .map(|id| format!("#{id}: không tìm thấy"))
        .collect::<Vec<_>>();
    let mut problems = missing;
    for id in &ids {
        let Some(contract) = store.contracts.get(id) else {
            continue;
        };
        let evidence = Evidence {
            has_invoice: contract
                .invoice_pdf_path
                .as_ref()
                .is_some_and(|path| !path.trim().is_empty()),
            has_payment_slip: contract
                .payment_slip_path
                .as_ref()
                .is_some_and(|path| !path.trim().is_empty()),
        };
        if let Err(error) = transition(contract, target, evidence) {
            problems.push(format!(
                "{}: {}",
                contract.contract_number,
                error.user_message()
            ));
        }
    }
    if !problems.is_empty() {
        let count = problems.len();
        let details = problems.into_iter().take(5).collect::<Vec<_>>().join("; ");
        let extra = if count > 5 {
            format!(" (và {} lỗi khác)", count - 5)
        } else {
            String::new()
        };
        return Err(ServiceError::Invalid(format!(
            "Không cập nhật hợp đồng nào. {count} hợp đồng không hợp lệ — {details}{extra}"
        )));
    }
    let now = ctx.clock().now_utc();
    let mut changes = Vec::new();
    for id in &ids {
        changes.extend(change_status_changes(
            ctx.store(),
            *id,
            target,
            "user",
            "",
            now,
        )?);
    }
    ctx.store_mut().apply(changes)?;
    Ok(ids.len())
}
