use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use cf_db::{
    change::{Change, ChangeValue},
    codec,
    model::CustomerDocument,
    store::Store,
};
use rusqlite::types::Value;

use crate::{ServiceError, ctx::Ctx};

const MAX_BYTES: usize = 5 * 1024 * 1024;
const FIXED_TYPES: &[&str] = &["cccd_front", "cccd_back", "establishment"];
const TYPES: &[&str] = &["cccd_front", "cccd_back", "establishment", "other"];

fn validate(bytes: &[u8]) -> Result<&'static str, ServiceError> {
    if bytes.starts_with(b"%PDF") {
        Ok("pdf")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) || bytes.starts_with(b"\x89PNG") {
        Ok("image")
    } else {
        Err(ServiceError::Invalid(
            "File không hợp lệ — chỉ chấp nhận PDF, JPG hoặc PNG".into(),
        ))
    }
}

fn safe_ext(name: &str, mime: &str) -> &'static str {
    match name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
    {
        Some(ext) if ext == "pdf" => ".pdf",
        Some(ext) if ext == "jpg" => ".jpg",
        Some(ext) if ext == "jpeg" => ".jpeg",
        Some(ext) if ext == "png" => ".png",
        _ if mime == "pdf" => ".pdf",
        _ => ".jpg",
    }
}

fn document_values(
    customer_id: i64,
    doc_type: &str,
    label: &str,
    path: &Path,
    original_name: &str,
    mime: &str,
    now: chrono::NaiveDateTime,
) -> BTreeMap<String, ChangeValue> {
    BTreeMap::from([
        (
            "customer_id".into(),
            ChangeValue::Sql(Value::Integer(customer_id)),
        ),
        (
            "doc_type".into(),
            ChangeValue::Sql(Value::Text(doc_type.to_owned())),
        ),
        (
            "label".into(),
            ChangeValue::Sql(codec::opt_text(Some(label.trim()))),
        ),
        (
            "file_path".into(),
            ChangeValue::Sql(Value::Text(path.to_string_lossy().into_owned())),
        ),
        (
            "file_name".into(),
            ChangeValue::Sql(Value::Text(original_name.to_owned())),
        ),
        (
            "mime_hint".into(),
            ChangeValue::Sql(Value::Text(mime.to_owned())),
        ),
        (
            "uploaded_at".into(),
            ChangeValue::Sql(codec::write_datetime(Some(now))),
        ),
    ])
}

pub fn upload_doc(
    ctx: &mut Ctx,
    customer_id: i64,
    doc_type: &str,
    label: &str,
    bytes: &[u8],
    original_name: &str,
) -> Result<i64, ServiceError> {
    if !TYPES.contains(&doc_type) {
        return Err(ServiceError::Invalid("Loại giấy tờ không hợp lệ".into()));
    }
    if doc_type == "other" && label.trim().is_empty() {
        return Err(ServiceError::Invalid("Vui lòng nhập tên giấy tờ".into()));
    }
    if bytes.len() > MAX_BYTES {
        return Err(ServiceError::Invalid(format!(
            "File quá lớn — tối đa 5 MB (hiện tại: {} KB)",
            bytes.len() / 1024
        )));
    }
    let mime = validate(bytes)?;
    let now = ctx.clock().now_utc();
    let directory = ctx.paths().customer_docs.join(customer_id.to_string());
    fs::create_dir_all(&directory)?;
    let ext = safe_ext(original_name, mime);
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let file_name = format!("{customer_id}_{doc_type}_{}{}", &nonce[..8], ext);
    let path = directory.join(file_name);
    fs::write(&path, bytes)?;

    let result = {
        let store = ctx.store_mut();
        if !store.customers.contains_key(&customer_id) {
            let _ = fs::remove_file(&path);
            return Err(ServiceError::NotFound(format!(
                "Không tìm thấy khách hàng id={customer_id}"
            )));
        }
        let old = if FIXED_TYPES.contains(&doc_type) {
            store
                .documents
                .values()
                .find(|doc| doc.customer_id == customer_id && doc.doc_type == doc_type)
                .cloned()
        } else {
            None
        };
        let change = if let Some(old) = &old {
            let mut values = document_values(
                customer_id,
                doc_type,
                label,
                &path,
                original_name,
                mime,
                now,
            );
            values.remove("customer_id");
            values.remove("doc_type");
            Change::update("customer_documents", old.id, values)
        } else {
            Change::insert(
                "customer_documents",
                document_values(
                    customer_id,
                    doc_type,
                    label,
                    &path,
                    original_name,
                    mime,
                    now,
                ),
            )
        };
        store.apply(vec![change]).map(|result| (old, result))
    };
    let (old, result) = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_file(&path);
            return Err(error.into());
        }
    };
    if let Some(old) = old {
        let old_path = cf_db::paths::resolve_stored(&ctx.paths().root, &old.file_path);
        if old_path != path {
            let _ = fs::remove_file(old_path);
        }
        Ok(old.id)
    } else {
        result
            .inserted_ids
            .first()
            .copied()
            .ok_or_else(|| ServiceError::Invalid("Không lưu được giấy tờ.".into()))
    }
}

pub fn delete_doc(ctx: &mut Ctx, customer_id: i64, doc_id: i64) -> Result<String, ServiceError> {
    let doc = ctx
        .store()
        .documents
        .get(&doc_id)
        .filter(|doc| doc.customer_id == customer_id)
        .cloned()
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy giấy tờ".into()))?;
    let label = doc.label.clone().unwrap_or_else(|| doc.doc_type.clone());
    ctx.store_mut()
        .apply(vec![Change::delete("customer_documents", doc_id)])?;
    let path = cf_db::paths::resolve_stored(&ctx.paths().root, &doc.file_path);
    let _ = fs::remove_file(path);
    Ok(label)
}

pub fn doc_path(
    store: &Store,
    root: &Path,
    customer_id: i64,
    doc_id: i64,
) -> Result<PathBuf, ServiceError> {
    let doc: &CustomerDocument = store
        .documents
        .get(&doc_id)
        .filter(|doc| doc.customer_id == customer_id)
        .ok_or_else(|| ServiceError::NotFound("Không tìm thấy giấy tờ".into()))?;
    Ok(cf_db::paths::resolve_stored(root, &doc.file_path))
}
