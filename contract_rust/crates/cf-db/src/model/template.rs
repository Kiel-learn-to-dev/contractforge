use std::collections::BTreeMap;

use cf_core::template_rules::TemplateInfo;
use chrono::NaiveDateTime;
use rusqlite::{Row, types::Value};

use crate::{DbError, model};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractTemplate {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub contract_type: String,
    pub product_group: Option<String>,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub mode: String,
    pub detected_placeholders: Vec<String>,
    pub field_mapping: BTreeMap<String, String>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl ContractTemplate {
    pub fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
        let detected_json: Option<String> = row.get("detected_placeholders")?;
        let mapping_json: Option<String> = row.get("field_mapping")?;
        let mut detected_placeholders = detected_json
            .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
            .unwrap_or_default();
        detected_placeholders.sort();
        detected_placeholders.dedup();
        let field_mapping = mapping_json
            .and_then(|raw| serde_json::from_str::<BTreeMap<String, String>>(&raw).ok())
            .unwrap_or_default();
        Ok(Self {
            id: model::required_integer(row, "id")?,
            code: model::required_text(row, "code")?,
            name: model::required_text(row, "name")?,
            contract_type: model::required_text(row, "contract_type")?,
            product_group: model::text(row, "product_group")?,
            file_path: model::text(row, "file_path")?,
            file_name: model::text(row, "file_name")?,
            mode: model::required_text(row, "mode")?,
            detected_placeholders,
            field_mapping,
            description: model::text(row, "description")?,
            is_active: model::boolean(row, "is_active")?,
            created_at: model::datetime(row, "created_at")?.ok_or_else(|| {
                DbError::Codec("contract_templates.created_at không được NULL.".into())
            })?,
            updated_at: model::datetime(row, "updated_at")?.ok_or_else(|| {
                DbError::Codec("contract_templates.updated_at không được NULL.".into())
            })?,
        })
    }

    pub fn to_values(&self) -> BTreeMap<String, Value> {
        let detected =
            serde_json::to_string(&self.detected_placeholders).unwrap_or_else(|_| "[]".into());
        let mapping = serde_json::to_string(&self.field_mapping).unwrap_or_else(|_| "{}".into());
        BTreeMap::from([
            ("id".into(), Value::Integer(self.id)),
            ("code".into(), Value::Text(self.code.clone())),
            ("name".into(), Value::Text(self.name.clone())),
            (
                "contract_type".into(),
                Value::Text(self.contract_type.clone()),
            ),
            (
                "product_group".into(),
                model::text_value(self.product_group.as_deref()),
            ),
            (
                "file_path".into(),
                model::text_value(self.file_path.as_deref()),
            ),
            (
                "file_name".into(),
                model::text_value(self.file_name.as_deref()),
            ),
            ("mode".into(), Value::Text(self.mode.clone())),
            ("detected_placeholders".into(), Value::Text(detected)),
            ("field_mapping".into(), Value::Text(mapping)),
            (
                "description".into(),
                model::text_value(self.description.as_deref()),
            ),
            ("is_active".into(), model::bool_value(self.is_active)),
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

    pub fn to_info(&self) -> TemplateInfo {
        TemplateInfo {
            id: self.id,
            name: self.name.clone(),
            contract_type: self.contract_type.clone(),
            product_group: self.product_group.clone(),
            is_active: self.is_active,
        }
    }
}
