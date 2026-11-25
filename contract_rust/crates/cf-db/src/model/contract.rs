use std::collections::BTreeMap;

use cf_core::{
    lifecycle::{ContractStatus, Evidence},
    render_context::ContractView,
};
use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::{Row, types::Value};
use rust_decimal::Decimal;
use serde_json::Value as JsonValue;

use crate::{DbError, model};

#[derive(Debug, Clone, PartialEq)]
pub struct Contract {
    pub id: i64,
    pub contract_number: String,
    pub status: ContractStatus,
    pub customer_id: i64,
    pub product_id: Option<i64>,
    pub template_id: Option<i64>,
    pub sign_date: Option<NaiveDate>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub month_count: Option<i64>,
    pub sign_location: Option<String>,
    pub unit_count: Option<i64>,
    pub quantity: Option<Decimal>,
    pub unit_price_pre_vat: Option<Decimal>,
    pub vat_rate: Option<Decimal>,
    pub unit_price_vat: Option<Decimal>,
    pub total_amount: Option<Decimal>,
    pub amount_text: Option<String>,
    pub pricing_description: Option<String>,
    pub party_b_representative: Option<String>,
    pub party_b_authorization_number: Option<String>,
    pub party_b_authorization_date: Option<String>,
    pub acceptance_date: Option<NaiveDate>,
    pub liquidation_date: Option<NaiveDate>,
    pub output_file_path: Option<String>,
    pub output_file_name: Option<String>,
    pub signed_pdf_path: Option<String>,
    pub signed_pdf_name: Option<String>,
    pub signed_pdf_uploaded_at: Option<NaiveDateTime>,
    pub invoice_date: Option<NaiveDate>,
    pub invoice_pdf_path: Option<String>,
    pub invoice_pdf_name: Option<String>,
    pub invoice_pdf_uploaded_at: Option<NaiveDateTime>,
    pub payment_date: Option<NaiveDate>,
    pub payment_slip_path: Option<String>,
    pub payment_slip_name: Option<String>,
    pub payment_slip_uploaded_at: Option<NaiveDateTime>,
    pub selected_unit_names: Vec<String>,
    pub render_data: JsonValue,
    pub notes: Option<String>,
    pub is_batch: bool,
    pub batch_job_id: Option<String>,
    pub pricing_model: String,
    pub total_pre_vat: Option<Decimal>,
    pub total_vat: Option<Decimal>,
    pub payment_mode: Option<String>,
    pub payment_cycle_months: Option<i64>,
    pub parent_contract_id: Option<i64>,
    pub renewed_from_id: Option<i64>,
    pub doc_kind: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Contract {
    pub fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        let status_text = model::required_text(row, "status")?;
        let selected_json: Option<String> = row.get("selected_unit_names")?;
        let render_json: Option<String> = row.get("render_data")?;
        Ok(Self {
            id: model::required_integer(row, "id")?,
            contract_number: model::required_text(row, "contract_number")?,
            status: status_text
                .parse::<ContractStatus>()
                .unwrap_or(ContractStatus::Unknown(status_text)),
            customer_id: model::required_integer(row, "customer_id")?,
            product_id: model::integer(row, "product_id")?,
            template_id: model::integer(row, "template_id")?,
            sign_date: model::date(row, "sign_date")?,
            start_date: model::date(row, "start_date")?,
            end_date: model::date(row, "end_date")?,
            month_count: model::integer(row, "month_count")?,
            sign_location: model::text(row, "sign_location")?,
            unit_count: model::integer(row, "unit_count")?,
            quantity: model::decimal(row, "quantity")?,
            unit_price_pre_vat: model::decimal(row, "unit_price_pre_vat")?,
            vat_rate: model::decimal(row, "vat_rate")?,
            unit_price_vat: model::decimal(row, "unit_price_vat")?,
            total_amount: model::decimal(row, "total_amount")?,
            amount_text: model::text(row, "amount_text")?,
            pricing_description: model::text(row, "pricing_description")?,
            party_b_representative: model::text(row, "party_b_representative")?,
            party_b_authorization_number: model::text(row, "party_b_authorization_number")?,
            party_b_authorization_date: model::text(row, "party_b_authorization_date")?,
            acceptance_date: model::date(row, "acceptance_date")?,
            liquidation_date: model::date(row, "liquidation_date")?,
            output_file_path: model::text(row, "output_file_path")?,
            output_file_name: model::text(row, "output_file_name")?,
            signed_pdf_path: model::text(row, "signed_pdf_path")?,
            signed_pdf_name: model::text(row, "signed_pdf_name")?,
            signed_pdf_uploaded_at: model::datetime(row, "signed_pdf_uploaded_at")?,
            invoice_date: model::date(row, "invoice_date")?,
            invoice_pdf_path: model::text(row, "invoice_pdf_path")?,
            invoice_pdf_name: model::text(row, "invoice_pdf_name")?,
            invoice_pdf_uploaded_at: model::datetime(row, "invoice_pdf_uploaded_at")?,
            payment_date: model::date(row, "payment_date")?,
            payment_slip_path: model::text(row, "payment_slip_path")?,
            payment_slip_name: model::text(row, "payment_slip_name")?,
            payment_slip_uploaded_at: model::datetime(row, "payment_slip_uploaded_at")?,
            selected_unit_names: selected_json
                .and_then(|raw| serde_json::from_str(&raw).ok())
                .unwrap_or_default(),
            render_data: render_json
                .and_then(|raw| serde_json::from_str(&raw).ok())
                .unwrap_or(JsonValue::Null),
            notes: model::text(row, "notes")?,
            is_batch: model::boolean(row, "is_batch")?,
            batch_job_id: model::text(row, "batch_job_id")?,
            pricing_model: model::required_text(row, "pricing_model")?,
            total_pre_vat: model::decimal(row, "total_pre_vat")?,
            total_vat: model::decimal(row, "total_vat")?,
            payment_mode: model::text(row, "payment_mode")?,
            payment_cycle_months: model::integer(row, "payment_cycle_months")?,
            parent_contract_id: model::integer(row, "parent_contract_id")?,
            renewed_from_id: model::integer(row, "renewed_from_id")?,
            doc_kind: model::required_text(row, "doc_kind")?,
            created_at: model::datetime(row, "created_at")?
                .ok_or_else(|| DbError::Codec("contracts.created_at không được NULL.".into()))?,
            updated_at: model::datetime(row, "updated_at")?
                .ok_or_else(|| DbError::Codec("contracts.updated_at không được NULL.".into()))?,
        })
    }

    pub fn to_values(&self) -> BTreeMap<String, Value> {
        let selected =
            serde_json::to_string(&self.selected_unit_names).unwrap_or_else(|_| "[]".into());
        let render = serde_json::to_string(&self.render_data).unwrap_or_else(|_| "null".into());
        BTreeMap::from([
            ("id".into(), Value::Integer(self.id)),
            (
                "contract_number".into(),
                Value::Text(self.contract_number.clone()),
            ),
            (
                "status".into(),
                Value::Text(self.status.as_str().to_owned()),
            ),
            ("customer_id".into(), Value::Integer(self.customer_id)),
            ("product_id".into(), model::int_value(self.product_id)),
            ("template_id".into(), model::int_value(self.template_id)),
            ("sign_date".into(), model::date_value(self.sign_date)),
            ("start_date".into(), model::date_value(self.start_date)),
            ("end_date".into(), model::date_value(self.end_date)),
            ("month_count".into(), model::int_value(self.month_count)),
            (
                "sign_location".into(),
                model::text_value(self.sign_location.as_deref()),
            ),
            ("unit_count".into(), model::int_value(self.unit_count)),
            ("quantity".into(), model::decimal_value(self.quantity)),
            (
                "unit_price_pre_vat".into(),
                model::decimal_value(self.unit_price_pre_vat),
            ),
            ("vat_rate".into(), model::decimal_value(self.vat_rate)),
            (
                "unit_price_vat".into(),
                model::decimal_value(self.unit_price_vat),
            ),
            (
                "total_amount".into(),
                model::decimal_value(self.total_amount),
            ),
            (
                "amount_text".into(),
                model::text_value(self.amount_text.as_deref()),
            ),
            (
                "pricing_description".into(),
                model::text_value(self.pricing_description.as_deref()),
            ),
            (
                "party_b_representative".into(),
                model::text_value(self.party_b_representative.as_deref()),
            ),
            (
                "party_b_authorization_number".into(),
                model::text_value(self.party_b_authorization_number.as_deref()),
            ),
            (
                "party_b_authorization_date".into(),
                model::text_value(self.party_b_authorization_date.as_deref()),
            ),
            (
                "acceptance_date".into(),
                model::date_value(self.acceptance_date),
            ),
            (
                "liquidation_date".into(),
                model::date_value(self.liquidation_date),
            ),
            (
                "output_file_path".into(),
                model::text_value(self.output_file_path.as_deref()),
            ),
            (
                "output_file_name".into(),
                model::text_value(self.output_file_name.as_deref()),
            ),
            (
                "signed_pdf_path".into(),
                model::text_value(self.signed_pdf_path.as_deref()),
            ),
            (
                "signed_pdf_name".into(),
                model::text_value(self.signed_pdf_name.as_deref()),
            ),
            (
                "signed_pdf_uploaded_at".into(),
                model::datetime_value(self.signed_pdf_uploaded_at),
            ),
            ("invoice_date".into(), model::date_value(self.invoice_date)),
            (
                "invoice_pdf_path".into(),
                model::text_value(self.invoice_pdf_path.as_deref()),
            ),
            (
                "invoice_pdf_name".into(),
                model::text_value(self.invoice_pdf_name.as_deref()),
            ),
            (
                "invoice_pdf_uploaded_at".into(),
                model::datetime_value(self.invoice_pdf_uploaded_at),
            ),
            ("payment_date".into(), model::date_value(self.payment_date)),
            (
                "payment_slip_path".into(),
                model::text_value(self.payment_slip_path.as_deref()),
            ),
            (
                "payment_slip_name".into(),
                model::text_value(self.payment_slip_name.as_deref()),
            ),
            (
                "payment_slip_uploaded_at".into(),
                model::datetime_value(self.payment_slip_uploaded_at),
            ),
            ("selected_unit_names".into(), Value::Text(selected)),
            ("render_data".into(), Value::Text(render)),
            ("notes".into(), model::text_value(self.notes.as_deref())),
            ("is_batch".into(), model::bool_value(self.is_batch)),
            (
                "batch_job_id".into(),
                model::text_value(self.batch_job_id.as_deref()),
            ),
            (
                "pricing_model".into(),
                Value::Text(self.pricing_model.clone()),
            ),
            (
                "total_pre_vat".into(),
                model::decimal_value(self.total_pre_vat),
            ),
            ("total_vat".into(), model::decimal_value(self.total_vat)),
            (
                "payment_mode".into(),
                model::text_value(self.payment_mode.as_deref()),
            ),
            (
                "payment_cycle_months".into(),
                model::int_value(self.payment_cycle_months),
            ),
            (
                "parent_contract_id".into(),
                model::int_value(self.parent_contract_id),
            ),
            (
                "renewed_from_id".into(),
                model::int_value(self.renewed_from_id),
            ),
            ("doc_kind".into(), Value::Text(self.doc_kind.clone())),
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

    pub fn to_view(&self) -> ContractView {
        ContractView {
            contract_number: self.contract_number.clone(),
            doc_kind: self.doc_kind.clone(),
            status: self.status.clone(),
            party_b_representative: self.party_b_representative.clone(),
            party_b_authorization_number: self.party_b_authorization_number.clone(),
            party_b_authorization_date: self.party_b_authorization_date.clone(),
            total_pre_vat: self.total_pre_vat,
            total_vat: self.total_vat,
            payment_cycle_months: self.payment_cycle_months,
            sign_location: self.sign_location.clone(),
            sign_date: self.sign_date,
            start_date: self.start_date,
            end_date: self.end_date,
            month_count: self.month_count,
            unit_count: self.unit_count,
            quantity: self.quantity,
            unit_price_pre_vat: self.unit_price_pre_vat,
            vat_rate: self.vat_rate,
            unit_price_vat: self.unit_price_vat,
            total_amount: self.total_amount,
            amount_text: self.amount_text.clone(),
            pricing_description: self.pricing_description.clone(),
            acceptance_date: self.acceptance_date,
            liquidation_date: self.liquidation_date,
            contract_id: self.id,
        }
    }

    pub fn evidence(&self) -> Evidence {
        Evidence {
            has_invoice: self.invoice_pdf_path.is_some(),
            has_payment_slip: self.payment_slip_path.is_some(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContractItem {
    pub id: i64,
    pub contract_id: i64,
    pub product_id: Option<i64>,
    pub sort_order: i64,
    pub description: String,
    pub unit_label: Option<String>,
    pub quantity: Decimal,
    pub period_count: Decimal,
    pub unit_price_pre_vat: Option<Decimal>,
    pub vat_rate: Decimal,
    pub amount_pre_vat: Decimal,
    pub vat_amount: Decimal,
    pub amount_total: Decimal,
    pub note: Option<String>,
}

impl ContractItem {
    pub fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        let required_decimal = |column: &str| {
            model::decimal(row, column)?
                .ok_or_else(|| DbError::Codec(format!("contract_items.{column} không được NULL.")))
        };
        Ok(Self {
            id: model::required_integer(row, "id")?,
            contract_id: model::required_integer(row, "contract_id")?,
            product_id: model::integer(row, "product_id")?,
            sort_order: model::required_integer(row, "sort_order")?,
            description: model::required_text(row, "description")?,
            unit_label: model::text(row, "unit_label")?,
            quantity: required_decimal("quantity")?,
            period_count: required_decimal("period_count")?,
            unit_price_pre_vat: model::decimal(row, "unit_price_pre_vat")?,
            vat_rate: required_decimal("vat_rate")?,
            amount_pre_vat: required_decimal("amount_pre_vat")?,
            vat_amount: required_decimal("vat_amount")?,
            amount_total: required_decimal("amount_total")?,
            note: model::text(row, "note")?,
        })
    }

    pub fn to_values(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("id".into(), Value::Integer(self.id)),
            ("contract_id".into(), Value::Integer(self.contract_id)),
            ("product_id".into(), model::int_value(self.product_id)),
            ("sort_order".into(), Value::Integer(self.sort_order)),
            ("description".into(), Value::Text(self.description.clone())),
            (
                "unit_label".into(),
                model::text_value(self.unit_label.as_deref()),
            ),
            ("quantity".into(), model::decimal_value(Some(self.quantity))),
            (
                "period_count".into(),
                model::decimal_value(Some(self.period_count)),
            ),
            (
                "unit_price_pre_vat".into(),
                model::decimal_value(self.unit_price_pre_vat),
            ),
            ("vat_rate".into(), model::decimal_value(Some(self.vat_rate))),
            (
                "amount_pre_vat".into(),
                model::decimal_value(Some(self.amount_pre_vat)),
            ),
            (
                "vat_amount".into(),
                model::decimal_value(Some(self.vat_amount)),
            ),
            (
                "amount_total".into(),
                model::decimal_value(Some(self.amount_total)),
            ),
            ("note".into(), model::text_value(self.note.as_deref())),
        ])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractEvent {
    pub id: i64,
    pub contract_id: i64,
    pub event_type: String,
    pub description: Option<String>,
    pub actor: Option<String>,
    pub meta_json: Option<String>,
    pub occurred_at: NaiveDateTime,
}

impl ContractEvent {
    pub fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: model::required_integer(row, "id")?,
            contract_id: model::required_integer(row, "contract_id")?,
            event_type: model::required_text(row, "event_type")?,
            description: model::text(row, "description")?,
            actor: model::text(row, "actor")?,
            meta_json: model::text(row, "meta_json")?,
            occurred_at: model::datetime(row, "occurred_at")?.ok_or_else(|| {
                DbError::Codec("contract_events.occurred_at không được NULL.".into())
            })?,
        })
    }

    pub fn to_values(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("id".into(), Value::Integer(self.id)),
            ("contract_id".into(), Value::Integer(self.contract_id)),
            ("event_type".into(), Value::Text(self.event_type.clone())),
            (
                "description".into(),
                model::text_value(self.description.as_deref()),
            ),
            ("actor".into(), model::text_value(self.actor.as_deref())),
            (
                "meta_json".into(),
                model::text_value(self.meta_json.as_deref()),
            ),
            (
                "occurred_at".into(),
                model::datetime_value(Some(self.occurred_at)),
            ),
        ])
    }
}
