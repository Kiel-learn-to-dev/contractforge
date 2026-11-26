//! Hồ sơ người phát hành báo giá, độc lập với cài đặt Bên B.

use std::collections::BTreeMap;

use cf_db::{
    change::{Change, ChangeValue},
    model::QuotationIssuer,
    store::Store,
};
use rusqlite::types::Value;
use sha2::{Digest, Sha256};

use crate::{Ctx, ServiceError};

pub const ISSUER_FIELDS: &[&str] = &[
    "name",
    "address",
    "tax_code",
    "representative",
    "title",
    "phone",
    "email",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IssuerInput {
    pub name: String,
    pub address: String,
    pub tax_code: String,
    pub representative: String,
    pub title: String,
    pub phone: String,
    pub email: String,
}

impl IssuerInput {
    fn fields(&self) -> BTreeMap<&'static str, String> {
        BTreeMap::from([
            ("name", self.name.trim().to_owned()),
            ("address", self.address.trim().to_owned()),
            ("tax_code", self.tax_code.trim().to_owned()),
            ("representative", self.representative.trim().to_owned()),
            ("title", self.title.trim().to_owned()),
            ("phone", self.phone.trim().to_owned()),
            ("email", self.email.trim().to_owned()),
        ])
    }
}

/// Tạo chuỗi JSON tương thích `json.dumps(fields, ensure_ascii=False, sort_keys=True)`.
pub fn fingerprint(fields: &BTreeMap<&str, String>) -> String {
    let body = fields
        .iter()
        .map(|(key, value)| format!("{}: {}", json_escape(key), json_escape(value)))
        .collect::<Vec<_>>()
        .join(", ");
    let digest = Sha256::digest(format!("{{{body}}}").as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch <= '\u{001f}' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Liệt kê hồ sơ theo tên rồi ID như dịch vụ Python.
pub fn list_issuers(store: &Store) -> Vec<QuotationIssuer> {
    let mut issuers = store.issuers.values().cloned().collect::<Vec<_>>();
    issuers.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    issuers
}

/// Lưu hồ sơ theo fingerprint; dữ liệu trùng trả lại bản đã có.
pub fn save_issuer(
    ctx: &mut Ctx,
    input: IssuerInput,
) -> Result<(QuotationIssuer, bool), ServiceError> {
    let fields = input.fields();
    if fields["name"].is_empty() {
        return Err(ServiceError::Invalid(
            "Nhập tên đơn vị báo giá trước khi lưu.".into(),
        ));
    }
    if fields.values().any(|value| value.chars().count() > 500) {
        return Err(ServiceError::Invalid(
            "Mỗi ô thông tin đơn vị báo giá tối đa 500 ký tự.".into(),
        ));
    }
    let fingerprint = fingerprint(&fields);
    if let Some(existing) = ctx
        .store()
        .issuers
        .values()
        .find(|issuer| issuer.fingerprint == fingerprint)
    {
        return Ok((existing.clone(), false));
    }
    let mut values = BTreeMap::new();
    for (field, value) in &fields {
        values.insert(
            (*field).to_owned(),
            ChangeValue::Sql(Value::Text(value.clone())),
        );
    }
    values.insert(
        "fingerprint".into(),
        ChangeValue::Sql(Value::Text(fingerprint.clone())),
    );
    if let Err(error) = ctx
        .store_mut()
        .apply(vec![Change::insert("quotation_issuers", values)])
    {
        ctx.store_mut().reload()?;
        if let Some(existing) = ctx
            .store()
            .issuers
            .values()
            .find(|issuer| issuer.fingerprint == fingerprint)
        {
            return Ok((existing.clone(), false));
        }
        return Err(error.into());
    }
    let issuer = ctx
        .store()
        .issuers
        .values()
        .find(|issuer| issuer.fingerprint == fingerprint)
        .cloned()
        .ok_or_else(|| ServiceError::Invalid("Không lưu được hồ sơ đơn vị báo giá.".into()))?;
    Ok((issuer, true))
}
