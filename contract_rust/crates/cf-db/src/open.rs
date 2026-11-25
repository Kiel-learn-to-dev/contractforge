use chrono::Utc;
use rusqlite::Connection;
use serde::Deserialize;

use crate::{DbError, schema};

#[derive(Deserialize)]
struct SeedProduct {
    code: String,
    name: String,
    product_type: String,
    default_price: i64,
    default_vat_rate: i64,
    default_duration_months: i64,
    kind: String,
    price_period_months: i64,
    quantity_source: String,
    is_active: i64,
    sort_order: i64,
}

#[derive(Deserialize)]
struct SeedFixture {
    seed_products: Vec<SeedProduct>,
}

/// Mở DB, bật foreign keys và timeout; DB mới được tạo từ baseline rồi seed sản phẩm.
pub fn open(path: impl AsRef<std::path::Path>) -> Result<Connection, DbError> {
    let path = path.as_ref();
    let is_new = !path.exists();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| DbError::Schema(error.to_string()))?;
    }
    let mut connection = Connection::open(path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.busy_timeout(std::time::Duration::from_millis(5000))?;
    if is_new {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("../../../fixtures/schema_baseline.sql"))?;
        let seed: SeedFixture = serde_json::from_str(include_str!("../../../fixtures/schema.json"))
            .map_err(|error| DbError::Schema(error.to_string()))?;
        let now = Utc::now()
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S%.6f")
            .to_string();
        for product in seed.seed_products {
            transaction.execute(
                "INSERT INTO products (code, name, product_type, default_price, default_vat_rate, default_duration_months, kind, price_period_months, quantity_source, is_active, sort_order, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                rusqlite::params![product.code, product.name, product.product_type, product.default_price, product.default_vat_rate, product.default_duration_months, product.kind, product.price_period_months, product.quantity_source, product.is_active, product.sort_order, now],
            )?;
        }
        transaction.commit()?;
    }
    schema::check_schema(&connection)?;
    Ok(connection)
}
