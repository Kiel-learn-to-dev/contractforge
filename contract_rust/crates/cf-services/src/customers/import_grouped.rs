use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
};

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use cf_db::{
    change::{Change, ChangeValue},
    codec,
    model::CustomerUnit,
};
use rusqlite::types::Value;

use crate::{Ctx, ServiceError, customers::edit};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupedImportResult {
    pub total_customers: usize,
    pub created: usize,
    pub updated: usize,
    pub total_units: usize,
    pub skipped: usize,
    pub errors: Vec<(usize, String)>,
    pub warnings: Vec<(usize, String)>,
}

#[derive(Default)]
struct Group {
    name: String,
    units: Vec<String>,
    row: usize,
}

pub fn import_grouped_from_excel(
    ctx: &mut Ctx,
    bytes: &[u8],
    sheet_name: Option<&str>,
) -> Result<GroupedImportResult, ServiceError> {
    let mut workbook: Xlsx<_> = match open_workbook_from_rs(Cursor::new(bytes)) {
        Ok(workbook) => workbook,
        Err(error) => {
            return Ok(GroupedImportResult {
                errors: vec![(0, format!("Không đọc được file Excel: {error}"))],
                ..Default::default()
            });
        }
    };
    let range = if let Some(name) = sheet_name {
        workbook.worksheet_range(name)
    } else if let Some(range) = workbook.worksheet_range_at(0) {
        range
    } else {
        return Ok(GroupedImportResult {
            errors: vec![(
                0,
                "Không đọc được file Excel: workbook không có sheet.".into(),
            )],
            ..Default::default()
        });
    };
    let range = match range {
        Ok(range) => range,
        Err(error) => {
            return Ok(GroupedImportResult {
                errors: vec![(0, format!("Không đọc được file Excel: {error}"))],
                ..Default::default()
            });
        }
    };
    let rows = range.rows().collect::<Vec<_>>();
    if rows.is_empty() {
        return Ok(GroupedImportResult {
            errors: vec![(0, "File Excel trống.".into())],
            ..Default::default()
        });
    }
    let header = rows.iter().take(8).enumerate().find_map(|(index, row)| {
        let cells = row
            .iter()
            .map(|cell| cell_text(cell).trim().to_lowercase())
            .collect::<Vec<_>>();
        let has_stt = cells.iter().any(|cell| cell.contains("stt"));
        let name = cells.iter().position(|cell| cell.contains("đơn vị"));
        let kind = cells.iter().position(|cell| cell.contains("loại"));
        (has_stt && name.is_some() && kind.is_some()).then_some((
            index,
            name.unwrap(),
            kind.unwrap(),
        ))
    });
    let Some((header_index, name_column, kind_column)) = header else {
        return Ok(GroupedImportResult { errors: vec![(0, "Không tìm thấy dòng tiêu đề hợp lệ (cần có 'STT', 'Đơn vị', 'Loại'). Vui lòng dùng đúng file danh sách trạm.".into())], ..Default::default() });
    };
    let mut result = GroupedImportResult::default();
    let mut groups = Vec::<Group>::new();
    let mut current: Option<usize> = None;
    for (offset, row) in rows.iter().skip(header_index + 1).enumerate() {
        let row_no = header_index + 2 + offset;
        if row.iter().all(|cell| cell_text(cell).trim().is_empty()) {
            continue;
        }
        let name = row
            .get(name_column)
            .map(cell_text)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let kind = row
            .get(kind_column)
            .map(cell_text)
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if name.is_empty() {
            continue;
        }
        if kind.contains("trạm y tế") || kind.contains("trạm y te") {
            groups.push(Group {
                name,
                units: Vec::new(),
                row: row_no,
            });
            current = Some(groups.len() - 1);
        } else if kind.contains("điểm trạm") || kind.contains("diem tram") {
            if let Some(index) = current {
                groups[index].units.push(name);
            } else {
                result.warnings.push((
                    row_no,
                    format!("Điểm trạm '{name}' không xác định được trạm chính — bỏ qua."),
                ));
            }
        }
    }
    if groups.is_empty() {
        result
            .errors
            .push((0, "Không tìm thấy dòng 'Trạm y tế' nào trong file.".into()));
        return Ok(result);
    }
    result.total_customers = groups.len();
    result.total_units = groups.iter().map(|group| group.units.len()).sum();
    let mut grouped = Vec::<(Group, usize)>::new();
    let mut group_index = BTreeMap::<String, usize>::new();
    for group in groups {
        let key = group.name.to_lowercase();
        if let Some(index) = group_index.get(&key).copied() {
            grouped[index].0 = group;
            grouped[index].1 += 1;
        } else {
            group_index.insert(key, grouped.len());
            grouped.push((group, 1));
        }
    }

    let mut changes = Vec::new();
    let name_to_id = ctx
        .store()
        .customers
        .values()
        .filter(|c| !c.is_deleted)
        .map(|customer| (customer.legal_name.trim().to_lowercase(), customer.id))
        .collect::<BTreeMap<_, _>>();
    let mut code_counter = edit::generate_next_code(ctx.store())
        .trim_start_matches("KH-")
        .parse::<u64>()
        .unwrap_or(0);
    let now = ctx.clock().now_utc();
    for (group, occurrences) in grouped {
        let key = group.name.to_lowercase();
        let customer_insert_index;
        let customer_id = if let Some(id) = name_to_id.get(&key).copied() {
            let customer = &ctx.store().customers[&id];
            let mut values = BTreeMap::from([
                ("is_active".to_owned(), ChangeValue::Sql(Value::Integer(1))),
                (
                    "updated_at".to_owned(),
                    ChangeValue::Sql(codec::write_datetime(Some(now))),
                ),
            ]);
            if customer.customer_type.trim().is_empty() {
                values.insert(
                    "customer_type".into(),
                    ChangeValue::Sql(Value::Text("Trạm y tế".into())),
                );
            }
            if customer
                .short_name
                .as_deref()
                .is_none_or(|name| name.is_empty())
            {
                values.insert(
                    "short_name".into(),
                    ChangeValue::Sql(Value::Text(make_short_name(&group.name))),
                );
            }
            changes.push(Change::update("customers", id, values));
            result.updated += occurrences;
            customer_insert_index = None;
            id
        } else {
            code_counter += 1;
            let code = format!("KH-{code_counter:03}");
            let values = BTreeMap::from([
                ("code".into(), ChangeValue::Sql(Value::Text(code))),
                (
                    "legal_name".into(),
                    ChangeValue::Sql(Value::Text(group.name.clone())),
                ),
                (
                    "short_name".into(),
                    ChangeValue::Sql(Value::Text(make_short_name(&group.name))),
                ),
                (
                    "customer_type".into(),
                    ChangeValue::Sql(Value::Text("Trạm y tế".into())),
                ),
                (
                    "province".into(),
                    ChangeValue::Sql(Value::Text("Đồng Nai".into())),
                ),
                ("is_active".into(), ChangeValue::Sql(Value::Integer(1))),
                ("is_deleted".into(), ChangeValue::Sql(Value::Integer(0))),
                (
                    "bill_main_station".into(),
                    ChangeValue::Sql(Value::Integer(1)),
                ),
                (
                    "created_at".into(),
                    ChangeValue::Sql(codec::write_datetime(Some(now))),
                ),
                (
                    "updated_at".into(),
                    ChangeValue::Sql(codec::write_datetime(Some(now))),
                ),
            ]);
            let index = changes.len();
            changes.push(Change::insert("customers", values));
            customer_insert_index = Some(index);
            result.created += 1;
            result.updated += occurrences.saturating_sub(1);
            0
        };

        let existing_units = if customer_insert_index.is_none() {
            ctx.store()
                .units
                .values()
                .filter(|unit| unit.customer_id == customer_id)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let incoming = group
            .units
            .iter()
            .map(|name| name.to_lowercase())
            .collect::<BTreeSet<_>>();
        let by_name = existing_units
            .iter()
            .map(|unit| (unit.name.trim().to_lowercase(), unit))
            .collect::<BTreeMap<_, _>>();
        for unit in &existing_units {
            if !incoming.contains(&unit.name.trim().to_lowercase()) && unit.is_active {
                changes.push(Change::update(
                    "customer_units",
                    unit.id,
                    BTreeMap::from([
                        ("is_active".into(), ChangeValue::Sql(Value::Integer(0))),
                        (
                            "updated_at".into(),
                            ChangeValue::Sql(codec::write_datetime(Some(now))),
                        ),
                    ]),
                ));
            }
        }
        for (sort_order, name) in group.units.iter().enumerate() {
            if let Some(unit) = by_name.get(&name.to_lowercase()) {
                changes.push(Change::update(
                    "customer_units",
                    unit.id,
                    BTreeMap::from([
                        ("is_active".into(), ChangeValue::Sql(Value::Integer(1))),
                        (
                            "sort_order".into(),
                            ChangeValue::Sql(Value::Integer(sort_order as i64)),
                        ),
                        (
                            "updated_at".into(),
                            ChangeValue::Sql(codec::write_datetime(Some(now))),
                        ),
                    ]),
                ));
            } else {
                let mut values = CustomerUnit {
                    id: 0,
                    customer_id: 0,
                    name: name.clone(),
                    unit_type: "Điểm trạm".into(),
                    address: None,
                    notes: None,
                    is_active: true,
                    include_in_billing: true,
                    sort_order: sort_order as i64,
                    created_at: now,
                    updated_at: now,
                }
                .to_values()
                .into_iter()
                .filter(|(key, _)| key != "id")
                .map(|(key, value)| (key, ChangeValue::Sql(value)))
                .collect::<BTreeMap<_, _>>();
                if let Some(index) = customer_insert_index {
                    values.insert("customer_id".into(), ChangeValue::Ref(index));
                } else {
                    values.insert(
                        "customer_id".into(),
                        ChangeValue::Sql(Value::Integer(customer_id)),
                    );
                }
                changes.push(Change::insert("customer_units", values));
            }
        }
        let _ = (group.row, occurrences);
    }
    if !changes.is_empty() {
        ctx.store_mut().apply(changes)?;
    }
    Ok(result)
}

fn make_short_name(name: &str) -> String {
    let value = name.trim();
    for prefix in ["Trạm y tế ", "Trạm Y tế ", "TRẠM Y TẾ ", "Trạm Y Tế "] {
        if value.to_lowercase().starts_with(&prefix.to_lowercase()) {
            return format!("TYT {}", &value[prefix.len()..]);
        }
    }
    value.to_owned()
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::DateTime(value) => value
            .as_datetime()
            .map(|d| d.to_string())
            .unwrap_or_else(|| value.to_string()),
        Data::DateTimeIso(value) | Data::DurationIso(value) | Data::String(value) => value.clone(),
        _ => cell.to_string(),
    }
}
