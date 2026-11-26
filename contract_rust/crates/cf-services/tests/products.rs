use cf_core::render_context::Feature;
use cf_db::store::Store;
use cf_services::products::{
    clean::{clean_form, feature_text, parse_feature_text},
    edit::{create_product, toggle_active, update_product},
    list::{in_service_counts, list_products},
};
use chrono::NaiveDateTime;
use serde_json::json;

fn seeded_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed");
    (dir, Store::load(db).expect("load"))
}

fn now() -> NaiveDateTime {
    NaiveDateTime::parse_from_str("2026-03-24 10:00:00", "%Y-%m-%d %H:%M:%S").expect("dt")
}

#[test]
fn list_products_sorts_by_group_main_first_sort_order_and_counts_in_service() {
    let (_dir, mut store) = seeded_store();
    // Add an ungrouped product to verify ungrouped goes last
    create_product(
        &mut store,
        &json!({
            "code": "ZZ-UNGROUPED",
            "name": "Gói lẻ",
            "product_type": "Dịch vụ",
            "kind": "main",
            "group_code": "",
        }),
        now(),
    )
    .expect("create ungrouped");

    let list = list_products(&store, false);
    let codes: Vec<_> = list.iter().map(|p| p.code.as_str()).collect();
    assert_eq!(codes, vec!["G1-MAIN", "G1-ADD", "G2-MAIN", "ZZ-UNGROUPED"]);

    // Seeded contracts: 1=Draft, 2=Generated, 3=Signed, 4=PaidActive, 5=Expired, all product_id=1.
    // IN_SERVICE = Invoiced, PaidActive, Active, ExpiringSoon -> id 4 -> count 1.
    let counts = in_service_counts(&store);
    assert_eq!(counts.get(&1).copied(), Some(1));
    assert_eq!(counts.get(&2).copied(), None);
}

#[test]
fn clean_form_collects_multiple_errors_and_validates_vat_and_addon_group() {
    let (_dir, store) = seeded_store();
    let empty_err = clean_form(&store, &json!({}), None).expect_err("empty form");
    let msg = empty_err.user_message();
    let lines: Vec<_> = msg.lines().collect();
    assert!(lines.len() >= 3);
    assert_eq!(lines[0], "Mã sản phẩm không được để trống.");
    assert_eq!(lines[1], "Tên sản phẩm không được để trống.");
    assert_eq!(lines[2], "Loại sản phẩm không được để trống.");

    let addon_err = clean_form(
        &store,
        &json!({
            "code": "ADD-1",
            "name": "Phụ lục",
            "product_type": "Dịch vụ",
            "kind": "addon",
            "group_code": "",
        }),
        None,
    )
    .expect_err("addon needs group");
    assert_eq!(
        addon_err.user_message(),
        "Gói bổ sung phải thuộc một nhóm để ghép với gói chính."
    );

    let vat_ok = clean_form(
        &store,
        &json!({
            "code": "VAT-OK",
            "name": "Gói 8.5",
            "product_type": "Dịch vụ",
            "default_vat_rate": "8.5",
        }),
        None,
    )
    .expect("8.5 vat valid");
    assert_eq!(
        vat_ok.default_vat_rate.map(|d| d.to_string()),
        Some("8.5".to_string())
    );

    let vat_err = clean_form(
        &store,
        &json!({
            "code": "VAT-BAD",
            "name": "Gói abc",
            "product_type": "Dịch vụ",
            "default_vat_rate": "abc",
        }),
        None,
    )
    .expect_err("abc vat invalid");
    assert_eq!(
        vat_err.user_message(),
        "Thuế suất phải là số, ví dụ 8 hoặc 8.5."
    );
}

#[test]
fn create_update_toggle_and_feature_text_roundtrip() {
    let (_dir, mut store) = seeded_store();
    let raw_features = "Tính năng A | Mô tả A\nTính năng B\n  \nTính năng C | Chi tiết C";
    let parsed = parse_feature_text(raw_features);
    assert_eq!(
        parsed,
        vec![
            Feature {
                name: "Tính năng A".into(),
                description: Some("Mô tả A".into())
            },
            Feature {
                name: "Tính năng B".into(),
                description: Some(String::new())
            },
            Feature {
                name: "Tính năng C".into(),
                description: Some("Chi tiết C".into())
            },
        ]
    );

    let id = create_product(
        &mut store,
        &json!({
            "code": "new-pkg",
            "name": "Gói Mới",
            "product_type": "Phần mềm",
            "group_code": "G3",
            "kind": "main",
            "default_price": "1.200.000",
            "default_vat_rate": "10",
            "feature_text": raw_features,
        }),
        now(),
    )
    .expect("create product");

    let created = &store.products[&id];
    assert_eq!(created.code, "NEW-PKG");
    assert_eq!(
        feature_text(created),
        "Tính năng A | Mô tả A\nTính năng B\nTính năng C | Chi tiết C"
    );

    update_product(
        &mut store,
        id,
        &json!({
            "code": "NEW-PKG",
            "name": "Gói Mới Sửa",
            "product_type": "Phần mềm",
            "group_code": "G3",
            "kind": "main",
        }),
        now(),
    )
    .expect("update product");
    assert_eq!(store.products[&id].name, "Gói Mới Sửa");

    let active = toggle_active(&mut store, id, now()).expect("toggle");
    assert!(!active);
    assert!(!store.products[&id].is_active);
}
