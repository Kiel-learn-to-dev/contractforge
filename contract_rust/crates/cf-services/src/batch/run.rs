//! Điều phối batch, compile template một lần và cô lập lỗi theo từng hàng.

use std::{
    fs,
    io::Cursor,
    sync::atomic::{AtomicBool, Ordering},
};

use cf_docx::write::write_new_file;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    Ctx, ServiceError,
    batch::row::{GeneratedRow, batch_row},
    contracts::create::ContractForm,
};

#[derive(Debug, Clone)]
pub struct BatchJob {
    pub id: String,
    pub template_id: i64,
    pub year: i32,
    pub rows: Vec<ContractForm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRowOutcome {
    pub customer_id: i64,
    pub contract_number: String,
    pub generated: Option<GeneratedRow>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchResult {
    pub job_id: String,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub rows: Vec<BatchRowOutcome>,
    pub zip_path: Option<std::path::PathBuf>,
}

/// Execute batch rows sequentially. Cancellation is observed after the current row.
pub fn run_batch(
    ctx: &mut Ctx,
    job: BatchJob,
    mut progress: impl FnMut(usize, usize),
    cancel: &AtomicBool,
) -> Result<BatchResult, ServiceError> {
    let template = ctx
        .store()
        .templates
        .get(&job.template_id)
        .cloned()
        .ok_or_else(|| ServiceError::Invalid("Mẫu hợp đồng không tồn tại.".into()))?;
    let template_path = template
        .file_path
        .as_deref()
        .map(|path| ctx.paths().resolve_stored(path))
        .filter(|path| path.is_file())
        .ok_or_else(|| ServiceError::FileMissing("File template không tồn tại trên đĩa.".into()))?;
    let compiled = ctx.templates_mut().get_or_compile(&template_path)?;
    let legacy_party_a_rows = compiled.analysis().optional_keys.is_empty();
    let total = job.rows.len();
    let mut outcomes = Vec::with_capacity(total);
    let mut files = Vec::<(String, std::path::PathBuf)>::new();
    let mut succeeded = 0;
    let mut failed = 0;
    let mut cancelled = false;

    for (index, form) in job.rows.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        let customer_id = form.customer_id;
        let contract_number = form.contract_number.clone();
        match batch_row(
            ctx,
            &job.id,
            form,
            job.year,
            compiled.as_ref(),
            legacy_party_a_rows,
        ) {
            Ok(generated) => {
                files.push((generated.file_name.clone(), generated.file_path.clone()));
                succeeded += 1;
                outcomes.push(BatchRowOutcome {
                    customer_id,
                    contract_number: generated.contract_number.clone(),
                    generated: Some(generated),
                    error: None,
                });
            }
            Err(error) => {
                failed += 1;
                outcomes.push(BatchRowOutcome {
                    customer_id,
                    contract_number,
                    generated: None,
                    error: Some(error.to_string()),
                });
            }
        }
        progress(index + 1, total);
        if cancel.load(Ordering::Relaxed) {
            cancelled = index + 1 < total;
            break;
        }
    }

    let zip_path = if files.is_empty() {
        None
    } else {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut cursor);
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            for (name, path) in files {
                let Ok(bytes) = fs::read(path) else {
                    continue;
                };
                zip.start_file(name, options)
                    .map_err(|error| ServiceError::Invalid(error.to_string()))?;
                std::io::Write::write_all(&mut zip, &bytes)
                    .map_err(|error| ServiceError::Invalid(error.to_string()))?;
            }
            zip.finish()
                .map_err(|error| ServiceError::Invalid(error.to_string()))?;
        }
        fs::create_dir_all(&ctx.paths().output_batch)?;
        let now = ctx.clock().now_local();
        let zip_name = format!("batch_{}_{}.zip", job.id, now.format("%Y%m%d_%H%M%S"));
        Some(
            write_new_file(&ctx.paths().output_batch, &zip_name, cursor.get_ref())
                .map_err(|error| {
                    ServiceError::Invalid(format!("Không ghi được file ZIP: {error}"))
                })?
                .0,
        )
    };

    Ok(BatchResult {
        job_id: job.id,
        total,
        succeeded,
        failed,
        cancelled,
        rows: outcomes,
        zip_path,
    })
}
