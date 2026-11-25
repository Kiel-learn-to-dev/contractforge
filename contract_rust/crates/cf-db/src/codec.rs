use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::types::{Value, ValueRef};
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::DbError;

pub fn read_decimal(value: ValueRef<'_>) -> Result<Option<Decimal>, DbError> {
    let parsed = match value {
        ValueRef::Null => return Ok(None),
        ValueRef::Integer(number) => Decimal::from(number),
        ValueRef::Real(number) => Decimal::from_str(&number.to_string())
            .map_err(|error| DbError::Codec(error.to_string()))?,
        ValueRef::Text(bytes) => Decimal::from_str(
            std::str::from_utf8(bytes).map_err(|error| DbError::Codec(error.to_string()))?,
        )
        .map_err(|error| DbError::Codec(error.to_string()))?,
        ValueRef::Blob(_) => return Err(DbError::Codec("Không thể đọc Decimal từ BLOB.".into())),
    };
    Ok(Some(parsed))
}

pub fn write_decimal(value: Option<Decimal>) -> Value {
    value.map_or(Value::Null, |decimal| {
        Value::Text(decimal.normalize().to_string())
    })
}

pub fn read_date(value: ValueRef<'_>) -> Result<Option<NaiveDate>, DbError> {
    let Some(text) = text_value(value)? else {
        return Ok(None);
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S"))
        .map(Some)
        .map_err(|error| DbError::Codec(error.to_string()))
}

pub fn write_date(value: Option<NaiveDate>) -> Value {
    value.map_or(Value::Null, |date| {
        Value::Text(date.format("%Y-%m-%d").to_string())
    })
}

pub fn read_datetime(value: ValueRef<'_>) -> Result<Option<NaiveDateTime>, DbError> {
    let Some(text) = text_value(value)? else {
        return Ok(None);
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S"))
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S%.f"))
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S"))
        .map(Some)
        .map_err(|error| DbError::Codec(error.to_string()))
}

pub fn write_datetime(value: Option<NaiveDateTime>) -> Value {
    value.map_or(Value::Null, |date| {
        Value::Text(date.format("%Y-%m-%d %H:%M:%S%.6f").to_string())
    })
}

fn text_value(value: ValueRef<'_>) -> Result<Option<String>, DbError> {
    match value {
        ValueRef::Null => Ok(None),
        ValueRef::Text(bytes) => String::from_utf8(bytes.to_vec())
            .map(Some)
            .map_err(|error| DbError::Codec(error.to_string())),
        ValueRef::Integer(number) => Ok(Some(number.to_string())),
        _ => Err(DbError::Codec("Giá trị không phải TEXT.".into())),
    }
}

pub fn read_bool(value: ValueRef<'_>) -> Result<Option<bool>, DbError> {
    match value {
        ValueRef::Null => Ok(None),
        ValueRef::Integer(0) => Ok(Some(false)),
        ValueRef::Integer(1) => Ok(Some(true)),
        ValueRef::Text(bytes) if bytes == b"0" => Ok(Some(false)),
        ValueRef::Text(bytes) if bytes == b"1" => Ok(Some(true)),
        _ => Err(DbError::Codec("Boolean phải là 0 hoặc 1.".into())),
    }
}

pub fn write_bool(value: Option<bool>) -> Value {
    value.map_or(Value::Null, |value| {
        Value::Integer(if value { 1 } else { 0 })
    })
}

pub fn opt_text(value: Option<&str>) -> Value {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map_or(Value::Null, |text| Value::Text(text.to_owned()))
}

pub fn read_opt_text(value: ValueRef<'_>) -> Result<Option<String>, DbError> {
    Ok(text_value(value)?.filter(|text| !text.trim().is_empty()))
}
