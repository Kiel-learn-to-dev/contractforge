use cf_core::{
    pricing,
    template_rules::{self, Role},
};
use cf_db::{
    model::{ContractTemplate, Product},
    store::Store,
};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductBrief {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub unit_label: String,
    pub price: String,
    pub vat: String,
    pub price_period_months: i64,
    pub quantity_source: String,
    pub default_quantity: String,
    pub template_id: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateOption {
    pub id: i64,
    pub name: String,
    pub kind: Option<String>,
    pub own: bool,
    pub preselect: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateOptions {
    pub contract: Vec<TemplateOption>,
    pub attachments: Vec<TemplateOption>,
    pub default_id: Option<i64>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductOptions {
    pub main: Option<ProductBrief>,
    pub addons: Vec<ProductBrief>,
    pub templates: TemplateOptions,
}

fn brief(product: &Product, record_count: Option<i64>, units: i64) -> ProductBrief {
    let rule = product.to_rule();
    ProductBrief {
        id: product.id,
        code: product.code.clone(),
        name: product.name.clone(),
        unit_label: product.unit_label.clone().unwrap_or_default(),
        price: product
            .default_price
            .map(|v| v.normalize().to_string())
            .unwrap_or_default(),
        vat: product
            .default_vat_rate
            .unwrap_or(Decimal::ZERO)
            .normalize()
            .to_string(),
        price_period_months: product.price_period_months.max(1),
        quantity_source: product.quantity_source.clone(),
        default_quantity: pricing::default_quantity(&rule, record_count, units),
        template_id: product.default_template_id,
    }
}

fn template_options(store: &Store, product: &Product) -> TemplateOptions {
    let group = product.group_code.as_deref();
    let contract_rows = crate::templates::list::templates_for(store, group, Role::Contract, true);
    let attachment_rows =
        crate::templates::list::templates_for(store, group, Role::Attachment, true);
    let contract = contract_rows
        .iter()
        .map(|template| TemplateOption {
            id: template.id,
            name: template.name.clone(),
            kind: None,
            own: template_rules::normalize_group(template.product_group.as_deref()).is_some(),
            preselect: false,
        })
        .collect::<Vec<_>>();
    let attachments = attachment_rows
        .iter()
        .map(|template| TemplateOption {
            id: template.id,
            name: template.name.clone(),
            kind: Some(template.contract_type.clone()),
            own: template_rules::normalize_group(template.product_group.as_deref()).is_some(),
            preselect: template_rules::preselected_attachment(&template.to_info()),
        })
        .collect::<Vec<_>>();
    let default_id = contract
        .iter()
        .find(|template| Some(template.id) == product.default_template_id)
        .or_else(|| contract.first())
        .map(|template| template.id);
    TemplateOptions {
        contract,
        attachments,
        default_id,
    }
}

pub fn product_options(
    store: &Store,
    product_id: i64,
    customer_id: i64,
    unit_count: i64,
) -> ProductOptions {
    let Some(main) = store
        .products
        .get(&product_id)
        .filter(|product| product.is_active && product.kind == "main")
    else {
        return ProductOptions::default();
    };
    let customer = store.customers.get(&customer_id);
    let record_count = customer.and_then(|customer| customer.record_count);
    let addons = if let Some(group) = main.group_code.as_deref() {
        let mut products = store
            .products
            .values()
            .filter(|product| {
                product.is_active
                    && product.kind != "main"
                    && product.group_code.as_deref() == Some(group)
            })
            .cloned()
            .collect::<Vec<_>>();
        products.sort_by(|a, b| {
            a.sort_order
                .cmp(&b.sort_order)
                .then_with(|| a.name.cmp(&b.name))
        });
        products
            .iter()
            .map(|product| brief(product, record_count, unit_count))
            .collect()
    } else {
        Vec::new()
    };
    ProductOptions {
        main: Some(brief(main, record_count, unit_count)),
        addons,
        templates: template_options(store, main),
    }
}

pub fn template_rows<'a>(
    store: &'a Store,
    product: &Product,
) -> (Vec<&'a ContractTemplate>, Vec<&'a ContractTemplate>) {
    (
        crate::templates::list::templates_for(
            store,
            product.group_code.as_deref(),
            Role::Contract,
            true,
        ),
        crate::templates::list::templates_for(
            store,
            product.group_code.as_deref(),
            Role::Attachment,
            true,
        ),
    )
}
