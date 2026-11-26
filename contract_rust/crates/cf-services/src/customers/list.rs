use cf_core::text::search_key;
use cf_db::{model::Customer, store::Store};

use crate::ServiceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerFilters {
    pub q: String,
    pub ward: String,
    pub customer_type: String,
    pub active_only: bool,
    pub page: usize,
    pub per_page: usize,
    pub sort_by: String,
    pub sort_dir: String,
}

impl Default for CustomerFilters {
    fn default() -> Self {
        Self {
            q: String::new(),
            ward: String::new(),
            customer_type: String::new(),
            active_only: true,
            page: 1,
            per_page: 25,
            sort_by: "ward".into(),
            sort_dir: "asc".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerFilterOptions {
    pub wards: Vec<String>,
    pub customer_types: Vec<String>,
}

/// Lọc, sắp xếp và phân trang khách hàng; hợp đồng bị xóa mềm vẫn được tính trong tổng.
pub fn list_customers(
    store: &Store,
    filters: &CustomerFilters,
) -> Result<(Vec<(Customer, usize)>, usize), ServiceError> {
    let query = search_key(filters.q.trim());
    let mut rows = store
        .customers
        .values()
        .filter(|customer| !customer.is_deleted)
        .filter(|customer| !filters.active_only || customer.is_active)
        .filter(|customer| {
            filters.ward.trim().is_empty() || customer.ward.as_deref() == Some(filters.ward.trim())
        })
        .filter(|customer| {
            filters.customer_type.trim().is_empty()
                || customer.customer_type == filters.customer_type.trim()
        })
        .filter(|customer| {
            query.is_empty()
                || search_key(&format!(
                    "{} {} {} {} {}",
                    customer.legal_name,
                    customer.short_name.as_deref().unwrap_or_default(),
                    customer.code,
                    customer.representative_name.as_deref().unwrap_or_default(),
                    customer.ward.as_deref().unwrap_or_default()
                ))
                .contains(&query)
        })
        .map(|customer| {
            let count = store
                .indexes
                .contracts_by_customer
                .get(&customer.id)
                .map_or(0, Vec::len);
            (customer.clone(), count)
        })
        .collect::<Vec<_>>();

    let desc = filters.sort_dir.eq_ignore_ascii_case("desc");
    rows.sort_by(|(a, ac), (b, bc)| {
        let ordering = match filters.sort_by.as_str() {
            "code" => a.code.cmp(&b.code),
            "legal_name" => a.legal_name.cmp(&b.legal_name),
            "customer_type" => a.customer_type.cmp(&b.customer_type),
            "province" => a.province.cmp(&b.province),
            "representative_name" => a.representative_name.cmp(&b.representative_name),
            "is_active" => a.is_active.cmp(&b.is_active),
            "contract_count" => ac.cmp(bc),
            _ => a.ward.cmp(&b.ward),
        };
        let ordering = if desc { ordering.reverse() } else { ordering };
        if filters.sort_by == "legal_name" || ordering != std::cmp::Ordering::Equal {
            ordering
        } else {
            ordering.then_with(|| a.legal_name.cmp(&b.legal_name))
        }
    });
    let total = rows.len();
    let page = filters.page.max(1);
    let per_page = filters.per_page.max(1);
    let start = page.saturating_sub(1).saturating_mul(per_page).min(total);
    let end = start.saturating_add(per_page).min(total);
    Ok((
        rows.into_iter().skip(start).take(end - start).collect(),
        total,
    ))
}

pub fn filter_options(store: &Store) -> CustomerFilterOptions {
    let mut wards = std::collections::BTreeSet::new();
    let mut customer_types = std::collections::BTreeSet::new();
    for customer in store
        .customers
        .values()
        .filter(|customer| !customer.is_deleted)
    {
        wards.extend(
            customer
                .ward
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
        );
        if !customer.customer_type.trim().is_empty() {
            customer_types.insert(customer.customer_type.trim().to_owned());
        }
    }
    CustomerFilterOptions {
        wards: wards.into_iter().collect(),
        customer_types: customer_types.into_iter().collect(),
    }
}
