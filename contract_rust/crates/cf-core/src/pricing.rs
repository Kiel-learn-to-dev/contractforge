//! Quy tắc định giá.

use crate::{
    dates::add_months,
    error::CoreError,
    fmt::{fmt_currency, fmt_money},
    money::{round_half_up, to_decimal},
    words::amount_to_words_dec,
};
use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};

/// Một dòng giá trị đã tính thuế.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub description: String,
    pub unit_label: String,
    pub quantity: Decimal,
    pub period_count: Decimal,
    pub unit_price_pre_vat: Decimal,
    pub vat_rate: Decimal,
    pub amount_pre_vat: Decimal,
    pub vat_amount: Decimal,
    pub amount_total: Decimal,
    pub product_id: Option<i64>,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct LineInput {
    pub description: String,
    pub unit_label: String,
    pub quantity: Decimal,
    pub months: Decimal,
    pub unit_price_pre_vat: Decimal,
    pub vat_rate: Decimal,
    pub price_period_months: Decimal,
    pub amount_pre_vat_override: Option<Decimal>,
    pub product_id: Option<i64>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Totals {
    pub pre_vat: Decimal,
    pub vat: Decimal,
    pub total: Decimal,
    pub amount_text: String,
}

/// Port của app/services/pricing.py::normalize_quantity.
pub fn normalize_quantity(
    raw: &str,
    min_quantity: Option<Decimal>,
    step: Option<Decimal>,
) -> Result<Decimal, CoreError> {
    let quantity = to_decimal(raw, "số lượng")?;
    if quantity <= Decimal::ZERO {
        return Err(CoreError::Invalid("Số lượng phải lớn hơn 0.".into()));
    }
    let mut quantity = min_quantity
        .filter(|minimum| *minimum != Decimal::ZERO)
        .map_or(quantity, |minimum| quantity.max(minimum));
    if let Some(step) = step.filter(|value| *value != Decimal::ZERO) {
        let steps =
            (quantity / step).round_dp_with_strategy(0, RoundingStrategy::ToPositiveInfinity);
        quantity = steps * step;
    }
    Ok(quantity)
}

/// Port của app/services/pricing.py::period_count.
pub fn period_count(months: &str, price_period_months: Option<&str>) -> Result<Decimal, CoreError> {
    let months = to_decimal(months, "số tháng")?;
    let period = match price_period_months.filter(|value| !value.is_empty()) {
        Some(raw) => to_decimal(raw, "kỳ giá")?,
        None => Decimal::ONE,
    };
    let period = if period.is_zero() {
        Decimal::ONE
    } else {
        period
    };
    if months <= Decimal::ZERO || period <= Decimal::ZERO {
        return Err(CoreError::Invalid(
            "Số tháng và kỳ giá phải lớn hơn 0.".into(),
        ));
    }
    Ok(months / period)
}

/// Port của app/services/pricing.py::price_line.
pub fn price_line(input: LineInput) -> Result<Line, CoreError> {
    let qty = input.quantity;
    let price = input.unit_price_pre_vat;
    let vat = input.vat_rate;
    if qty <= Decimal::ZERO {
        return Err(CoreError::Invalid("Số lượng phải lớn hơn 0.".into()));
    }
    if price < Decimal::ZERO {
        return Err(CoreError::Invalid("Đơn giá không được âm.".into()));
    }
    if !(Decimal::ZERO..=Decimal::from(100)).contains(&vat) {
        return Err(CoreError::Invalid(
            "Thuế suất phải trong khoảng 0–100.".into(),
        ));
    }
    let price_period = if input.price_period_months.is_zero() {
        Decimal::ONE
    } else {
        input.price_period_months
    };
    if input.months <= Decimal::ZERO || price_period <= Decimal::ZERO {
        return Err(CoreError::Invalid(
            "Số tháng và kỳ giá phải lớn hơn 0.".into(),
        ));
    }
    let period = input.months / price_period;
    let pre = if let Some(override_amount) = input.amount_pre_vat_override {
        if input.note.trim().is_empty() {
            return Err(CoreError::Invalid(
                "Ghi đè thành tiền phải kèm ghi chú lý do.".into(),
            ));
        }
        round_half_up(override_amount)
    } else {
        round_half_up(price * qty * period)
    };
    let vat_amount = round_half_up(pre * vat / Decimal::from(100));
    Ok(Line {
        description: input.description,
        unit_label: input.unit_label,
        quantity: qty,
        period_count: period,
        unit_price_pre_vat: price,
        vat_rate: vat,
        amount_pre_vat: pre,
        vat_amount,
        amount_total: pre + vat_amount,
        product_id: input.product_id,
        note: input.note,
    })
}

/// Port của app/services/pricing.py::sum_lines.
pub fn sum_lines(lines: &[Line]) -> Result<Totals, CoreError> {
    if lines.is_empty() {
        return Err(CoreError::Invalid(
            "Hợp đồng phải có ít nhất một dòng.".into(),
        ));
    }
    let pre_vat: Decimal = lines.iter().map(|line| line.amount_pre_vat).sum();
    let vat: Decimal = lines.iter().map(|line| line.vat_amount).sum();
    let total = pre_vat + vat;
    let amount_text = if total.is_zero() {
        String::new()
    } else {
        amount_to_words_dec(total)?
    };
    Ok(Totals {
        pre_vat,
        vat,
        total,
        amount_text,
    })
}

/// Port của app/services/pricing.py::unit_price_with_vat.
pub fn unit_price_with_vat(price: Decimal, vat_rate: Decimal) -> Decimal {
    round_half_up(price * (Decimal::ONE + vat_rate / Decimal::from(100)))
}

/// Port của app/services/pricing.py::describe.
pub fn describe(lines: &[Line], totals: &Totals) -> String {
    let parts = lines
        .iter()
        .map(|line| format!("{}: {} VNĐ", line.description, fmt_money(line.amount_total)))
        .collect::<Vec<_>>();
    format!(
        "{}. Tổng cộng: {} VNĐ",
        parts.join("; "),
        fmt_money(totals.total)
    )
}

/// Port của app/services/pricing.py::addon_months.
pub fn addon_months(start: NaiveDate, main_end: NaiveDate) -> Result<i32, CoreError> {
    if start > main_end {
        return Err(CoreError::Invalid(
            "Ngày bắt đầu gói bổ sung sau ngày hết hạn gói chính.".into(),
        ));
    }
    let mut months = 1;
    while add_months(start, months) <= main_end {
        months += 1;
        if months > 600 {
            return Err(CoreError::Invalid("Khoảng thời gian không hợp lệ.".into()));
        }
    }
    Ok(months)
}

#[derive(Debug, Clone, Default)]
pub struct LegacyAmounts {
    pub unit_count: Option<Decimal>,
    pub month_count: Option<Decimal>,
    pub unit_price_pre_vat: Option<Decimal>,
    pub vat_rate: Option<Decimal>,
    pub unit_price_vat: Option<Decimal>,
    pub quantity: Option<Decimal>,
    pub total_amount: Option<Decimal>,
    pub amount_text: String,
}

/// Tính các trường tiền legacy theo thứ tự nhánh của bản Python; giữ nguyên Decimal, không làm tròn.
pub fn legacy_amounts(
    unit_count: Option<Decimal>,
    month_count: Option<Decimal>,
    unit_price_pre_vat: Option<Decimal>,
    vat_rate: Option<Decimal>,
    unit_price_vat: Option<Decimal>,
    quantity: Option<Decimal>,
    total_amount: Option<Decimal>,
) -> LegacyAmounts {
    let mut result = LegacyAmounts {
        unit_count,
        month_count,
        unit_price_pre_vat,
        vat_rate,
        unit_price_vat,
        quantity,
        total_amount,
        amount_text: String::new(),
    };
    if result.unit_price_vat.is_none()
        && let (Some(price), Some(vat)) = (result.unit_price_pre_vat, result.vat_rate)
    {
        result.unit_price_vat = Some(price * (Decimal::ONE + vat / Decimal::from(100)));
    }
    if result.quantity.is_none()
        && let (Some(units), Some(months)) = (result.unit_count, result.month_count)
    {
        result.quantity = Some(units * months);
    }
    if result.total_amount.is_none()
        && let (Some(price), Some(quantity)) = (result.unit_price_vat, result.quantity)
    {
        result.total_amount = Some(price * quantity);
    }
    if let Some(total) = result.total_amount {
        result.amount_text = amount_to_words_dec(total).unwrap_or_default();
    }
    result
}

/// Port của app/services/batch_generator.py::_auto_pricing_description.
pub fn auto_pricing_description(
    unit_price_vat: Option<Decimal>,
    unit_count: i64,
    unit_names: &[String],
    month_count: Option<i64>,
) -> String {
    let (Some(price), Some(months)) = (
        unit_price_vat.filter(|value| !value.is_zero()),
        month_count.filter(|value| *value != 0),
    ) else {
        return String::new();
    };
    let names = if unit_names.len() > 1 {
        format!(" ({})", unit_names.join("; "))
    } else {
        String::new()
    };
    let total = price * Decimal::from(unit_count) * Decimal::from(months);
    format!(
        "Số tiền = Đơn giá có VAT ({} VNĐ) × {} cơ sở{} × {} tháng = {} VNĐ",
        fmt_currency(Some(price)),
        unit_count,
        names,
        months,
        fmt_currency(Some(total))
    )
}

#[derive(Debug, Clone)]
pub struct ProductRule {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub group_code: Option<String>,
    pub is_active: bool,
    pub unit_label: String,
    pub default_price: Option<Decimal>,
    pub default_vat_rate: Option<Decimal>,
    pub price_period_months: Option<Decimal>,
    pub min_quantity: Option<Decimal>,
    pub quantity_step: Option<Decimal>,
    pub quantity_source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ItemInput {
    pub product_id: i64,
    pub description: String,
    pub quantity: String,
    pub unit_price_pre_vat: Option<String>,
    pub vat_rate: Option<String>,
    pub amount_pre_vat_override: Option<String>,
    pub note: String,
}

/// Port của app/services/contract_service.py::_default_quantity.
pub fn default_quantity(
    product: &ProductRule,
    record_count: Option<i64>,
    unit_count: i64,
) -> String {
    match product.quantity_source.as_deref().unwrap_or("units") {
        "units" => Decimal::from(unit_count.max(1)).to_string(),
        "records" => record_count
            .filter(|count| *count != 0)
            .and_then(|count| {
                normalize_quantity(
                    &count.to_string(),
                    product.min_quantity,
                    product.quantity_step,
                )
                .ok()
            })
            .map_or_else(String::new, |value| value.normalize().to_string()),
        _ => "1".into(),
    }
}

/// Port của app/services/contract_service.py::_build_lines.
pub fn build_lines(
    items: &[ItemInput],
    products: impl Fn(i64) -> Option<ProductRule>,
    month_count: i64,
    require_main: bool,
) -> Result<Vec<Line>, CoreError> {
    let mut lines = Vec::with_capacity(items.len());
    let mut main_group: Option<String> = None;
    for (index, item) in items.iter().enumerate() {
        let row = index + 1;
        let product = products(item.product_id).ok_or_else(|| {
            CoreError::Invalid(format!(
                "Dòng {row}: gói {} không tồn tại.",
                item.product_id
            ))
        })?;
        if !product.is_active {
            return Err(CoreError::Invalid(format!(
                "Dòng {row}: gói '{}' đã ngừng bán.",
                product.name
            )));
        }
        if require_main && row == 1 {
            if product.kind != "main" {
                return Err(CoreError::Invalid("Dòng 1 phải là gói chính.".into()));
            }
            main_group = product.group_code.clone();
        } else {
            if product.kind == "main" {
                let message = if require_main {
                    format!("Dòng {row}: chỉ dòng đầu tiên được là gói chính.")
                } else {
                    format!(
                        "Dòng {row}: chỉ nhận gói bổ sung cùng nhóm với gói chính của hợp đồng gốc."
                    )
                };
                return Err(CoreError::Invalid(message));
            }
            if require_main && (product.group_code.is_none() || product.group_code != main_group) {
                return Err(CoreError::Invalid(format!(
                    "Dòng {row}: gói bổ sung phải cùng nhóm với gói chính."
                )));
            }
        }
        let build = || -> Result<Line, CoreError> {
            let quantity =
                normalize_quantity(&item.quantity, product.min_quantity, product.quantity_step)?;
            let price = item
                .unit_price_pre_vat
                .as_deref()
                .filter(|raw| !raw.trim().is_empty())
                .map(|raw| to_decimal(raw, "đơn giá"))
                .unwrap_or_else(|| {
                    product
                        .default_price
                        .ok_or_else(|| CoreError::Invalid("Thiếu đơn giá.".into()))
                })?;
            let vat = item
                .vat_rate
                .as_deref()
                .filter(|raw| !raw.trim().is_empty())
                .map(|raw| to_decimal(raw, "thuế suất"))
                .unwrap_or(Ok(product.default_vat_rate.unwrap_or(Decimal::ZERO)))?;
            let override_amount = item
                .amount_pre_vat_override
                .as_deref()
                .filter(|raw| !raw.is_empty())
                .map(|raw| to_decimal(raw, "thành tiền"))
                .transpose()?;
            price_line(LineInput {
                description: if item.description.trim().is_empty() {
                    product.name.clone()
                } else {
                    item.description.trim().to_owned()
                },
                unit_label: product.unit_label.clone(),
                quantity,
                months: Decimal::from(month_count),
                unit_price_pre_vat: price,
                vat_rate: vat,
                price_period_months: product
                    .price_period_months
                    .filter(|value| !value.is_zero())
                    .unwrap_or(Decimal::ONE),
                amount_pre_vat_override: override_amount,
                product_id: Some(product.id),
                note: item.note.trim().to_owned(),
            })
        };
        lines.push(build().map_err(|error| CoreError::Invalid(format!("Dòng {row}: {error}")))?);
    }
    Ok(lines)
}
