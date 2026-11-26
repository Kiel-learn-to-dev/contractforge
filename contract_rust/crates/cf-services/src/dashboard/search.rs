//! Tìm kiếm nhanh hợp đồng và khách hàng theo khóa không dấu.

use cf_core::text::search_key;
use cf_db::store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchTarget {
    ContractDetail { contract_id: i64 },
    CustomerDetail { customer_id: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub kind: &'static str,
    pub id: i64,
    pub label: String,
    pub sub: String,
    pub status: Option<String>,
    pub target: SearchTarget,
}

fn matches(value: &str, query: &str) -> bool {
    search_key(value).contains(query)
}

/// Port của ContractForge/app/routers/dashboard.py::api_search (KB-11).
pub fn api_search(store: &Store, raw_query: &str, limit: Option<usize>) -> Vec<SearchResult> {
    let query = raw_query.trim();
    if query.chars().count() < 2 {
        return Vec::new();
    }
    let query = search_key(query);
    let mut contracts = store
        .contracts
        .values()
        .filter_map(|contract| {
            let customer = store.customers.get(&contract.customer_id);
            let matches = matches(&contract.contract_number, &query)
                || customer.is_some_and(|customer| {
                    matches(&customer.legal_name, &query)
                        || customer
                            .short_name
                            .as_deref()
                            .is_some_and(|name| matches(name, &query))
                });
            matches.then_some((contract, customer))
        })
        .collect::<Vec<_>>();
    contracts.sort_by(|(a, _), (b, _)| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });

    let requested_limit = limit.unwrap_or(10);
    let mut results = contracts
        .into_iter()
        .take(requested_limit)
        .map(|(contract, customer)| SearchResult {
            kind: "contract",
            id: contract.id,
            label: contract.contract_number.clone(),
            sub: customer.map_or_else(
                || "—".into(),
                |customer| {
                    customer
                        .short_name
                        .as_deref()
                        .filter(|name| !name.is_empty())
                        .unwrap_or(&customer.legal_name)
                        .to_owned()
                },
            ),
            status: Some(contract.status.label().to_owned()),
            target: SearchTarget::ContractDetail {
                contract_id: contract.id,
            },
        })
        .collect::<Vec<_>>();

    let customers = store
        .customers
        .values()
        .filter(|customer| customer.is_active && !customer.is_deleted)
        .filter(|customer| {
            matches(&customer.legal_name, &query)
                || customer
                    .short_name
                    .as_deref()
                    .is_some_and(|name| matches(name, &query))
        })
        .take(5)
        .map(|customer| SearchResult {
            kind: "customer",
            id: customer.id,
            label: customer
                .short_name
                .as_deref()
                .filter(|name| !name.is_empty())
                .unwrap_or(&customer.legal_name)
                .to_owned(),
            sub: "Khách hàng".into(),
            status: None,
            target: SearchTarget::CustomerDetail {
                customer_id: customer.id,
            },
        });
    results.extend(customers);
    results.truncate(requested_limit);
    results
}
