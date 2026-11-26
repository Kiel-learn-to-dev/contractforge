use std::collections::BTreeMap;

use cf_core::{
    fmt::{fmt_currency, fmt_number, fmt_vat},
    money::round_half_up,
    pricing::{self, ItemInput},
    render_context::{self, Feature},
    template_rules::{self, Role},
    words::amount_to_words_dec,
};
use cf_db::{
    model::{ContractTemplate, Customer, Product},
    store::Store,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::Ctx;
use crate::ServiceError;
use cf_docx::RenderData;

#[derive(Debug, Clone, PartialEq)]
pub struct QuotationSelection {
    pub product: Product,
    pub template: ContractTemplate,
    pub context: BTreeMap<String, String>,
    pub unit_price_with_vat: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedQuotation {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

pub fn list_quotation_products(store: &Store) -> Vec<Product> {
    let mut rows = store
        .products
        .values()
        .filter(|p| p.is_active && p.kind == "main")
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.code.cmp(&b.code)));
    rows
}

/// Build and render a catalog quotation in memory, with optional itemized lines.
pub fn generate_catalog_quotation(
    ctx: &mut Ctx,
    customer_id: i64,
    product_id: i64,
    template_id: i64,
    quote_date: NaiveDate,
    month_count: i64,
    items: &[ItemInput],
) -> Result<GeneratedQuotation, ServiceError> {
    let selection = resolve_quotation_selection(
        ctx.store(),
        customer_id,
        product_id,
        template_id,
        quote_date,
        month_count,
    )?;
    let mut values = selection.context;
    let mut lists = None;
    if !items.is_empty() {
        let lines = pricing::build_lines(
            items,
            |id| ctx.store().products.get(&id).map(|p| p.to_rule()),
            month_count,
            true,
        )?;
        if lines.first().and_then(|line| line.product_id) != Some(product_id) {
            return Err(ServiceError::Invalid(
                "Gói chính trong dòng báo giá không khớp với sản phẩm đã chọn.".into(),
            ));
        }
        let totals = pricing::sum_lines(&lines)?;
        let pricing_description = pricing::describe(&lines, &totals);
        let main = ctx.store().products.get(&product_id).ok_or_else(|| {
            ServiceError::Invalid("Sản phẩm không tồn tại hoặc đã ngừng sử dụng.".into())
        })?;
        let main_line = &lines[0];
        values.extend(BTreeMap::from([
            ("UNIT_COUNT".into(), fmt_number(Some(main_line.quantity))),
            ("PRODUCT_CODE".into(), main.code.clone()),
            ("PRODUCT_NAME".into(), main.name.clone()),
            (
                "UNIT_PRICE_PRE_VAT".into(),
                fmt_currency(Some(main_line.unit_price_pre_vat)),
            ),
            ("VAT_RATE".into(), fmt_vat(Some(main_line.vat_rate))),
            (
                "UNIT_PRICE_VAT".into(),
                fmt_currency(Some(pricing::unit_price_with_vat(
                    main_line.unit_price_pre_vat,
                    main_line.vat_rate,
                ))),
            ),
            (
                "QUANTITY".into(),
                fmt_number(Some(main_line.quantity * main_line.period_count)),
            ),
            ("TOTAL_PRE_VAT".into(), fmt_currency(Some(totals.pre_vat))),
            ("TOTAL_VAT".into(), fmt_currency(Some(totals.vat))),
            ("TOTAL_AMOUNT".into(), fmt_currency(Some(totals.total))),
            ("AMOUNT_TEXT".into(), totals.amount_text.clone()),
            ("PRICING_DESCRIPTION".into(), pricing_description),
        ]));
        let feature_lists = lines
            .iter()
            .map(|line| {
                line.product_id
                    .and_then(|id| ctx.store().products.get(&id))
                    .map(|p| p.feature_list.clone())
                    .unwrap_or_default()
            })
            .collect::<Vec<Vec<Feature>>>();
        lists = Some(render_context::lists_from_lines(&lines, &feature_lists));
    }
    values = render_context::apply_template_mapping(
        &values,
        &render_context::TemplateMapping(selection.template.field_mapping.clone()),
    );
    let template_path = selection
        .template
        .file_path
        .as_deref()
        .map(|path| ctx.paths().resolve_stored(path))
        .filter(|path| path.is_file())
        .ok_or_else(|| ServiceError::FileMissing("Mẫu đã chọn chưa có file DOCX.".into()))?;
    let compiled = ctx.templates_mut().get_or_compile(&template_path)?;
    let bytes = compiled
        .render(&RenderData {
            values,
            lists,
            ..RenderData::default()
        })
        .map_err(|error| ServiceError::Invalid(format!("Lỗi khi sinh file: {error}")))?;
    let customer = ctx
        .store()
        .customers
        .get(&customer_id)
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy khách hàng.".into()))?;
    let slug_source = customer
        .short_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or(&customer.legal_name);
    let slug = slug_source
        .replace(' ', "_")
        .chars()
        .take(30)
        .collect::<String>();
    let file_name = format!(
        "BaoGia_{}_{}_{}.docx",
        selection.product.code,
        slug,
        quote_date.format("%Y%m%d")
    );
    Ok(GeneratedQuotation { file_name, bytes })
}

pub fn list_quotation_templates(store: &Store) -> Vec<ContractTemplate> {
    let mut rows = store
        .templates
        .values()
        .filter(|t| t.is_active && t.contract_type == template_rules::QUOTATION_TYPE)
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.code.cmp(&b.code)));
    rows
}

pub fn product_price_with_vat(product: &Product) -> Decimal {
    let price = product.default_price.unwrap_or_default();
    let vat = product.default_vat_rate.unwrap_or_default();
    round_half_up(price * (Decimal::ONE + vat / Decimal::from(100)))
}

/// Resolve and validate the selected product/template, then build legacy quotation context.
pub fn resolve_quotation_selection(
    store: &Store,
    customer_id: i64,
    product_id: i64,
    template_id: i64,
    quote_date: NaiveDate,
    month_count: i64,
) -> Result<QuotationSelection, ServiceError> {
    let product = store
        .products
        .get(&product_id)
        .filter(|p| p.is_active && p.kind == "main")
        .ok_or_else(|| {
            ServiceError::Invalid("Sản phẩm không tồn tại hoặc đã ngừng sử dụng.".into())
        })?;
    if product
        .default_price
        .is_none_or(|price| price <= Decimal::ZERO)
    {
        return Err(ServiceError::Invalid(
            "Sản phẩm chưa được cấu hình đơn giá hợp lệ.".into(),
        ));
    }
    let template = store
        .templates
        .get(&template_id)
        .filter(|t| t.is_active)
        .ok_or_else(|| ServiceError::Invalid("Mẫu không tồn tại hoặc đã ngừng sử dụng.".into()))?;
    if template.contract_type != template_rules::QUOTATION_TYPE {
        return Err(ServiceError::Invalid(format!(
            "'{}' là mẫu {}, không phải mẫu {}. Hãy chọn một mẫu {}.",
            template.name,
            template.contract_type,
            template_rules::QUOTATION_TYPE,
            template_rules::QUOTATION_TYPE,
        )));
    }
    template_rules::check_template_for(
        &template.to_info(),
        product.group_code.as_deref(),
        Role::Quotation,
    )?;
    let customer = store
        .customers
        .get(&customer_id)
        .filter(|c| c.is_active && !c.is_deleted)
        .ok_or_else(|| {
            ServiceError::Invalid("Khách hàng không tồn tại hoặc đã ngừng sử dụng.".into())
        })?;
    if !(1..=120).contains(&month_count) {
        return Err(ServiceError::Invalid("Số tháng phải từ 1 đến 120.".into()));
    }
    let price = product.default_price.unwrap_or_default();
    let vat = product.default_vat_rate.unwrap_or_default();
    let unit_price_vat = product_price_with_vat(product);
    let names = pricing_unit_names(store, customer);
    let unit_count = names.len().max(1);
    let total = unit_price_vat * Decimal::from(unit_count as u64) * Decimal::from(month_count);
    let (day, month, year) = cf_core::dates::split_date_parts(Some(quote_date));
    let total_text = amount_to_words_dec(total).unwrap_or_default();
    let units = names.join("; ");
    let unit_detail = if names.len() > 1 {
        format!(" ({units})")
    } else {
        String::new()
    };
    let pricing_description = format!(
        "Số tiền = Đơn giá có VAT ({} VNĐ) × {unit_count} đơn vị{unit_detail} × {month_count} tháng = {} VNĐ",
        fmt_currency(Some(unit_price_vat)),
        fmt_currency(Some(total)),
    );
    let context = BTreeMap::from([
        ("PARTY_A_NAME".into(), customer.legal_name.clone()),
        (
            "QUOTE_DATE".into(),
            quote_date.format("%d/%m/%Y").to_string(),
        ),
        ("QUOTE_DATE_DAY".into(), day),
        ("QUOTE_DATE_MONTH".into(), month),
        ("QUOTE_DATE_YEAR".into(), year),
        ("UNIT_COUNT".into(), unit_count.to_string()),
        ("MONTH_COUNT".into(), month_count.to_string()),
        ("PRODUCT_CODE".into(), product.code.clone()),
        ("PRODUCT_NAME".into(), product.name.clone()),
        ("UNIT_PRICE_PRE_VAT".into(), fmt_currency(Some(price))),
        ("VAT_RATE".into(), fmt_vat(Some(vat))),
        ("UNIT_PRICE_VAT".into(), fmt_currency(Some(unit_price_vat))),
        ("TOTAL_AMOUNT".into(), fmt_currency(Some(total))),
        ("AMOUNT_TEXT".into(), total_text),
        (
            "QUANTITY".into(),
            (unit_count as i64 * month_count).to_string(),
        ),
        ("PRICING_UNITS".into(), units),
        ("PRICING_DESCRIPTION".into(), pricing_description),
    ]);
    Ok(QuotationSelection {
        product: product.clone(),
        template: template.clone(),
        context,
        unit_price_with_vat: unit_price_vat,
    })
}

fn pricing_unit_names(store: &Store, customer: &Customer) -> Vec<String> {
    let mut units = store
        .units
        .values()
        .filter(|unit| unit.customer_id == customer.id && unit.is_active && unit.include_in_billing)
        .collect::<Vec<_>>();
    units.sort_by_key(|unit| (unit.sort_order, unit.id));
    let mut names = Vec::new();
    if customer.bill_main_station {
        names.push(
            customer
                .short_name
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&customer.legal_name)
                .to_owned(),
        );
    }
    names.extend(units.into_iter().map(|unit| unit.name.clone()));
    names
}
