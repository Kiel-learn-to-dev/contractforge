use std::collections::BTreeMap;

use cf_core::render_context::{CustomerView, UnitView};
use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::{Row, types::Value};

use crate::{DbError, model};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Customer {
    pub id: i64,
    pub code: String,
    pub legal_name: String,
    pub short_name: Option<String>,
    pub customer_type: String,
    pub address: Option<String>,
    pub ward: Option<String>,
    pub district: Option<String>,
    pub province: Option<String>,
    pub tax_code: Option<String>,
    pub representative_name: Option<String>,
    pub representative_gender: Option<String>,
    pub representative_title: Option<String>,
    pub bank_account: Option<String>,
    pub bank_name: Option<String>,
    pub beneficiary_name: Option<String>,
    pub contact_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
    pub is_deleted: bool,
    pub deleted_at: Option<NaiveDateTime>,
    pub bill_main_station: bool,
    pub record_count: Option<i64>,
    pub budget_relation_code: Option<String>,
    pub kcb_code: Option<String>,
    pub establishment_decision: Option<String>,
    pub establishment_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Customer {
    pub fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: model::required_integer(row, "id")?,
            code: model::required_text(row, "code")?,
            legal_name: model::required_text(row, "legal_name")?,
            short_name: model::text(row, "short_name")?,
            customer_type: model::required_text(row, "customer_type")?,
            address: model::text(row, "address")?,
            ward: model::text(row, "ward")?,
            district: model::text(row, "district")?,
            province: model::text(row, "province")?,
            tax_code: model::text(row, "tax_code")?,
            representative_name: model::text(row, "representative_name")?,
            representative_gender: model::text(row, "representative_gender")?,
            representative_title: model::text(row, "representative_title")?,
            bank_account: model::text(row, "bank_account")?,
            bank_name: model::text(row, "bank_name")?,
            beneficiary_name: model::text(row, "beneficiary_name")?,
            contact_name: model::text(row, "contact_name")?,
            phone: model::text(row, "phone")?,
            email: model::text(row, "email")?,
            is_active: model::boolean(row, "is_active")?,
            is_deleted: model::boolean(row, "is_deleted")?,
            deleted_at: model::datetime(row, "deleted_at")?,
            bill_main_station: model::boolean(row, "bill_main_station")?,
            record_count: model::integer(row, "record_count")?,
            budget_relation_code: model::text(row, "budget_relation_code")?,
            kcb_code: model::text(row, "kcb_code")?,
            establishment_decision: model::text(row, "establishment_decision")?,
            establishment_date: model::date(row, "establishment_date")?,
            notes: model::text(row, "notes")?,
            created_at: model::datetime(row, "created_at")?
                .ok_or_else(|| DbError::Codec("customers.created_at không được NULL.".into()))?,
            updated_at: model::datetime(row, "updated_at")?
                .ok_or_else(|| DbError::Codec("customers.updated_at không được NULL.".into()))?,
        })
    }

    pub fn to_values(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("id".into(), Value::Integer(self.id)),
            ("code".into(), Value::Text(self.code.clone())),
            ("legal_name".into(), Value::Text(self.legal_name.clone())),
            (
                "short_name".into(),
                model::text_value(self.short_name.as_deref()),
            ),
            (
                "customer_type".into(),
                Value::Text(self.customer_type.clone()),
            ),
            ("address".into(), model::text_value(self.address.as_deref())),
            ("ward".into(), model::text_value(self.ward.as_deref())),
            (
                "district".into(),
                model::text_value(self.district.as_deref()),
            ),
            (
                "province".into(),
                model::text_value(self.province.as_deref()),
            ),
            (
                "tax_code".into(),
                model::text_value(self.tax_code.as_deref()),
            ),
            (
                "representative_name".into(),
                model::text_value(self.representative_name.as_deref()),
            ),
            (
                "representative_gender".into(),
                model::text_value(self.representative_gender.as_deref()),
            ),
            (
                "representative_title".into(),
                model::text_value(self.representative_title.as_deref()),
            ),
            (
                "bank_account".into(),
                model::text_value(self.bank_account.as_deref()),
            ),
            (
                "bank_name".into(),
                model::text_value(self.bank_name.as_deref()),
            ),
            (
                "beneficiary_name".into(),
                model::text_value(self.beneficiary_name.as_deref()),
            ),
            (
                "contact_name".into(),
                model::text_value(self.contact_name.as_deref()),
            ),
            ("phone".into(), model::text_value(self.phone.as_deref())),
            ("email".into(), model::text_value(self.email.as_deref())),
            ("is_active".into(), model::bool_value(self.is_active)),
            ("is_deleted".into(), model::bool_value(self.is_deleted)),
            ("deleted_at".into(), model::datetime_value(self.deleted_at)),
            (
                "bill_main_station".into(),
                model::bool_value(self.bill_main_station),
            ),
            ("record_count".into(), model::int_value(self.record_count)),
            (
                "budget_relation_code".into(),
                model::text_value(self.budget_relation_code.as_deref()),
            ),
            (
                "kcb_code".into(),
                model::text_value(self.kcb_code.as_deref()),
            ),
            (
                "establishment_decision".into(),
                model::text_value(self.establishment_decision.as_deref()),
            ),
            (
                "establishment_date".into(),
                model::date_value(self.establishment_date),
            ),
            ("notes".into(), model::text_value(self.notes.as_deref())),
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

    pub fn to_view(&self) -> CustomerView {
        self.to_view_with_units(&[])
    }

    pub fn to_view_with_units(&self, units: &[CustomerUnit]) -> CustomerView {
        CustomerView {
            legal_name: self.legal_name.clone(),
            short_name: self.short_name.clone().unwrap_or_default(),
            code: self.code.clone(),
            address: self.address.clone().unwrap_or_default(),
            ward: self.ward.clone().unwrap_or_default(),
            province: self.province.clone().unwrap_or_default(),
            representative_name: self.representative_name.clone().unwrap_or_default(),
            representative_gender: self.representative_gender.clone().unwrap_or_default(),
            representative_title: self.representative_title.clone().unwrap_or_default(),
            tax_code: self.tax_code.clone().unwrap_or_default(),
            bank_account: self.bank_account.clone().unwrap_or_default(),
            bank_name: self.bank_name.clone().unwrap_or_default(),
            beneficiary_name: self.beneficiary_name.clone().unwrap_or_default(),
            budget_relation_code: self.budget_relation_code.clone().unwrap_or_default(),
            phone: self.phone.clone().unwrap_or_default(),
            email: self.email.clone().unwrap_or_default(),
            contact_name: self.contact_name.clone().unwrap_or_default(),
            establishment_decision: self.establishment_decision.clone().unwrap_or_default(),
            establishment_date: self.establishment_date,
            bill_main_station: self.bill_main_station,
            record_count: self.record_count,
            units: units
                .iter()
                .map(|unit| UnitView {
                    name: unit.name.clone(),
                    is_active: unit.is_active,
                    include_in_billing: unit.include_in_billing,
                    unit_type: unit.unit_type.clone(),
                })
                .collect::<Vec<UnitView>>(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerUnit {
    pub id: i64,
    pub customer_id: i64,
    pub name: String,
    pub unit_type: String,
    pub address: Option<String>,
    pub notes: Option<String>,
    pub is_active: bool,
    pub include_in_billing: bool,
    pub sort_order: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl CustomerUnit {
    pub fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: model::required_integer(row, "id")?,
            customer_id: model::required_integer(row, "customer_id")?,
            name: model::required_text(row, "name")?,
            unit_type: model::required_text(row, "unit_type")?,
            address: model::text(row, "address")?,
            notes: model::text(row, "notes")?,
            is_active: model::boolean(row, "is_active")?,
            include_in_billing: model::boolean(row, "include_in_billing")?,
            sort_order: model::required_integer(row, "sort_order")?,
            created_at: model::datetime(row, "created_at")?.ok_or_else(|| {
                DbError::Codec("customer_units.created_at không được NULL.".into())
            })?,
            updated_at: model::datetime(row, "updated_at")?.ok_or_else(|| {
                DbError::Codec("customer_units.updated_at không được NULL.".into())
            })?,
        })
    }

    pub fn to_values(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("id".into(), Value::Integer(self.id)),
            ("customer_id".into(), Value::Integer(self.customer_id)),
            ("name".into(), Value::Text(self.name.clone())),
            ("unit_type".into(), Value::Text(self.unit_type.clone())),
            ("address".into(), model::text_value(self.address.as_deref())),
            ("notes".into(), model::text_value(self.notes.as_deref())),
            ("is_active".into(), model::bool_value(self.is_active)),
            (
                "include_in_billing".into(),
                model::bool_value(self.include_in_billing),
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerDocument {
    pub id: i64,
    pub customer_id: i64,
    pub doc_type: String,
    pub label: Option<String>,
    pub file_path: String,
    pub file_name: String,
    pub mime_hint: Option<String>,
    pub uploaded_at: NaiveDateTime,
}

impl CustomerDocument {
    pub fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: model::required_integer(row, "id")?,
            customer_id: model::required_integer(row, "customer_id")?,
            doc_type: model::required_text(row, "doc_type")?,
            label: model::text(row, "label")?,
            file_path: model::required_text(row, "file_path")?,
            file_name: model::required_text(row, "file_name")?,
            mime_hint: model::text(row, "mime_hint")?,
            uploaded_at: model::datetime(row, "uploaded_at")?.ok_or_else(|| {
                DbError::Codec("customer_documents.uploaded_at không được NULL.".into())
            })?,
        })
    }

    pub fn to_values(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("id".into(), Value::Integer(self.id)),
            ("customer_id".into(), Value::Integer(self.customer_id)),
            ("doc_type".into(), Value::Text(self.doc_type.clone())),
            ("label".into(), model::text_value(self.label.as_deref())),
            ("file_path".into(), Value::Text(self.file_path.clone())),
            ("file_name".into(), Value::Text(self.file_name.clone())),
            (
                "mime_hint".into(),
                model::text_value(self.mime_hint.as_deref()),
            ),
            (
                "uploaded_at".into(),
                model::datetime_value(Some(self.uploaded_at)),
            ),
        ])
    }
}
