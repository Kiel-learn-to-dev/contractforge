//! Dữ liệu ánh xạ sang mẫu tài liệu.

use crate::{
    fmt::{fmt_currency, fmt_number, fmt_vat},
    lifecycle::ContractStatus,
    numbering::parse_addendum_seq,
    words::{amount_to_words, amount_to_words_chan},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitView {
    pub name: String,
    pub is_active: bool,
    pub include_in_billing: bool,
    pub unit_type: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomerView {
    pub legal_name: String,
    pub short_name: String,
    pub code: String,
    pub address: String,
    pub ward: String,
    pub province: String,
    pub representative_name: String,
    pub representative_gender: String,
    pub representative_title: String,
    pub tax_code: String,
    pub bank_account: String,
    pub bank_name: String,
    pub beneficiary_name: String,
    pub budget_relation_code: String,
    pub phone: String,
    pub email: String,
    pub contact_name: String,
    pub establishment_decision: String,
    pub establishment_date: Option<NaiveDate>,
    pub bill_main_station: bool,
    pub record_count: Option<i64>,
    pub units: Vec<UnitView>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContractView {
    pub contract_number: String,
    pub doc_kind: String,
    pub status: ContractStatus,
    pub party_b_representative: Option<String>,
    pub party_b_authorization_number: Option<String>,
    pub party_b_authorization_date: Option<String>,
    pub total_pre_vat: Option<Decimal>,
    pub total_vat: Option<Decimal>,
    pub payment_cycle_months: Option<i64>,
    pub sign_location: Option<String>,
    pub sign_date: Option<NaiveDate>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub month_count: Option<i64>,
    pub unit_count: Option<i64>,
    pub quantity: Option<Decimal>,
    pub unit_price_pre_vat: Option<Decimal>,
    pub vat_rate: Option<Decimal>,
    pub unit_price_vat: Option<Decimal>,
    pub total_amount: Option<Decimal>,
    pub amount_text: Option<String>,
    pub pricing_description: Option<String>,
    pub acceptance_date: Option<NaiveDate>,
    pub liquidation_date: Option<NaiveDate>,
    pub contract_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentView {
    pub contract_number: String,
    pub sign_date: Option<NaiveDate>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductView {
    pub name: String,
    pub code: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartyB {
    pub name: String,
    pub address: String,
    pub bank_account: String,
    pub bank_name: String,
    pub beneficiary: String,
    pub tax_code: String,
    pub title: String,
    pub representative: String,
    pub authorization_number: String,
    pub authorization_date: String,
    pub phone: String,
    pub contact_name: String,
    pub contact_email: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateMapping(pub BTreeMap<String, String>);

/// Các trường chuẩn được ánh xạ bởi bộ phân tích mẫu.
pub const CANONICAL_MAP: &[(&str, &str)] = &[
    ("CONTRACT_NUMBER", "contract.number"),
    ("SIGN_DATE_DAY", "contract.sign_date_day"),
    ("SIGN_DATE_MONTH", "contract.sign_date_month"),
    ("SIGN_DATE_YEAR", "contract.sign_date_year"),
    ("START_DATE", "contract.start_date"),
    ("END_DATE", "contract.end_date"),
    ("MONTH_COUNT", "contract.month_count"),
    ("UNIT_COUNT", "contract.unit_count"),
    ("QUANTITY", "contract.quantity"),
    ("UNIT_PRICE_PRE_VAT", "contract.unit_price_pre_vat"),
    ("VAT_RATE", "contract.vat_rate"),
    ("UNIT_PRICE_VAT", "contract.unit_price_vat"),
    ("TOTAL_AMOUNT", "contract.total_amount"),
    ("AMOUNT_TEXT", "contract.amount_text"),
    ("AMOUNT_TEXT_CHAN", "contract.amount_text_chan"),
    ("PRICING_DESCRIPTION", "contract.pricing_description"),
    ("PRICING_UNITS", "quotation.pricing_units"),
    ("QUOTE_DATE", "quotation.date"),
    ("QUOTE_DATE_DAY", "quotation.date_day"),
    ("QUOTE_DATE_MONTH", "quotation.date_month"),
    ("QUOTE_DATE_YEAR", "quotation.date_year"),
    ("QUOTE_NUMBER", "quotation.number"),
    ("QUOTE_PLACE", "quotation.place"),
    ("QUOTE_SUBJECT", "quotation.subject"),
    ("QUOTE_VALID_UNTIL", "quotation.valid_until"),
    ("QUOTE_PAYMENT_TERMS", "quotation.payment_terms"),
    ("QUOTE_DELIVERY_TERMS", "quotation.delivery_terms"),
    ("QUOTE_SUPPORT_TERMS", "quotation.support_terms"),
    ("QUOTE_NOTE", "quotation.note"),
    ("QUOTE_ISSUER_NAME", "quotation.issuer.name"),
    ("QUOTE_ISSUER_ADDRESS", "quotation.issuer.address"),
    ("QUOTE_ISSUER_TAX_CODE", "quotation.issuer.tax_code"),
    (
        "QUOTE_ISSUER_REPRESENTATIVE",
        "quotation.issuer.representative",
    ),
    ("QUOTE_ISSUER_TITLE", "quotation.issuer.title"),
    ("QUOTE_ISSUER_PHONE", "quotation.issuer.phone"),
    ("QUOTE_ISSUER_EMAIL", "quotation.issuer.email"),
    ("REQUEST_DATE", "request.date"),
    ("REQUEST_DATE_DAY", "request.date_day"),
    ("REQUEST_DATE_MONTH", "request.date_month"),
    ("REQUEST_DATE_YEAR", "request.date_year"),
    ("REQUEST_DEADLINE", "request.deadline"),
    ("REQUEST_PLACE", "request.place"),
    ("REQUEST_ISSUER_NAME", "request.issuer_name"),
    ("REQUEST_ADDRESSEE", "request.addressee"),
    ("REQUEST_SUBJECT", "request.subject"),
    ("REQUEST_NOTE", "request.note"),
    ("PRODUCT_CODE", "product.code"),
    ("PRODUCT_NAME", "product.name"),
    ("ACCEPTANCE_DATE", "contract.acceptance_date"),
    ("ACCEPTANCE_DATE_DAY", "contract.acceptance_date_day"),
    ("ACCEPTANCE_DATE_MONTH", "contract.acceptance_date_month"),
    ("ACCEPTANCE_DATE_YEAR", "contract.acceptance_date_year"),
    ("BUDGET_UNIT_NAME", "form08a.budget_unit_name"),
    ("BUDGET_UNIT_CODE", "form08a.budget_unit_code"),
    ("WORK_CONTENT", "form08a.work_content"),
    ("PARTY_A_TITLE_UPPER", "form08a.party_a_title_upper"),
    ("PARTY_B_TITLE_UPPER", "form08a.party_b_title_upper"),
    ("LIQUIDATION_DATE_DAY", "contract.liquidation_date_day"),
    ("LIQUIDATION_DATE_MONTH", "contract.liquidation_date_month"),
    ("LIQUIDATION_DATE_YEAR", "contract.liquidation_date_year"),
    ("PARTY_A_NAME", "party_a.name"),
    ("PARTY_A_ADDRESS", "party_a.address"),
    ("PARTY_A_REPRESENTATIVE", "party_a.representative"),
    ("PARTY_A_TITLE", "party_a.title"),
    ("PARTY_A_NAME_WARD", "party_a.name_ward"),
    ("PARTY_A_TAX_CODE", "party_a.tax_code"),
    ("PARTY_A_BANK_ACCOUNT", "party_a.bank_account"),
    ("PARTY_A_BANK_NAME", "party_a.bank_name"),
    ("PARTY_A_BENEFICIARY", "party_a.beneficiary"),
    ("PARTY_A_BUDGET_CODE", "party_a.budget_code"),
    ("PARTY_B_REPRESENTATIVE", "party_b.representative"),
    (
        "PARTY_B_AUTHORIZATION_NUMBER",
        "party_b.authorization_number",
    ),
    ("PARTY_B_AUTHORIZATION_DATE", "party_b.authorization_date"),
    ("PARTY_B_NAME", "party_b.name"),
    ("PARTY_B_ADDRESS", "party_b.address"),
    ("PARTY_B_BANK_ACCOUNT", "party_b.bank_account"),
    ("PARTY_B_BANK_NAME", "party_b.bank_name"),
    ("PARTY_B_BENEFICIARY", "party_b.beneficiary"),
    ("PARTY_B_TAX_CODE", "party_b.tax_code"),
    ("PARTY_B_TITLE", "party_b.title"),
    ("PARTY_A_PHONE", "party_a.phone"),
    ("PARTY_A_EMAIL", "party_a.email"),
    ("PARTY_A_CONTACT_NAME", "party_a.contact_name"),
    (
        "PARTY_A_ESTABLISHMENT_DECISION",
        "party_a.establishment_decision",
    ),
    ("PARTY_A_ESTABLISHMENT_DATE", "party_a.establishment_date"),
    ("PARTY_B_PHONE", "party_b.phone"),
    ("PARTY_B_CONTACT_NAME", "party_b.contact_name"),
    ("PARTY_B_CONTACT_EMAIL", "party_b.contact_email"),
    ("TOTAL_PRE_VAT", "contract.total_pre_vat"),
    ("TOTAL_VAT", "contract.total_vat"),
    ("PAYMENT_CYCLE_MONTHS", "contract.payment_cycle_months"),
    ("PARENT_CONTRACT_NUMBER", "parent.number"),
    ("PARENT_SIGN_DATE", "parent.sign_date"),
    ("ADDENDUM_SEQ", "contract.addendum_seq"),
    ("SIGN_DATE", "contract.sign_date"),
    ("SIGN_LOCATION", "contract.sign_location"),
    ("LIQUIDATION_DATE", "contract.liquidation_date"),
    ("PARTY_A_REPRESENTATIVE_FULL", "party_a.representative_full"),
];
pub const ROW_PLACEHOLDERS: &[(&str, &str)] = &[
    ("ITEM_NO", "items.ITEM_NO"),
    ("ITEM_NAME", "items.ITEM_NAME"),
    ("ITEM_UNIT", "items.ITEM_UNIT"),
    ("ITEM_QTY", "items.ITEM_QTY"),
    ("ITEM_PRICE", "items.ITEM_PRICE"),
    ("ITEM_AMOUNT_PRE_VAT", "items.ITEM_AMOUNT_PRE_VAT"),
    ("ITEM_VAT_RATE", "items.ITEM_VAT_RATE"),
    ("ITEM_VAT_AMOUNT", "items.ITEM_VAT_AMOUNT"),
    ("ITEM_AMOUNT_TOTAL", "items.ITEM_AMOUNT_TOTAL"),
    ("FEATURE_NO", "features.FEATURE_NO"),
    ("FEATURE_NAME", "features.FEATURE_NAME"),
    ("FEATURE_DESC", "features.FEATURE_DESC"),
];
pub static ALL_CANONICAL_FIELDS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut fields: Vec<_> = CANONICAL_MAP.iter().map(|(_, field)| *field).collect();
    fields.sort_unstable();
    fields.dedup();
    fields
});

fn date_text(value: Option<NaiveDate>) -> String {
    value
        .map(|date| date.format("%d/%m/%Y").to_string())
        .unwrap_or_default()
}
fn date_parts(prefix: &str, value: Option<NaiveDate>, context: &mut BTreeMap<String, String>) {
    let (day, month, year) = value
        .map(|date| {
            (
                date.format("%d").to_string(),
                date.format("%m").to_string(),
                date.format("%Y").to_string(),
            )
        })
        .unwrap_or_default();
    context.insert(format!("{prefix}_DAY"), day);
    context.insert(format!("{prefix}_MONTH"), month);
    context.insert(format!("{prefix}_YEAR"), year);
}

/// Port của app/services/docx_renderer.py::build_render_context.
pub fn build_render_context(
    contract: &ContractView,
    customer: &CustomerView,
    parent: Option<&ParentView>,
    product: Option<&ProductView>,
    party_b: &PartyB,
    addendum_format: Option<&str>,
) -> BTreeMap<String, String> {
    let mut context = BTreeMap::new();
    let rep_prefix = match customer.representative_gender.as_str() {
        "Nam" => "(Ông)",
        "Nữ" => "(Bà)",
        _ => "",
    };
    let full_rep = if rep_prefix.is_empty() {
        customer.representative_name.clone()
    } else {
        format!("{rep_prefix} {}", customer.representative_name)
            .trim()
            .to_owned()
    };
    for (key, value) in [
        ("PARTY_A_NAME", customer.legal_name.clone()),
        ("PARTY_A_ADDRESS", crate::customer::full_address(customer)),
        (
            "PARTY_A_REPRESENTATIVE",
            customer.representative_name.clone(),
        ),
        ("PARTY_A_REPRESENTATIVE_FULL", full_rep),
        ("PARTY_A_TITLE", customer.representative_title.clone()),
        (
            "PARTY_A_NAME_WARD",
            if customer.ward.is_empty() {
                customer.legal_name.clone()
            } else {
                format!("{}, {}", customer.legal_name, customer.ward)
            },
        ),
        ("PARTY_A_TAX_CODE", customer.tax_code.clone()),
        ("PARTY_A_BANK_ACCOUNT", customer.bank_account.clone()),
        ("PARTY_A_BANK_NAME", customer.bank_name.clone()),
        ("PARTY_A_BENEFICIARY", customer.beneficiary_name.clone()),
        ("PARTY_A_BUDGET_CODE", customer.budget_relation_code.clone()),
        ("PARTY_A_PHONE", customer.phone.clone()),
        ("PARTY_A_EMAIL", customer.email.clone()),
        ("PARTY_A_CONTACT_NAME", customer.contact_name.clone()),
        (
            "PARTY_A_ESTABLISHMENT_DECISION",
            customer.establishment_decision.clone(),
        ),
        (
            "PARTY_A_ESTABLISHMENT_DATE",
            date_text(customer.establishment_date),
        ),
        ("PARTY_B_NAME", party_b.name.clone()),
        ("PARTY_B_ADDRESS", party_b.address.clone()),
        ("PARTY_B_BANK_ACCOUNT", party_b.bank_account.clone()),
        ("PARTY_B_BANK_NAME", party_b.bank_name.clone()),
        ("PARTY_B_BENEFICIARY", party_b.beneficiary.clone()),
        ("PARTY_B_TAX_CODE", party_b.tax_code.clone()),
        ("PARTY_B_TITLE", party_b.title.clone()),
        ("PARTY_B_PHONE", party_b.phone.clone()),
        ("PARTY_B_CONTACT_NAME", party_b.contact_name.clone()),
        ("PARTY_B_CONTACT_EMAIL", party_b.contact_email.clone()),
        (
            "PARTY_B_REPRESENTATIVE",
            contract
                .party_b_representative
                .clone()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| party_b.representative.clone()),
        ),
        (
            "PARTY_B_AUTHORIZATION_NUMBER",
            contract
                .party_b_authorization_number
                .clone()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| party_b.authorization_number.clone()),
        ),
        (
            "PARTY_B_AUTHORIZATION_DATE",
            contract
                .party_b_authorization_date
                .clone()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| party_b.authorization_date.clone()),
        ),
        ("CONTRACT_NUMBER", contract.contract_number.clone()),
        ("TOTAL_PRE_VAT", fmt_currency(contract.total_pre_vat)),
        ("TOTAL_VAT", fmt_currency(contract.total_vat)),
        (
            "PAYMENT_CYCLE_MONTHS",
            contract
                .payment_cycle_months
                .filter(|v| *v != 0)
                .map(|v| v.to_string())
                .unwrap_or_default(),
        ),
        (
            "PRODUCT_NAME",
            product.map_or_else(String::new, |value| value.name.clone()),
        ),
        (
            "PARENT_CONTRACT_NUMBER",
            parent.map_or_else(String::new, |value| value.contract_number.clone()),
        ),
        (
            "PARENT_SIGN_DATE",
            parent.map_or_else(String::new, |value| date_text(value.sign_date)),
        ),
        (
            "SIGN_LOCATION",
            contract
                .sign_location
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or(&customer.legal_name)
                .to_owned(),
        ),
        ("SIGN_DATE", date_text(contract.sign_date)),
        ("START_DATE", date_text(contract.start_date)),
        ("END_DATE", date_text(contract.end_date)),
        (
            "MONTH_COUNT",
            contract
                .month_count
                .filter(|v| *v != 0)
                .map(|v| v.to_string())
                .unwrap_or_default(),
        ),
        (
            "UNIT_COUNT",
            contract
                .unit_count
                .filter(|value| *value != 0)
                .unwrap_or(1)
                .to_string(),
        ),
        ("QUANTITY", fmt_number(contract.quantity)),
        (
            "UNIT_PRICE_PRE_VAT",
            fmt_currency(contract.unit_price_pre_vat),
        ),
        ("VAT_RATE", fmt_vat(contract.vat_rate)),
        ("UNIT_PRICE_VAT", fmt_currency(contract.unit_price_vat)),
        ("TOTAL_AMOUNT", fmt_currency(contract.total_amount)),
        (
            "PRICING_DESCRIPTION",
            contract.pricing_description.clone().unwrap_or_default(),
        ),
        ("ACCEPTANCE_DATE", date_text(contract.acceptance_date)),
        ("LIQUIDATION_DATE", date_text(contract.liquidation_date)),
    ] {
        context.insert(key.to_owned(), value);
    }
    date_parts("SIGN_DATE", contract.sign_date, &mut context);
    date_parts("ACCEPTANCE_DATE", contract.acceptance_date, &mut context);
    date_parts("LIQUIDATION_DATE", contract.liquidation_date, &mut context);
    let rounded_to_integer = contract
        .total_amount
        .filter(|amount| !amount.is_zero())
        .and_then(|amount| amount.trunc().to_string().parse::<u64>().ok());
    let amount_text = contract
        .amount_text
        .clone()
        .filter(|value| !value.is_empty())
        .or_else(|| rounded_to_integer.map(amount_to_words));
    context.insert("AMOUNT_TEXT".into(), amount_text.unwrap_or_default());
    context.insert(
        "AMOUNT_TEXT_CHAN".into(),
        rounded_to_integer
            .map(amount_to_words_chan)
            .unwrap_or_default(),
    );
    let seq = if matches!(
        contract.doc_kind.as_str(),
        "addendum_addon" | "addendum_renewal"
    ) && parent.is_some()
    {
        parse_addendum_seq(&contract.contract_number, addendum_format)
            .map(|value| format!("{value:02}"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    context.insert("ADDENDUM_SEQ".into(), seq);
    context
}

/// Port của app/services/form08a_service.py::_work_content.
pub fn work_content(product: Option<&ProductView>) -> String {
    let name = product.map_or("", |value| value.name.trim());
    if name.is_empty() {
        return "Phần mềm".into();
    }
    let cleaned = regex::Regex::new(r"\s*\([^()]*\)\s*$")
        .ok()
        .map(|pattern| pattern.replace(name, "").trim().to_owned())
        .unwrap_or_default();
    if cleaned.is_empty() {
        name.to_owned()
    } else {
        cleaned
    }
}

/// Port của app/services/form08a_service.py::build_08a_context.
pub fn build_08a_context(
    mut context: BTreeMap<String, String>,
    customer: &CustomerView,
    product: Option<&ProductView>,
) -> BTreeMap<String, String> {
    context.insert(
        "BUDGET_UNIT_NAME".into(),
        customer.legal_name.to_uppercase(),
    );
    context.insert(
        "BUDGET_UNIT_CODE".into(),
        customer.budget_relation_code.trim().to_owned(),
    );
    context.insert("WORK_CONTENT".into(), work_content(product));
    context.insert(
        "PARTY_A_TITLE_UPPER".into(),
        context
            .get("PARTY_A_TITLE")
            .cloned()
            .unwrap_or_default()
            .to_uppercase(),
    );
    context.insert(
        "PARTY_B_TITLE_UPPER".into(),
        context
            .get("PARTY_B_TITLE")
            .cloned()
            .unwrap_or_default()
            .to_uppercase(),
    );
    context
}

/// Chèn giá trị canonical vào các placeholder tùy chỉnh của mẫu.
pub fn apply_template_mapping(
    context: &BTreeMap<String, String>,
    mapping: &TemplateMapping,
) -> BTreeMap<String, String> {
    let mut mapped = context.clone();
    for (placeholder, canonical) in &mapping.0 {
        if let Some((source, _)) = CANONICAL_MAP.iter().find(|(_, field)| *field == canonical)
            && let Some(value) = context.get(*source)
        {
            mapped.insert(placeholder.clone(), value.clone());
        }
    }
    mapped
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Feature {
    pub name: String,
    pub description: Option<String>,
}

/// Port của app/services/docx_renderer.py::lists_from_lines.
pub fn lists_from_lines(
    lines: &[crate::pricing::Line],
    feature_lists: &[Vec<Feature>],
) -> BTreeMap<String, Vec<BTreeMap<String, String>>> {
    let items = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let mut row = BTreeMap::new();
            let values = [
                ("ITEM_NO", (index + 1).to_string()),
                ("ITEM_NAME", line.description.clone()),
                ("ITEM_UNIT", line.unit_label.clone()),
                (
                    "ITEM_QTY",
                    fmt_number(Some(line.quantity * line.period_count)),
                ),
                ("ITEM_PRICE", fmt_currency(Some(line.unit_price_pre_vat))),
                (
                    "ITEM_AMOUNT_PRE_VAT",
                    fmt_currency(Some(line.amount_pre_vat)),
                ),
                ("ITEM_VAT_RATE", fmt_vat(Some(line.vat_rate))),
                ("ITEM_VAT_AMOUNT", fmt_currency(Some(line.vat_amount))),
                ("ITEM_AMOUNT_TOTAL", fmt_currency(Some(line.amount_total))),
            ];
            for (key, value) in values {
                row.insert(key.into(), value);
            }
            row
        })
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut features = Vec::new();
    for feature in feature_lists.iter().flatten() {
        let name = feature.name.trim();
        if name.is_empty() || !seen.insert(name.to_owned()) {
            continue;
        }
        let mut row = BTreeMap::new();
        row.insert("FEATURE_NO".into(), (features.len() + 1).to_string());
        row.insert("FEATURE_NAME".into(), name.to_owned());
        row.insert(
            "FEATURE_DESC".into(),
            feature.description.clone().unwrap_or_default(),
        );
        features.push(row);
    }
    BTreeMap::from([("items".into(), items), ("features".into(), features)])
}
