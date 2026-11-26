use std::{collections::BTreeMap, fs, path::PathBuf};

use cf_core::{
    lifecycle::{ContractStatus, Evidence},
    names::safe_number_part,
};
use cf_db::{
    change::{Change, ChangeValue},
    codec,
    model::Contract,
};
use rusqlite::types::Value;

use crate::{
    Ctx, ServiceError,
    events::{self, ChangeRef},
};

const MAX_EVIDENCE_BYTES: usize = 5 * 1024 * 1024;
const PDF_MAGIC: &[u8] = b"%PDF";
const JPG_MAGIC: &[u8] = &[0xff, 0xd8, 0xff];
const PNG_MAGIC: &[u8] = b"\x89PNG";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    Scan,
    Invoice,
    PaymentSlip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceUploadResult {
    pub file_path: PathBuf,
    pub file_name: String,
    pub message: String,
}

impl EvidenceKind {
    fn column_names(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Scan => (
                "signed_pdf_path",
                "signed_pdf_name",
                "signed_pdf_uploaded_at",
            ),
            Self::Invoice => (
                "invoice_pdf_path",
                "invoice_pdf_name",
                "invoice_pdf_uploaded_at",
            ),
            Self::PaymentSlip => (
                "payment_slip_path",
                "payment_slip_name",
                "payment_slip_uploaded_at",
            ),
        }
    }

    fn event_name(self) -> &'static str {
        match self {
            Self::Scan => "scan_uploaded",
            Self::Invoice => "invoice_uploaded",
            Self::PaymentSlip => "payment_slip_uploaded",
        }
    }

    fn directory(self, ctx: &Ctx) -> &std::path::Path {
        match self {
            Self::Scan => &ctx.paths().signed_scans,
            Self::Invoice => &ctx.paths().invoice_docs,
            Self::PaymentSlip => &ctx.paths().payment_slips,
        }
    }
}

/// Port của app/routers/contracts.py::upload_scan, upload_invoice, upload_payment_slip.
pub fn upload_evidence(
    ctx: &mut Ctx,
    contract_id: i64,
    kind: EvidenceKind,
    bytes: &[u8],
    original_name: Option<&str>,
    date: Option<&str>,
) -> Result<EvidenceUploadResult, ServiceError> {
    let extension = validate_file(kind, bytes)?;
    let contract = ctx
        .store()
        .contracts
        .get(&contract_id)
        .cloned()
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy hợp đồng".into()))?;
    let raw_number = if contract.contract_number.is_empty() {
        contract_id.to_string()
    } else {
        contract.contract_number.clone()
    };
    let safe_number = safe_number_part(&raw_number);
    let prefix = match kind {
        EvidenceKind::Scan => "scan",
        EvidenceKind::Invoice => "invoice",
        EvidenceKind::PaymentSlip => "payment",
    };
    let generated_name = format!("{prefix}_{contract_id}_{safe_number}{extension}");
    let directory = kind.directory(ctx);
    fs::create_dir_all(directory)?;
    let (file_path, _) = cf_docx::write::write_new_file(directory, &generated_name, bytes)
        .map_err(|error| ServiceError::Invalid(format!("Không thể lưu file lên đĩa: {error}")))?;
    let file_name = original_name
        .filter(|name| !name.is_empty())
        .unwrap_or(&generated_name)
        .to_owned();
    let now = ctx.clock().now_utc();
    let (path_column, name_column, uploaded_column) = kind.column_names();
    let mut fields = BTreeMap::from([
        (
            path_column.into(),
            ChangeValue::Sql(Value::Text(file_path.to_string_lossy().into_owned())),
        ),
        (
            name_column.into(),
            ChangeValue::Sql(Value::Text(file_name.clone())),
        ),
        (
            uploaded_column.into(),
            ChangeValue::Sql(codec::write_datetime(Some(now))),
        ),
        (
            "updated_at".into(),
            ChangeValue::Sql(codec::write_datetime(Some(now))),
        ),
    ]);
    if let Some(date) =
        date.and_then(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
    {
        let date_column = match kind {
            EvidenceKind::Invoice => Some("invoice_date"),
            EvidenceKind::PaymentSlip => Some("payment_date"),
            EvidenceKind::Scan => None,
        };
        if let Some(column) = date_column {
            fields.insert(
                column.into(),
                ChangeValue::Sql(codec::write_date(Some(date))),
            );
        }
    }

    let mut changes = vec![Change::update("contracts", contract_id, fields)];
    let auto_transition = match kind {
        EvidenceKind::Scan
            if matches!(
                contract.status,
                ContractStatus::Generated | ContractStatus::Sent
            ) =>
        {
            Some(("Signed", "Đã ký"))
        }
        EvidenceKind::Invoice
            if matches!(
                contract.status,
                ContractStatus::Signed | ContractStatus::Active
            ) =>
        {
            Some(("Invoiced", "Đã xuất hóa đơn"))
        }
        EvidenceKind::PaymentSlip if contract.status == ContractStatus::Invoiced => {
            Some(("PaidActive", "Đã thanh toán"))
        }
        _ => None,
    };
    let status_note = if let Some((target, label)) = auto_transition {
        let evidence = Evidence {
            has_invoice: kind == EvidenceKind::Invoice || contract.invoice_pdf_path.is_some(),
            has_payment_slip: kind == EvidenceKind::PaymentSlip
                || contract.payment_slip_path.is_some(),
        };
        let status_note = match kind {
            EvidenceKind::Scan => "tự động sau khi upload scan PDF",
            EvidenceKind::Invoice => "tự động sau khi upload hóa đơn",
            EvidenceKind::PaymentSlip => "tự động sau khi upload ủy nhiệm chi",
        };
        match super::status::change_status_changes_with_evidence(
            ctx.store(),
            contract_id,
            target,
            "system",
            status_note,
            now,
            evidence,
        ) {
            Ok(status_changes) => changes.extend(status_changes),
            Err(error) => {
                let _ = fs::remove_file(&file_path);
                return Err(error);
            }
        }
        format!(" — trạng thái tự động cập nhật → {label}")
    } else {
        String::new()
    };
    let description = match kind {
        EvidenceKind::Scan => format!("Upload scan PDF: {file_name}"),
        EvidenceKind::Invoice => format!("Upload hóa đơn PDF: {file_name}"),
        EvidenceKind::PaymentSlip => format!("Upload ủy nhiệm chi: {file_name}"),
    };
    changes.push(events::event(
        ChangeRef::Existing(contract_id),
        kind.event_name(),
        &description,
        "user",
        now,
    ));
    if let Err(error) = ctx.store_mut().apply(changes) {
        let _ = fs::remove_file(&file_path);
        return Err(error.into());
    }
    if let Some(old_path) = old_evidence_path(&contract, kind) {
        let old_path = ctx.paths().resolve_stored(old_path);
        if old_path != file_path {
            let _ = fs::remove_file(old_path);
        }
    }
    let message = match kind {
        EvidenceKind::Scan => format!("Đã upload scan PDF thành công{status_note}"),
        EvidenceKind::Invoice => format!("Đã upload hóa đơn thành công{status_note}"),
        EvidenceKind::PaymentSlip => format!("Đã upload ủy nhiệm chi thành công{status_note}"),
    };
    Ok(EvidenceUploadResult {
        file_path,
        file_name,
        message,
    })
}

/// Port của app/routers/contracts.py::delete_scan, delete_invoice, delete_payment_slip.
pub fn delete_evidence(
    ctx: &mut Ctx,
    contract_id: i64,
    kind: EvidenceKind,
) -> Result<bool, ServiceError> {
    let contract = ctx
        .store()
        .contracts
        .get(&contract_id)
        .cloned()
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy hợp đồng".into()))?;
    let (path_column, name_column, uploaded_column) = kind.column_names();
    let old_path = old_evidence_path(&contract, kind);
    let Some(old_path) = old_path else {
        return Ok(false);
    };
    let old_name = old_evidence_name(&contract, kind).unwrap_or_default();
    let resolved_path = ctx.paths().resolve_stored(old_path);
    if resolved_path.exists() {
        fs::remove_file(&resolved_path)
            .map_err(|error| ServiceError::Invalid(format!("Không thể xóa file: {error}")))?;
    }
    let now = ctx.clock().now_utc();
    let fields = BTreeMap::from([
        (path_column.into(), ChangeValue::Sql(Value::Null)),
        (name_column.into(), ChangeValue::Sql(Value::Null)),
        (uploaded_column.into(), ChangeValue::Sql(Value::Null)),
        (
            "updated_at".into(),
            ChangeValue::Sql(codec::write_datetime(Some(now))),
        ),
    ]);
    let description = match kind {
        EvidenceKind::Scan => format!("Đã xóa scan PDF: {old_name}"),
        EvidenceKind::Invoice => format!("Đã xóa hóa đơn PDF: {old_name}"),
        EvidenceKind::PaymentSlip => format!("Đã xóa ủy nhiệm chi: {old_name}"),
    };
    ctx.store_mut().apply(vec![
        Change::update("contracts", contract_id, fields),
        events::event(
            ChangeRef::Existing(contract_id),
            deleted_event_name(kind),
            &description,
            "user",
            now,
        ),
    ])?;
    Ok(true)
}

fn validate_file(kind: EvidenceKind, bytes: &[u8]) -> Result<&'static str, ServiceError> {
    let valid = match kind {
        EvidenceKind::Scan => bytes.starts_with(PDF_MAGIC),
        EvidenceKind::Invoice => bytes.starts_with(PDF_MAGIC),
        EvidenceKind::PaymentSlip => {
            bytes.starts_with(PDF_MAGIC)
                || bytes.starts_with(JPG_MAGIC)
                || bytes.starts_with(PNG_MAGIC)
        }
    };
    if !valid {
        let message = match kind {
            EvidenceKind::Scan => "File không hợp lệ — chỉ chấp nhận PDF (.pdf)",
            EvidenceKind::Invoice => "File không hợp lệ — hóa đơn chỉ chấp nhận PDF (.pdf)",
            EvidenceKind::PaymentSlip => "File không hợp lệ — chỉ chấp nhận PDF, JPG hoặc PNG",
        };
        return Err(ServiceError::Invalid(message.into()));
    }
    if bytes.len() > MAX_EVIDENCE_BYTES {
        return Err(ServiceError::Invalid(format!(
            "File quá lớn — tối đa 5 MB (file hiện tại: {} KB)",
            bytes.len() / 1024
        )));
    }
    Ok(if bytes.starts_with(PDF_MAGIC) {
        ".pdf"
    } else if bytes.starts_with(JPG_MAGIC) {
        ".jpg"
    } else {
        ".png"
    })
}

fn old_evidence_path(contract: &Contract, kind: EvidenceKind) -> Option<&str> {
    match kind {
        EvidenceKind::Scan => contract.signed_pdf_path.as_deref(),
        EvidenceKind::Invoice => contract.invoice_pdf_path.as_deref(),
        EvidenceKind::PaymentSlip => contract.payment_slip_path.as_deref(),
    }
}

fn old_evidence_name(contract: &Contract, kind: EvidenceKind) -> Option<&str> {
    match kind {
        EvidenceKind::Scan => contract.signed_pdf_name.as_deref(),
        EvidenceKind::Invoice => contract.invoice_pdf_name.as_deref(),
        EvidenceKind::PaymentSlip => contract.payment_slip_name.as_deref(),
    }
}

fn deleted_event_name(kind: EvidenceKind) -> &'static str {
    match kind {
        EvidenceKind::Scan => "scan_deleted",
        EvidenceKind::Invoice => "invoice_deleted",
        EvidenceKind::PaymentSlip => "payment_slip_deleted",
    }
}
