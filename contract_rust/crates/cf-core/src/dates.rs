//! Ngày tháng nghiệp vụ.

use chrono::{Datelike, NaiveDate};

use crate::error::CoreError;

/// Port của app/utils/date_helpers.py::add_months.
pub fn add_months(start: NaiveDate, months: i32) -> NaiveDate {
    let month_index = start.month0() as i64 + i64::from(months);
    let year = i64::from(start.year()) + month_index.div_euclid(12);
    let month = month_index.rem_euclid(12) as u32 + 1;
    let year = year.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(start);
    let last_day = (first + chrono::Months::new(1) - chrono::Duration::days(1)).day();
    NaiveDate::from_ymd_opt(year, month, start.day().min(last_day)).unwrap_or(start)
}

/// Port của app/utils/date_helpers.py::compute_end_date.
pub fn compute_end_date(start: NaiveDate, months: i32) -> NaiveDate {
    add_months(start, months) - chrono::Duration::days(1)
}

/// Port của app/utils/date_helpers.py::months_between.
pub fn months_between(start: NaiveDate, end: NaiveDate) -> i32 {
    let inclusive_end = end + chrono::Duration::days(1);
    (inclusive_end.year() - start.year()) * 12
        + (inclusive_end.month() as i32 - start.month() as i32)
}

/// Port của app/utils/date_helpers.py::parse_date_input.
pub fn parse_date_input(raw: Option<&str>) -> Result<Option<NaiveDate>, CoreError> {
    let Some(value) = raw.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    for separator in ["/", "-"] {
        let parts: Vec<_> = value.split(separator).collect();
        if parts.len() == 3
            && parts[2].len() == 2
            && let (Ok(day), Ok(month), Ok(short_year)) = (
                parts[0].parse::<u32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            )
        {
            let year = if short_year <= 68 {
                2000 + short_year as i32
            } else {
                1900 + short_year as i32
            };
            if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                return Ok(Some(date));
            }
        }
    }
    for format in ["%Y-%m-%d", "%d/%m/%Y", "%d-%m-%Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(value, format) {
            return Ok(Some(date));
        }
    }
    Err(CoreError::Invalid(format!(
        "Không nhận dạng được định dạng ngày: '{value}'"
    )))
}

/// Port của app/utils/date_helpers.py::format_date_vn.
pub fn format_date_vn(date: Option<NaiveDate>) -> String {
    date.map(|value| value.format("%d/%m/%Y").to_string())
        .unwrap_or_default()
}

/// Port của app/utils/date_helpers.py::split_date_parts.
pub fn split_date_parts(date: Option<NaiveDate>) -> (String, String, String) {
    date.map(|value| {
        (
            value.format("%d").to_string(),
            value.format("%m").to_string(),
            value.format("%Y").to_string(),
        )
    })
    .unwrap_or_default()
}
