use std::collections::BTreeMap;

use cf_core::{pricing::ProductRule, render_context::Feature};
use chrono::NaiveDateTime;
use rusqlite::{Row, types::Value};
use rust_decimal::Decimal;

use crate::{DbError, model};

#[derive(Debug, Clone, PartialEq)]
pub struct Product {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub product_type: String,
    pub default_price: Option<Decimal>,
    pub default_vat_rate: Option<Decimal>,
    pub default_duration_months: Option<i64>,
    pub default_template_id: Option<i64>,
    pub contract_number_format: Option<String>,
    pub is_active: bool,
    pub group_code: Option<String>,
    pub kind: String,
    pub unit_label: Option<String>,
    pub price_period_months: i64,
    pub quantity_source: String,
    pub min_quantity: Option<i64>,
    pub quantity_step: Option<i64>,
    pub account_limit: Option<i64>,
    pub feature_list: Vec<Feature>,
    pub replaced_by_id: Option<i64>,
    pub sort_order: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Product {
    pub fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        let feature_json: Option<String> = row.get("feature_list")?;
        let feature_list = feature_json
            .and_then(|raw| serde_json::from_str::<Vec<Feature>>(&raw).ok())
            .unwrap_or_default();
        Ok(Self {
            id: model::required_integer(row, "id")?,
            code: model::required_text(row, "code")?,
            name: model::required_text(row, "name")?,
            product_type: model::required_text(row, "product_type")?,
            default_price: model::decimal(row, "default_price")?,
            default_vat_rate: model::decimal(row, "default_vat_rate")?,
            default_duration_months: model::integer(row, "default_duration_months")?,
            default_template_id: model::integer(row, "default_template_id")?,
            contract_number_format: model::text(row, "contract_number_format")?,
            is_active: model::boolean(row, "is_active")?,
            group_code: model::text(row, "group_code")?,
            kind: model::required_text(row, "kind")?,
            unit_label: model::text(row, "unit_label")?,
            price_period_months: model::required_integer(row, "price_period_months")?,
            quantity_source: model::required_text(row, "quantity_source")?,
            min_quantity: model::integer(row, "min_quantity")?,
            quantity_step: model::integer(row, "quantity_step")?,
            account_limit: model::integer(row, "account_limit")?,
            feature_list,
            replaced_by_id: model::integer(row, "replaced_by_id")?,
            sort_order: model::required_integer(row, "sort_order")?,
            created_at: model::datetime(row, "created_at")?
                .ok_or_else(|| DbError::Codec("products.created_at không được NULL.".into()))?,
            updated_at: model::datetime(row, "updated_at")?
                .ok_or_else(|| DbError::Codec("products.updated_at không được NULL.".into()))?,
        })
    }

    pub fn to_values(&self) -> BTreeMap<String, Value> {
        let features = serde_json::to_string(&self.feature_list).unwrap_or_else(|_| "[]".into());
        BTreeMap::from([
            ("id".into(), Value::Integer(self.id)),
            ("code".into(), Value::Text(self.code.clone())),
            ("name".into(), Value::Text(self.name.clone())),
            (
                "product_type".into(),
                Value::Text(self.product_type.clone()),
            ),
            (
                "default_price".into(),
                model::decimal_value(self.default_price),
            ),
            (
                "default_vat_rate".into(),
                model::decimal_value(self.default_vat_rate),
            ),
            (
                "default_duration_months".into(),
                model::int_value(self.default_duration_months),
            ),
            (
                "default_template_id".into(),
                model::int_value(self.default_template_id),
            ),
            (
                "contract_number_format".into(),
                model::text_value(self.contract_number_format.as_deref()),
            ),
            ("is_active".into(), model::bool_value(self.is_active)),
            (
                "group_code".into(),
                model::text_value(self.group_code.as_deref()),
            ),
            ("kind".into(), Value::Text(self.kind.clone())),
            (
                "unit_label".into(),
                model::text_value(self.unit_label.as_deref()),
            ),
            (
                "price_period_months".into(),
                Value::Integer(self.price_period_months),
            ),
            (
                "quantity_source".into(),
                Value::Text(self.quantity_source.clone()),
            ),
            ("min_quantity".into(), model::int_value(self.min_quantity)),
            ("quantity_step".into(), model::int_value(self.quantity_step)),
            ("account_limit".into(), model::int_value(self.account_limit)),
            ("feature_list".into(), Value::Text(features)),
            (
                "replaced_by_id".into(),
                model::int_value(self.replaced_by_id),
            ),
            ("sort_order".into(), Value::Integer(self.sort_order)),
            (
                "created_at".into(),
                model::datetime_value(Some(self.created_at)),
            ),
            (
                "updated_at".into(),
                model::datetime_value(Some(self.updated_at)),
            ),
        ])
    }

    pub fn to_rule(&self) -> ProductRule {
        let as_decimal = |value: Option<i64>| value.map(Decimal::from);
        ProductRule {
            id: self.id,
            name: self.name.clone(),
            kind: self.kind.clone(),
            group_code: self.group_code.clone(),
            is_active: self.is_active,
            unit_label: self.unit_label.clone().unwrap_or_default(),
            default_price: self.default_price,
            default_vat_rate: self.default_vat_rate,
            price_period_months: Some(Decimal::from(self.price_period_months)),
            min_quantity: as_decimal(self.min_quantity),
            quantity_step: as_decimal(self.quantity_step),
            quantity_source: Some(self.quantity_source.clone()),
        }
    }
}

pub fn parse_features(raw: Option<&str>) -> Vec<Feature> {
    raw.and_then(|text| serde_json::from_str::<Vec<Feature>>(text).ok())
        .unwrap_or_default()
}
