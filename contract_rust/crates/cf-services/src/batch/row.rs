//! Dựng và sinh một hợp đồng của batch.

use std::fs;

use cf_core::{
    lifecycle::ContractStatus,
    names::make_output_filename,
    render_context::{self, ParentView, ProductView, TemplateMapping},
    template_rules::Role,
};
use cf_db::store::Store;
use cf_docx::{RenderData, Template, write::write_new_file};
use rust_decimal::Decimal;
use serde_json::{Value, json};

use crate::{
    Ctx, ServiceError,
    contracts::{
        create::{self, ContractForm, CreateOptions},
        suggest,
    },
    events::{self, ChangeRef},
    settings,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedRow {
    pub contract_id: i64,
    pub customer_id: i64,
    pub contract_number: String,
    pub file_path: std::path::PathBuf,
    pub file_name: String,
}

/// Render trước, sau đó ghi hợp đồng, các dòng và event trong một Store::apply.
pub fn batch_row(
    ctx: &mut Ctx,
    job_id: &str,
    mut form: ContractForm,
    year: i32,
    compiled: &Template,
    legacy_party_a_rows: bool,
) -> Result<GeneratedRow, ServiceError> {
    let customer = ctx
        .store()
        .customers
        .get(&form.customer_id)
        .cloned()
        .ok_or_else(|| {
            ServiceError::NotFound(format!("Không tìm thấy khách hàng id={}", form.customer_id))
        })?;
    let template_id = form
        .template_id
        .ok_or_else(|| ServiceError::Invalid("Chưa chọn mẫu hợp đồng.".into()))?;
    let template = ctx
        .store()
        .templates
        .get(&template_id)
        .cloned()
        .ok_or_else(|| ServiceError::Invalid("Mẫu hợp đồng không tồn tại.".into()))?;
    if form.contract_number.trim().is_empty() {
        form.contract_number =
            suggest::suggest_number(ctx.store(), customer.id, form.product_id, year)?;
    }
    let number = form.contract_number.trim().to_owned();
    if ctx
        .store()
        .contracts
        .values()
        .any(|c| c.contract_number == number)
    {
        return Err(ServiceError::Invalid(format!(
            "Số HĐ '{number}' đã tồn tại"
        )));
    }
    let now = ctx.clock().now_utc();
    let prepared = create::prepare_contract(ctx, form, CreateOptions::default(), now, false)?;
    let mut contract = prepared.contract;
    contract.status = ContractStatus::Generated;
    contract.is_batch = true;
    contract.batch_job_id = Some(job_id.to_owned());
    if contract.pricing_model == "legacy"
        && contract
            .pricing_description
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
    {
        let text = cf_core::pricing::auto_pricing_description(
            contract.unit_price_vat,
            contract.unit_count.unwrap_or(1),
            &contract.selected_unit_names,
            contract.month_count,
        );
        if !text.is_empty() {
            contract.pricing_description = Some(text);
        }
    }
    let product = contract
        .product_id
        .and_then(|id| ctx.store().products.get(&id));
    cf_core::template_rules::check_template_for(
        &template.to_info(),
        product.and_then(|p| p.group_code.as_deref()),
        Role::Contract,
    )?;
    let units = ctx
        .store()
        .units
        .values()
        .filter(|u| u.customer_id == customer.id)
        .cloned()
        .collect::<Vec<_>>();
    let customer_view = customer.to_view_with_units(&units);
    let product_view = product.map(|p| ProductView {
        name: p.name.clone(),
        code: p.code.clone(),
    });
    let parent = contract
        .parent_contract_id
        .and_then(|id| ctx.store().contracts.get(&id));
    let parent_view = parent.map(|p| ParentView {
        contract_number: p.contract_number.clone(),
        sign_date: p.sign_date,
    });
    let mut values = render_context::build_render_context(
        &contract.to_view(),
        &customer_view,
        parent_view.as_ref(),
        product_view.as_ref(),
        &settings::get_party_b_settings(ctx.store()),
        Some(&settings::addendum_format(ctx.store())),
    );
    let features = prepared
        .lines
        .iter()
        .map(|line| {
            line.product_id
                .and_then(|id| ctx.store().products.get(&id))
                .map(|p| p.feature_list.clone())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let lists = render_context::lists_from_lines(&prepared.lines, &features);
    let has_08a = template
        .detected_placeholders
        .iter()
        .filter_map(|key| {
            render_context::CANONICAL_MAP
                .iter()
                .find(|(placeholder, _)| placeholder == key)
                .map(|(_, field)| *field)
        })
        .chain(template.field_mapping.values().map(String::as_str))
        .any(|field| field.starts_with("form08a."));
    if has_08a {
        values = render_context::build_08a_context(values, &customer_view, product_view.as_ref());
    }
    values = render_context::apply_template_mapping(
        &values,
        &TemplateMapping(template.field_mapping.clone()),
    );
    let render_data = RenderData {
        values,
        lists: Some(lists),
        legacy_party_a_rows,
        ..RenderData::default()
    };
    let bytes = compiled
        .render(&render_data)
        .map_err(|e| ServiceError::Invalid(format!("Lỗi khi render .docx: {e}")))?;

    let proposed = make_output_filename(
        &number,
        &customer.code,
        &template.contract_type,
        ctx.clock().now_local(),
    );
    fs::create_dir_all(&ctx.paths().output_batch)?;
    let (file_path, file_name) = write_new_file(&ctx.paths().output_batch, &proposed, &bytes)
        .map_err(|e| ServiceError::Invalid(format!("Không ghi được file hợp đồng: {e}")))?;
    contract.output_file_path = Some(file_path.to_string_lossy().into_owned());
    contract.output_file_name = Some(file_name.clone());
    contract.render_data = json!({"values": render_data.values, "lists": render_data.lists});
    contract.updated_at = now;
    let mut changes = create::insert_contract_changes(&contract, &prepared.lines);
    changes.push(events::event(
        ChangeRef::New(0),
        "generated",
        &format!("[Batch {job_id}] Sinh file {file_name}"),
        "batch",
        now,
    ));
    match ctx.store_mut().apply(changes) {
        Ok(result) => {
            let contract_id =
                result.inserted_ids.first().copied().ok_or_else(|| {
                    ServiceError::Invalid("Không tạo được hợp đồng trong lô.".into())
                })?;
            Ok(GeneratedRow {
                contract_id,
                customer_id: customer.id,
                contract_number: number,
                file_path,
                file_name,
            })
        }
        Err(error) => {
            let _ = fs::remove_file(&file_path);
            Err(error.into())
        }
    }
}

pub fn product_id_by_code(store: &Store, code: &str) -> Result<i64, ServiceError> {
    store
        .products
        .values()
        .find(|p| p.code.eq_ignore_ascii_case(code.trim()))
        .map(|p| p.id)
        .ok_or_else(|| ServiceError::Invalid(format!("Không tìm thấy gói có mã {}.", code.trim())))
}

/// Convert merged shared/Excel string values into the typed contract form.
pub fn form_from_params(
    store: &Store,
    customer_id: i64,
    template_id: i64,
    params: &std::collections::BTreeMap<String, String>,
) -> Result<ContractForm, ServiceError> {
    let customer = store
        .customers
        .get(&customer_id)
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy khách hàng.".into()))?;
    let parse_date = |key: &str| -> Result<Option<chrono::NaiveDate>, ServiceError> {
        match params
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(raw) => cf_core::dates::parse_date_input(Some(raw)).map_err(ServiceError::from),
            None => Ok(None),
        }
    };
    let parse_decimal = |key: &str| -> Result<Option<Decimal>, ServiceError> {
        params
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|raw| cf_core::money::to_decimal(raw, key).map_err(ServiceError::from))
            .transpose()
    };
    let parse_integer = |key: &str| -> Result<Option<i64>, ServiceError> {
        params
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|raw| {
                raw.parse::<i64>()
                    .map_err(|_| ServiceError::Invalid(format!("Giá trị {key} không hợp lệ.")))
            })
            .transpose()
    };
    let sign_date = parse_date("sign_date")?;
    let start_date = parse_date("start_date")?;
    let end_date = parse_date("end_date")?;
    let month_count = parse_integer("month_count")?.or_else(|| {
        start_date
            .zip(end_date)
            .map(|(s, e)| i64::from(cf_core::dates::months_between(s, e)))
    });
    let included_units = store
        .units
        .values()
        .filter(|unit| unit.customer_id == customer_id && unit.is_active && unit.include_in_billing)
        .collect::<Vec<_>>();
    let mut selected_unit_names = Vec::new();
    if customer.bill_main_station {
        selected_unit_names.push(
            customer
                .short_name
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&customer.legal_name)
                .to_owned(),
        );
    }
    selected_unit_names.extend(included_units.iter().map(|unit| unit.name.clone()));
    let unit_count =
        parse_integer("unit_count")?.unwrap_or_else(|| selected_unit_names.len().max(1) as i64);

    let raw_items = if let Some(main_code) = params
        .get("main_code")
        .map(String::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let mut codes = vec![main_code.to_owned()];
        if let Some(addons) = params.get("addon_codes") {
            codes.extend(
                addons
                    .split([';', ','])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            );
        }
        let mut rows = Vec::with_capacity(codes.len());
        for code in codes {
            rows.push(json!({"product_id": product_id_by_code(store, &code)?}));
        }
        rows
    } else if let Some(raw) = params.get("items_json").filter(|s| !s.trim().is_empty()) {
        serde_json::from_str::<Vec<Value>>(raw)
            .map_err(|_| ServiceError::Invalid("Danh sách hạng mục không hợp lệ.".into()))?
    } else {
        Vec::new()
    };

    let mut items = Vec::with_capacity(raw_items.len());
    for (index, item) in raw_items.iter().enumerate() {
        let Some(id) = item.get("product_id").and_then(value_as_i64) else {
            return Err(ServiceError::Invalid(format!(
                "Dòng {} thiếu gói hợp lệ.",
                index + 1
            )));
        };
        let product = store.products.get(&id).ok_or_else(|| {
            ServiceError::Invalid(format!("Dòng {}: gói {id} không tồn tại.", index + 1))
        })?;
        let quantity = item
            .get("quantity")
            .map(value_text)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                cf_core::pricing::default_quantity(
                    &product.to_rule(),
                    customer.record_count,
                    unit_count,
                )
            });
        if quantity.is_empty() {
            return Err(ServiceError::Invalid(format!(
                "Gói {}: thiếu số hồ sơ của khách hàng.",
                product.code
            )));
        }
        items.push(cf_core::pricing::ItemInput {
            product_id: id,
            description: item.get("description").map(value_text).unwrap_or_default(),
            quantity,
            unit_price_pre_vat: item
                .get("unit_price_pre_vat")
                .map(value_text)
                .filter(|s| !s.is_empty()),
            vat_rate: item
                .get("vat_rate")
                .map(value_text)
                .filter(|s| !s.is_empty()),
            amount_pre_vat_override: item
                .get("amount_pre_vat_override")
                .map(value_text)
                .filter(|s| !s.is_empty()),
            note: item.get("note").map(value_text).unwrap_or_default(),
        });
    }
    Ok(ContractForm {
        contract_number: params.get("contract_number").cloned().unwrap_or_default(),
        customer_id,
        product_id: parse_integer("product_id")?
            .or_else(|| items.first().map(|item| item.product_id)),
        template_id: Some(template_id),
        sign_date,
        start_date,
        end_date,
        month_count,
        sign_location: params.get("sign_location").cloned(),
        unit_count: Some(unit_count),
        unit_price_pre_vat: parse_decimal("unit_price_pre_vat")?,
        vat_rate: parse_decimal("vat_rate")?,
        unit_price_vat: parse_decimal("unit_price_vat")?,
        quantity: parse_decimal("quantity")?,
        total_amount: parse_decimal("total_amount")?,
        amount_text: params.get("amount_text").cloned(),
        pricing_description: params.get("pricing_description").cloned(),
        party_b_representative: params.get("party_b_representative").cloned(),
        party_b_authorization_number: params.get("party_b_authorization_number").cloned(),
        party_b_authorization_date: params.get("party_b_authorization_date").cloned(),
        notes: params.get("notes").cloned(),
        selected_unit_names,
        items,
        ..ContractForm::default()
    })
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.trim().parse().ok())
}
fn value_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}
