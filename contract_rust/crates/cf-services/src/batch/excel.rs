use std::{collections::BTreeMap, io::Cursor};

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use rust_xlsxwriter::{Color, Format, Workbook};

use crate::ServiceError;

pub const BATCH_EXCEL_COLUMNS: &[(&str, &str)] = &[
    ("mã khách hàng", "customer_code"),
    ("tên khách hàng", "customer_name"),
    ("số hợp đồng", "contract_number"),
    ("ngày ký", "sign_date"),
    ("ngày bắt đầu", "start_date"),
    ("số tháng", "month_count"),
    ("số đơn vị", "unit_count"),
    ("đơn giá có vat", "unit_price_vat"),
    ("tổng giá trị", "total_amount"),
    ("mô tả giá", "pricing_description"),
    ("mã gói chính", "main_code"),
    ("mã gói bổ sung", "addon_codes"),
    ("đại diện bên b", "party_b_representative"),
    ("số uỷ quyền", "party_b_authorization_number"),
    ("ngày uỷ quyền", "party_b_authorization_date"),
];

const HEADERS: &[&str] = &[
    "Mã khách hàng",
    "Số hợp đồng",
    "Ngày ký",
    "Ngày bắt đầu",
    "Số tháng",
    "Số đơn vị",
    "Đơn giá có VAT",
    "Tổng giá trị",
    "Mô tả giá",
    "Đại diện Bên B",
    "Số uỷ quyền",
    "Ngày uỷ quyền",
    "Mã gói chính",
    "Mã gói bổ sung",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportExcelResult {
    /// Các dòng hợp lệ, trong đó có customer_id dạng chuỗi.
    pub rows: Vec<BTreeMap<String, String>>,
    pub errors: Vec<(usize, String)>,
    pub total_rows: usize,
}

/// Port của app/services/batch_generator.py::make_batch_template_xlsx.
pub fn make_batch_template_xlsx() -> Result<Vec<u8>, ServiceError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("Batch Import")
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    let header_format = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1A2238))
        .set_text_wrap();
    for (column, header) in HEADERS.iter().enumerate() {
        worksheet
            .write_string_with_format(0, column as u16, *header, &header_format)
            .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    }
    let sample_rows: [[&str; 14]; 2] = [
        [
            "KH-MAU-01",
            "HD-MAU-01",
            "15/01/2025",
            "15/01/2025",
            "12",
            "1",
            "24000000",
            "24000000",
            "",
            "Người đại diện mẫu",
            "",
            "",
            "",
            "",
        ],
        [
            "KH-MAU-02",
            "HD-MAU-02",
            "15/01/2025",
            "15/01/2025",
            "12",
            "1",
            "24000000",
            "24000000",
            "",
            "Người đại diện mẫu",
            "",
            "",
            "",
            "",
        ],
    ];
    for (row_index, row) in sample_rows.iter().enumerate() {
        for (column, value) in row.iter().enumerate() {
            worksheet
                .write_string((row_index + 1) as u32, column as u16, *value)
                .map_err(|error| ServiceError::Invalid(error.to_string()))?;
        }
    }
    for (column, width) in [
        14.0, 22.0, 12.0, 12.0, 10.0, 10.0, 15.0, 15.0, 20.0, 20.0, 14.0, 14.0, 18.0, 22.0,
    ]
    .into_iter()
    .enumerate()
    {
        worksheet
            .set_column_width(column as u16, width)
            .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    }
    worksheet
        .set_row_height(0, 28.0)
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    workbook
        .save_to_buffer()
        .map_err(|error| ServiceError::Invalid(error.to_string()))
}

/// Port của app/services/batch_generator.py::import_excel_mapping.
pub fn import_excel_mapping(
    file_bytes: &[u8],
    customer_lookup: &BTreeMap<String, i64>,
) -> ImportExcelResult {
    let mut result = ImportExcelResult::default();
    let mut workbook: Xlsx<_> = match open_workbook_from_rs(Cursor::new(file_bytes)) {
        Ok(workbook) => workbook,
        Err(error) => {
            result
                .errors
                .push((0, format!("Không đọc được file Excel: {error}")));
            return result;
        }
    };
    let Some(range) = workbook.worksheet_range_at(0) else {
        result.errors.push((
            0,
            "Không đọc được file Excel: workbook không có sheet.".into(),
        ));
        return result;
    };
    let range = match range {
        Ok(range) => range,
        Err(error) => {
            result
                .errors
                .push((0, format!("Không đọc được file Excel: {error}")));
            return result;
        }
    };
    let rows = range.rows().collect::<Vec<_>>();
    if rows.len() < 2 {
        result
            .errors
            .push((0, "File Excel trống hoặc thiếu dữ liệu.".into()));
        return result;
    }

    let header = rows.iter().take(5).enumerate().find_map(|(index, row)| {
        let columns = row
            .iter()
            .enumerate()
            .filter_map(|(column, cell)| {
                let value = cell_text(cell).trim().to_lowercase();
                (!value.is_empty()).then_some((value, column))
            })
            .collect::<BTreeMap<_, _>>();
        (columns.contains_key("mã khách hàng") || columns.contains_key("tên khách hàng"))
            .then_some((index, columns))
    });
    let Some((header_index, header_columns)) = header else {
        result.errors.push((
            0,
            "Không tìm thấy header. Cần cột 'Mã khách hàng' hoặc 'Tên khách hàng'.".into(),
        ));
        return result;
    };
    let mapped_columns = BATCH_EXCEL_COLUMNS
        .iter()
        .filter_map(|(label, field)| {
            header_columns
                .get(*label)
                .map(|column| ((*field).to_owned(), *column))
        })
        .collect::<Vec<_>>();

    for (offset, row) in rows.iter().skip(header_index + 1).enumerate() {
        let row_number = header_index + 2 + offset;
        if row.iter().all(|cell| cell_text(cell).trim().is_empty()) {
            continue;
        }
        result.total_rows += 1;
        let mut values = BTreeMap::new();
        for (field, column) in &mapped_columns {
            if let Some(cell) = row.get(*column) {
                let value = cell_text(cell).trim().to_owned();
                if !value.is_empty() {
                    values.insert(field.clone(), value);
                }
            }
        }
        let customer_code = values.remove("customer_code").unwrap_or_default();
        let customer_name = values.remove("customer_name").unwrap_or_default();
        let customer_id = [customer_code.as_str(), customer_name.as_str()]
            .into_iter()
            .filter(|key| !key.is_empty())
            .find_map(|key| customer_lookup.get(key).copied());
        let Some(customer_id) = customer_id else {
            result.errors.push((
                row_number,
                format!("Không tìm thấy KH với mã/tên: {customer_code:?} / {customer_name:?}"),
            ));
            continue;
        };
        values.insert("customer_id".into(), customer_id.to_string());
        result.rows.push(values);
    }
    result
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) | Data::DurationIso(value) | Data::String(value) => value.clone(),
        _ => cell.to_string(),
    }
}
