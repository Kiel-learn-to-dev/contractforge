use std::collections::BTreeSet;
use std::io::{Cursor, Write};

use cf_core::{
    names::make_output_filename,
    template_rules::{self, Role},
};
use cf_db::model::ContractTemplate;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{Ctx, ServiceError};

/// Port của app/services/attachment_service.py::resolve_attachment_templates.
pub fn resolve_attachment_templates(
    store: &cf_db::store::Store,
    ids: &[i64],
    contract_id: i64,
) -> Result<Vec<ContractTemplate>, ServiceError> {
    resolve_attachment_templates_with_root(store, None, ids, contract_id)
}

pub(crate) fn resolve_attachment_templates_with_root(
    store: &cf_db::store::Store,
    root: Option<&std::path::Path>,
    ids: &[i64],
    contract_id: i64,
) -> Result<Vec<ContractTemplate>, ServiceError> {
    let contract = store
        .contracts
        .get(&contract_id)
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy hợp đồng.".into()))?;
    let root_contract = contract
        .parent_contract_id
        .and_then(|id| store.contracts.get(&id))
        .unwrap_or(contract);
    let group = root_contract
        .product_id
        .and_then(|id| store.products.get(&id))
        .and_then(|product| product.group_code.as_deref());
    let mut seen = BTreeSet::new();
    let mut templates = Vec::new();
    for id in ids.iter().copied().filter(|id| seen.insert(*id)) {
        let template = store.templates.get(&id).ok_or_else(|| {
            ServiceError::Invalid(format!("Không tìm thấy văn bản kèm theo #{id}."))
        })?;
        if !template.is_active {
            return Err(ServiceError::Invalid(format!(
                "Mẫu '{}' đang tắt.",
                template.name
            )));
        }
        template_rules::check_template_for(&template.to_info(), group, Role::Attachment)?;
        template
            .file_path
            .as_deref()
            .filter(|path| {
                root.map_or_else(
                    || std::path::PathBuf::from(path),
                    |dir| cf_db::paths::resolve_stored(dir, path),
                )
                .is_file()
            })
            .ok_or_else(|| {
                ServiceError::Invalid(format!(
                    "File của mẫu '{}' không tồn tại trên đĩa.",
                    template.name
                ))
            })?;
        templates.push(template.clone());
    }
    Ok(templates)
}

/// Port của app/services/attachment_service.py::render_attachments.
pub fn render_attachments(
    ctx: &mut Ctx,
    contract_id: i64,
    templates: &[ContractTemplate],
) -> Result<Vec<(String, Vec<u8>)>, ServiceError> {
    let mut rendered = Vec::with_capacity(templates.len());
    for template in templates {
        let result = render_attachment(ctx, contract_id, template);
        match result {
            Ok(item) => rendered.push(item),
            Err(error) => {
                return Err(ServiceError::Invalid(format!(
                    "Không sinh được '{}': {}",
                    template.name,
                    error.user_message()
                )));
            }
        }
    }
    Ok(rendered)
}

fn render_attachment(
    ctx: &mut Ctx,
    contract_id: i64,
    template: &ContractTemplate,
) -> Result<(String, Vec<u8>), ServiceError> {
    let (contract_number, customer_code, path) = {
        let store = ctx.store();
        let contract = store
            .contracts
            .get(&contract_id)
            .ok_or_else(|| ServiceError::NotFound("Không tìm thấy hợp đồng.".into()))?;
        let customer = store
            .customers
            .get(&contract.customer_id)
            .ok_or_else(|| ServiceError::NotFound("Không tìm thấy khách hàng.".into()))?;
        let path = template
            .file_path
            .clone()
            .ok_or_else(|| ServiceError::FileMissing("File mẫu không tồn tại trên đĩa.".into()))?;
        (
            contract.contract_number.clone(),
            customer.code.clone(),
            path,
        )
    };
    let resolved_path = ctx.paths().resolve_stored(&path);
    let compiled = ctx.templates_mut().get_or_compile(&resolved_path)?;
    let legacy_rows = compiled.analysis().optional_keys.is_empty();
    let render_data = super::render::render_data_for_with_legacy_rows(
        ctx.store(),
        contract_id,
        template.id,
        Some(legacy_rows),
    )?;
    let bytes = compiled.render(&render_data)?;
    let name = make_output_filename(
        &contract_number,
        &customer_code,
        &template.contract_type,
        ctx.clock().now_local(),
    );
    Ok((name, bytes))
}

/// Port của app/services/attachment_service.py::build_bundle_zip.
pub fn build_bundle_zip(items: &[(String, Vec<u8>)]) -> Result<Vec<u8>, ServiceError> {
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let mut used = BTreeSet::new();
    for (filename, bytes) in items {
        let filename = unique_name(filename, &mut used);
        output
            .start_file(
                filename,
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .map_err(|error| ServiceError::Invalid(error.to_string()))?;
        output.write_all(bytes)?;
    }
    let cursor = output
        .finish()
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    Ok(cursor.into_inner())
}

fn unique_name(filename: &str, used: &mut BTreeSet<String>) -> String {
    let mut candidate = filename.to_owned();
    let mut suffix = 2;
    while !used.insert(candidate.clone()) {
        let path = std::path::Path::new(filename);
        let stem = path
            .file_stem()
            .and_then(|part| part.to_str())
            .unwrap_or(filename);
        let extension = path.extension().and_then(|part| part.to_str());
        candidate = match extension {
            Some(extension) => format!("{stem}_{suffix}.{extension}"),
            None => format!("{stem}_{suffix}"),
        };
        suffix += 1;
    }
    candidate
}
