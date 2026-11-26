use std::collections::BTreeMap;

use cf_core::lifecycle::{ACTIVE_LIFECYCLE, IN_SERVICE, UNPAID};
use cf_db::store::Store;
use rust_decimal::Decimal;

/// Các số liệu tổng hợp của dashboard; status_counts giữ cả trạng thái mở rộng.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardSummary {
    pub total_contracts: usize,
    pub in_force: usize,
    pub in_service: usize,
    pub active: usize,
    pub paid_active: usize,
    pub invoiced: usize,
    pub signed: usize,
    pub expiring_soon: usize,
    pub expired: usize,
    pub generated: usize,
    pub terminated: usize,
    pub draft: usize,
    pub total_value_active: Decimal,
    pub value_paid: Decimal,
    pub value_invoiced: Decimal,
    pub value_unpaid: Decimal,
    pub total_customers: usize,
    pub total_templates: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardSummaryData {
    pub summary: DashboardSummary,
    pub status_counts: BTreeMap<String, usize>,
}

/// Tổng hợp dashboard bằng một lượt duyệt các tập dữ liệu trong Store.
pub fn get_summary(store: &Store, today: chrono::NaiveDate) -> DashboardSummaryData {
    let mut status_counts = BTreeMap::<String, usize>::new();
    let mut in_force = 0;
    let mut in_service = 0;
    let mut active = 0;
    let mut paid_active = 0;
    let mut invoiced = 0;
    let mut signed = 0;
    let mut expiring_soon = 0;
    let mut expired = 0;
    let mut generated = 0;
    let mut terminated = 0;
    let mut draft = 0;
    let (mut total_value_active, mut value_paid, mut value_invoiced, mut value_unpaid) =
        (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);

    for contract in store.contracts.values() {
        let status = contract.status.as_str();
        *status_counts.entry(status.to_owned()).or_default() += 1;
        if ACTIVE_LIFECYCLE
            .iter()
            .any(|candidate| candidate.as_str() == status)
        {
            in_force += 1;
            total_value_active += contract.total_amount.unwrap_or_default();
            if contract.doc_kind != "addendum_addon"
                && !excluded_by_renewal(store, contract.id)
                && contract
                    .end_date
                    .is_some_and(|end| end > today && (end - today).num_days() <= 30)
            {
                expiring_soon += 1;
            }
        }
        if IN_SERVICE
            .iter()
            .any(|candidate| candidate.as_str() == status)
        {
            in_service += 1;
        }
        match status {
            "Active" => active += 1,
            "PaidActive" => {
                active += 1;
                paid_active += 1;
                value_paid += contract.total_amount.unwrap_or_default();
            }
            "Invoiced" => {
                invoiced += 1;
                value_invoiced += contract.total_amount.unwrap_or_default();
            }
            "Expired" => expired += 1,
            "Terminated" => terminated += 1,
            "Draft" => draft += 1,
            _ => {}
        }
        if UNPAID.iter().any(|candidate| candidate.as_str() == status) {
            value_unpaid += contract.total_amount.unwrap_or_default();
        }
        if contract
            .output_file_path
            .as_deref()
            .is_some_and(|path| !path.trim().is_empty())
        {
            generated += 1;
        }
        if status == "Signed" {
            signed += 1;
        }
    }

    let summary = DashboardSummary {
        total_contracts: store.contracts.len(),
        in_force,
        in_service,
        active,
        paid_active,
        invoiced,
        signed,
        expiring_soon,
        expired,
        generated,
        terminated,
        draft,
        total_value_active,
        value_paid,
        value_invoiced,
        value_unpaid,
        total_customers: store
            .customers
            .values()
            .filter(|c| c.is_active && !c.is_deleted)
            .count(),
        total_templates: store.templates.values().filter(|t| t.is_active).count(),
    };
    DashboardSummaryData {
        summary,
        status_counts,
    }
}

fn excluded_by_renewal(store: &Store, contract_id: i64) -> bool {
    store.contracts.values().any(|other| {
        cf_core::lifecycle::RENEWAL_COUNTS.contains(&other.status)
            && ((other.parent_contract_id == Some(contract_id)
                && other.doc_kind == "addendum_renewal")
                || other.renewed_from_id == Some(contract_id))
    })
}
