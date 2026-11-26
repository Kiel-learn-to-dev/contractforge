use std::collections::BTreeMap;

use cf_core::lifecycle::ACTIVE_LIFECYCLE;
use cf_db::store::Store;
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq)]
pub struct ProductBreakdown {
    pub name: String,
    pub code: String,
    pub count: usize,
    pub total_value: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecentActivity {
    pub event_id: i64,
    pub contract_id: i64,
    pub contract_number: String,
    pub customer_name: String,
    pub event_type: String,
    pub description: String,
    pub occurred_at: chrono::NaiveDateTime,
}

/// Port của dashboard_service.py::_contracts_by_product.
pub fn contracts_by_product(store: &Store) -> Vec<ProductBreakdown> {
    let mut groups = BTreeMap::<(Option<i64>, String, String), (usize, Decimal)>::new();
    for contract in store.contracts.values().filter(|contract| {
        ACTIVE_LIFECYCLE
            .iter()
            .any(|status| status == &contract.status)
    }) {
        let key = match contract.product_id {
            None => (None, "Không phân loại".to_owned(), String::new()),
            Some(id) => match store.products.get(&id) {
                Some(product) => (Some(product.id), product.name.clone(), product.code.clone()),
                None => continue,
            },
        };
        let (count, amount) = groups.entry(key).or_default();
        *count += 1;
        *amount += contract.total_amount.unwrap_or_default();
    }
    let mut rows = groups
        .into_iter()
        .map(|((_, name, code), (count, total_value))| ProductBreakdown {
            name: if name.trim().is_empty() {
                "—".into()
            } else {
                name
            },
            code,
            count,
            total_value,
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.name.cmp(&right.name))
    });
    rows
}

/// Tám hoạt động gần nhất theo dashboard_service.py::get_dashboard_data.
pub fn recent_activity(store: &Store) -> Vec<RecentActivity> {
    let mut rows = store
        .events
        .values()
        .filter(|event| {
            !matches!(event.event_type.as_str(), "expiring_soon_alert" | "expired")
                && store.contracts.contains_key(&event.contract_id)
        })
        .map(|event| {
            let contract = &store.contracts[&event.contract_id];
            let customer_name = store
                .customers
                .get(&contract.customer_id)
                .map(|customer| {
                    customer
                        .short_name
                        .as_deref()
                        .filter(|name| !name.is_empty())
                        .unwrap_or(&customer.legal_name)
                        .to_owned()
                })
                .unwrap_or_else(|| "—".into());
            RecentActivity {
                event_id: event.id,
                contract_id: contract.id,
                contract_number: contract.contract_number.clone(),
                customer_name,
                event_type: event.event_type.clone(),
                description: event.description.clone().unwrap_or_default(),
                occurred_at: event.occurred_at,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.occurred_at
            .cmp(&a.occurred_at)
            .then_with(|| b.event_id.cmp(&a.event_id))
    });
    rows.truncate(8);
    rows
}
