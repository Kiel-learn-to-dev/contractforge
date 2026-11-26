use cf_core::{
    numbering, parse,
    template_rules::{self, Role},
};
use cf_db::{
    model::{Product, template::ContractTemplate},
    store::Store,
};
use rust_decimal::Decimal;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductError(pub Vec<String>);

impl ProductError {
    pub fn user_message(&self) -> String {
        self.0.join("\n")
    }
}

impl std::fmt::Display for ProductError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.user_message())
    }
}

impl std::error::Error for ProductError {}

#[derive(Debug, Clone)]
pub struct ProductDraft {
    pub code: String,
    pub name: String,
    pub product_type: String,
    pub default_price: Option<Decimal>,
    pub default_vat_rate: Option<Decimal>,
    pub default_duration_months: Option<i64>,
    pub default_template_id: Option<i64>,
    pub contract_number_format: Option<String>,
    pub is_active: Option<bool>,
    pub group_code: Option<String>,
    pub kind: String,
    pub unit_label: Option<String>,
    pub price_period_months: i64,
    pub quantity_source: String,
    pub min_quantity: Option<i64>,
    pub quantity_step: Option<i64>,
    pub account_limit: Option<i64>,
    pub feature_list: Option<Vec<cf_core::render_context::Feature>>,
    pub replaced_by_id: Option<i64>,
    pub sort_order: i64,
}

fn text(data: &Value, key: &str) -> String {
    match &data[key] {
        Value::String(value) => value.trim().to_owned(),
        Value::Null => String::new(),
        value => value.to_string(),
    }
}

fn optional_int(data: &Value, key: &str, errors: &mut Vec<String>, label: &str) -> Option<i64> {
    let raw = text(data, key);
    if raw.is_empty() {
        return None;
    }
    match raw.parse::<i64>() {
        Ok(value) => Some(value),
        Err(_) => {
            errors.push(format!("{label} phải là số nguyên."));
            None
        }
    }
}

fn parse_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::Null => Some(true),
        _ => match match value {
            Value::String(v) => v.trim().to_lowercase(),
            Value::Number(v) => v.to_string(),
            _ => String::new(),
        }
        .as_str()
        {
            "" | "1" | "true" | "yes" | "y" | "có" | "x" => Some(true),
            "0" | "false" | "no" | "n" | "không" => Some(false),
            _ => None,
        },
    }
}

pub fn parse_feature_text(raw: &str) -> Vec<cf_core::render_context::Feature> {
    raw.lines()
        .filter_map(|line| {
            let (name, desc) = line.split_once('|').unwrap_or((line, ""));
            let name = name.trim();
            (!name.is_empty()).then(|| cf_core::render_context::Feature {
                name: name.to_owned(),
                description: Some(desc.trim().to_owned()),
            })
        })
        .collect()
}

pub fn feature_text(product: &Product) -> String {
    product
        .feature_list
        .iter()
        .filter_map(|feature| {
            let name = feature.name.trim();
            if name.is_empty() {
                return None;
            }
            let desc = feature.description.as_deref().unwrap_or_default().trim();
            Some(if desc.is_empty() {
                name.to_owned()
            } else {
                format!("{name} | {desc}")
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn clean_form(
    store: &Store,
    data: &Value,
    product_id: Option<i64>,
) -> Result<ProductDraft, ProductError> {
    let mut errors = Vec::new();
    let code = text(data, "code").to_uppercase();
    if code.is_empty() {
        errors.push("Mã sản phẩm không được để trống.".into());
    } else if store
        .indexes
        .product_by_code(store, &code)
        .is_some_and(|product| Some(product.id) != product_id)
    {
        errors.push(format!("Mã {code} đã tồn tại."));
    }
    let name = text(data, "name");
    if name.is_empty() {
        errors.push("Tên sản phẩm không được để trống.".into());
    }
    let product_type = text(data, "product_type");
    if product_type.is_empty() {
        errors.push("Loại sản phẩm không được để trống.".into());
    }
    let kind = text(data, "kind");
    let kind = if kind.is_empty() {
        "main".to_owned()
    } else {
        kind
    };
    if !["main", "addon", "storage"].contains(&kind.as_str()) {
        errors.push("Kiểu sản phẩm không hợp lệ.".into());
    }
    let group_code = text(data, "group_code");
    let group_code = (!group_code.is_empty()).then_some(group_code);
    if kind != "main" && group_code.is_none() {
        errors.push("Gói bổ sung phải thuộc một nhóm để ghép với gói chính.".into());
    }
    let unit_label = text(data, "unit_label");
    let raw_price = text(data, "default_price");
    let default_price = if raw_price.is_empty() {
        None
    } else {
        parse::safe_decimal(&raw_price)
    };
    if default_price.is_some_and(|value| value < Decimal::ZERO) {
        errors.push("Đơn giá không được âm.".into());
    }
    let vat_raw = text(data, "default_vat_rate");
    let default_vat_rate = if vat_raw.is_empty() {
        Some(Decimal::ZERO)
    } else {
        vat_raw.parse().ok()
    };
    if default_vat_rate.is_none() {
        errors.push("Thuế suất phải là số, ví dụ 8 hoặc 8.5.".into());
    }
    if default_vat_rate.is_some_and(|vat| !(Decimal::ZERO..=Decimal::from(100)).contains(&vat)) {
        errors.push("Thuế suất phải trong khoảng 0 đến 100.".into());
    }
    let mut price_period_months =
        optional_int(data, "price_period_months", &mut errors, "Kỳ giá").unwrap_or(1);
    if price_period_months < 1 {
        errors.push("Kỳ giá phải từ 1 tháng trở lên.".into());
        price_period_months = 1;
    }
    let quantity_source = {
        let value = text(data, "quantity_source");
        if value.is_empty() {
            "units".into()
        } else {
            value
        }
    };
    if !["units", "records", "manual"].contains(&quantity_source.as_str()) {
        errors.push("Nguồn số lượng không hợp lệ.".into());
    }
    let min_quantity = optional_int(data, "min_quantity", &mut errors, "Số lượng tối thiểu");
    if min_quantity.is_some_and(|v| v < 0) {
        errors.push("Số lượng tối thiểu không được âm.".into());
    }
    let quantity_step = optional_int(data, "quantity_step", &mut errors, "Bước làm tròn");
    if quantity_step.is_some_and(|v| v < 1) {
        errors.push("Bước làm tròn phải từ 1 trở lên.".into());
    }
    let account_limit = optional_int(data, "account_limit", &mut errors, "Số tài khoản");
    if account_limit.is_some_and(|v| v < 0) {
        errors.push("Số tài khoản không được âm.".into());
    }
    let default_duration_months = optional_int(
        data,
        "default_duration_months",
        &mut errors,
        "Thời hạn mặc định",
    );
    if default_duration_months.is_some_and(|v| v < 1) {
        errors.push("Thời hạn mặc định phải từ 1 tháng trở lên.".into());
    }
    let default_template_id =
        optional_int(data, "default_template_id", &mut errors, "Mẫu hợp đồng");
    let template: Option<&ContractTemplate> =
        default_template_id.and_then(|id| store.templates.get(&id));
    if default_template_id.is_some() && template.is_none() {
        errors.push("Mẫu hợp đồng mặc định không tồn tại.".into());
    }
    if let Some(template) = template.filter(|_| kind == "main") {
        let info = template.to_info();
        if let Err(error) =
            template_rules::check_template_for(&info, group_code.as_deref(), Role::Contract)
        {
            errors.push(error.to_string());
        }
    }
    let number_format = text(data, "contract_number_format");
    if !number_format.is_empty()
        && let Err(error) = numbering::validate_format(&number_format)
    {
        errors.push(error.to_string());
    }
    let replaced_by_id = optional_int(data, "replaced_by_id", &mut errors, "Gói thay thế");
    if replaced_by_id == product_id && replaced_by_id.is_some() {
        errors.push("Gói thay thế không được là chính sản phẩm này.".into());
    }
    if replaced_by_id.is_some_and(|id| !store.products.contains_key(&id)) {
        errors.push("Gói thay thế không tồn tại.".into());
    }
    let sort_order = optional_int(data, "sort_order", &mut errors, "Thứ tự hiển thị").unwrap_or(0);
    let is_active = if data.get("is_active").is_some() {
        parse_bool(&data["is_active"])
    } else {
        None
    };
    if data.get("is_active").is_some() && is_active.is_none() {
        errors.push("Cột hiển thị (is_active) chỉ nhận 1 hoặc 0.".into());
    }
    let feature_list = data
        .get("feature_text")
        .map(|value| parse_feature_text(value.as_str().unwrap_or_default()));
    if errors.is_empty() {
        Ok(ProductDraft {
            code,
            name,
            product_type,
            default_price,
            default_vat_rate,
            default_duration_months,
            default_template_id,
            contract_number_format: (!number_format.is_empty()).then_some(number_format),
            is_active,
            group_code,
            kind,
            unit_label: (!unit_label.is_empty()).then_some(unit_label),
            price_period_months,
            quantity_source,
            min_quantity,
            quantity_step,
            account_limit,
            feature_list,
            replaced_by_id,
            sort_order,
        })
    } else {
        Err(ProductError(errors))
    }
}
