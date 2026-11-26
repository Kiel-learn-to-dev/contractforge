use std::collections::BTreeSet;

use cf_core::{
    fmt::fmt_currency,
    lifecycle::{self, ContractStatus},
};
use cf_db::model::{Contract, Customer, CustomerUnit};
use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq)]
pub struct TierContract {
    pub contract: Contract,
    pub product_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerTier {
    pub tier: u8,
    pub action_text: String,
    pub days_to_expiry: i64,
    pub contract_id: Option<i64>,
}

/// Port của app/routers/customers.py::export_report::_classify.
pub fn classify(
    customer: &Customer,
    units_rows: &[CustomerUnit],
    contracts: &[TierContract],
    catalog_labels: &BTreeSet<String>,
    today: NaiveDate,
) -> CustomerTier {
    let active = contracts
        .iter()
        .filter(|row| lifecycle::ACTIVE_LIFECYCLE.contains(&row.contract.status))
        .collect::<Vec<_>>();
    let drafts = contracts
        .iter()
        .filter(|row| {
            matches!(
                row.contract.status,
                ContractStatus::Draft | ContractStatus::Generated | ContractStatus::Sent
            )
        })
        .collect::<Vec<_>>();
    let inactive = contracts
        .iter()
        .filter(|row| {
            matches!(
                row.contract.status,
                ContractStatus::Expired | ContractStatus::Terminated
            )
        })
        .collect::<Vec<_>>();
    let units = usize::from(customer.bill_main_station)
        + units_rows
            .iter()
            .filter(|unit| unit.is_active && unit.include_in_billing)
            .count();
    let products_of = |rows: &[&TierContract]| -> BTreeSet<String> {
        rows.iter()
            .filter_map(|row| row.product_label.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let active_products = products_of(&active);
    let all_products = products_of(&contracts.iter().collect::<Vec<_>>());
    let missing_products = if all_products.is_empty() {
        BTreeSet::new()
    } else {
        catalog_labels.difference(&all_products).cloned().collect()
    };

    let expiring_soon = active
        .iter()
        .filter_map(|row| {
            row.contract
                .end_date
                .map(|end| (row, (end - today).num_days()))
        })
        .filter(|(_, days)| (0..=30).contains(days))
        .collect::<Vec<_>>();
    if let Some((row, days)) = expiring_soon
        .into_iter()
        .min_by_key(|(row, _)| row.contract.end_date)
    {
        let product = first_product_label(row).unwrap_or("HĐ");
        return make_tier(
            1,
            format!(
                "Gia hạn {product} — còn {days} ngày | {units} cơ sở | {}đ",
                money(row)
            ),
            days,
            Some(row.contract.id),
        );
    }

    let expiring_60 = active
        .iter()
        .filter_map(|row| {
            row.contract
                .end_date
                .map(|end| (row, (end - today).num_days()))
        })
        .filter(|(_, days)| (31..=60).contains(days))
        .collect::<Vec<_>>();
    if let Some((row, days)) = expiring_60
        .into_iter()
        .min_by_key(|(row, _)| row.contract.end_date)
    {
        let product = first_product_label(row).unwrap_or("HĐ");
        return make_tier(
            2,
            format!(
                "Chuẩn bị gia hạn {product} — còn {days} ngày | {units} cơ sở | {}đ",
                money(row)
            ),
            days,
            Some(row.contract.id),
        );
    }

    if active.is_empty() && !inactive.is_empty() {
        let Some(row) = inactive
            .iter()
            .max_by_key(|row| row.contract.end_date.unwrap_or(NaiveDate::MIN))
            .copied()
        else {
            return make_tier(
                5,
                format!("Chưa có hợp đồng | {units} cơ sở — cần tiếp cận"),
                0,
                None,
            );
        };
        let days = row
            .contract
            .end_date
            .map_or(0, |end| (end - today).num_days());
        return make_tier(
            2,
            format!(
                "HĐ {} hết hạn {} — chưa ký mới | {units} cơ sở",
                row.contract.contract_number,
                format_date(row.contract.end_date)
            ),
            days,
            Some(row.contract.id),
        );
    }

    if active.is_empty() && !drafts.is_empty() {
        let old_drafts = drafts
            .iter()
            .filter(|row| (today - row.contract.created_at.date()).num_days() > 14)
            .collect::<Vec<_>>();
        if let Some(row) = old_drafts
            .into_iter()
            .max_by_key(|row| row.contract.created_at)
        {
            let age = (today - row.contract.created_at.date()).num_days();
            return make_tier(
                2,
                format!(
                    "HĐ nháp {} từ {age} ngày trước — chưa ký | {units} cơ sở",
                    row.contract.contract_number
                ),
                age,
                Some(row.contract.id),
            );
        }
    }

    if !active.is_empty() && !missing_products.is_empty() {
        let missing = missing_products.into_iter().collect::<Vec<_>>().join(" & ");
        let has = if active_products.is_empty() {
            "?".into()
        } else {
            active_products.into_iter().collect::<Vec<_>>().join(" & ")
        };
        let Some(row) = active
            .iter()
            .max_by_key(|row| row.contract.end_date.unwrap_or(NaiveDate::MIN))
            .copied()
        else {
            return make_tier(
                5,
                format!("Chưa có hợp đồng | {units} cơ sở — cần tiếp cận"),
                0,
                None,
            );
        };
        let days = row
            .contract
            .end_date
            .map_or(0, |end| (end - today).num_days());
        return make_tier(
            3,
            format!("Upsell {missing} — đang có {has} | {units} cơ sở | còn {days} ngày"),
            days,
            Some(row.contract.id),
        );
    }

    if !active.is_empty() {
        let Some(row) = active
            .iter()
            .max_by_key(|row| row.contract.end_date.unwrap_or(NaiveDate::MIN))
            .copied()
        else {
            return make_tier(
                5,
                format!("Chưa có hợp đồng | {units} cơ sở — cần tiếp cận"),
                0,
                None,
            );
        };
        let days = row
            .contract
            .end_date
            .map_or(0, |end| (end - today).num_days());
        let products = if active_products.is_empty() {
            "HĐ".into()
        } else {
            active_products.into_iter().collect::<Vec<_>>().join(" & ")
        };
        let total = active
            .iter()
            .filter_map(|row| row.contract.total_amount)
            .sum::<rust_decimal::Decimal>();
        return make_tier(
            4,
            format!(
                "{products} hiệu lực đến {} | {units} cơ sở | {}đ",
                format_date(row.contract.end_date),
                fmt_currency(Some(total))
            ),
            days,
            Some(row.contract.id),
        );
    }

    if !drafts.is_empty() && inactive.is_empty() {
        let Some(row) = drafts
            .iter()
            .max_by_key(|row| row.contract.created_at)
            .copied()
        else {
            return make_tier(
                5,
                format!("Chưa có hợp đồng | {units} cơ sở — cần tiếp cận"),
                0,
                None,
            );
        };
        return make_tier(
            5,
            format!(
                "HĐ nháp {} — {} | {units} cơ sở",
                row.contract.contract_number,
                row.contract.status.label()
            ),
            0,
            Some(row.contract.id),
        );
    }

    make_tier(
        5,
        format!("Chưa có hợp đồng | {units} cơ sở — cần tiếp cận"),
        0,
        None,
    )
}

fn first_product_label(row: &TierContract) -> Option<&str> {
    row.product_label
        .as_deref()
        .filter(|value| !value.trim().is_empty())
}

fn money(row: &TierContract) -> String {
    row.contract
        .total_amount
        .filter(|value| !value.is_zero())
        .map_or_else(|| "?".into(), |value| fmt_currency(Some(value)))
}

fn format_date(date: Option<NaiveDate>) -> String {
    date.map(|value| value.format("%d/%m/%Y").to_string())
        .unwrap_or_default()
}

fn make_tier(
    tier: u8,
    action_text: String,
    days_to_expiry: i64,
    contract_id: Option<i64>,
) -> CustomerTier {
    CustomerTier {
        tier,
        action_text,
        days_to_expiry,
        contract_id,
    }
}
