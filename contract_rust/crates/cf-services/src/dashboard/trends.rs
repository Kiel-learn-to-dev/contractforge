use std::collections::BTreeMap;

use cf_core::lifecycle::{ACTIVE_LIFECYCLE, RENEWAL_COUNTS};
use cf_db::store::Store;
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq)]
pub struct MonthlyStats {
    pub month_label: String,
    pub new_contracts: usize,
    pub signed: usize,
    pub expired_count: usize,
    pub new_value: Decimal,
    pub vs_new: isize,
    pub vs_signed: isize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopCustomer {
    pub id: i64,
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiryForecastMonth {
    pub month: String,
    pub label: String,
    pub count: usize,
}

fn month_start(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1).expect("valid month start")
}

fn shift_month(year: i32, month: u32, offset: i32) -> (i32, u32) {
    let index = year * 12 + month as i32 - 1 + offset;
    (index.div_euclid(12), index.rem_euclid(12) as u32 + 1)
}

fn count_created(store: &Store, start: NaiveDateTime, end: NaiveDateTime) -> usize {
    store
        .contracts
        .values()
        .filter(|c| c.created_at >= start && c.created_at < end)
        .count()
}

fn count_signed(store: &Store, start: NaiveDate, end: NaiveDate, today: NaiveDate) -> usize {
    store
        .contracts
        .values()
        .filter(|c| {
            RENEWAL_COUNTS.contains(&c.status)
                || matches!(c.status.as_str(), "Expired" | "Terminated")
        })
        .filter(|c| {
            c.sign_date
                .is_some_and(|date| date >= start && date < end && date <= today)
        })
        .count()
}

pub fn monthly_stats(store: &Store, today: NaiveDate) -> MonthlyStats {
    let (year, month) = (today.year(), today.month());
    let (next_year, next_month) = shift_month(year, month, 1);
    let (prev_year, prev_month) = shift_month(year, month, -1);
    let this_start = month_start(year, month);
    let next_start = month_start(next_year, next_month);
    let prev_start = month_start(prev_year, prev_month);
    let to_dt = |date: NaiveDate| date.and_hms_opt(0, 0, 0).expect("midnight is valid");
    let this_new = count_created(store, to_dt(this_start), to_dt(next_start));
    let prev_new = count_created(store, to_dt(prev_start), to_dt(this_start));
    let this_signed = count_signed(store, this_start, next_start, today);
    let prev_signed = count_signed(store, prev_start, this_start, today);
    let expired_count = store
        .contracts
        .values()
        .filter(|c| {
            c.status.as_str() == "Expired"
                && c.end_date
                    .is_some_and(|d| d >= this_start && d < next_start && d <= today)
        })
        .count();
    let new_value = store
        .contracts
        .values()
        .filter(|c| c.created_at >= to_dt(this_start) && c.created_at < to_dt(next_start))
        .map(|c| c.total_amount.unwrap_or_default())
        .sum();
    MonthlyStats {
        month_label: format!("Tháng {month}/{year}"),
        new_contracts: this_new,
        signed: this_signed,
        expired_count,
        new_value,
        vs_new: this_new as isize - prev_new as isize,
        vs_signed: this_signed as isize - prev_signed as isize,
    }
}

pub fn top_customers(store: &Store, limit: usize) -> Vec<TopCustomer> {
    let mut counts = BTreeMap::<i64, usize>::new();
    for contract in store
        .contracts
        .values()
        .filter(|c| ACTIVE_LIFECYCLE.contains(&c.status))
    {
        if store
            .customers
            .get(&contract.customer_id)
            .is_some_and(|customer| customer.is_active && !customer.is_deleted)
        {
            *counts.entry(contract.customer_id).or_default() += 1;
        }
    }
    let mut result = counts
        .into_iter()
        .filter_map(|(id, count)| {
            store.customers.get(&id).map(|customer| TopCustomer {
                id,
                name: customer
                    .short_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&customer.legal_name)
                    .to_owned(),
                count,
            })
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
    result.truncate(limit);
    result
}

fn excluded_from_expiry(store: &Store, contract_id: i64) -> bool {
    store.contracts.values().any(|other| {
        RENEWAL_COUNTS.contains(&other.status)
            && ((other.parent_contract_id == Some(contract_id)
                && other.doc_kind == "addendum_renewal")
                || other.renewed_from_id == Some(contract_id))
    })
}

pub fn monthly_expiry_forecast(
    store: &Store,
    today: NaiveDate,
    months: usize,
) -> Vec<ExpiryForecastMonth> {
    let (year, month) = (today.year(), today.month());
    let (cutoff_year, cutoff_month) = shift_month(year, month, months as i32);
    let cutoff = month_start(cutoff_year, cutoff_month);
    let mut counts = BTreeMap::<(i32, u32), usize>::new();
    for contract in store.contracts.values().filter(|c| {
        ACTIVE_LIFECYCLE.contains(&c.status)
            && c.doc_kind != "addendum_addon"
            && !excluded_from_expiry(store, c.id)
    }) {
        if let Some(end) = contract
            .end_date
            .filter(|end| *end > today && *end < cutoff)
        {
            *counts.entry((end.year(), end.month())).or_default() += 1;
        }
    }
    (0..months)
        .map(|offset| {
            let (y, m) = shift_month(year, month, offset as i32);
            ExpiryForecastMonth {
                month: format!("{y}-{m:02}"),
                label: format!("T{m}/{:02}", y.rem_euclid(100)),
                count: counts.get(&(y, m)).copied().unwrap_or(0),
            }
        })
        .collect()
}
