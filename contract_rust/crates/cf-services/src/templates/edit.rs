use std::collections::BTreeMap;

use cf_db::{
    change::{Change, ChangeValue},
    codec,
    store::Store,
};
use rusqlite::types::Value;

use crate::{ServiceError, ctx::Ctx};

/// Đọc và phân tích file mẫu mà không thay đổi dữ liệu đã lưu.
pub fn inspect_analysis(ctx: &Ctx, id: i64) -> Result<cf_docx::Analysis, ServiceError> {
    let template = ctx
        .store()
        .templates
        .get(&id)
        .ok_or_else(|| ServiceError::NotFound(format!("Không tìm thấy template id={id}")))?;
    let raw_path = template
        .file_path
        .as_deref()
        .ok_or_else(|| ServiceError::FileMissing("File không tồn tại: None".into()))?;
    let path = cf_db::paths::resolve_stored(&ctx.paths().root, raw_path);
    let bytes = std::fs::read(&path).map_err(|_| {
        ServiceError::FileMissing(format!("File không tồn tại: '{}'.", path.display()))
    })?;
    let compiled = cf_docx::Template::compile(&bytes)
        .map_err(|error| ServiceError::Invalid(format!("Phân tích thất bại: {error}")))?;
    Ok(compiled.analysis())
}

pub fn update_metadata(
    ctx: &mut Ctx,
    id: i64,
    name: &str,
    contract_type: &str,
    description: &str,
    product_group: &str,
) -> Result<(), ServiceError> {
    if !ctx.store().templates.contains_key(&id) {
        return Err(ServiceError::NotFound(format!(
            "Không tìm thấy template id={id}"
        )));
    }
    let name = name.trim();
    if name.is_empty() {
        return Err(ServiceError::Invalid(
            "Tên template không được để trống.".into(),
        ));
    }
    let group = (!product_group.trim().is_empty()).then(|| product_group.trim());
    let now = ctx.clock().now_utc();
    ctx.store_mut().apply(vec![Change::update(
        "contract_templates",
        id,
        BTreeMap::from([
            ("name".into(), ChangeValue::Sql(Value::Text(name.into()))),
            (
                "contract_type".into(),
                ChangeValue::Sql(Value::Text(contract_type.into())),
            ),
            (
                "product_group".into(),
                ChangeValue::Sql(codec::opt_text(group)),
            ),
            (
                "description".into(),
                ChangeValue::Sql(codec::opt_text(Some(description))),
            ),
            (
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            ),
        ]),
    )])?;
    Ok(())
}

pub fn update_mapping(
    store: &mut Store,
    id: i64,
    mapping: &BTreeMap<String, String>,
    now: chrono::NaiveDateTime,
) -> Result<(), ServiceError> {
    let template = store
        .templates
        .get(&id)
        .ok_or_else(|| ServiceError::NotFound(format!("Không tìm thấy template id={id}")))?;
    let cleaned = template
        .detected_placeholders
        .iter()
        .map(|placeholder| {
            (
                placeholder.clone(),
                mapping
                    .get(placeholder)
                    .map_or(String::new(), |value| value.trim().to_owned()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let json = serde_json::to_string(&cleaned)
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    store.apply(vec![Change::update(
        "contract_templates",
        id,
        BTreeMap::from([
            ("field_mapping".into(), ChangeValue::Sql(Value::Text(json))),
            (
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            ),
        ]),
    )])?;
    Ok(())
}

pub fn reanalyze(ctx: &mut Ctx, id: i64) -> Result<cf_docx::Analysis, ServiceError> {
    let template = ctx
        .store()
        .templates
        .get(&id)
        .cloned()
        .ok_or_else(|| ServiceError::NotFound(format!("Không tìm thấy template id={id}")))?;
    let path = template
        .file_path
        .as_deref()
        .map(|raw| cf_db::paths::resolve_stored(&ctx.paths().root, raw))
        .ok_or_else(|| ServiceError::FileMissing("File không tồn tại: None".into()))?;
    let bytes = std::fs::read(&path).map_err(|_| {
        ServiceError::FileMissing(format!(
            "File không tồn tại: '{:?}'",
            path.to_string_lossy()
        ))
    })?;
    let compiled = cf_docx::Template::compile(&bytes)
        .map_err(|error| ServiceError::Invalid(format!("Phân tích thất bại: {error}")))?;
    let analysis = compiled.analysis();
    let mapping = analysis
        .placeholders
        .iter()
        .map(|placeholder| {
            let value = template
                .field_mapping
                .get(placeholder)
                .cloned()
                .or_else(|| {
                    super::mapping::default_mapping(std::slice::from_ref(placeholder))
                        .remove(placeholder)
                })
                .unwrap_or_default();
            (placeholder.clone(), value)
        })
        .collect::<BTreeMap<_, _>>();
    let mapping = serde_json::to_string(&mapping)
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    let now = ctx.clock().now_utc();
    ctx.store_mut().apply(vec![Change::update(
        "contract_templates",
        id,
        BTreeMap::from([
            (
                "detected_placeholders".into(),
                ChangeValue::Sql(Value::Text(
                    serde_json::to_string(&analysis.placeholders)
                        .map_err(|error| ServiceError::Invalid(error.to_string()))?,
                )),
            ),
            (
                "field_mapping".into(),
                ChangeValue::Sql(Value::Text(mapping)),
            ),
            (
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            ),
        ]),
    )])?;
    Ok(analysis)
}

pub fn toggle_active(
    store: &mut Store,
    id: i64,
    now: chrono::NaiveDateTime,
) -> Result<bool, ServiceError> {
    let template = store
        .templates
        .get(&id)
        .ok_or_else(|| ServiceError::NotFound(format!("Không tìm thấy template id={id}")))?;
    let active = !template.is_active;
    store.apply(vec![Change::update(
        "contract_templates",
        id,
        BTreeMap::from([
            (
                "is_active".into(),
                ChangeValue::Sql(Value::Integer(i64::from(active))),
            ),
            (
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            ),
        ]),
    )])?;
    Ok(active)
}
