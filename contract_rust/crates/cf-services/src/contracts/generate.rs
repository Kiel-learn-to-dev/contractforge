use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use cf_core::{
    fmt::fmt_currency, lifecycle::ContractStatus, names::make_output_filename, template_rules,
};
use cf_db::{
    change::{Change, ChangeValue},
    codec,
    model::Contract,
};
use cf_docx::{RenderData, write::write_new_file};
use rusqlite::types::Value;
use rust_decimal::Decimal;
use serde_json::Value as Json;

use crate::{
    Ctx, ServiceError,
    events::{self, ChangeRef},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateResult {
    pub file_path: PathBuf,
    pub file_name: String,
    pub bundle_path: Option<PathBuf>,
    pub attachments_count: usize,
}

/// Port của app/services/contract_service.py::generate_docx.
pub fn generate_docx(
    ctx: &mut Ctx,
    contract_id: i64,
    attachment_ids: &[i64],
) -> Result<GenerateResult, ServiceError> {
    let (contract, template, customer, group, old_file_path) = {
        let store = ctx.store();
        let contract = store.contracts.get(&contract_id).cloned().ok_or_else(|| {
            ServiceError::NotFound(format!("Không tìm thấy hợp đồng id={contract_id}"))
        })?;
        let template_id = contract.template_id.ok_or_else(|| {
            ServiceError::Invalid("Hợp đồng chưa chọn mẫu hợp đồng (template).".into())
        })?;
        let template = store
            .templates
            .get(&template_id)
            .cloned()
            .ok_or_else(|| ServiceError::Invalid("Mẫu hợp đồng không tồn tại.".into()))?;
        let root = contract
            .parent_contract_id
            .and_then(|id| store.contracts.get(&id))
            .unwrap_or(&contract);
        let product = root.product_id.and_then(|id| store.products.get(&id));
        let group = product
            .and_then(|product| product.group_code.as_deref())
            .map(str::to_owned);
        let customer = store
            .customers
            .get(&contract.customer_id)
            .cloned()
            .ok_or_else(|| {
                ServiceError::NotFound(format!(
                    "Khách hàng id={} không tồn tại.",
                    contract.customer_id
                ))
            })?;
        (
            contract.clone(),
            template,
            customer,
            group,
            contract.output_file_path.clone(),
        )
    };

    template_rules::check_template_for(
        &template.to_info(),
        group.as_deref(),
        template_rules::role_of_doc_kind(Some(&contract.doc_kind)),
    )?;
    let template_path = template
        .file_path
        .as_deref()
        .map(|path| ctx.paths().resolve_stored(path))
        .filter(|path| path.is_file())
        .ok_or_else(|| {
            ServiceError::FileMissing(format!(
                "File template không tìm thấy trên disk: {:?}",
                template.file_path
            ))
        })?;

    let attachment_templates = super::attachments::resolve_attachment_templates_with_root(
        ctx.store(),
        Some(&ctx.paths().root),
        attachment_ids,
        contract_id,
    )?;
    let auto_description = if contract.pricing_model == "legacy"
        && contract
            .pricing_description
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        auto_pricing_description(ctx.store(), &contract, &customer)
    } else {
        None
    };

    let compiled = ctx.templates_mut().get_or_compile(&template_path)?;
    let legacy_party_a_rows = compiled.analysis().optional_keys.is_empty();
    let mut render_data = super::render::render_data_for_with_legacy_rows(
        ctx.store(),
        contract_id,
        template.id,
        Some(legacy_party_a_rows),
    )?;
    if let Some(description) = auto_description.as_deref() {
        render_data
            .values
            .insert("PRICING_DESCRIPTION".into(), description.into());
        for (placeholder, canonical) in &template.field_mapping {
            if canonical == "contract.pricing_description" {
                render_data
                    .values
                    .insert(placeholder.clone(), description.into());
            }
        }
    }
    let rendered = compiled
        .render(&render_data)
        .map_err(|error| ServiceError::Invalid(format!("Lỗi khi render .docx: {error}")))?;
    let attachments =
        super::attachments::render_attachments(ctx, contract_id, &attachment_templates)?;

    let now_local = ctx.clock().now_local();
    let file_name = make_output_filename(
        &contract.contract_number,
        &customer.code,
        &template.contract_type,
        now_local,
    );
    fs::create_dir_all(&ctx.paths().output_contracts)?;
    let (file_path, file_name) =
        write_new_file(&ctx.paths().output_contracts, &file_name, &rendered).map_err(|error| {
            ServiceError::Invalid(format!("Không ghi được file hợp đồng: {error}"))
        })?;

    let bundle_path = if attachments.is_empty() {
        None
    } else {
        let mut items = Vec::with_capacity(attachments.len() + 1);
        items.push((file_name.clone(), rendered));
        items.extend(attachments.iter().cloned());
        let bytes = match super::attachments::build_bundle_zip(&items) {
            Ok(bytes) => bytes,
            Err(error) => {
                remove_quietly(&file_path);
                return Err(error);
            }
        };
        let directory = ctx.paths().output_bundles.join(contract_id.to_string());
        if let Err(error) = fs::create_dir_all(&directory) {
            remove_quietly(&file_path);
            return Err(error.into());
        }
        let stem = Path::new(&file_name)
            .file_stem()
            .and_then(|part| part.to_str())
            .unwrap_or("contract");
        match write_new_file(&directory, &format!("{stem}_kem_theo.zip"), &bytes) {
            Ok((path, _)) => Some(path),
            Err(error) => {
                remove_quietly(&file_path);
                return Err(ServiceError::Invalid(format!(
                    "Không ghi được file ZIP văn bản kèm theo: {error}"
                )));
            }
        }
    };

    let now_utc = ctx.clock().now_utc();
    let snapshot = render_snapshot(&render_data);
    let mut fields = BTreeMap::from([
        (
            "output_file_path".into(),
            ChangeValue::Sql(Value::Text(file_path.to_string_lossy().into_owned())),
        ),
        (
            "output_file_name".into(),
            ChangeValue::Sql(Value::Text(file_name.clone())),
        ),
        (
            "render_data".into(),
            ChangeValue::Sql(Value::Text(snapshot.to_string())),
        ),
        (
            "updated_at".into(),
            ChangeValue::Sql(codec::write_datetime(Some(now_utc))),
        ),
    ]);
    if let Some(description) = auto_description {
        fields.insert(
            "pricing_description".into(),
            ChangeValue::Sql(Value::Text(description)),
        );
    }
    let mut changes = vec![Change::update("contracts", contract_id, fields)];
    if contract.status == ContractStatus::Draft {
        match super::status::change_status_changes(
            ctx.store(),
            contract_id,
            "Generated",
            "system",
            "",
            now_utc,
        ) {
            Ok(status_changes) => changes.extend(status_changes),
            Err(error) => {
                cleanup_new_files(&file_path, bundle_path.as_deref());
                return Err(error);
            }
        }
    }
    changes.push(events::event(
        ChangeRef::Existing(contract_id),
        "generated",
        &format!("Sinh file {file_name}"),
        "system",
        now_utc,
    ));
    if let Err(error) = ctx.store_mut().apply(changes) {
        cleanup_new_files(&file_path, bundle_path.as_deref());
        return Err(error.into());
    }

    if let Some(old_path) = old_file_path {
        let old_path = ctx.paths().resolve_stored(&old_path);
        if old_path != file_path {
            remove_quietly(&old_path);
        }
    }
    cleanup_old_bundles(
        &ctx.paths().output_bundles.join(contract_id.to_string()),
        bundle_path.as_deref(),
    );

    Ok(GenerateResult {
        file_path,
        file_name,
        bundle_path,
        attachments_count: attachment_templates.len(),
    })
}

fn auto_pricing_description(
    store: &cf_db::store::Store,
    contract: &Contract,
    customer: &cf_db::model::Customer,
) -> Option<String> {
    let unit_price = contract.unit_price_vat.filter(|price| !price.is_zero())?;
    let months = contract.month_count.filter(|months| *months != 0)?;
    let names = if contract.selected_unit_names.is_empty() {
        let main_unit = customer.bill_main_station.then(|| {
            customer
                .short_name
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| customer.legal_name.clone())
        });
        let mut units = store
            .units
            .values()
            .filter(|unit| {
                unit.customer_id == customer.id && unit.is_active && unit.include_in_billing
            })
            .collect::<Vec<_>>();
        units.sort_by_key(|unit| (unit.sort_order, unit.id));
        main_unit
            .into_iter()
            .chain(units.into_iter().map(|unit| unit.name.clone()))
            .collect::<Vec<_>>()
    } else {
        contract.selected_unit_names.clone()
    };
    let unit_count = contract
        .unit_count
        .filter(|count| *count != 0)
        .unwrap_or_else(|| {
            if !names.is_empty() {
                names.len() as i64
            } else {
                i64::from(customer.bill_main_station)
                    + store
                        .units
                        .values()
                        .filter(|unit| {
                            unit.customer_id == customer.id
                                && unit.is_active
                                && unit.include_in_billing
                        })
                        .count() as i64
            }
        });
    let total = unit_price * Decimal::from(unit_count) * Decimal::from(months);
    let detail = if names.len() > 1 {
        format!(" ({})", names.join("; "))
    } else {
        String::new()
    };
    Some(format!(
        "Số tiền = Đơn giá có VAT ({} VNĐ) × {} cơ sở{} × {} tháng = {} VNĐ",
        fmt_currency(Some(unit_price)),
        unit_count,
        detail,
        months,
        fmt_currency(Some(total)),
    ))
}

fn render_snapshot(data: &RenderData) -> Json {
    let mut snapshot = data
        .values
        .iter()
        .map(|(key, value)| (key.clone(), Json::String(value.clone())))
        .collect::<serde_json::Map<_, _>>();
    if let Some(lists) = &data.lists {
        snapshot.insert(
            "_lists".into(),
            serde_json::to_value(lists).unwrap_or(Json::Null),
        );
    }
    Json::Object(snapshot)
}

fn cleanup_new_files(file_path: &Path, bundle_path: Option<&Path>) {
    remove_quietly(file_path);
    if let Some(path) = bundle_path {
        remove_quietly(path);
    }
}

fn cleanup_old_bundles(directory: &Path, keep: Option<&Path>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if keep != Some(path.as_path()) {
            remove_quietly(&path);
        }
    }
    if keep.is_none() {
        let _ = fs::remove_dir(directory);
    }
}

fn remove_quietly(path: &Path) {
    let _ = fs::remove_file(path);
}
