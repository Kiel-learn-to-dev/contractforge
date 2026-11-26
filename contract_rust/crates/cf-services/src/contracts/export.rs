use cf_core::{fmt::fmt_currency, lifecycle::status_label};
use cf_db::{
    model::{Contract, ContractItem},
    store::Store,
};
use chrono::NaiveDate;
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::{Color, Format, FormatAlign, Workbook, Worksheet};

use super::list::{self, ContractFilters, ContractRow};
use crate::ServiceError;

const CONTRACT_HEADERS: &[&str] = &[
    "STT",
    "Số hợp đồng",
    "Khách hàng",
    "Sản phẩm",
    "Ngày ký",
    "Ngày hết hạn",
    "Giá trị (VNĐ)",
    "Trạng thái",
    "Ghi chú",
    "Trước thuế",
    "Thuế",
    "Loại văn bản",
    "Hợp đồng gốc",
];
const CONTRACT_WIDTHS: &[f64] = &[
    5.0, 30.0, 35.0, 15.0, 12.0, 12.0, 18.0, 15.0, 30.0, 18.0, 14.0, 20.0, 30.0,
];
const ITEM_HEADERS: &[&str] = &[
    "Số hợp đồng",
    "TT",
    "Dịch vụ",
    "ĐVT",
    "Số lượng",
    "Số kỳ",
    "Đơn giá",
    "Trước thuế",
    "Thuế",
    "Sau thuế",
];
const ITEM_WIDTHS: &[f64] = &[30.0, 6.0, 40.0, 12.0, 12.0, 10.0, 16.0, 18.0, 16.0, 18.0];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractExport {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

/// Xuất toàn bộ hợp đồng khớp bộ lọc, không giới hạn theo trang đang xem.
pub fn export_contracts(
    store: &Store,
    filters: &ContractFilters,
    today: NaiveDate,
) -> Result<ContractExport, ServiceError> {
    let mut export_filters = filters.clone();
    export_filters.page = 1;
    export_filters.per_page = 100;
    let first_page = list::list_contracts(store, &export_filters, today);
    let total = first_page.total;
    let pages = total.div_ceil(100);
    let mut rows = first_page.rows;
    for page in 2..=pages {
        export_filters.page = page;
        rows.extend(list::list_contracts(store, &export_filters, today).rows);
    }

    let mut workbook = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xD6E4F7))
        .set_align(FormatAlign::VerticalCenter)
        .set_align(FormatAlign::Center);
    write_contract_sheet(&mut workbook, store, &rows, &header)?;
    write_item_sheet(&mut workbook, store, &rows, &header)?;
    let bytes = workbook.save_to_buffer().map_err(export_error)?;
    Ok(ContractExport {
        file_name: format!("hop-dong-{}.xlsx", today.format("%Y%m%d")),
        bytes,
    })
}

fn write_contract_sheet(
    workbook: &mut Workbook,
    store: &Store,
    rows: &[ContractRow],
    header: &Format,
) -> Result<(), ServiceError> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Danh sách hợp đồng").map_err(export_error)?;
    write_headers(sheet, CONTRACT_HEADERS, CONTRACT_WIDTHS, header)?;
    sheet.set_row_height(0, 20.0).map_err(export_error)?;

    for (index, row) in rows.iter().enumerate() {
        let contract = &row.contract;
        let product = contract.product_id.and_then(|id| store.products.get(&id));
        let parent_number = contract
            .parent_contract_id
            .and_then(|id| store.contracts.get(&id))
            .map_or_else(String::new, |parent| parent.contract_number.clone());
        let values = [
            contract.contract_number.clone(),
            row.customer_name.clone(),
            product.map_or_else(String::new, |product| product.name.clone()),
            date_text(contract.sign_date),
            date_text(contract.end_date),
            fmt_currency(contract.total_amount),
            status_label(contract.status.as_str()).to_owned(),
            contract.notes.clone().unwrap_or_default(),
            fmt_currency(contract.total_pre_vat),
            fmt_currency(contract.total_vat),
            doc_kind_label(&contract.doc_kind).to_owned(),
            parent_number,
        ];
        let row_number = (index + 1) as u32;
        sheet
            .write_number(row_number, 0, index.saturating_add(1) as f64)
            .map_err(export_error)?;
        for (column, value) in values.iter().enumerate() {
            sheet
                .write_string(row_number, (column + 1) as u16, value)
                .map_err(export_error)?;
        }
    }
    Ok(())
}

fn write_item_sheet(
    workbook: &mut Workbook,
    store: &Store,
    rows: &[ContractRow],
    header: &Format,
) -> Result<(), ServiceError> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Chi tiết dòng").map_err(export_error)?;
    write_headers(sheet, ITEM_HEADERS, ITEM_WIDTHS, header)?;
    let mut row_index = 1u32;
    for contract_row in rows {
        let contract = &contract_row.contract;
        let mut items = store
            .items
            .values()
            .filter(|item| item.contract_id == contract.id)
            .collect::<Vec<_>>();
        items.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then(a.id.cmp(&b.id)));
        for (position, item) in items.into_iter().enumerate() {
            write_item_row(sheet, row_index, contract, position + 1, item)?;
            row_index = row_index.saturating_add(1);
        }
    }
    Ok(())
}

fn write_item_row(
    sheet: &mut Worksheet,
    row: u32,
    contract: &Contract,
    position: usize,
    item: &ContractItem,
) -> Result<(), ServiceError> {
    for (column, value) in [
        contract.contract_number.as_str(),
        item.description.as_str(),
        item.unit_label.as_deref().unwrap_or_default(),
    ]
    .into_iter()
    .enumerate()
    {
        let target_column = match column {
            0 => 0,
            1 => 2,
            _ => 3,
        };
        sheet
            .write_string(row, target_column, value)
            .map_err(export_error)?;
    }
    sheet
        .write_number(row, 1, position as f64)
        .map_err(export_error)?;
    write_decimal_number(sheet, row, 4, item.quantity.to_f64())?;
    write_decimal_number(sheet, row, 5, item.period_count.to_f64())?;
    for (column, value) in [
        (6, item.unit_price_pre_vat),
        (7, Some(item.amount_pre_vat)),
        (8, Some(item.vat_amount)),
        (9, Some(item.amount_total)),
    ] {
        sheet
            .write_string(row, column, fmt_currency(value))
            .map_err(export_error)?;
    }
    Ok(())
}

fn write_decimal_number(
    sheet: &mut Worksheet,
    row: u32,
    column: u16,
    value: Option<f64>,
) -> Result<(), ServiceError> {
    if let Some(value) = value {
        sheet
            .write_number(row, column, value)
            .map_err(export_error)?;
    } else {
        sheet.write_string(row, column, "").map_err(export_error)?;
    }
    Ok(())
}

fn write_headers(
    sheet: &mut Worksheet,
    headers: &[&str],
    widths: &[f64],
    format: &Format,
) -> Result<(), ServiceError> {
    for (column, text) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(0, column as u16, *text, format)
            .map_err(export_error)?;
    }
    for (column, width) in widths.iter().enumerate() {
        sheet
            .set_column_width(column as u16, *width)
            .map_err(export_error)?;
    }
    Ok(())
}

fn date_text(date: Option<chrono::NaiveDate>) -> String {
    date.map_or_else(String::new, |date| date.format("%d/%m/%Y").to_string())
}

fn doc_kind_label(doc_kind: &str) -> &str {
    match doc_kind {
        "contract" => "Hợp đồng",
        "addendum_addon" => "Phụ lục bổ sung",
        "addendum_renewal" => "Phụ lục gia hạn",
        other => other,
    }
}

fn export_error(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Invalid(error.to_string())
}
