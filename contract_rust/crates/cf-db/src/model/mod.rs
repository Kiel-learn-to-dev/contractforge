pub mod contract;
pub mod customer;
pub mod product;
pub mod setting;
pub mod template;

pub use contract::{Contract, ContractEvent, ContractItem};
pub use customer::{Customer, CustomerDocument, CustomerUnit};
pub use product::Product;
pub use setting::{AppSetting, QuotationIssuer};
pub use template::ContractTemplate;

use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::{
    Row,
    types::{Value, ValueRef},
};
use rust_decimal::Decimal;

use crate::{DbError, codec};

pub(crate) fn text(row: &Row<'_>, column: &str) -> Result<Option<String>, DbError> {
    codec::read_opt_text(row.get_ref(column)?)
}

pub(crate) fn required_text(row: &Row<'_>, column: &str) -> Result<String, DbError> {
    Ok(text(row, column)?.unwrap_or_default())
}

pub(crate) fn integer(row: &Row<'_>, column: &str) -> Result<Option<i64>, DbError> {
    match row.get_ref(column)? {
        ValueRef::Null => Ok(None),
        ValueRef::Integer(value) => Ok(Some(value)),
        ValueRef::Text(value) => std::str::from_utf8(value)
            .ok()
            .and_then(|raw| raw.parse().ok())
            .map(Some)
            .ok_or_else(|| DbError::Codec(format!("Cột '{column}' không phải số nguyên."))),
        _ => Err(DbError::Codec(format!(
            "Cột '{column}' không phải số nguyên."
        ))),
    }
}

pub(crate) fn required_integer(row: &Row<'_>, column: &str) -> Result<i64, DbError> {
    integer(row, column)?.ok_or_else(|| DbError::Codec(format!("Cột '{column}' không được NULL.")))
}

pub(crate) fn boolean(row: &Row<'_>, column: &str) -> Result<bool, DbError> {
    Ok(codec::read_bool(row.get_ref(column)?)?.unwrap_or(false))
}

pub(crate) fn decimal(row: &Row<'_>, column: &str) -> Result<Option<Decimal>, DbError> {
    codec::read_decimal(row.get_ref(column)?)
}

pub(crate) fn date(row: &Row<'_>, column: &str) -> Result<Option<NaiveDate>, DbError> {
    codec::read_date(row.get_ref(column)?)
}

pub(crate) fn datetime(row: &Row<'_>, column: &str) -> Result<Option<NaiveDateTime>, DbError> {
    codec::read_datetime(row.get_ref(column)?)
}

pub(crate) fn text_value(value: Option<&str>) -> Value {
    codec::opt_text(value)
}
pub(crate) fn int_value(value: Option<i64>) -> Value {
    value.map_or(Value::Null, Value::Integer)
}
pub(crate) fn decimal_value(value: Option<Decimal>) -> Value {
    codec::write_decimal(value)
}
pub(crate) fn date_value(value: Option<NaiveDate>) -> Value {
    codec::write_date(value)
}
pub(crate) fn datetime_value(value: Option<NaiveDateTime>) -> Value {
    codec::write_datetime(value)
}
pub(crate) fn bool_value(value: bool) -> Value {
    codec::write_bool(Some(value))
}
