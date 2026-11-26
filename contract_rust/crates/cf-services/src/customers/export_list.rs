use rust_xlsxwriter::{Color, Format, Workbook};

use crate::ServiceError;

const HEADERS: &[&str] = &[
    "Mã khách hàng",
    "Tên pháp lý",
    "Tên ngắn",
    "Loại đơn vị",
    "Địa chỉ",
    "Xã / Phường / Đặc khu",
    "Tỉnh / Thành phố",
    "Mã số thuế",
    "Mã cơ sở KCB",
    "Số QĐ thành lập",
    "Ngày thành lập",
    "Đại diện pháp lý",
    "Giới tính đại diện",
    "Chức vụ",
    "Số tài khoản",
    "Ngân hàng",
    "Tên thụ hưởng",
    "Đầu mối liên hệ",
    "Điện thoại",
    "Email",
    "Ghi chú",
];

/// Port của app/routers/customers.py::download_template.
pub fn make_customer_import_template_xlsx() -> Result<Vec<u8>, ServiceError> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("Danh sách khách hàng")
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
    let example = [
        "KH-MAU",
        "Đơn vị mẫu",
        "Đơn vị mẫu",
        "Đơn vị",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "Người đại diện mẫu",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
    ];
    for (column, value) in example.iter().enumerate() {
        worksheet
            .write_string(1, column as u16, *value)
            .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    }
    let widths = [
        14.0, 35.0, 20.0, 15.0, 25.0, 22.0, 16.0, 14.0, 16.0, 20.0, 14.0, 22.0, 12.0, 16.0, 18.0,
        25.0, 25.0, 20.0, 14.0, 22.0, 20.0,
    ];
    for (column, width) in widths.into_iter().enumerate() {
        worksheet
            .set_column_width(column as u16, width)
            .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    }
    worksheet
        .set_row_height(0, 30.0)
        .map_err(|error| ServiceError::Invalid(error.to_string()))?;
    workbook
        .save_to_buffer()
        .map_err(|error| ServiceError::Invalid(error.to_string()))
}
