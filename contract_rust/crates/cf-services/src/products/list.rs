use std::collections::BTreeMap;

use cf_core::lifecycle::IN_SERVICE;
use cf_db::{model::Product, store::Store};

pub fn list_products(store: &Store, include_inactive: bool) -> Vec<Product> {
    let mut products = store
        .products
        .values()
        .filter(|product| include_inactive || product.is_active)
        .cloned()
        .collect::<Vec<_>>();
    products.sort_by(|a, b| {
        (
            a.group_code.is_none(),
            a.group_code.as_deref().unwrap_or_default(),
        )
            .cmp(&(
                b.group_code.is_none(),
                b.group_code.as_deref().unwrap_or_default(),
            ))
            .then_with(|| (a.kind != "main").cmp(&(b.kind != "main")))
            .then_with(|| a.sort_order.cmp(&b.sort_order))
            .then_with(|| a.name.cmp(&b.name))
    });
    products
}

pub fn in_service_counts(store: &Store) -> BTreeMap<i64, usize> {
    let mut counts = BTreeMap::new();
    for contract in store.contracts.values() {
        if IN_SERVICE.iter().any(|status| status == &contract.status)
            && let Some(product_id) = contract.product_id
        {
            *counts.entry(product_id).or_default() += 1;
        }
    }
    counts
}
