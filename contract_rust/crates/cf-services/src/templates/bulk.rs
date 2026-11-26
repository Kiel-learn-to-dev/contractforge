use std::path::Path;

use crate::{ServiceError, ctx::Ctx};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkTemplateResult {
    pub filename: String,
    pub template_id: Option<i64>,
    pub error: Option<String>,
}

pub(crate) fn heading_for_import(paragraphs: &[String]) -> String {
    let lines = paragraphs
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .take(10)
        .collect::<Vec<_>>();
    let Some(start) = lines.iter().position(|line| line.starts_with("BIÊN BẢN")) else {
        return String::new();
    };
    lines
        .iter()
        .skip(start)
        .take(3)
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn bulk_upload(ctx: &mut Ctx, paths: &[impl AsRef<Path>]) -> Vec<BulkTemplateResult> {
    let mut results = Vec::with_capacity(paths.len());
    for path in paths {
        let path = path.as_ref();
        let filename = path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_owned();
        let result = (|| -> Result<i64, ServiceError> {
            let bytes = std::fs::read(path)?;
            let compiled = cf_docx::Template::compile(&bytes)
                .map_err(|error| ServiceError::Invalid(error.to_string()))?;
            let analysis = compiled.analysis();
            let text = cf_docx::template::paragraph_texts(&bytes)
                .map_err(|error| ServiceError::Invalid(error.to_string()))?;
            let paragraphs = text.get("word/document.xml").cloned().unwrap_or_default();
            let contract_type = cf_core::template_rules::template_kind(
                &analysis.placeholders,
                &heading_for_import(&paragraphs),
            );
            let name = path
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or_default()
                .replace('_', " ");
            super::create::create_template(
                ctx,
                super::create::TemplateDraft {
                    code: "",
                    name: &name,
                    contract_type,
                    file_bytes: &bytes,
                    original_filename: &filename,
                    description: "",
                    mode: "placeholder",
                    product_group: "",
                },
            )
        })();
        results.push(match result {
            Ok(template_id) => BulkTemplateResult {
                filename,
                template_id: Some(template_id),
                error: None,
            },
            Err(error) => BulkTemplateResult {
                filename,
                template_id: None,
                error: Some(error.user_message()),
            },
        });
    }
    results
}
