//! Phụ lục bổ sung gói giữa kỳ.

use std::collections::BTreeSet;

use cf_core::{lifecycle, numbering, pricing};
use cf_db::{model::Contract, store::Store};
use chrono::NaiveDate;

use crate::{
    Ctx, ServiceError,
    contracts::create::{ContractForm, CreateOptions, create_draft},
    settings::get_setting,
};

pub const KIND_ADDON: &str = "addon";
pub const DOC_KIND_ADDON: &str = "addendum_addon";

#[derive(Debug, Clone)]
pub struct AddendumDefaults {
    pub number: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub month_count: i64,
    pub candidate_product_ids: Vec<i64>,
}

/// Port của ContractForge/app/services/addendum_service.py::can_add_addendum.
pub fn can_add_addendum(contract: &Contract) -> bool {
    contract.doc_kind == "contract"
        && lifecycle::IN_SERVICE.contains(&contract.status)
        && contract.end_date.is_some()
}

fn parent(store: &Store, id: i64) -> Result<&Contract, ServiceError> {
    let c = store
        .contracts
        .get(&id)
        .ok_or_else(|| ServiceError::Invalid("Không tìm thấy hợp đồng gốc.".into()))?;
    if c.doc_kind != "contract" {
        return Err(ServiceError::Invalid(
            "Chỉ hợp đồng gốc mới có phụ lục.".into(),
        ));
    }
    if !lifecycle::IN_SERVICE.contains(&c.status) {
        return Err(ServiceError::Invalid(
            "Hợp đồng gốc chưa ở trạng thái đang phục vụ.".into(),
        ));
    }
    if c.end_date.is_none() || c.start_date.is_none() {
        return Err(ServiceError::Invalid(
            "Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into(),
        ));
    }
    Ok(c)
}

fn format(store: &Store) -> String {
    get_setting(
        store,
        "addendum_number_format",
        cf_core::numbering::DEFAULT_ADDENDUM_FORMAT,
    )
}
fn used(store: &Store, parent_id: i64, format: &str) -> BTreeSet<u32> {
    store
        .indexes
        .children_of(store, parent_id)
        .into_iter()
        .filter_map(|child| numbering::parse_addendum_seq(&child.contract_number, Some(format)))
        .collect()
}
fn group<'a>(store: &'a Store, c: &Contract) -> Option<&'a str> {
    c.product_id
        .and_then(|id| store.products.get(&id))
        .and_then(|p| p.group_code.as_deref())
}

/// Port của ContractForge/app/services/addendum_service.py::addendum_defaults.
pub fn addendum_defaults(
    store: &Store,
    parent_id: i64,
    kind: &str,
    start: NaiveDate,
) -> Result<AddendumDefaults, ServiceError> {
    let p = parent(store, parent_id)?;
    if kind != KIND_ADDON {
        return Err(ServiceError::Invalid(
            "Loại phụ lục này chưa được hỗ trợ.".into(),
        ));
    }
    let start_contract = p.start_date.ok_or_else(|| {
        ServiceError::Invalid("Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into())
    })?;
    let end = p.end_date.ok_or_else(|| {
        ServiceError::Invalid("Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into())
    })?;
    if start < start_contract {
        return Err(ServiceError::Invalid(
            "Ngày bắt đầu trước ngày hiệu lực của hợp đồng gốc.".into(),
        ));
    }
    let months = pricing::addon_months(start, end)?;
    let fmt = format(store);
    let seq = numbering::next_addendum_seq(&used(store, p.id, &fmt));
    let number = numbering::build_addendum_number(&p.contract_number, &seq, Some(&fmt))?;
    let g = group(store, p);
    let mut candidate_product_ids = store
        .products
        .values()
        .filter(|x| x.is_active && x.kind != "main" && g.is_some() && x.group_code.as_deref() == g)
        .map(|x| x.id)
        .collect::<Vec<_>>();
    candidate_product_ids.sort_by_key(|id| {
        store
            .products
            .get(id)
            .map(|x| (x.sort_order, x.name.to_lowercase()))
    });
    Ok(AddendumDefaults {
        number,
        start_date: start,
        end_date: end,
        month_count: i64::from(months),
        candidate_product_ids,
    })
}

#[derive(Debug, Clone)]
pub struct AddonForm {
    pub start_date: NaiveDate,
    pub sign_date: Option<NaiveDate>,
    pub template_id: Option<i64>,
    pub items: Vec<AddonItem>,
}
#[derive(Debug, Clone)]
pub struct AddonItem {
    pub product_id: i64,
    pub description: Option<String>,
    pub quantity: Option<String>,
    pub unit_price_pre_vat: Option<String>,
    pub vat_rate: Option<String>,
    pub amount_pre_vat_override: Option<String>,
    pub note: String,
}

/// Port của ContractForge/app/services/addendum_service.py::create_addendum.
pub fn create_addendum(
    ctx: &mut Ctx,
    parent_id: i64,
    kind: &str,
    form: AddonForm,
) -> Result<i64, ServiceError> {
    let p = parent(ctx.store(), parent_id)?.clone();
    if kind != KIND_ADDON {
        return Err(ServiceError::Invalid(
            "Loại phụ lục này chưa được hỗ trợ.".into(),
        ));
    }
    let start0 = p.start_date.ok_or_else(|| {
        ServiceError::Invalid("Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into())
    })?;
    let end = p.end_date.ok_or_else(|| {
        ServiceError::Invalid("Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into())
    })?;
    if form.start_date < start0 {
        return Err(ServiceError::Invalid(
            "Ngày bắt đầu trước ngày hiệu lực của hợp đồng gốc.".into(),
        ));
    }
    let months = pricing::addon_months(form.start_date, end)?;
    if form.items.is_empty() {
        return Err(ServiceError::Invalid(
            "Chọn ít nhất một gói bổ sung.".into(),
        ));
    }
    let group = group(ctx.store(), &p).map(str::to_owned);
    let customer = ctx
        .store()
        .customers
        .get(&p.customer_id)
        .ok_or_else(|| ServiceError::Invalid("Khách hàng không tồn tại.".into()))?;
    let mut items = Vec::new();
    for (i, row) in form.items.iter().enumerate() {
        let product = ctx
            .store()
            .products
            .get(&row.product_id)
            .filter(|x| x.kind != "main" && group.is_some() && x.group_code == group)
            .ok_or_else(|| {
                ServiceError::Invalid(format!(
                    "Dòng {}: chỉ nhận gói bổ sung cùng nhóm với gói chính của hợp đồng gốc.",
                    i + 1
                ))
            })?;
        let quantity = row
            .quantity
            .clone()
            .filter(|x| !x.trim().is_empty())
            .or_else(|| {
                cf_core::pricing::default_quantity(
                    &product.to_rule(),
                    customer.record_count,
                    p.unit_count.unwrap_or(1),
                )
                .into()
            });
        if quantity.as_deref().unwrap_or_default().is_empty() {
            return Err(ServiceError::Invalid(format!(
                "Dòng {}: thiếu số hồ sơ của khách hàng.",
                i + 1
            )));
        }
        items.push(cf_core::pricing::ItemInput {
            product_id: product.id,
            description: row.description.clone().unwrap_or_default(),
            quantity: quantity.unwrap_or_default(),
            unit_price_pre_vat: row.unit_price_pre_vat.clone(),
            vat_rate: row.vat_rate.clone(),
            amount_pre_vat_override: row.amount_pre_vat_override.clone(),
            note: row.note.clone(),
        });
    }
    let fmt = format(ctx.store());
    let seq = numbering::next_addendum_seq(&used(ctx.store(), p.id, &fmt));
    let number = numbering::build_addendum_number(&p.contract_number, &seq, Some(&fmt))?;
    let template = match form.template_id {
        Some(id) => {
            let t = ctx
                .store()
                .templates
                .get(&id)
                .ok_or_else(|| ServiceError::Invalid("Mẫu phụ lục không tồn tại.".into()))?;
            cf_core::template_rules::check_template_for(
                &cf_core::template_rules::TemplateInfo {
                    id: t.id,
                    name: t.name.clone(),
                    contract_type: t.contract_type.clone(),
                    product_group: t.product_group.clone(),
                    is_active: t.is_active,
                },
                group.as_deref(),
                cf_core::template_rules::Role::Addon,
            )?;
            Some(id)
        }
        None => {
            let candidates = ctx
                .store()
                .templates
                .values()
                .map(|t| cf_core::template_rules::TemplateInfo {
                    id: t.id,
                    name: t.name.clone(),
                    contract_type: t.contract_type.clone(),
                    product_group: t.product_group.clone(),
                    is_active: t.is_active,
                })
                .collect::<Vec<_>>();
            cf_core::template_rules::default_template_for(
                &candidates,
                group.as_deref(),
                cf_core::template_rules::Role::Addon,
            )
            .map(|t| t.id)
        }
    };
    let child_desc = format!("Phụ lục bổ sung của {}", p.contract_number);
    let parent_desc = format!("Đã lập phụ lục {}", number);
    let id = create_draft(
        ctx,
        ContractForm {
            contract_number: number.clone(),
            customer_id: p.customer_id,
            product_id: items.first().map(|x| x.product_id),
            template_id: template,
            sign_date: Some(form.sign_date.unwrap_or_else(|| ctx.clock().today())),
            start_date: Some(form.start_date),
            end_date: Some(end),
            month_count: Some(i64::from(months)),
            sign_location: p.sign_location.clone(),
            unit_count: p.unit_count,
            items,
            ..ContractForm::default()
        },
        CreateOptions {
            require_main: false,
            template_role: "addon",
            template_group: group.as_deref(),
            parent_contract_id: Some(p.id),
            parent_event: Some(("addendum", &parent_desc, "user")),
            parent_event_contract_id: Some(p.id),
            renewed_from_id: None,
            doc_kind: DOC_KIND_ADDON,
            created_event: None,
            child_event: Some(("addendum", &child_desc, "user")),
        },
    )?;
    Ok(id)
}
