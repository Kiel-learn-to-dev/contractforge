//! Tạo hai dạng gia hạn hợp đồng.

use cf_core::{numbering, template_rules};

use super::addon::{AddonForm, AddonItem};
use crate::{
    Ctx, ServiceError,
    contracts::create::{ContractForm, CreateOptions, create_draft},
};

pub const DOC_KIND_RENEWAL: &str = "addendum_renewal";

fn parent(ctx: &Ctx, id: i64) -> Result<cf_db::model::Contract, ServiceError> {
    let p = ctx
        .store()
        .contracts
        .get(&id)
        .ok_or_else(|| ServiceError::Invalid("Không tìm thấy hợp đồng gốc.".into()))?;
    if p.doc_kind != "contract" {
        return Err(ServiceError::Invalid(
            "Chỉ hợp đồng gốc mới có phụ lục.".into(),
        ));
    }
    if !cf_core::lifecycle::IN_SERVICE.contains(&p.status) {
        return Err(ServiceError::Invalid(
            "Hợp đồng gốc chưa ở trạng thái đang phục vụ.".into(),
        ));
    }
    if p.start_date.is_none() || p.end_date.is_none() {
        return Err(ServiceError::Invalid(
            "Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into(),
        ));
    }
    Ok(p.clone())
}

fn clean_items(
    ctx: &Ctx,
    p: &cf_db::model::Contract,
    rows: Vec<AddonItem>,
) -> Result<Vec<cf_core::pricing::ItemInput>, ServiceError> {
    if rows.is_empty() {
        return Err(ServiceError::Invalid(
            "Chọn ít nhất gói chính cho gia hạn.".into(),
        ));
    }
    let customer = ctx
        .store()
        .customers
        .get(&p.customer_id)
        .ok_or_else(|| ServiceError::Invalid("Khách hàng không tồn tại.".into()))?;
    rows.into_iter()
        .enumerate()
        .map(|(i, row)| {
            let product = ctx
                .store()
                .products
                .get(&row.product_id)
                .filter(|x| x.is_active)
                .ok_or_else(|| {
                    ServiceError::Invalid(format!(
                        "Dòng {}: chưa chọn gói đang bán cho gia hạn.",
                        i + 1
                    ))
                })?;
            let quantity = row
                .quantity
                .filter(|x| !x.is_empty())
                .or_else(|| {
                    let q = cf_core::pricing::default_quantity(
                        &product.to_rule(),
                        customer.record_count,
                        p.unit_count.unwrap_or(1),
                    );
                    (!q.is_empty()).then_some(q)
                })
                .ok_or_else(|| {
                    ServiceError::Invalid(format!("Dòng {}: thiếu số hồ sơ của khách hàng.", i + 1))
                })?;
            Ok(cf_core::pricing::ItemInput {
                product_id: product.id,
                description: row.description.unwrap_or_default(),
                quantity,
                unit_price_pre_vat: row.unit_price_pre_vat,
                vat_rate: row.vat_rate,
                amount_pre_vat_override: row.amount_pre_vat_override,
                note: row.note.trim().to_owned(),
            })
        })
        .collect()
}

fn renewal_template(
    ctx: &Ctx,
    raw: Option<i64>,
    p: &cf_db::model::Contract,
    role: template_rules::Role,
) -> Result<Option<i64>, ServiceError> {
    let group = p
        .product_id
        .and_then(|id| ctx.store().products.get(&id))
        .and_then(|x| x.group_code.as_deref());
    if let Some(id) = raw {
        let t = ctx
            .store()
            .templates
            .get(&id)
            .ok_or_else(|| ServiceError::Invalid("Mẫu phụ lục không tồn tại.".into()))?;
        let info = template_rules::TemplateInfo {
            id: t.id,
            name: t.name.clone(),
            contract_type: t.contract_type.clone(),
            product_group: t.product_group.clone(),
            is_active: t.is_active,
        };
        template_rules::check_template_for(&info, group, role)?;
        if !t.is_active {
            return Err(ServiceError::Invalid(format!("Mẫu '{}' đang tắt.", t.name)));
        }
        return Ok(Some(id));
    }
    let all = ctx
        .store()
        .templates
        .values()
        .map(|t| template_rules::TemplateInfo {
            id: t.id,
            name: t.name.clone(),
            contract_type: t.contract_type.clone(),
            product_group: t.product_group.clone(),
            is_active: t.is_active,
        })
        .collect::<Vec<_>>();
    Ok(template_rules::default_template_for(&all, group, role).map(|t| t.id))
}

fn sequence(ctx: &Ctx, p: &cf_db::model::Contract) -> Result<String, ServiceError> {
    let fmt = crate::settings::get_setting(
        ctx.store(),
        "addendum_number_format",
        numbering::DEFAULT_ADDENDUM_FORMAT,
    );
    let used = ctx
        .store()
        .indexes
        .children_of(ctx.store(), p.id)
        .into_iter()
        .filter_map(|c| numbering::parse_addendum_seq(&c.contract_number, Some(&fmt)))
        .collect();
    let seq = numbering::next_addendum_seq(&used);
    Ok(numbering::build_addendum_number(
        &p.contract_number,
        &seq,
        Some(&fmt),
    )?)
}

/// Port của ContractForge/app/services/addendum_service.py::create_renewal_appendix.
pub fn create_renewal_appendix(
    ctx: &mut Ctx,
    parent_id: i64,
    form: AddonForm,
    month_count: i64,
) -> Result<i64, ServiceError> {
    let p = parent(ctx, parent_id)?;
    let start0 = p.start_date.ok_or_else(|| {
        ServiceError::Invalid("Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into())
    })?;
    let end = p.end_date.ok_or_else(|| {
        ServiceError::Invalid("Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into())
    })?;
    if form.start_date <= end {
        return Err(ServiceError::Invalid(
            "Gia hạn phải bắt đầu sau ngày hết hạn của hợp đồng gốc.".into(),
        ));
    }
    if form.start_date < start0 {
        return Err(ServiceError::Invalid(
            "Gia hạn phải bắt đầu sau ngày hết hạn của hợp đồng gốc.".into(),
        ));
    }
    if month_count < 1 {
        return Err(ServiceError::Invalid(
            "Số tháng gia hạn phải từ 1 trở lên.".into(),
        ));
    }
    let months = i32::try_from(month_count)
        .map_err(|_| ServiceError::Invalid("Số tháng gia hạn phải từ 1 trở lên.".into()))?;
    let items = clean_items(ctx, &p, form.items)?;
    let number = sequence(ctx, &p)?;
    let template = renewal_template(ctx, form.template_id, &p, template_rules::Role::Renewal)?;
    let child_desc = format!("Phụ lục gia hạn của {}", p.contract_number);
    let parent_desc = format!("Đã lập phụ lục gia hạn {}", number);
    let group = p
        .product_id
        .and_then(|id| ctx.store().products.get(&id))
        .and_then(|x| x.group_code.clone());
    create_draft(
        ctx,
        ContractForm {
            contract_number: number,
            customer_id: p.customer_id,
            product_id: items.first().map(|x| x.product_id),
            template_id: template,
            sign_date: Some(form.sign_date.unwrap_or_else(|| ctx.clock().today())),
            start_date: Some(form.start_date),
            end_date: Some(cf_core::dates::compute_end_date(form.start_date, months)),
            month_count: Some(month_count),
            sign_location: p.sign_location,
            unit_count: p.unit_count,
            items,
            ..ContractForm::default()
        },
        CreateOptions {
            require_main: true,
            template_role: "renewal",
            template_group: group.as_deref(),
            parent_contract_id: Some(p.id),
            parent_event: Some(("renewal", &parent_desc, "user")),
            parent_event_contract_id: Some(p.id),
            renewed_from_id: None,
            doc_kind: DOC_KIND_RENEWAL,
            created_event: None,
            child_event: Some(("renewal", &child_desc, "user")),
        },
    )
}

/// Port của ContractForge/app/services/addendum_service.py::create_renewal_contract.
pub fn create_renewal_contract(
    ctx: &mut Ctx,
    parent_id: i64,
    number: &str,
    form: AddonForm,
    month_count: i64,
) -> Result<i64, ServiceError> {
    let p = parent(ctx, parent_id)?;
    let number = number.trim();
    if number.is_empty() {
        return Err(ServiceError::Invalid("Nhập số hợp đồng mới.".into()));
    }
    let end = p.end_date.ok_or_else(|| {
        ServiceError::Invalid("Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.".into())
    })?;
    if form.start_date <= end {
        return Err(ServiceError::Invalid(
            "Hợp đồng gia hạn phải bắt đầu sau ngày hết hạn của hợp đồng cũ.".into(),
        ));
    }
    let items = clean_items(ctx, &p, form.items)?;
    let first = items
        .first()
        .and_then(|x| ctx.store().products.get(&x.product_id));
    let template = match form.template_id {
        Some(id) => Some(id),
        None => first.and_then(|x| x.default_template_id),
    };
    let child_desc = format!("Gia hạn từ hợp đồng {}", p.contract_number);
    let parent_desc = format!("Đã gia hạn bằng hợp đồng {}", number);
    create_draft(
        ctx,
        ContractForm {
            contract_number: number.into(),
            customer_id: p.customer_id,
            product_id: items.first().map(|x| x.product_id),
            template_id: template,
            sign_date: Some(form.sign_date.unwrap_or_else(|| ctx.clock().today())),
            start_date: Some(form.start_date),
            month_count: (month_count != 0).then_some(month_count),
            sign_location: p.sign_location,
            unit_count: p.unit_count,
            items,
            ..ContractForm::default()
        },
        CreateOptions {
            require_main: true,
            template_role: "contract",
            template_group: None,
            parent_contract_id: None,
            parent_event: Some(("renewal", &parent_desc, "user")),
            parent_event_contract_id: Some(p.id),
            renewed_from_id: Some(p.id),
            doc_kind: "contract",
            created_event: None,
            child_event: Some(("renewal", &child_desc, "user")),
        },
    )
}
