use std::{collections::BTreeMap, fs};

use cf_db::{
    change::{Change, ChangeValue},
    codec,
    store::Store,
};
use rusqlite::types::Value;

use crate::{ServiceError, ctx::Ctx};

const MAX_BYTES: usize = 10 * 1024 * 1024;

pub struct TemplateDraft<'a> {
    pub code: &'a str,
    pub name: &'a str,
    pub contract_type: &'a str,
    pub file_bytes: &'a [u8],
    pub original_filename: &'a str,
    pub description: &'a str,
    pub mode: &'a str,
    pub product_group: &'a str,
}

pub fn generate_next_code(store: &Store) -> String {
    let max = store
        .templates
        .values()
        .filter_map(|template| template.code.strip_prefix("TPL-")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    format!("TPL-{:03}", max + 1)
}

fn safe_basename(original: &str) -> String {
    let base = original.rsplit(['/', '\\']).next().unwrap_or_default();
    base.chars()
        .map(|ch| {
            if ch.is_alphanumeric() || "-_.".contains(ch) {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn values(
    code: &str,
    name: &str,
    contract_type: &str,
    product_group: Option<&str>,
    description: &str,
    mode: &str,
    now: chrono::NaiveDateTime,
) -> BTreeMap<String, ChangeValue> {
    BTreeMap::from([
        ("code".into(), ChangeValue::Sql(Value::Text(code.into()))),
        ("name".into(), ChangeValue::Sql(Value::Text(name.into()))),
        (
            "contract_type".into(),
            ChangeValue::Sql(Value::Text(contract_type.into())),
        ),
        (
            "product_group".into(),
            ChangeValue::Sql(codec::opt_text(product_group)),
        ),
        (
            "description".into(),
            ChangeValue::Sql(codec::opt_text(Some(description.trim()))),
        ),
        ("mode".into(), ChangeValue::Sql(Value::Text(mode.into()))),
        (
            "detected_placeholders".into(),
            ChangeValue::Sql(Value::Text("[]".into())),
        ),
        (
            "field_mapping".into(),
            ChangeValue::Sql(Value::Text("{}".into())),
        ),
        ("is_active".into(), ChangeValue::Sql(Value::Integer(1))),
        (
            "created_at".into(),
            ChangeValue::Sql(codec::write_datetime(Some(now))),
        ),
        (
            "updated_at".into(),
            ChangeValue::Sql(codec::write_datetime(Some(now))),
        ),
    ])
}

pub fn create_template(ctx: &mut Ctx, draft: TemplateDraft<'_>) -> Result<i64, ServiceError> {
    let TemplateDraft {
        code,
        name,
        contract_type,
        file_bytes,
        original_filename,
        description,
        mode,
        product_group,
    } = draft;
    let code = code.trim();
    let name = name.trim();
    if name.is_empty() {
        return Err(ServiceError::Invalid(
            "Tên template không được để trống.".into(),
        ));
    }
    if contract_type.trim().is_empty() {
        return Err(ServiceError::Invalid(
            "Loại hợp đồng không được để trống.".into(),
        ));
    }
    if file_bytes.is_empty() {
        return Err(ServiceError::Invalid(
            "File .docx không được để trống.".into(),
        ));
    }
    if file_bytes.len() > MAX_BYTES {
        return Err(ServiceError::Invalid(
            "File quá lớn (giới hạn 10 MB).".into(),
        ));
    }
    let ext = std::path::Path::new(original_filename)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default();
    if !ext.eq_ignore_ascii_case("docx") {
        return Err(ServiceError::Invalid(format!(
            "Chỉ chấp nhận file .docx (nhận được '.{ext}')."
        )));
    }
    let template = cf_docx::Template::compile(file_bytes)
        .map_err(|error| ServiceError::Invalid(format!("Không phân tích được file: {error}")))?;
    let analysis = template.analysis();
    let now = ctx.clock().now_utc();
    let id = {
        let store = ctx.store_mut();
        let code = if code.is_empty() {
            generate_next_code(store)
        } else {
            code.to_owned()
        };
        if store
            .templates
            .values()
            .any(|template| template.code == code)
        {
            return Err(ServiceError::Invalid(format!(
                "Mã template '{code}' đã tồn tại."
            )));
        }
        let group = (!product_group.trim().is_empty()).then(|| product_group.trim());
        let result = store.apply(vec![Change::insert(
            "contract_templates",
            values(&code, name, contract_type, group, description, mode, now),
        )])?;
        result
            .inserted_ids
            .first()
            .copied()
            .ok_or_else(|| ServiceError::Invalid("Không tạo được template.".into()))?
    };
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    let file_name = format!("{id}_{}_{}", &uuid[..8], safe_basename(original_filename));
    let path = ctx.paths().templates.join(&file_name);
    if let Err(error) = fs::create_dir_all(&ctx.paths().templates) {
        let _ = ctx
            .store_mut()
            .apply(vec![Change::delete("contract_templates", id)]);
        return Err(error.into());
    }
    if let Err(error) = fs::write(&path, file_bytes) {
        let _ = ctx
            .store_mut()
            .apply(vec![Change::delete("contract_templates", id)]);
        return Err(error.into());
    }
    let mapping = super::mapping::default_mapping(&analysis.placeholders);
    let update = Change::update(
        "contract_templates",
        id,
        BTreeMap::from([
            (
                "file_path".into(),
                ChangeValue::Sql(Value::Text(path.to_string_lossy().into_owned())),
            ),
            (
                "file_name".into(),
                ChangeValue::Sql(Value::Text(original_filename.to_owned())),
            ),
            (
                "detected_placeholders".into(),
                ChangeValue::Sql(Value::Text(
                    serde_json::to_string(&analysis.placeholders).unwrap_or_else(|_| "[]".into()),
                )),
            ),
            (
                "field_mapping".into(),
                ChangeValue::Sql(Value::Text(
                    serde_json::to_string(&mapping).unwrap_or_else(|_| "{}".into()),
                )),
            ),
            (
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(ctx.clock().now_utc()))),
            ),
        ]),
    );
    if let Err(error) = ctx.store_mut().apply(vec![update]) {
        let _ = fs::remove_file(&path);
        if let Err(cleanup_error) = ctx
            .store_mut()
            .apply(vec![Change::delete("contract_templates", id)])
        {
            return Err(ServiceError::Invalid(format!(
                "{error}; đồng thời không xóa được bản ghi tạm: {cleanup_error}"
            )));
        }
        return Err(error.into());
    }
    Ok(id)
}
