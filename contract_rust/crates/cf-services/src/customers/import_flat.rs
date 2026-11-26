use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
};

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use cf_db::{
    change::{Change, ChangeValue},
    codec,
};
use rusqlite::types::Value as SqlValue;
use serde_json::Value as JsonValue;

use crate::{Ctx, ServiceError, customers::edit};

const COLUMN_MAP: &[(&str, &str)] = &[
    ("mã khách hàng", "code"),
    ("tên pháp lý", "legal_name"),
    ("tên ngắn", "short_name"),
    ("loại đơn vị", "customer_type"),
    ("địa chỉ", "address"),
    ("xã / phường", "ward"),
    ("xã / phường / đặc khu", "ward"),
    ("tỉnh / thành phố", "province"),
    ("tỉnh / tp", "province"),
    ("huyện / quận", "district"),
    ("mã số thuế", "tax_code"),
    ("đại diện pháp lý", "representative_name"),
    ("chức vụ", "representative_title"),
    ("số tài khoản", "bank_account"),
    ("ngân hàng", "bank_name"),
    ("tên thụ hưởng", "beneficiary_name"),
    ("đầu mối liên hệ", "contact_name"),
    ("điện thoại", "phone"),
    ("email", "email"),
    ("ghi chú", "notes"),
    ("mã cơ sở kcb", "kcb_code"),
    ("mã kcb bhyt", "kcb_code"),
    ("số quyết định thành lập", "establishment_decision"),
    ("số qđ thành lập", "establishment_decision"),
    ("ngày thành lập", "establishment_date"),
    ("giới tính đại diện", "representative_gender"),
    ("giới tính", "representative_gender"),
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub total_rows: usize,
    pub errors: Vec<(usize, String)>,
}

/// Read flat customer Excel and apply all valid upserts in one SQLite transaction.
pub fn import_flat(ctx: &mut Ctx, bytes: &[u8]) -> Result<ImportResult, ServiceError> {
    let mut workbook: Xlsx<_> = match open_workbook_from_rs(Cursor::new(bytes)) {
        Ok(workbook) => workbook,
        Err(error) => {
            return Ok(ImportResult {
                errors: vec![(0, format!("Không đọc được file Excel: {error}"))],
                ..ImportResult::default()
            });
        }
    };
    let Some(range) = workbook.worksheet_range_at(0) else {
        return Ok(ImportResult {
            errors: vec![(
                0,
                "Không đọc được file Excel: workbook không có sheet.".into(),
            )],
            ..ImportResult::default()
        });
    };
    let range = match range {
        Ok(range) => range,
        Err(error) => {
            return Ok(ImportResult {
                errors: vec![(0, format!("Không đọc được file Excel: {error}"))],
                ..ImportResult::default()
            });
        }
    };
    let rows = range.rows().collect::<Vec<_>>();
    if rows.is_empty() {
        return Ok(ImportResult {
            errors: vec![(0, "File Excel trống.".into())],
            ..ImportResult::default()
        });
    }
    let header = rows.iter().take(5).enumerate().find_map(|(index, row)| {
        let columns = row
            .iter()
            .enumerate()
            .map(|(i, cell)| (cell_text(cell).trim().to_lowercase(), i))
            .filter(|(name, _)| !name.is_empty())
            .collect::<BTreeMap<_, _>>();
        let mapped = COLUMN_MAP
            .iter()
            .filter_map(|(label, field)| {
                columns
                    .get(*label)
                    .map(|column| ((*field).to_owned(), *column))
            })
            .collect::<BTreeMap<_, _>>();
        (!mapped.is_empty()).then_some((index, mapped))
    });
    let Some((header_index, columns)) = header else {
        return Ok(ImportResult { errors: vec![(0, "Không tìm thấy hàng tiêu đề hợp lệ trong 5 dòng đầu. Vui lòng dùng file mẫu Excel với đúng tên cột.".into())], ..ImportResult::default() });
    };
    if !columns.contains_key("legal_name") {
        return Ok(ImportResult {
            errors: vec![(
                0,
                "Thiếu cột bắt buộc 'Tên pháp lý' trong file Excel.".into(),
            )],
            ..ImportResult::default()
        });
    }

    let mut result = ImportResult::default();
    let mut changes = Vec::new();
    let mut created_codes = BTreeSet::new();
    let mut next_code = edit::generate_next_code(ctx.store())
        .trim_start_matches("KH-")
        .parse::<u64>()
        .unwrap_or(0);
    let now = ctx.clock().now_utc();
    for (offset, row) in rows.iter().skip(header_index + 1).enumerate() {
        if row.iter().all(|cell| cell_text(cell).trim().is_empty()) {
            continue;
        }
        result.total_rows += 1;
        let row_number = header_index + 2 + offset;
        let mut data = BTreeMap::<String, JsonValue>::new();
        for (field, column) in &columns {
            if let Some(cell) = row.get(*column) {
                let text = cell_text(cell).trim().to_owned();
                if !text.is_empty() {
                    data.insert(field.clone(), JsonValue::String(text));
                }
            }
        }
        if data
            .get("legal_name")
            .and_then(JsonValue::as_str)
            .is_none_or(str::is_empty)
        {
            result
                .errors
                .push((row_number, "Thiếu 'Tên pháp lý' — bỏ qua dòng này.".into()));
            result.skipped += 1;
            continue;
        }
        data.entry("province".into())
            .or_insert_with(|| JsonValue::String("Đồng Nai".into()));
        data.entry("customer_type".into())
            .or_insert_with(|| JsonValue::String("Trạm y tế".into()));
        data.remove("district");
        let supplied_code = data
            .get("code")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let code = if supplied_code.is_empty() {
            next_code += 1;
            format!("KH-{next_code:03}")
        } else {
            supplied_code
        };
        if !created_codes.insert(code.to_lowercase()) {
            result.errors.push((
                row_number,
                format!("Mã khách hàng '{code}' bị lặp trong file."),
            ));
            result.skipped += 1;
            continue;
        }
        data.insert("code".into(), JsonValue::String(code.clone()));
        let existing_id = ctx.store().indexes.customer_by_code.get(&code).copied();
        let mut values = match edit::clean(&data) {
            Ok(values) => values,
            Err(error) => {
                result.errors.push((row_number, error.to_string()));
                result.skipped += 1;
                continue;
            }
        };
        if let Some(id) = existing_id {
            values.insert(
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            );
            changes.push(Change::update("customers", id, values));
            result.updated += 1;
        } else {
            if data
                .get("legal_name")
                .is_none_or(|v| v.as_str().is_none_or(|s| s.is_empty()))
            {
                result
                    .errors
                    .push((row_number, "Vui lòng nhập tên pháp lý.".into()));
                result.skipped += 1;
                continue;
            }
            if data
                .get("customer_type")
                .is_none_or(|v| v.as_str().is_none_or(|s| s.is_empty()))
            {
                result
                    .errors
                    .push((row_number, "Vui lòng chọn loại khách hàng.".into()));
                result.skipped += 1;
                continue;
            }
            values.insert(
                "created_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            );
            values.insert(
                "updated_at".into(),
                ChangeValue::Sql(codec::write_datetime(Some(now))),
            );
            values.insert("is_active".into(), ChangeValue::Sql(SqlValue::Integer(1)));
            values.insert("is_deleted".into(), ChangeValue::Sql(SqlValue::Integer(0)));
            values.insert(
                "bill_main_station".into(),
                ChangeValue::Sql(SqlValue::Integer(1)),
            );
            changes.push(Change::insert("customers", values));
            result.created += 1;
        }
    }
    if !changes.is_empty()
        && let Err(error) = ctx.store_mut().apply(changes)
    {
        return Err(ServiceError::Invalid(format!(
            "Lỗi khi lưu file nhập khách hàng: {error}"
        )));
    }
    Ok(result)
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::DateTime(value) => value
            .as_datetime()
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| value.to_string()),
        Data::DateTimeIso(value) | Data::DurationIso(value) | Data::String(value) => value.clone(),
        _ => cell.to_string(),
    }
}
