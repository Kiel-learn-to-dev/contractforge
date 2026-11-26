use cf_core::pricing::{self, ItemInput};
use cf_db::store::Store;
use cf_services::contracts::{
    options::product_options,
    preview::price_preview,
    suggest::{customer_facilities, suggest_number},
};

fn seeded_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed");
    (dir, Store::load(db).expect("load"))
}

#[test]
fn product_options_returns_empty_for_addon_and_falls_back_to_first_template() {
    let (_dir, mut store) = seeded_store();
    // Product 2 is addon -> empty options
    let addon_opts = product_options(&store, 2, 1, 3);
    assert!(addon_opts.main.is_none());
    assert!(addon_opts.addons.is_empty());

    // Product 1 is main in group G1; set default_template_id to a nonexistent template (999)
    store.products.get_mut(&1).expect("p1").default_template_id = Some(999);
    let opts = product_options(&store, 1, 1, 3);
    assert_eq!(opts.main.as_ref().map(|m| m.id), Some(1));
    assert_eq!(opts.addons.len(), 1);
    assert_eq!(opts.addons[0].id, 2);
    assert!(!opts.templates.contract.is_empty());
    assert_eq!(
        opts.templates.default_id,
        Some(opts.templates.contract[0].id)
    );
}

#[test]
fn price_preview_matches_core_sum_lines_and_rejects_zero_months() {
    let (_dir, store) = seeded_store();
    let err = price_preview(&store, &[], 0, true).expect_err("zero months");
    assert_eq!(err.user_message(), "Phải có số tháng để tính tiền.");

    let items = vec![ItemInput {
        product_id: 1,
        description: "Gói chính".into(),
        quantity: "2".into(),
        unit_price_pre_vat: None,
        vat_rate: None,
        amount_pre_vat_override: None,
        note: String::new(),
    }];
    let preview = price_preview(&store, &items, 12, true).expect("preview");
    let expected = pricing::sum_lines(&preview.lines).expect("sum");
    assert_eq!(preview.totals, expected);
}

#[test]
fn suggest_number_increments_existing_and_facilities_exclude_inactive_units() {
    let (_dir, mut store) = seeded_store();
    // Seeded customer 1 ("Đồng Tâm") has contracts 01/G1/2026, 02/G1/2026, 04/G1/2026
    // Product 1 has no custom format -> DEFAULT_FORMAT {seq}/{slug}/{year} -> slug of "Đồng Tâm"
    // Let's set product 1 contract_number_format to "{seq}/G1/{year}"
    store
        .products
        .get_mut(&1)
        .expect("p1")
        .contract_number_format = Some("{seq}/G1/{year}".into());
    // Customer 1 has 01/G1/2026, 02/G1/2026, 04/G1/2026 -> first available seq is 03
    let next = suggest_number(&store, 1, Some(1), 2026).expect("suggest");
    assert_eq!(next, "03/G1/2026");

    // Disable unit 2 -> should not appear in customer_facilities
    store.units.get_mut(&2).expect("u2").is_active = false;
    let facilities = customer_facilities(&store, Some(1));
    assert_eq!(facilities.total_units, 2); // main + unit 1
    assert!(facilities.facilities.iter().any(|f| f.key == "main"));
    assert!(facilities.facilities.iter().any(|f| f.key == "u1"));
    assert!(!facilities.facilities.iter().any(|f| f.key == "u2"));
}
