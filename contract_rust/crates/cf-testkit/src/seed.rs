use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use rusqlite::Connection;

/// Tạo DB tạm với dữ liệu hư cấu và một bản sao mẫu DOCX.
pub fn seeded_db(directory: impl AsRef<Path>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let directory = directory.as_ref();
    fs::create_dir_all(directory)?;
    let db_path = directory.join("contract_manager.db");
    let assets = directory.join("assets");
    fs::create_dir_all(&assets)?;
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("ContractForge")
        .join("assets")
        .join("default_templates")
        .join("sample_service_contract.docx");
    let template_copy = assets.join("sample_service_contract.docx");
    fs::copy(source, &template_copy)?;

    let mut connection = Connection::open(&db_path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    let transaction = connection.transaction()?;
    transaction.execute_batch(include_str!("../../../fixtures/schema_baseline.sql"))?;
    let now = Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S%.6f")
        .to_string();
    transaction.execute("INSERT INTO products (id, code, name, product_type, default_price, default_vat_rate, is_active, group_code, kind, unit_label, price_period_months, quantity_source, min_quantity, quantity_step, sort_order, created_at, updated_at) VALUES (1, 'G1-MAIN', 'Gói chính G1', 'Dịch vụ', 440909, 10, 1, 'G1', 'main', 'cơ sở', 1, 'units', 1, 1, 0, ?1, ?1)", [&now])?;
    transaction.execute("INSERT INTO products (id, code, name, product_type, default_price, default_vat_rate, is_active, group_code, kind, unit_label, price_period_months, quantity_source, min_quantity, quantity_step, sort_order, created_at, updated_at) VALUES (2, 'G1-ADD', 'Gói bổ sung G1', 'Dịch vụ', 100000, 10, 1, 'G1', 'addon', 'cơ sở', 1, 'units', 1, 1, 1, ?1, ?1)", [&now])?;
    transaction.execute("INSERT INTO products (id, code, name, product_type, default_price, default_vat_rate, is_active, group_code, kind, unit_label, price_period_months, quantity_source, min_quantity, quantity_step, sort_order, created_at, updated_at) VALUES (3, 'G2-MAIN', 'Gói chính G2', 'Dịch vụ', 250000, 10, 1, 'G2', 'main', 'cơ sở', 1, 'units', 1, 1, 2, ?1, ?1)", [&now])?;

    for (id, code, name, active, deleted) in [
        (1_i64, "KH-FAKE-001", "Đồng Tâm", 1_i64, 0_i64),
        (2, "KH-FAKE-002", "Bình An", 0, 0),
        (3, "KH-FAKE-003", "Tân Lộc", 1, 1),
    ] {
        transaction.execute("INSERT INTO customers (id, code, legal_name, short_name, customer_type, address, ward, province, is_active, is_deleted, bill_main_station, created_at, updated_at) VALUES (?1, ?2, ?3, ?3, 'Trường học', 'Địa chỉ mẫu', 'Phường Mẫu', 'Tỉnh Mẫu', ?4, ?5, 1, ?6, ?6)", rusqlite::params![id, code, name, active, deleted, now])?;
    }
    for (id, name) in [(1_i64, "Điểm mẫu A"), (2, "Điểm mẫu B")] {
        transaction.execute("INSERT INTO customer_units (id, customer_id, name, unit_type, is_active, include_in_billing, sort_order, created_at, updated_at) VALUES (?1, 1, ?2, 'Điểm trạm', 1, 1, ?1, ?3, ?3)", rusqlite::params![id, name, now])?;
    }

    for (id, code, name, kind, group) in [
        (
            1_i64,
            "TPL-G1-CONTRACT",
            "Mẫu hợp đồng G1",
            "Hợp đồng",
            Some("G1"),
        ),
        (
            2,
            "TPL-G1-ACCEPT",
            "Mẫu nghiệm thu G1",
            "Nghiệm thu",
            Some("G1"),
        ),
        (
            3,
            "TPL-ADDON",
            "Mẫu phụ lục bổ sung",
            "Phụ lục bổ sung",
            None,
        ),
        (4, "TPL-G1-QUOTE", "Mẫu báo giá G1", "Báo giá", Some("G1")),
    ] {
        transaction.execute("INSERT INTO contract_templates (id, code, name, contract_type, product_group, file_path, file_name, mode, detected_placeholders, field_mapping, is_active, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'assets/sample_service_contract.docx', 'sample_service_contract.docx', 'placeholder', '[]', '{}', 1, ?6, ?6)", rusqlite::params![id, code, name, kind, group, now])?;
    }
    for (id, number, status, customer) in [
        (1_i64, "01/G1/2026", "Draft", 1_i64),
        (2, "02/G1/2026", "Generated", 1),
        (3, "03/G1/2026", "Signed", 2),
        (4, "04/G1/2026", "PaidActive", 1),
        (5, "05/G1/2026", "Expired", 3),
    ] {
        transaction.execute("INSERT INTO contracts (id, contract_number, status, customer_id, product_id, template_id, pricing_model, doc_kind, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 1, 1, 'items', 'contract', ?5, ?5)", rusqlite::params![id, number, status, customer, now])?;
    }
    transaction.commit()?;
    Ok(db_path)
}
