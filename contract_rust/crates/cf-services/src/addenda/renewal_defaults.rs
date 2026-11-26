//! Gợi ý thông tin gia hạn từ hợp đồng hiện hành.

use cf_core::{
    lifecycle,
    money::round_half_up,
    pricing::{self, LineInput},
};
use cf_db::{
    model::{Contract, Product},
    store::Store,
};
use rust_decimal::Decimal;

use crate::ServiceError;

pub const MAX_REPLACEMENT_HOPS: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct RenewalLine {
    pub item_id: i64,
    pub old_product_id: Option<i64>,
    pub product_id: Option<i64>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RenewalComparison {
    pub old_per_month: Decimal,
    pub new_per_month: Decimal,
    pub diff: Decimal,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RenewalDefaults {
    pub start_date: chrono::NaiveDate,
    pub month_count: i64,
    pub lines: Vec<RenewalLine>,
    pub comparison: RenewalComparison,
    pub candidate_product_ids: Vec<i64>,
}

/// Port của ContractForge/app/services/addendum_service.py::_current_product.
pub fn current_product<'a>(store: &'a Store, product: Option<&Product>) -> Option<&'a Product> {
    let mut current = product?;
    let mut seen = std::collections::BTreeSet::new();
    let mut hops = 0;
    while !current.is_active {
        if !seen.insert(current.id) || hops >= MAX_REPLACEMENT_HOPS {
            return None;
        }
        hops += 1;
        current = store.products.get(&current.replaced_by_id?)?;
    }
    store.products.get(&current.id)
}

/// Port của ContractForge/app/services/addendum_service.py::renewal_lines.
pub fn renewal_lines(store: &Store, parent: &Contract) -> Vec<RenewalLine> {
    store
        .indexes
        .items_of(store, parent.id)
        .into_iter()
        .map(|item| RenewalLine {
            item_id: item.id,
            old_product_id: item.product_id,
            product_id: current_product(
                store,
                item.product_id.and_then(|id| store.products.get(&id)),
            )
            .map(|p| p.id),
        })
        .collect()
}

/// Port của ContractForge/app/services/addendum_service.py::renewal_comparison.
pub fn renewal_comparison(
    store: &Store,
    parent: &Contract,
    lines: &[RenewalLine],
) -> Result<RenewalComparison, ServiceError> {
    let old_per_month = round_half_up(
        parent.total_amount.unwrap_or_default()
            / Decimal::from(parent.month_count.filter(|m| *m != 0).unwrap_or(1)),
    );
    let mut new = Decimal::ZERO;
    for line in lines {
        if let Some(product) = line.product_id.and_then(|id| store.products.get(&id)) {
            let input = LineInput {
                description: product.name.clone(),
                unit_label: product.unit_label.clone().unwrap_or_default(),
                quantity: Decimal::ONE,
                months: Decimal::ONE,
                unit_price_pre_vat: product.default_price.unwrap_or_default(),
                vat_rate: product.default_vat_rate.unwrap_or_default(),
                price_period_months: if product.price_period_months == 0 {
                    Decimal::ONE
                } else {
                    Decimal::from(product.price_period_months)
                },
                amount_pre_vat_override: None,
                product_id: Some(product.id),
                note: String::new(),
            };
            new += pricing::price_line(input)
                .map_err(ServiceError::from)?
                .amount_total;
        }
    }
    let new_per_month = round_half_up(new);
    Ok(RenewalComparison {
        old_per_month,
        new_per_month,
        diff: new_per_month - old_per_month,
    })
}

/// Port của ContractForge/app/services/addendum_service.py::renewal_defaults.
pub fn renewal_defaults(store: &Store, parent_id: i64) -> Result<RenewalDefaults, ServiceError> {
    let parent = store
        .contracts
        .get(&parent_id)
        .ok_or_else(|| ServiceError::Invalid("Không tìm thấy hợp đồng gốc.".into()))?;
    if parent.doc_kind != "contract" {
        return Err(ServiceError::Invalid(
            "Chỉ hợp đồng gốc mới có phụ lục.".into(),
        ));
    }
    if !lifecycle::IN_SERVICE.contains(&parent.status) {
        return Err(ServiceError::Invalid(
            "Hợp đồng gốc chưa ở trạng thái đang phục vụ.".into(),
        ));
    }
    let _start_date = parent.start_date.ok_or_else(|| {
        ServiceError::Invalid("Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into())
    })?;
    let end = parent.end_date.ok_or_else(|| {
        ServiceError::Invalid("Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into())
    })?;
    let start = end + chrono::Duration::days(1);
    let main = parent.product_id.and_then(|id| store.products.get(&id));
    let months = main
        .and_then(|p| p.default_duration_months.filter(|m| *m != 0))
        .or(parent.month_count.filter(|m| *m != 0))
        .unwrap_or(12);
    let lines = renewal_lines(store, parent);
    let comparison = renewal_comparison(store, parent, &lines)?;
    let mut candidate_product_ids = store
        .products
        .values()
        .filter(|p| p.is_active)
        .map(|p| p.id)
        .collect::<Vec<_>>();
    candidate_product_ids.sort_by_key(|id| store.products.get(id).map(|p| p.name.clone()));
    Ok(RenewalDefaults {
        start_date: start,
        month_count: months,
        lines,
        comparison,
        candidate_product_ids,
    })
}
