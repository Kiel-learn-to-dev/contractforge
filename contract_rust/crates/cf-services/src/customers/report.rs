use std::collections::BTreeSet;

use cf_core::lifecycle::{self, ContractStatus};
use cf_db::{
    model::{Contract, Customer},
    store::Store,
};
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::{Color, ExcelDateTime, Format, Workbook, Worksheet};

use crate::{
    ServiceError,
    customers::tier::{self, TierContract},
};

struct Row {
    customer: Customer,
    contracts: Vec<Contract>,
    active: Vec<Contract>,
    signed: Vec<Contract>,
    tier: u8,
    action: String,
    days: i64,
    near: Option<Contract>,
    unit_count: usize,
    active_value: Decimal,
    lifetime_value: Decimal,
    risk30: Decimal,
    risk60: Decimal,
    last_sign: Option<NaiveDate>,
    last_contract: Option<Contract>,
    active_products: String,
    all_products: String,
    missing_products: String,
    representative: String,
}

/// Export the customer report as the same four-sheet workbook as the Python route.
pub fn export_report(store: &Store, today: NaiveDate) -> Result<Vec<u8>, ServiceError> {
    let catalog = store
        .products
        .values()
        .filter(|p| p.is_active)
        .map(|p| product_label(&p.code, &p.name))
        .filter(|s| !s.is_empty())
        .collect::<BTreeSet<_>>();
    let mut customers = store
        .customers
        .values()
        .filter(|c| !c.is_deleted)
        .cloned()
        .collect::<Vec<_>>();
    customers.sort_by(|a, b| a.code.cmp(&b.code));
    let mut rows = Vec::new();
    for customer in customers {
        let mut contracts = store
            .contracts
            .values()
            .filter(|c| c.customer_id == customer.id)
            .cloned()
            .collect::<Vec<_>>();
        contracts.sort_by_key(|c| c.id);
        let tier_contracts = contracts
            .iter()
            .map(|c| TierContract {
                contract: c.clone(),
                product_label: c
                    .product_id
                    .and_then(|id| store.products.get(&id))
                    .map(|p| product_label(&p.code, &p.name)),
            })
            .collect::<Vec<_>>();
        let units = store
            .units
            .values()
            .filter(|u| u.customer_id == customer.id)
            .cloned()
            .collect::<Vec<_>>();
        let classified = tier::classify(&customer, &units, &tier_contracts, &catalog, today);
        let active = contracts
            .iter()
            .filter(|c| lifecycle::ACTIVE_LIFECYCLE.contains(&c.status))
            .cloned()
            .collect::<Vec<_>>();
        let signed = contracts
            .iter()
            .filter(|c| {
                matches!(
                    c.status,
                    ContractStatus::Expired | ContractStatus::Terminated
                ) || lifecycle::ACTIVE_LIFECYCLE.contains(&c.status)
            })
            .cloned()
            .collect::<Vec<_>>();
        let labels = |items: &[Contract]| {
            items
                .iter()
                .filter_map(|c| c.product_id.and_then(|id| store.products.get(&id)))
                .map(|p| product_label(&p.code, &p.name))
                .filter(|s| !s.is_empty())
                .collect::<BTreeSet<_>>()
        };
        let active_labels = labels(&active);
        let all_labels = labels(&contracts);
        let missing = if all_labels.is_empty() {
            BTreeSet::new()
        } else {
            catalog.difference(&all_labels).cloned().collect()
        };
        let active_value: Decimal = active.iter().filter_map(|c| c.total_amount).sum();
        let lifetime_value: Decimal = contracts.iter().filter_map(|c| c.total_amount).sum();
        let risk = |days: i64| {
            active
                .iter()
                .filter(|c| {
                    c.total_amount.is_some()
                        && c.end_date
                            .is_some_and(|d| (0..=days).contains(&(d - today).num_days()))
                })
                .filter_map(|c| c.total_amount)
                .sum()
        };
        let last_sign = contracts.iter().filter_map(|c| c.sign_date).max();
        let last_contract = contracts
            .iter()
            .filter(|c| c.sign_date.is_some())
            .max_by_key(|c| c.sign_date)
            .or_else(|| contracts.iter().max_by_key(|c| c.id))
            .cloned();
        let gender = match customer.representative_gender.as_deref().unwrap_or("") {
            "Nam" => "(Ông) ",
            "Nữ" => "(Bà) ",
            _ => "",
        };
        let representative = customer
            .representative_name
            .as_deref()
            .map(|name| format!("{gender}{name}"))
            .unwrap_or_default();
        let risk30 = risk(30);
        let risk60 = risk(60);
        rows.push(Row {
            unit_count: usize::from(customer.bill_main_station)
                + units
                    .iter()
                    .filter(|u| u.is_active && u.include_in_billing)
                    .count(),
            customer,
            contracts,
            active,
            signed,
            tier: classified.tier,
            action: classified.action_text,
            days: classified.days_to_expiry,
            near: classified
                .contract_id
                .and_then(|id| store.contracts.get(&id))
                .cloned(),
            active_value,
            lifetime_value,
            risk30,
            risk60,
            last_sign,
            last_contract,
            active_products: joined(&active_labels),
            all_products: joined(&all_labels),
            missing_products: joined(&missing),
            representative,
        });
    }
    let mut book = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1A2238))
        .set_text_wrap();
    let money = Format::new().set_num_format("#,##0");
    let date = Format::new().set_num_format("dd/mm/yyyy");
    write_priority(&mut book, &rows, &header, &money, &date)?;
    write_profiles(&mut book, &rows, &header, &money, &date)?;
    write_history(&mut book, store, &rows, &header, &money, &date)?;
    write_summary(&mut book, &rows, &catalog, today, &header, &money)?;
    book.save_to_buffer().map_err(err)
}

fn write_priority(
    book: &mut Workbook,
    rows: &[Row],
    header: &Format,
    money: &Format,
    date: &Format,
) -> Result<(), ServiceError> {
    let sheet = book.add_worksheet();
    sheet.set_name("Ưu tiên hành động").map_err(err)?;
    let heads = [
        "STT",
        "Tier",
        "Mã KH",
        "Tên ngắn",
        "Loại đơn vị",
        "Hành động gợi ý",
        "Ngày hết hạn HĐ gần nhất",
        "Còn (ngày)",
        "Số HĐ gần nhất",
        "Sản phẩm active",
        "Còn thiếu",
        "Số cơ sở",
        "Giá trị active (VNĐ)",
        "Nguy cơ mất 30 ngày",
        "Nguy cơ mất 60 ngày",
    ];
    headers(sheet, &heads, header)?;
    let mut sorted = rows.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| {
        a.tier.cmp(&b.tier).then_with(|| {
            if a.tier <= 2 {
                a.days.cmp(&b.days)
            } else {
                b.days.cmp(&a.days)
            }
        })
    });
    for (i, r) in sorted.iter().enumerate() {
        let color = [0xFFD7D7, 0xFFE8C8, 0xFFF9C4, 0xD7F2D7, 0xEBEBEB]
            [(r.tier.saturating_sub(1) as usize).min(4)];
        let mut f = Format::new()
            .set_background_color(Color::RGB(color))
            .set_text_wrap();
        if !r.customer.is_active {
            f = f.set_font_color(Color::RGB(0x999999)).set_italic();
        }
        let label = [
            "🔴 KHẨN",
            "🟠 Cần xử lý",
            "🟡 Upsell",
            "✅ Ổn định",
            "⚫ Không HĐ",
        ][r.tier.saturating_sub(1) as usize];
        let n = i as u32 + 1;
        text(sheet, n, 0, &(i + 1).to_string(), &f)?;
        text(sheet, n, 1, label, &f)?;
        text(sheet, n, 2, &r.customer.code, &f)?;
        text(
            sheet,
            n,
            3,
            r.customer
                .short_name
                .as_deref()
                .unwrap_or(&r.customer.legal_name),
            &f,
        )?;
        text(sheet, n, 4, &r.customer.customer_type, &f)?;
        text(sheet, n, 5, &r.action, &f)?;
        if let Some(end) = r.near.as_ref().and_then(|c| c.end_date) {
            date_cell(sheet, n, 6, end, date)?;
        } else {
            blank(sheet, n, 6, &f)?;
        }
        if r.near.is_some() {
            number(sheet, n, 7, r.days as f64, &f)?;
        } else {
            blank(sheet, n, 7, &f)?;
        }
        text(
            sheet,
            n,
            8,
            r.near
                .as_ref()
                .map(|c| c.contract_number.as_str())
                .unwrap_or(""),
            &f,
        )?;
        text(sheet, n, 9, &r.active_products, &f)?;
        text(sheet, n, 10, &r.missing_products, &f)?;
        number(sheet, n, 11, r.unit_count as f64, &f)?;
        money_cell(sheet, n, 12, r.active_value, money)?;
        money_cell(sheet, n, 13, r.risk30, money)?;
        money_cell(sheet, n, 14, r.risk60, money)?;
    }
    sheet.set_freeze_panes(1, 0).map_err(err)?;
    sheet.autofilter(0, 0, 0, 14).map_err(err)?;
    Ok(())
}

fn write_profiles(
    book: &mut Workbook,
    rows: &[Row],
    header: &Format,
    money: &Format,
    date: &Format,
) -> Result<(), ServiceError> {
    let s = book.add_worksheet();
    s.set_name("Hồ sơ đầy đủ").map_err(err)?;
    let h = [
        "STT",
        "Mã KH",
        "Tên pháp lý",
        "Tên ngắn",
        "Loại đơn vị",
        "Xã/Phường",
        "Tỉnh/TP",
        "Trạng thái KH",
        "Số cơ sở",
        "Đại diện pháp lý",
        "Chức vụ",
        "SĐT",
        "Email",
        "Mã số thuế",
        "Mã QHNS",
        "Mã KCB BHYT",
        "Số QĐ thành lập",
        "Ngày thành lập",
        "Ngân hàng",
        "Số TK",
        "Tổng HĐ đã ký",
        "HĐ đang hiệu lực",
        "Sản phẩm đang có",
        "Sản phẩm còn thiếu",
        "Giá trị active",
        "Giá trị lifetime",
        "Ngày ký gần nhất",
        "Số HĐ gần nhất",
        "Ghi chú",
    ];
    headers(s, &h, header)?;
    for (i, r) in rows.iter().enumerate() {
        let n = i as u32 + 1;
        let c = &r.customer;
        let textvals = [
            c.code.as_str(),
            c.legal_name.as_str(),
            c.short_name.as_deref().unwrap_or(""),
            c.customer_type.as_str(),
            c.ward.as_deref().unwrap_or(""),
            c.province.as_deref().unwrap_or(""),
            if c.is_active {
                "Hoạt động"
            } else {
                "Tạm ngưng"
            },
            "",
            r.representative.as_str(),
            c.representative_title.as_deref().unwrap_or(""),
            c.phone.as_deref().unwrap_or(""),
            c.email.as_deref().unwrap_or(""),
            c.tax_code.as_deref().unwrap_or(""),
            c.budget_relation_code.as_deref().unwrap_or(""),
            c.kcb_code.as_deref().unwrap_or(""),
            c.establishment_decision.as_deref().unwrap_or(""),
            "",
            c.bank_name.as_deref().unwrap_or(""),
            c.bank_account.as_deref().unwrap_or(""),
            "",
            "",
            r.all_products.as_str(),
            r.missing_products.as_str(),
            "",
            "",
            "",
            r.last_contract
                .as_ref()
                .map(|x| x.contract_number.as_str())
                .unwrap_or(""),
            c.notes.as_deref().unwrap_or(""),
        ];
        number(s, n, 0, (i + 1) as f64, header)?;
        for (col, v) in textvals.iter().enumerate() {
            if v.is_empty() {
                continue;
            }
            text(s, n, (col + 1) as u16, v, &Format::new())?;
        }
        number(s, n, 8, r.unit_count as f64, &Format::new())?;
        number(s, n, 20, r.signed.len() as f64, &Format::new())?;
        number(s, n, 21, r.active.len() as f64, &Format::new())?;
        money_cell(s, n, 24, r.active_value, money)?;
        money_cell(s, n, 25, r.lifetime_value, money)?;
        if let Some(d) = c.establishment_date {
            date_cell(s, n, 17, d, date)?;
        }
        if let Some(d) = r.last_sign {
            date_cell(s, n, 26, d, date)?;
        }
    }
    s.set_freeze_panes(1, 2).map_err(err)?;
    s.autofilter(0, 0, 0, 28).map_err(err)?;
    Ok(())
}

fn write_history(
    book: &mut Workbook,
    store: &Store,
    rows: &[Row],
    header: &Format,
    money: &Format,
    date: &Format,
) -> Result<(), ServiceError> {
    let s = book.add_worksheet();
    s.set_name("Lịch sử hợp đồng").map_err(err)?;
    let h = [
        "STT",
        "Mã KH",
        "Tên KH",
        "Số hợp đồng",
        "Loại SP",
        "Trạng thái",
        "Ngày ký",
        "Hiệu lực từ",
        "Hết hạn",
        "Số tháng",
        "Số cơ sở",
        "Đơn giá (trước VAT)",
        "Đơn giá (sau VAT)",
        "Tổng tiền (VNĐ)",
    ];
    headers(s, &h, header)?;
    let mut i = 0u32;
    for r in rows {
        let mut contracts = r.contracts.clone();
        contracts.sort_by_key(|contract| std::cmp::Reverse(contract.sign_date));
        for c in contracts {
            i += 1;
            number(s, i, 0, i as f64, &Format::new())?;
            text(s, i, 1, &r.customer.code, &Format::new())?;
            text(
                s,
                i,
                2,
                r.customer
                    .short_name
                    .as_deref()
                    .unwrap_or(&r.customer.legal_name),
                &Format::new(),
            )?;
            text(s, i, 3, &c.contract_number, &Format::new())?;
            let product = c.product_id.and_then(|id| store.products.get(&id));
            text(
                s,
                i,
                4,
                &product
                    .map(|p| product_label(&p.code, &p.name))
                    .unwrap_or_default(),
                &Format::new(),
            )?;
            text(s, i, 5, c.status.label(), &Format::new())?;
            for (col, d) in [(6, c.sign_date), (7, c.start_date), (8, c.end_date)] {
                if let Some(d) = d {
                    date_cell(s, i, col, d, date)?;
                }
            }
            for (col, v) in [(9, c.month_count), (10, c.unit_count)] {
                if let Some(v) = v {
                    number(s, i, col, v as f64, &Format::new())?;
                }
            }
            for (col, v) in [
                (11, c.unit_price_pre_vat),
                (12, c.unit_price_vat),
                (13, c.total_amount),
            ] {
                if let Some(v) = v {
                    number(s, i, col, v.to_f64().unwrap_or_default(), money)?;
                }
            }
        }
    }
    s.set_freeze_panes(1, 3).map_err(err)?;
    s.autofilter(0, 0, 0, 13).map_err(err)?;
    Ok(())
}

fn write_summary(
    book: &mut Workbook,
    rows: &[Row],
    catalog: &BTreeSet<String>,
    today: NaiveDate,
    header: &Format,
    money: &Format,
) -> Result<(), ServiceError> {
    let s = book.add_worksheet();
    s.set_name("Tóm tắt").map_err(err)?;
    s.set_column_width(0, 34.0).map_err(err)?;
    s.set_column_width(1, 20.0).map_err(err)?;
    let title = Format::new().set_bold().set_font_size(13);
    s.write_string_with_format(
        0,
        0,
        format!("BÁO CÁO TỔNG HỢP KHÁCH HÀNG — {}", today.format("%d/%m/%Y")),
        &title,
    )
    .map_err(err)?;
    let mut row = 2;
    let active = rows.iter().filter(|r| r.customer.is_active).count();
    let total_active: Decimal = rows.iter().map(|r| r.active_value).sum();
    let lifetime: Decimal = rows.iter().map(|r| r.lifetime_value).sum();
    let risk30: Decimal = rows.iter().map(|r| r.risk30).sum();
    let risk60: Decimal = rows.iter().map(|r| r.risk60).sum();
    section(s, &mut row, "TỔNG QUAN KHÁCH HÀNG", header)?;
    summary(s, &mut row, "Tổng số khách hàng", rows.len() as f64, None)?;
    summary(s, &mut row, "Đang hoạt động", active as f64, None)?;
    summary(s, &mut row, "Tạm ngưng", (rows.len() - active) as f64, None)?;
    summary(
        s,
        &mut row,
        "Chưa có hợp đồng nào",
        rows.iter().filter(|r| r.contracts.is_empty()).count() as f64,
        None,
    )?;
    section(s, &mut row, "PHÂN LOẠI THEO ƯU TIÊN", header)?;
    for t in 1..=5 {
        summary(
            s,
            &mut row,
            [
                "🔴 KHẨN",
                "🟠 Cần xử lý",
                "🟡 Upsell",
                "✅ Ổn định",
                "⚫ Không HĐ",
            ][t - 1],
            rows.iter().filter(|r| r.tier as usize == t).count() as f64,
            None,
        )?;
    }
    section(s, &mut row, "DOANH THU & RỦI RO", header)?;
    summary(
        s,
        &mut row,
        "Tổng giá trị HĐ đang hiệu lực (VNĐ)",
        total_active.to_f64().unwrap_or_default(),
        Some(money),
    )?;
    summary(
        s,
        &mut row,
        "Giá trị có nguy cơ mất trong 30 ngày",
        risk30.to_f64().unwrap_or_default(),
        Some(money),
    )?;
    summary(
        s,
        &mut row,
        "Giá trị có nguy cơ mất trong 60 ngày",
        risk60.to_f64().unwrap_or_default(),
        Some(money),
    )?;
    summary(
        s,
        &mut row,
        "Tổng giá trị lifetime (tất cả HĐ)",
        lifetime.to_f64().unwrap_or_default(),
        Some(money),
    )?;
    section(s, &mut row, "CƠ HỘI", header)?;
    summary(
        s,
        &mut row,
        "KH có thể upsell thêm sản phẩm",
        rows.iter().filter(|r| r.tier == 3).count() as f64,
        None,
    )?;
    for p in catalog {
        summary(
            s,
            &mut row,
            &format!("  Chỉ đang dùng {p}"),
            rows.iter().filter(|r| r.active_products == *p).count() as f64,
            None,
        )?;
    }
    summary(
        s,
        &mut row,
        "Đang dùng từ 2 sản phẩm trở lên",
        rows.iter()
            .filter(|r| r.active_products.split(" & ").count() >= 2 && r.active_products != "—")
            .count() as f64,
        None,
    )?;
    section(s, &mut row, "TOP 5 THEO GIÁ TRỊ HĐ ACTIVE", header)?;
    let mut top = rows.iter().collect::<Vec<_>>();
    top.sort_by_key(|row| std::cmp::Reverse(row.active_value));
    for r in top.into_iter().take(5) {
        summary(
            s,
            &mut row,
            &format!(
                "{} — {}",
                r.customer.code,
                r.customer
                    .short_name
                    .as_deref()
                    .unwrap_or(&r.customer.legal_name)
            ),
            r.active_value.to_f64().unwrap_or_default(),
            Some(money),
        )?;
    }
    Ok(())
}

fn headers(s: &mut Worksheet, values: &[&str], f: &Format) -> Result<(), ServiceError> {
    for (i, v) in values.iter().enumerate() {
        s.write_string_with_format(0, i as u16, *v, f)
            .map_err(err)?;
    }
    Ok(())
}
fn text(s: &mut Worksheet, r: u32, c: u16, v: &str, f: &Format) -> Result<(), ServiceError> {
    s.write_string_with_format(r, c, v, f).map_err(err)?;
    Ok(())
}
fn number(s: &mut Worksheet, r: u32, c: u16, v: f64, f: &Format) -> Result<(), ServiceError> {
    s.write_number_with_format(r, c, v, f).map_err(err)?;
    Ok(())
}
fn blank(s: &mut Worksheet, r: u32, c: u16, f: &Format) -> Result<(), ServiceError> {
    s.write_blank(r, c, f).map_err(err)?;
    Ok(())
}
fn date_cell(
    s: &mut Worksheet,
    r: u32,
    c: u16,
    d: NaiveDate,
    f: &Format,
) -> Result<(), ServiceError> {
    let date =
        ExcelDateTime::from_ymd(d.year() as u16, d.month() as u8, d.day() as u8).map_err(err)?;
    s.write_datetime_with_format(r, c, date, f).map_err(err)?;
    Ok(())
}
fn money_cell(
    s: &mut Worksheet,
    r: u32,
    c: u16,
    v: Decimal,
    f: &Format,
) -> Result<(), ServiceError> {
    if v.is_zero() {
        blank(s, r, c, f)
    } else {
        number(s, r, c, v.to_f64().unwrap_or_default(), f)
    }
}
fn section(
    s: &mut Worksheet,
    row: &mut u32,
    text: &str,
    format: &Format,
) -> Result<(), ServiceError> {
    s.merge_range(*row, 0, *row, 1, text, format).map_err(err)?;
    *row += 1;
    Ok(())
}
fn summary(
    s: &mut Worksheet,
    row: &mut u32,
    label: &str,
    value: f64,
    format: Option<&Format>,
) -> Result<(), ServiceError> {
    s.write_string(*row, 0, label).map_err(err)?;
    number(s, *row, 1, value, format.unwrap_or(&Format::new()))?;
    *row += 1;
    Ok(())
}
fn product_label(code: &str, name: &str) -> String {
    if !code.trim().is_empty() {
        code.trim().to_owned()
    } else {
        name.chars().take(10).collect()
    }
}
fn joined(set: &BTreeSet<String>) -> String {
    if set.is_empty() {
        "—".into()
    } else {
        set.iter().cloned().collect::<Vec<_>>().join(" & ")
    }
}
fn err(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Invalid(error.to_string())
}
