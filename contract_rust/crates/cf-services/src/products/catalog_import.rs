use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
    str::FromStr,
};

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use cf_db::{
    change::{Change, ChangeValue},
    model::Product,
    store::Store,
};
use rust_decimal::Decimal;
use serde_json::{Map, Value as Json};

use super::clean::{self, ProductDraft};
use crate::{Ctx, ServiceError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogAction {
    Create,
    Update,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogRow {
    pub row: usize,
    pub action: CatalogAction,
    pub code: String,
    pub name: String,
    pub data: Option<Json>,
    pub error: String,
}

/// Port của app/services/product_service.py::parse_catalog_xlsx.
pub fn parse_catalog_xlsx(
    store: &Store,
    file_bytes: &[u8],
) -> Result<Vec<CatalogRow>, ServiceError> {
    let mut workbook: Xlsx<_> = open_workbook_from_rs(Cursor::new(file_bytes))
        .map_err(|_| ServiceError::Invalid("Không đọc được file Excel (chỉ nhận .xlsx).".into()))?;
    let range = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| ServiceError::Invalid("File trống.".into()))?
        .map_err(|_| ServiceError::Invalid("Không đọc được file Excel (chỉ nhận .xlsx).".into()))?;
    let all_rows = range.rows().collect::<Vec<_>>();
    if all_rows.is_empty() {
        return Err(ServiceError::Invalid("File trống.".into()));
    }
    let header = all_rows[0].iter().map(cell_text).collect::<Vec<_>>();
    let column_at = header
        .iter()
        .enumerate()
        .filter(|(_, name)| !name.is_empty())
        .map(|(index, name)| (name.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let missing = super::catalog_export::IMPORT_COLUMNS
        .iter()
        .filter(|column| !column_at.contains_key(**column))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ServiceError::Invalid(format!(
            "File thiếu cột: {}.",
            missing.join(", ")
        )));
    }

    let mut seen_codes = BTreeSet::new();
    let mut results = Vec::new();
    for (offset, cells) in all_rows.iter().skip(1).enumerate() {
        if cells.iter().all(|cell| cell_text(cell).is_empty()) {
            continue;
        }
        let row_number = offset + 2;
        let empty = Data::Empty;
        let cell = |name: &str| {
            column_at
                .get(name)
                .and_then(|index| cells.get(*index))
                .unwrap_or(&empty)
        };
        let mut raw = Map::new();
        for column in super::catalog_export::IMPORT_COLUMNS
            .iter()
            .filter(|name| **name != "default_price")
        {
            raw.insert((*column).into(), Json::String(cell_text(cell(column))));
        }
        let price_cell = cell("default_price");
        let (price, mut error) = match cell_price(price_cell) {
            Ok(value) => (value, String::new()),
            Err(message) => (None, message),
        };
        let code = raw
            .get("code")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_uppercase();
        if !code.is_empty() && !seen_codes.insert(code.clone()) && error.is_empty() {
            error = format!("Mã {code} xuất hiện nhiều lần trong file.");
        }
        let name = raw
            .get("name")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned();
        if !error.is_empty() {
            results.push(CatalogRow {
                row: row_number,
                action: CatalogAction::Error,
                code,
                name,
                data: None,
                error,
            });
            continue;
        }
        if let Some(price) = price {
            raw.insert("default_price".into(), Json::String(price.to_string()));
        } else {
            raw.insert("default_price".into(), Json::String(String::new()));
        }
        if let Some(Json::String(code_value)) = raw.get_mut("code") {
            *code_value = code.clone();
        }
        let raw_json = Json::Object(raw);
        let existing = store.products.values().find(|product| product.code == code);
        match clean::clean_form(store, &raw_json, existing.map(|product| product.id)) {
            Ok(mut draft) => {
                // Excel prices are machine values; do not pass their decimal point through
                // the human form's thousands-separator parser.
                draft.default_price = price;
                let data = draft_json(&draft);
                results.push(CatalogRow {
                    row: row_number,
                    action: if existing.is_some() {
                        CatalogAction::Update
                    } else {
                        CatalogAction::Create
                    },
                    code,
                    name,
                    data: Some(data),
                    error: String::new(),
                });
            }
            Err(error) => results.push(CatalogRow {
                row: row_number,
                action: CatalogAction::Error,
                code,
                name,
                data: None,
                error: error.user_message().replace('\n', "; "),
            }),
        }
    }
    Ok(results)
}

/// Port của app/services/product_service.py::apply_catalog_rows.
pub fn apply_catalog_rows(
    ctx: &mut Ctx,
    rows: &[CatalogRow],
) -> Result<(usize, usize), ServiceError> {
    let errors = rows
        .iter()
        .filter(|row| row.action == CatalogAction::Error)
        .map(|row| format!("Dòng {}: {}", row.row, row.error))
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        return Err(ServiceError::Invalid(format!(
            "Chưa ghi file nào. {}",
            errors.join("; ")
        )));
    }

    let now = ctx.clock().now_utc();
    let mut plans = Vec::with_capacity(rows.len());
    let mut validation_errors = Vec::new();
    for row in rows {
        let Some(data) = &row.data else {
            validation_errors.push(format!("Dòng {}: dữ liệu xem trước không hợp lệ.", row.row));
            continue;
        };
        let code = data
            .get("code")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .trim()
            .to_uppercase();
        let existing = ctx
            .store()
            .products
            .values()
            .find(|product| product.code == code)
            .cloned();
        match clean::clean_form(
            ctx.store(),
            data,
            existing.as_ref().map(|product| product.id),
        ) {
            Ok(mut draft) => {
                draft.default_price = data
                    .get("default_price")
                    .and_then(Json::as_str)
                    .filter(|value| !value.is_empty())
                    .and_then(|value| Decimal::from_str(value).ok());
                plans.push((existing, draft));
            }
            Err(error) => validation_errors.push(format!(
                "Dòng {}: {}",
                row.row,
                error.user_message().replace('\n', "; ")
            )),
        }
    }
    if !validation_errors.is_empty() {
        return Err(ServiceError::Invalid(format!(
            "Chưa ghi file nào. {}",
            validation_errors.join("; ")
        )));
    }

    let mut changes = Vec::with_capacity(plans.len());
    let mut created = 0;
    let mut updated = 0;
    for (existing, draft) in plans {
        let product = product_from_draft(&draft, existing.as_ref(), now);
        let mut fields = product
            .to_values()
            .into_iter()
            .map(|(key, value)| (key, ChangeValue::Sql(value)))
            .collect::<BTreeMap<_, _>>();
        fields.remove("id");
        if let Some(existing) = existing {
            updated += 1;
            for column in [
                "default_template_id",
                "replaced_by_id",
                "feature_list",
                "created_at",
            ] {
                fields.remove(column);
            }
            changes.push(Change::update("products", existing.id, fields));
        } else {
            created += 1;
            changes.push(Change::insert("products", fields));
        }
    }
    ctx.store_mut().apply(changes)?;
    Ok((created, updated))
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::Bool(value) => if *value { "1" } else { "0" }.into(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) if value.fract() == 0.0 => format!("{value:.0}"),
        Data::Float(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) | Data::DurationIso(value) | Data::String(value) => {
            value.trim().to_owned()
        }
        _ => cell.to_string(),
    }
}

fn cell_price(cell: &Data) -> Result<Option<Decimal>, String> {
    match cell {
        Data::Empty => Ok(None),
        Data::String(value) if value.trim().is_empty() => Ok(None),
        Data::Bool(_) => Err("Đơn giá không được là giá trị đúng/sai.".into()),
        Data::Int(value) => Ok(Some(Decimal::from(*value))),
        Data::Float(value) => Decimal::from_str(&value.to_string())
            .map(Some)
            .map_err(|_| format!("Đơn giá không phải là số: {value:?}.")),
        Data::String(value) => Decimal::from_str(value.trim())
            .map(Some)
            .map_err(|_| format!("Đơn giá không phải là số: {value:?}.")),
        value => Decimal::from_str(&cell_text(value))
            .map(Some)
            .map_err(|_| format!("Đơn giá không phải là số: {:?}.", cell_text(value))),
    }
}

fn draft_json(draft: &ProductDraft) -> Json {
    let mut data = Map::new();
    for (key, value) in [
        ("code", draft.code.clone()),
        ("name", draft.name.clone()),
        ("product_type", draft.product_type.clone()),
        ("kind", draft.kind.clone()),
        ("quantity_source", draft.quantity_source.clone()),
    ] {
        data.insert(key.into(), Json::String(value));
    }
    for (key, value) in [
        ("group_code", draft.group_code.clone()),
        ("unit_label", draft.unit_label.clone()),
        (
            "contract_number_format",
            draft.contract_number_format.clone(),
        ),
    ] {
        data.insert(key.into(), value.map(Json::String).unwrap_or(Json::Null));
    }
    data.insert(
        "default_price".into(),
        draft
            .default_price
            .map(|value| Json::String(value.to_string()))
            .unwrap_or(Json::Null),
    );
    data.insert(
        "default_vat_rate".into(),
        draft
            .default_vat_rate
            .map(|value| Json::String(value.to_string()))
            .unwrap_or(Json::Null),
    );
    for (key, value) in [
        ("price_period_months", Some(draft.price_period_months)),
        ("min_quantity", draft.min_quantity),
        ("quantity_step", draft.quantity_step),
        ("account_limit", draft.account_limit),
        ("default_duration_months", draft.default_duration_months),
        ("sort_order", Some(draft.sort_order)),
    ] {
        data.insert(
            key.into(),
            value
                .map(|number| Json::Number(number.into()))
                .unwrap_or(Json::Null),
        );
    }
    data.insert(
        "is_active".into(),
        draft.is_active.map(Json::Bool).unwrap_or(Json::Null),
    );
    Json::Object(data)
}

fn product_from_draft(
    draft: &ProductDraft,
    old: Option<&Product>,
    now: chrono::NaiveDateTime,
) -> Product {
    Product {
        id: old.map_or(0, |product| product.id),
        code: draft.code.clone(),
        name: draft.name.clone(),
        product_type: draft.product_type.clone(),
        default_price: draft.default_price,
        default_vat_rate: draft.default_vat_rate,
        default_duration_months: draft.default_duration_months,
        default_template_id: old.and_then(|product| product.default_template_id),
        contract_number_format: draft.contract_number_format.clone(),
        is_active: draft
            .is_active
            .unwrap_or_else(|| old.is_none_or(|product| product.is_active)),
        group_code: draft.group_code.clone(),
        kind: draft.kind.clone(),
        unit_label: draft.unit_label.clone(),
        price_period_months: draft.price_period_months,
        quantity_source: draft.quantity_source.clone(),
        min_quantity: draft.min_quantity,
        quantity_step: draft.quantity_step,
        account_limit: draft.account_limit,
        feature_list: old.map_or_else(Vec::new, |product| product.feature_list.clone()),
        replaced_by_id: old.and_then(|product| product.replaced_by_id),
        sort_order: draft.sort_order,
        created_at: old.map_or(now, |product| product.created_at),
        updated_at: now,
    }
}
