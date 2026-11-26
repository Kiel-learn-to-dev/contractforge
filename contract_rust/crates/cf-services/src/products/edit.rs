use cf_db::{
    change::{Change, ChangeValue},
    model::Product,
    store::Store,
};
use rusqlite::types::Value;
use serde_json::Value as Json;
use std::collections::BTreeMap;

use super::clean::{self, ProductDraft};
use crate::ServiceError;

fn values(
    draft: &ProductDraft,
    now: chrono::NaiveDateTime,
    is_new: bool,
) -> BTreeMap<String, ChangeValue> {
    let product = Product {
        id: 0,
        code: draft.code.clone(),
        name: draft.name.clone(),
        product_type: draft.product_type.clone(),
        default_price: draft.default_price,
        default_vat_rate: draft.default_vat_rate,
        default_duration_months: draft.default_duration_months,
        default_template_id: draft.default_template_id,
        contract_number_format: draft.contract_number_format.clone(),
        is_active: draft.is_active.unwrap_or(is_new),
        group_code: draft.group_code.clone(),
        kind: draft.kind.clone(),
        unit_label: draft.unit_label.clone(),
        price_period_months: draft.price_period_months,
        quantity_source: draft.quantity_source.clone(),
        min_quantity: draft.min_quantity,
        quantity_step: draft.quantity_step,
        account_limit: draft.account_limit,
        feature_list: draft.feature_list.clone().unwrap_or_default(),
        replaced_by_id: draft.replaced_by_id,
        sort_order: draft.sort_order,
        created_at: now,
        updated_at: now,
    };
    let mut result = product
        .to_values()
        .into_iter()
        .map(|(key, value)| (key, ChangeValue::Sql(value)))
        .collect::<BTreeMap<_, _>>();
    result.remove("id");
    if !is_new {
        result.remove("created_at");
        if draft.is_active.is_none() {
            result.remove("is_active");
        }
        if draft.feature_list.is_none() {
            result.remove("feature_list");
        }
    }
    result
}

fn invalid(error: super::clean::ProductError) -> ServiceError {
    ServiceError::Invalid(error.user_message())
}

pub fn create_product(
    store: &mut Store,
    data: &Json,
    now: chrono::NaiveDateTime,
) -> Result<i64, ServiceError> {
    let draft = clean::clean_form(store, data, None).map_err(invalid)?;
    let result = store.apply(vec![Change::insert("products", values(&draft, now, true))])?;
    result
        .inserted_ids
        .first()
        .copied()
        .ok_or_else(|| ServiceError::Invalid("Không tạo được sản phẩm.".into()))
}

pub fn update_product(
    store: &mut Store,
    id: i64,
    data: &Json,
    now: chrono::NaiveDateTime,
) -> Result<(), ServiceError> {
    if !store.products.contains_key(&id) {
        return Err(ServiceError::Invalid("Không tìm thấy sản phẩm.".into()));
    }
    let draft = clean::clean_form(store, data, Some(id)).map_err(invalid)?;
    store.apply(vec![Change::update(
        "products",
        id,
        values(&draft, now, false),
    )])?;
    Ok(())
}

pub fn toggle_active(
    store: &mut Store,
    id: i64,
    now: chrono::NaiveDateTime,
) -> Result<bool, ServiceError> {
    let product = store
        .products
        .get(&id)
        .ok_or_else(|| ServiceError::Invalid("Không tìm thấy sản phẩm.".into()))?;
    let active = !product.is_active;
    store.apply(vec![Change::update(
        "products",
        id,
        BTreeMap::from([
            (
                "is_active".into(),
                ChangeValue::Sql(Value::Integer(i64::from(active))),
            ),
            (
                "updated_at".into(),
                ChangeValue::Sql(cf_db::codec::write_datetime(Some(now))),
            ),
        ]),
    )])?;
    Ok(active)
}
