use cf_db::{model::Product, store::Store};
use rust_xlsxwriter::Workbook;

use crate::ServiceError;

pub const IMPORT_COLUMNS: &[&str] = &[
    "code",
    "name",
    "product_type",
    "group_code",
    "kind",
    "unit_label",
    "default_price",
    "default_vat_rate",
    "price_period_months",
    "quantity_source",
    "min_quantity",
    "quantity_step",
    "account_limit",
    "default_duration_months",
    "contract_number_format",
    "is_active",
    "sort_order",
];

const TEMPLATE_EXAMPLES: [[&str; 17]; 2] = [
    [
        "EX-MAIN",
        "Gói chính mẫu",
        "Dịch vụ",
        "NHOM-MAU",
        "main",
        "cơ sở",
        "500000",
        "10",
        "1",
        "units",
        "2000",
        "500",
        "",
        "12",
        "{seq}/{slug}/{year}",
        "1",
        "0",
    ],
    [
        "EX-ADDON",
        "Gói bổ sung mẫu",
        "Dịch vụ",
        "NHOM-MAU",
        "addon",
        "cơ sở",
        "150000",
        "10",
        "1",
        "units",
        "",
        "",
        "3",
        "12",
        "",
        "1",
        "1",
    ],
];

/// Port của app/services/product_service.py::make_catalog_xlsx.
pub fn make_catalog_xlsx(store: Option<&Store>, with_data: bool) -> Result<Vec<u8>, ServiceError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("Danh mục")
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    for (column, header) in IMPORT_COLUMNS.iter().enumerate() {
        worksheet
            .write_string(0, column as u16, *header)
            .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    }
    if with_data {
        if let Some(store) = store {
            for (offset, product) in super::list::list_products(store, true).iter().enumerate() {
                write_product(worksheet, offset as u32 + 1, product)?;
            }
        }
    } else {
        for (row, example) in TEMPLATE_EXAMPLES.iter().enumerate() {
            for (column, value) in example.iter().enumerate() {
                worksheet
                    .write_string(row as u32 + 1, column as u16, *value)
                    .map_err(|error| ServiceError::Invalid(error.to_string()))?;
            }
        }
    }
    workbook
        .save_to_buffer()
        .map_err(|error| ServiceError::Invalid(error.to_string()))
}

fn write_product(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    product: &Product,
) -> Result<(), ServiceError> {
    let text = [
        product.code.as_str(),
        product.name.as_str(),
        product.product_type.as_str(),
        product.group_code.as_deref().unwrap_or_default(),
        product.kind.as_str(),
        product.unit_label.as_deref().unwrap_or_default(),
    ];
    for (column, value) in text.iter().enumerate() {
        worksheet
            .write_string(row, column as u16, *value)
            .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    }
    let numbers = [product.default_price, product.default_vat_rate];
    for (offset, value) in numbers.into_iter().enumerate() {
        if let Some(value) = value
            && let Ok(number) = value.to_string().parse::<f64>()
        {
            worksheet
                .write_number(row, offset as u16 + 6, number)
                .map_err(|error| ServiceError::Invalid(error.to_string()))?;
        }
    }
    worksheet
        .write_number(row, 8, product.price_period_months as f64)
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    worksheet
        .write_string(row, 9, &product.quantity_source)
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    let optional_integers = [
        product.min_quantity,
        product.quantity_step,
        product.account_limit,
        product.default_duration_months,
    ];
    for (offset, value) in optional_integers.into_iter().enumerate() {
        if let Some(value) = value {
            worksheet
                .write_number(row, offset as u16 + 10, value as f64)
                .map_err(|error| ServiceError::Invalid(error.to_string()))?;
        }
    }
    worksheet
        .write_string(
            row,
            14,
            product
                .contract_number_format
                .as_deref()
                .unwrap_or_default(),
        )
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    worksheet
        .write_number(row, 15, if product.is_active { 1.0 } else { 0.0 })
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    worksheet
        .write_number(row, 16, product.sort_order as f64)
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    Ok(())
}
