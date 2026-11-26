use cf_core::lifecycle::{self, ACTIVE_LIFECYCLE};
use cf_db::{model::Contract, store::Store};
use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq)]
pub struct ExpiringContract {
    pub contract: Contract,
    pub contract_id: i64,
    pub contract_number: String,
    pub customer_name: String,
    pub customer_id: Option<i64>,
    pub end_date: NaiveDate,
    pub days_left: i64,
    pub status: String,
    pub total_amount: Option<Decimal>,
    pub urgency: &'static str,
}

/// The router clamps the requested window to 1–365 days before querying.
pub fn get_expiring_contracts(store: &Store, today: NaiveDate, days: i64) -> Vec<ExpiringContract> {
    let days = days.clamp(1, 365);
    let cutoff = today + Duration::days(days);
    let mut rows = store
        .contracts
        .values()
        .filter_map(|contract| {
            if !ACTIVE_LIFECYCLE.contains(&contract.status)
                || contract.doc_kind == "addendum_addon"
                || excluded_by_renewal(store, contract.id)
            {
                return None;
            }
            let end_date = contract
                .end_date
                .filter(|end| *end > today && *end <= cutoff)?;
            let customer = store.customers.get(&contract.customer_id);
            let days_left = (end_date - today).num_days();
            let urgency = match lifecycle::expiry_bucket(Some(end_date), today) {
                Some(lifecycle::Bucket::Expired) => "expired",
                Some(lifecycle::Bucket::Critical) => "critical",
                Some(lifecycle::Bucket::Warning) => "warning",
                Some(lifecycle::Bucket::Info) | None => "info",
            };
            Some(ExpiringContract {
                contract: contract.clone(),
                contract_id: contract.id,
                contract_number: contract.contract_number.clone(),
                customer_name: customer
                    .map(|c| {
                        c.short_name
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .unwrap_or(&c.legal_name)
                            .to_owned()
                    })
                    .unwrap_or_else(|| "—".into()),
                customer_id: customer.map(|c| c.id),
                end_date,
                days_left,
                status: contract.status.as_str().to_owned(),
                total_amount: contract.total_amount,
                urgency,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        a.end_date
            .cmp(&b.end_date)
            .then_with(|| a.contract_id.cmp(&b.contract_id))
    });
    rows
}

/// CSV dùng được trực tiếp bằng Excel; BOM giúp Excel Windows nhận UTF-8.
pub fn export_expiring_csv(rows: &[ExpiringContract], days: i64) -> (String, Vec<u8>) {
    let days = days.clamp(1, 365);
    let mut output = String::from(
        "\u{feff}Số hợp đồng,Khách hàng,Ngày hết hạn,Còn lại (ngày),Trạng thái,Giá trị (VNĐ)\r\n",
    );
    for row in rows {
        let amount = row
            .total_amount
            .map(cf_core::money::trunc_int)
            .map(|v| v.to_string())
            .unwrap_or_default();
        let fields = [
            csv_field(&row.contract_number),
            csv_field(&row.customer_name),
            row.end_date.format("%d/%m/%Y").to_string(),
            row.days_left.to_string(),
            csv_field(lifecycle::status_label(&row.status)),
            amount,
        ];
        output.push_str(&fields.join(","));
        output.push_str("\r\n");
    }
    (format!("sap_het_han_{days}ngay.csv"), output.into_bytes())
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn excluded_by_renewal(store: &Store, id: i64) -> bool {
    store.contracts.values().any(|other| {
        cf_core::lifecycle::RENEWAL_COUNTS.contains(&other.status)
            && ((other.parent_contract_id == Some(id) && other.doc_kind == "addendum_renewal")
                || other.renewed_from_id == Some(id))
    })
}
