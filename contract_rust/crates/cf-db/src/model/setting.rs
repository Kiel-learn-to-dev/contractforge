use std::collections::BTreeMap;

use cf_core::render_context::PartyB;
use rusqlite::{Row, types::Value};

use crate::{DbError, model};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSetting {
    pub key: String,
    pub value: Option<String>,
}

impl AppSetting {
    pub fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            key: model::required_text(row, "key")?,
            value: model::text(row, "value")?,
        })
    }

    pub fn to_values(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("key".into(), Value::Text(self.key.clone())),
            ("value".into(), model::text_value(self.value.as_deref())),
        ])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotationIssuer {
    pub id: i64,
    pub name: String,
    pub address: String,
    pub tax_code: String,
    pub representative: String,
    pub title: String,
    pub phone: String,
    pub email: String,
    pub fingerprint: String,
}

impl QuotationIssuer {
    pub fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        Ok(Self {
            id: model::required_integer(row, "id")?,
            name: model::required_text(row, "name")?,
            address: model::required_text(row, "address")?,
            tax_code: model::required_text(row, "tax_code")?,
            representative: model::required_text(row, "representative")?,
            title: model::required_text(row, "title")?,
            phone: model::required_text(row, "phone")?,
            email: model::required_text(row, "email")?,
            fingerprint: model::required_text(row, "fingerprint")?,
        })
    }

    pub fn to_values(&self) -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("id".into(), Value::Integer(self.id)),
            ("name".into(), Value::Text(self.name.clone())),
            ("address".into(), Value::Text(self.address.clone())),
            ("tax_code".into(), Value::Text(self.tax_code.clone())),
            (
                "representative".into(),
                Value::Text(self.representative.clone()),
            ),
            ("title".into(), Value::Text(self.title.clone())),
            ("phone".into(), Value::Text(self.phone.clone())),
            ("email".into(), Value::Text(self.email.clone())),
            ("fingerprint".into(), Value::Text(self.fingerprint.clone())),
        ])
    }
}

pub fn get_setting(settings: &[AppSetting], key: &str, default: &str) -> String {
    settings
        .iter()
        .find(|setting| setting.key == key)
        .and_then(|setting| setting.value.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_owned()
}

pub fn party_b(settings: &[AppSetting]) -> PartyB {
    PartyB {
        name: get_setting(settings, "party_b_name", ""),
        address: get_setting(settings, "party_b_address", ""),
        bank_account: get_setting(settings, "party_b_bank_account", ""),
        bank_name: get_setting(settings, "party_b_bank_name", ""),
        beneficiary: get_setting(settings, "party_b_beneficiary", ""),
        tax_code: get_setting(settings, "party_b_tax_code", ""),
        title: get_setting(settings, "party_b_title", ""),
        representative: get_setting(settings, "party_b_representative", ""),
        authorization_number: get_setting(settings, "party_b_authorization_number", ""),
        authorization_date: get_setting(settings, "party_b_authorization_date", ""),
        phone: get_setting(settings, "party_b_phone", ""),
        contact_name: get_setting(settings, "party_b_contact_name", ""),
        contact_email: get_setting(settings, "party_b_contact_email", ""),
    }
}
