use std::collections::BTreeMap;

use cf_core::pricing::ItemInput;
use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    quotations::{
        catalog::{generate_catalog_quotation, resolve_quotation_selection},
        manual::build_document,
    },
};
use chrono::{NaiveDate, NaiveDateTime};

fn context(root: &std::path::Path) -> Ctx {
    let db = cf_testkit::seeded_db(root).expect("seed");
    let store = Store::load(db).expect("load");
    let clock = FixedClock {
        date: NaiveDate::from_ymd_opt(2026, 3, 24).expect("date"),
        utc: NaiveDateTime::parse_from_str("2026-03-24 04:40:37", "%Y-%m-%d %H:%M:%S").expect("dt"),
        local: NaiveDateTime::parse_from_str("2026-03-24 11:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("dt"),
    };
    Ctx::new(store, DataPaths::new(root), Box::new(clock))
}

#[test]
fn catalog_quotation_validates_template_role_months_line1_and_builds_filename() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    let quote_date = NaiveDate::from_ymd_opt(2026, 3, 24).expect("date");

    // Template 1 is "Hợp đồng", not "Báo giá"
    let err = resolve_quotation_selection(ctx.store(), 1, 1, 1, quote_date, 12)
        .expect_err("contract template rejected");
    assert!(err.user_message().contains("không phải mẫu Báo giá"));

    // Month 121 rejected
    let err_months = resolve_quotation_selection(ctx.store(), 1, 1, 4, quote_date, 121)
        .expect_err("121 months rejected");
    assert_eq!(err_months.user_message(), "Số tháng phải từ 1 đến 120.");

    // Legacy context has TOTAL_AMOUNT & PRICING_DESCRIPTION
    let sel =
        resolve_quotation_selection(ctx.store(), 1, 1, 4, quote_date, 12).expect("valid selection");
    assert!(sel.context.contains_key("TOTAL_AMOUNT"));
    assert!(sel.context.contains_key("PRICING_DESCRIPTION"));

    // Itemized line 1 has different product_id (3 instead of 1) -> error
    let mismatch = generate_catalog_quotation(
        &mut ctx,
        1,
        1,
        4,
        quote_date,
        12,
        &[ItemInput {
            product_id: 3,
            description: "Khác gói".into(),
            quantity: "1".into(),
            unit_price_pre_vat: None,
            vat_rate: None,
            amount_pre_vat_override: None,
            note: String::new(),
        }],
    )
    .expect_err("product mismatch");
    assert_eq!(
        mismatch.user_message(),
        "Gói chính trong dòng báo giá không khớp với sản phẩm đã chọn."
    );

    // Valid itemized generation -> filename BaoGia_{code}_{name}_{YYYYMMDD}.docx
    let generated = generate_catalog_quotation(
        &mut ctx,
        1,
        1,
        4,
        quote_date,
        12,
        &[ItemInput {
            product_id: 1,
            description: "Gói chính G1".into(),
            quantity: "1".into(),
            unit_price_pre_vat: None,
            vat_rate: None,
            amount_pre_vat_override: None,
            note: String::new(),
        }],
    )
    .expect("generate quotation");
    assert!(generated.file_name.starts_with("BaoGia_G1-MAIN_"));
    assert!(generated.file_name.ends_with("_20260324.docx"));
    assert!(!generated.bytes.is_empty());
}

#[test]
fn manual_quotation_matches_docx_render_fixture_and_rejects_101_items() {
    let fixture = cf_testkit::fixture("docx_render");
    let case = &fixture["manual_quotation"][0];
    let values: BTreeMap<String, String> = case["values"]
        .as_object()
        .expect("values")
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
        .collect();
    let expected_ctx = case["context"].as_object().expect("context");
    let expected_items = case["lists"]["items"].as_array().expect("items");

    let doc = build_document(&values, &[]).expect("build manual quotation");
    assert_eq!(doc.file_name, "BaoGia_features_05/01/2026.docx");
    for (k, v) in expected_ctx {
        assert_eq!(
            doc.context.get(k).map(String::as_str),
            v.as_str(),
            "mismatch at {k}"
        );
    }
    let items = &doc.lists["items"];
    assert_eq!(items.len(), expected_items.len());
    for (idx, exp) in expected_items.iter().enumerate() {
        for (k, v) in exp.as_object().unwrap() {
            assert_eq!(
                items[idx].get(k).map(String::as_str),
                v.as_str(),
                "item {idx} mismatch at {k}"
            );
        }
    }

    // 101 items -> error
    let many = (1..=101)
        .map(|i| serde_json::json!({"name": format!("Item {i}"), "quantity": "1", "periods": "1", "price": "1000", "vat": "0"}))
        .collect::<Vec<_>>();
    let mut bad_values = values;
    bad_values.insert("items_json".into(), serde_json::to_string(&many).unwrap());
    let err = build_document(&bad_values, &[]).expect_err("101 items rejected");
    assert_eq!(err.user_message(), "Báo giá cần từ 1 đến 100 hạng mục.");
}
