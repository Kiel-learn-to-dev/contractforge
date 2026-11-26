use std::path::Path;

use crate::{ServiceError, ctx::Ctx};

pub fn bundled_code(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    format!(
        "BUNDLED-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5]
    )
}

pub fn seed_from_project_files(ctx: &mut Ctx, directory: &Path) -> Result<usize, ServiceError> {
    let mut paths = std::fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("docx"))
        })
        .filter(|path| {
            !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("sample_"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut imported = 0;
    for path in paths {
        let bytes = std::fs::read(&path)?;
        let code = bundled_code(&bytes);
        if ctx
            .store()
            .templates
            .values()
            .any(|template| template.code == code)
        {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .replace('_', " ");
        let compiled = cf_docx::Template::compile(&bytes)
            .map_err(|error| ServiceError::Invalid(error.to_string()))?;
        let analysis = compiled.analysis();
        let text = cf_docx::template::paragraph_texts(&bytes)
            .map_err(|error| ServiceError::Invalid(error.to_string()))?;
        let body = text.get("word/document.xml").cloned().unwrap_or_default();
        let heading = super::bulk::heading_for_import(&body);
        let kind = cf_core::template_rules::template_kind(&analysis.placeholders, &heading);
        super::create::create_template(
            ctx,
            super::create::TemplateDraft {
                code: &code,
                name: &name,
                contract_type: kind,
                file_bytes: &bytes,
                original_filename: filename,
                description: "",
                mode: "placeholder",
                product_group: "",
            },
        )?;
        imported += 1;
    }
    Ok(imported)
}

/// Seed the embedded sample service contract when the catalog has no templates yet.
pub fn seed_embedded_sample_contract(ctx: &mut Ctx) -> Result<bool, ServiceError> {
    let filename = "sample_service_contract.docx";
    let bytes = crate::assets::default_template_bytes(filename)
        .expect("the embedded sample service contract is available");
    let code = bundled_code(&bytes);
    if ctx
        .store()
        .templates
        .values()
        .any(|template| template.code == code)
    {
        return Ok(false);
    }

    let compiled = cf_docx::Template::compile(&bytes)
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    let analysis = compiled.analysis();
    let text = cf_docx::template::paragraph_texts(&bytes)
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    let body = text.get("word/document.xml").cloned().unwrap_or_default();
    let heading = super::bulk::heading_for_import(&body);
    let kind = cf_core::template_rules::template_kind(&analysis.placeholders, &heading);
    super::create::create_template(
        ctx,
        super::create::TemplateDraft {
            code: &code,
            name: "Mẫu hợp đồng dịch vụ (đi kèm)",
            contract_type: kind,
            file_bytes: &bytes,
            original_filename: filename,
            description: "Mẫu hợp đồng dịch vụ mẫu đi kèm ContractForge.",
            mode: "placeholder",
            product_group: "",
        },
    )?;
    Ok(true)
}
