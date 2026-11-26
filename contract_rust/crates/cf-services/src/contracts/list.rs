use cf_core::{lifecycle, text::search_key};
use cf_db::{model::Contract, store::Store};
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct ContractFilters {
    pub q: String,
    pub status: String,
    pub customer_id: Option<i64>,
    pub product_id: Option<i64>,
    pub alert_filter: String,
    pub year: i32,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub date_field: String,
    pub page: usize,
    pub per_page: usize,
    pub sort_by: String,
    pub sort_dir: String,
}

impl Default for ContractFilters {
    fn default() -> Self {
        Self {
            q: String::new(),
            status: String::new(),
            customer_id: None,
            product_id: None,
            alert_filter: String::new(),
            year: 0,
            date_from: None,
            date_to: None,
            date_field: "sign_date".into(),
            page: 1,
            per_page: 25,
            sort_by: "created_at".into(),
            sort_dir: "desc".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContractRow {
    pub contract: Contract,
    pub customer_name: String,
    pub flag_unsent: bool,
    pub flag_expiring: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListResult {
    pub rows: Vec<ContractRow>,
    pub total: usize,
    pub sum_total: Decimal,
    pub years: Vec<i32>,
    pub cnt_unsent: usize,
    pub cnt_expiring: usize,
}

fn in_expiring_window(store: &Store, contract: &Contract, today: NaiveDate) -> bool {
    contract.end_date.is_some_and(|end| {
        let days = (end - today).num_days();
        days > 0 && days <= 60
    }) && lifecycle::ACTIVE_LIFECYCLE.contains(&contract.status)
        && contract.doc_kind != "addendum_addon"
        && super::expiring::not_renewed(store, contract.id)
}

fn matches(
    store: &Store,
    contract: &Contract,
    filters: &ContractFilters,
    today: NaiveDate,
) -> bool {
    if !filters.q.trim().is_empty() {
        let customer = store.customers.get(&contract.customer_id);
        let key = search_key(&format!(
            "{} {} {} {}",
            contract.contract_number,
            contract.notes.as_deref().unwrap_or_default(),
            customer.map_or("", |c| c.legal_name.as_str()),
            customer
                .and_then(|c| c.short_name.as_deref())
                .unwrap_or_default()
        ));
        if !key.contains(&search_key(filters.q.trim())) {
            return false;
        }
    }
    if !filters.status.is_empty() && contract.status.as_str() != filters.status {
        return false;
    }
    if filters
        .customer_id
        .is_some_and(|id| id != contract.customer_id)
    {
        return false;
    }
    if filters
        .product_id
        .is_some_and(|id| Some(id) != contract.product_id)
    {
        return false;
    }
    if filters.year != 0
        && contract
            .sign_date
            .is_none_or(|date| date.year() != filters.year)
    {
        return false;
    }
    if filters.date_from.is_some() || filters.date_to.is_some() {
        let value = if filters.date_field == "end_date" {
            contract.end_date
        } else {
            contract.sign_date
        };
        let Some(value) = value else {
            return false;
        };
        if filters.date_from.is_some_and(|from| value < from)
            || filters.date_to.is_some_and(|to| value > to)
        {
            return false;
        }
    }
    match filters.alert_filter.as_str() {
        "unsent_10d" => {
            contract.status.as_str() == "Sent"
                && contract.updated_at.date() < today - chrono::Duration::days(10)
        }
        "expiring_60" => in_expiring_window(store, contract, today),
        "generated" => contract
            .output_file_path
            .as_deref()
            .is_some_and(|path| !path.trim().is_empty()),
        "all_active" => lifecycle::IN_SERVICE.contains(&contract.status),
        _ => true,
    }
}

fn sort_rows(rows: &mut [ContractRow], sort_by: &str, desc: bool) {
    rows.sort_by(|a, b| {
        let order = match sort_by {
            "sign_date" => a.contract.sign_date.cmp(&b.contract.sign_date),
            "end_date" => a.contract.end_date.cmp(&b.contract.end_date),
            "total_amount" => a.contract.total_amount.cmp(&b.contract.total_amount),
            "contract_number" => a.contract.contract_number.cmp(&b.contract.contract_number),
            "customer_name" => a.customer_name.cmp(&b.customer_name),
            "status" => a.contract.status.as_str().cmp(b.contract.status.as_str()),
            _ => a.contract.created_at.cmp(&b.contract.created_at),
        };
        if desc { order.reverse() } else { order }
    });
}

pub fn list_contracts(store: &Store, filters: &ContractFilters, today: NaiveDate) -> ListResult {
    let page_size = if [10, 25, 50, 100].contains(&filters.per_page) {
        filters.per_page
    } else {
        25
    };
    let matched = store
        .contracts
        .values()
        .filter(|contract| matches(store, contract, filters, today))
        .cloned()
        .collect::<Vec<_>>();
    let total = matched.len();
    let sum_total = matched
        .iter()
        .filter_map(|contract| contract.total_amount)
        .sum();
    let mut years = store
        .contracts
        .values()
        .filter_map(|contract| contract.sign_date.map(|date| date.year()))
        .collect::<Vec<_>>();
    years.sort_unstable_by(|a, b| b.cmp(a));
    years.dedup();
    let cutoff = today - chrono::Duration::days(10);
    let cnt_unsent = store
        .contracts
        .values()
        .filter(|contract| {
            contract.status.as_str() == "Sent" && contract.updated_at.date() < cutoff
        })
        .count();
    let cnt_expiring = store
        .contracts
        .values()
        .filter(|contract| in_expiring_window(store, contract, today))
        .count();
    let mut rows = matched
        .into_iter()
        .map(|contract| {
            let flag_unsent =
                contract.status.as_str() == "Sent" && contract.updated_at.date() < cutoff;
            let flag_expiring = in_expiring_window(store, &contract, today);
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
            ContractRow {
                contract,
                customer_name,
                flag_unsent,
                flag_expiring,
            }
        })
        .collect::<Vec<_>>();
    sort_rows(&mut rows, &filters.sort_by, filters.sort_dir == "desc");
    let start = filters
        .page
        .max(1)
        .saturating_sub(1)
        .saturating_mul(page_size)
        .min(total);
    let rows = rows.into_iter().skip(start).take(page_size).collect();
    ListResult {
        rows,
        total,
        sum_total,
        years,
        cnt_unsent,
        cnt_expiring,
    }
}
