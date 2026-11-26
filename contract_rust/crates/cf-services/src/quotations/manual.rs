use std::collections::BTreeMap;

use cf_core::{
    fmt::{fmt_money, fmt_number},
    money::to_decimal,
    pricing::{self, Line},
    render_context::{self, Feature},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value;

use crate::ServiceError;

pub const LAYOUTS: &[(&str, &str, &str)] = &[
    (
        "compact",
        "Báo giá gọn",
        "Bảng giá và thông tin chính, ưu tiên một trang.",
    ),
    (
        "formal",
        "Báo giá trang trọng",
        "Đầu văn bản, nội dung chào giá và chữ ký.",
    ),
    (
        "landscape",
        "Báo giá bảng ngang",
        "Bảng rộng, tách rõ số lượng, kỳ và thuế.",
    ),
    (
        "features",
        "Báo giá kèm chức năng",
        "Bảng giá và phạm vi chức năng của dịch vụ.",
    ),
    (
        "terms",
        "Báo giá kèm điều kiện cung cấp",
        "Triển khai, hỗ trợ và điều kiện thanh toán.",
    ),
];
pub const ISSUER_FIELDS: &[&str] = &[
    "name",
    "address",
    "tax_code",
    "representative",
    "title",
    "phone",
    "email",
];

#[derive(Debug, Clone, PartialEq)]
pub struct ManualQuotationDocument {
    pub layout: String,
    pub context: BTreeMap<String, String>,
    pub lists: BTreeMap<String, Vec<BTreeMap<String, String>>>,
    pub file_name: String,
}

pub fn parse_rows(raw: &str) -> Result<Vec<Value>, ServiceError> {
    let rows: Value = serde_json::from_str(if raw.trim().is_empty() { "[]" } else { raw })
        .map_err(|_| ServiceError::Invalid("Danh sách hạng mục không hợp lệ.".into()))?;
    let rows = rows
        .as_array()
        .filter(|rows| (1..=100).contains(&rows.len()) && rows.iter().all(Value::is_object))
        .ok_or_else(|| ServiceError::Invalid("Báo giá cần từ 1 đến 100 hạng mục.".into()))?;
    Ok(rows.clone())
}

pub fn build_lines(raw: &str) -> Result<(Vec<Line>, pricing::Totals), ServiceError> {
    let mut lines = Vec::new();
    for (index, row) in parse_rows(raw)?.iter().enumerate() {
        let row_no = index + 1;
        let name = field_text(row, "name");
        if name.is_empty() {
            return Err(ServiceError::Invalid(format!(
                "Hạng mục {row_no} thiếu tên."
            )));
        }
        if name.chars().count() > 500 {
            return Err(ServiceError::Invalid(format!(
                "Tên hạng mục {row_no} quá dài (tối đa 500 ký tự)."
            )));
        }
        let quantity = parse_number(
            row.get("quantity"),
            &format!("Số lượng dòng {row_no}"),
            false,
        )?;
        let periods = parse_number(row.get("periods"), &format!("Số kỳ dòng {row_no}"), false)?;
        let price = parse_number(row.get("price"), &format!("Đơn giá dòng {row_no}"), true)?;
        let vat = parse_number(row.get("vat"), &format!("Thuế suất dòng {row_no}"), false)?;
        let result = pricing::price_line(pricing::LineInput {
            description: name,
            unit_label: field_text(row, "unit"),
            quantity,
            months: periods,
            unit_price_pre_vat: price,
            vat_rate: vat,
            price_period_months: Decimal::ONE,
            amount_pre_vat_override: None,
            product_id: None,
            note: String::new(),
        })
        .map_err(|error| {
            ServiceError::Invalid(format!("Giá trị dòng {row_no} không hợp lệ: {error}"))
        })?;
        lines.push(result);
    }
    let totals = pricing::sum_lines(&lines)?;
    Ok((lines, totals))
}

pub fn build_document(
    values: &BTreeMap<String, String>,
    features: &[Feature],
) -> Result<ManualQuotationDocument, ServiceError> {
    let layout = clean(values, "layout");
    if !LAYOUTS.iter().any(|(key, _, _)| *key == layout) {
        return Err(ServiceError::Invalid("Mẫu báo giá không hợp lệ.".into()));
    }
    if clean(values, "issuer_name").is_empty() {
        return Err(ServiceError::Invalid("Nhập tên đơn vị báo giá.".into()));
    }
    if clean(values, "recipient_name").is_empty() {
        return Err(ServiceError::Invalid(
            "Nhập tên khách hàng nhận báo giá.".into(),
        ));
    }
    let quote_date = parse_date(
        values.get("quote_date"),
        "Ngày báo giá hoặc ngày hết hiệu lực không hợp lệ.",
    )?
    .ok_or_else(|| ServiceError::Invalid("Ngày báo giá không hợp lệ.".into()))?;
    let valid_until = if clean(values, "valid_until").is_empty() {
        None
    } else {
        Some(
            parse_date(
                values.get("valid_until"),
                "Ngày báo giá hoặc ngày hết hiệu lực không hợp lệ.",
            )?
            .ok_or_else(|| ServiceError::Invalid("Ngày hết hiệu lực không hợp lệ.".into()))?,
        )
    };
    if valid_until.is_some_and(|date| date < quote_date) {
        return Err(ServiceError::Invalid(
            "Ngày hết hiệu lực không được trước ngày báo giá.".into(),
        ));
    }
    let (lines, totals) = build_lines(
        values
            .get("items_json")
            .map(String::as_str)
            .unwrap_or_default(),
    )?;
    let mut context = BTreeMap::new();
    for field in ISSUER_FIELDS {
        context.insert(
            format!("QUOTE_ISSUER_{}", field.to_uppercase()),
            clean(values, &format!("issuer_{field}")),
        );
    }
    context.extend(BTreeMap::from([
        ("PARTY_A_NAME".into(), clean(values, "recipient_name")),
        ("PARTY_A_ADDRESS".into(), clean(values, "recipient_address")),
        (
            "PARTY_A_TAX_CODE".into(),
            clean(values, "recipient_tax_code"),
        ),
        (
            "PARTY_A_CONTACT_NAME".into(),
            clean(values, "recipient_contact"),
        ),
        (
            "QUOTE_DATE".into(),
            quote_date.format("%d/%m/%Y").to_string(),
        ),
        ("QUOTE_DATE_DAY".into(), quote_date.day().to_string()),
        ("QUOTE_DATE_MONTH".into(), quote_date.month().to_string()),
        ("QUOTE_DATE_YEAR".into(), quote_date.year().to_string()),
        (
            "QUOTE_VALID_UNTIL".into(),
            valid_until
                .map(|date| date.format("%d/%m/%Y").to_string())
                .unwrap_or_default(),
        ),
        ("TOTAL_PRE_VAT".into(), fmt_money(totals.pre_vat)),
        ("TOTAL_VAT".into(), fmt_money(totals.vat)),
        ("TOTAL_AMOUNT".into(), fmt_money(totals.total)),
        ("AMOUNT_TEXT".into(), totals.amount_text.clone()),
        ("QUOTE_NUMBER".into(), clean(values, "quote_number")),
        ("QUOTE_PLACE".into(), clean(values, "place")),
        ("QUOTE_SUBJECT".into(), clean(values, "subject")),
        ("QUOTE_PAYMENT_TERMS".into(), clean(values, "payment_terms")),
        (
            "QUOTE_DELIVERY_TERMS".into(),
            clean(values, "delivery_terms"),
        ),
        ("QUOTE_SUPPORT_TERMS".into(), clean(values, "support_terms")),
        ("QUOTE_NOTE".into(), clean(values, "note")),
    ]));
    let mut lists = render_context::lists_from_lines(&lines, &[features.to_vec()]);
    let item_rows = lists
        .get_mut("items")
        .expect("lists_from_lines always adds item rows");
    for (row, line) in item_rows.iter_mut().zip(&lines) {
        row.insert("ITEM_QTY".into(), fmt_number(Some(line.quantity)));
        row.insert("ITEM_PERIODS".into(), fmt_number(Some(line.period_count)));
        row.insert(
            "ITEM_PRICE".into(),
            unit_price_text(line.unit_price_pre_vat),
        );
    }
    let raw_date = clean(values, "quote_date");
    let file_name = format!("BaoGia_{layout}_{raw_date}.docx");
    Ok(ManualQuotationDocument {
        layout,
        context,
        lists,
        file_name,
    })
}

pub fn render_document(
    template_bytes: &[u8],
    document: &ManualQuotationDocument,
) -> Result<Vec<u8>, ServiceError> {
    let template = cf_docx::Template::compile(template_bytes)?;
    template
        .render(&cf_docx::RenderData {
            values: document.context.clone(),
            lists: Some(document.lists.clone()),
            omit_empty_tables: ["features".to_owned()].into_iter().collect(),
            legacy_party_a_rows: false,
        })
        .map_err(|error| ServiceError::Invalid(format!("Không sinh được báo giá: {error}")))
}

fn parse_number(value: Option<&Value>, label: &str, money: bool) -> Result<Decimal, ServiceError> {
    let mut raw = value
        .map(value_text)
        .unwrap_or_default()
        .trim()
        .replace(' ', "");
    if raw.contains(',') {
        raw = raw.replace('.', "").replace(',', ".");
    } else if money && grouped_thousands(&raw) {
        raw = raw.replace('.', "");
    }
    to_decimal(&raw, label).map_err(ServiceError::from)
}
fn grouped_thousands(value: &str) -> bool {
    let mut parts = value.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    !first.is_empty()
        && first.len() <= 3
        && first.chars().all(|c| c.is_ascii_digit())
        && parts.clone().count() > 0
        && parts.all(|part| part.len() == 3 && part.chars().all(|c| c.is_ascii_digit()))
}
fn value_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}
fn field_text(row: &Value, key: &str) -> String {
    row.get(key)
        .map(value_text)
        .unwrap_or_default()
        .trim()
        .to_owned()
}
fn clean(values: &BTreeMap<String, String>, key: &str) -> String {
    values
        .get(key)
        .map(String::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}
fn parse_date(raw: Option<&String>, error: &str) -> Result<Option<NaiveDate>, ServiceError> {
    cf_core::dates::parse_date_input(raw.map(String::as_str))
        .map_err(|_| ServiceError::Invalid(error.into()))
}
fn unit_price_text(value: Decimal) -> String {
    let raw = value.normalize().to_string();
    let (integer, fraction) = raw.split_once('.').unwrap_or((&raw, ""));
    let mut grouped = String::new();
    for (index, ch) in integer.chars().enumerate() {
        if index > 0 && (integer.chars().count() - index).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    if fraction.is_empty() {
        grouped
    } else {
        format!("{grouped},{fraction}")
    }
}

use chrono::Datelike;
