use cf_core::lifecycle::{self, ContractStatus};
use cf_db::{model::Contract, store::Store};

#[derive(Debug, Clone, PartialEq)]
pub struct ExpiringRow {
    pub contract: Contract,
    pub customer_name: String,
    pub days_left: i64,
    pub urgency: lifecycle::Bucket,
}

pub fn not_renewed(store: &Store, id: i64) -> bool {
    let renewed_by_appendix = store
        .indexes
        .children_by_parent
        .get(&id)
        .into_iter()
        .flatten()
        .filter_map(|child_id| store.contracts.get(child_id))
        .any(|child| {
            child.doc_kind == "addendum_renewal"
                && lifecycle::RENEWAL_COUNTS.contains(&child.status)
        });
    let renewed_by_contract = store
        .indexes
        .renewals_by_old
        .get(&id)
        .into_iter()
        .flatten()
        .filter_map(|successor_id| store.contracts.get(successor_id))
        .any(|successor| lifecycle::RENEWAL_COUNTS.contains(&successor.status));
    !renewed_by_appendix && !renewed_by_contract
}

pub fn expiring(store: &Store, today: chrono::NaiveDate, days: i64) -> Vec<ExpiringRow> {
    if days <= 0 {
        return Vec::new();
    }
    let cutoff = today + chrono::Duration::days(days);
    let mut rows = store
        .contracts
        .values()
        .filter_map(|contract| {
            if !lifecycle::ACTIVE_LIFECYCLE.contains(&contract.status)
                || contract.doc_kind == "addendum_addon"
                || !not_renewed(store, contract.id)
            {
                return None;
            }
            let end = contract.end_date?;
            if end <= today || end > cutoff {
                return None;
            }
            let days_left = (end - today).num_days();
            let urgency =
                lifecycle::expiry_bucket(Some(end), today).unwrap_or(lifecycle::Bucket::Info);
            let customer_name = store
                .customers
                .get(&contract.customer_id)
                .map(|customer| {
                    customer
                        .short_name
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&customer.legal_name)
                        .to_owned()
                })
                .unwrap_or_else(|| "—".into());
            Some(ExpiringRow {
                contract: contract.clone(),
                customer_name,
                days_left,
                urgency,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.contract.end_date);
    rows
}

pub fn is_live_status(status: &ContractStatus) -> bool {
    lifecycle::ACTIVE_LIFECYCLE.contains(status)
}
