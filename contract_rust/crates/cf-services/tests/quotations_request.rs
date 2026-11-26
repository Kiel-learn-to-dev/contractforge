use cf_db::{model::Customer, store::Store};

#[test]
fn request_context_and_lists_contain_no_money_fields() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
    let store = Store::load(db).expect("load database");
    let customer = store.customers.get(&1).expect("customer");
    let units = store
        .units
        .values()
        .filter(|unit| unit.customer_id == 1)
        .cloned()
        .collect::<Vec<_>>();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 24).expect("date");
    let context = cf_services::quotations::request::build_request_context(
        customer,
        &units,
        cf_services::quotations::request::RequestContextInput {
            request_date: date,
            deadline: None,
            month_count: 12,
            addressee: "",
            subject: "Chào giá",
            note: "Ghi chú",
        },
    );
    assert!(!context.keys().any(|key| {
        ["PRICE", "AMOUNT", "TOTAL", "VAT"]
            .iter()
            .any(|part| key.contains(part))
    }));
    assert_eq!(
        context.get("REQUEST_ADDRESSEE").map(String::as_str),
        Some(cf_services::quotations::request::ADDRESSEE_BLANK)
    );
}

#[test]
fn request_templates_require_request_quote_role() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
    let store = Store::load(db).expect("load database");
    let error = cf_services::quotations::request::resolve_request_template(&store, 1, Some("G1"))
        .expect_err("contract template must fail");
    assert!(error.user_message().contains("không dùng được ở đây"));
    let _customer_type: Option<&Customer> = store.customers.get(&1);
}

#[test]
fn installing_sample_request_templates_twice_does_not_duplicate_them() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
    let store = Store::load(db).expect("load database");
    let clock = cf_db::clock::FixedClock {
        date: chrono::NaiveDate::from_ymd_opt(2026, 3, 24).expect("date"),
        utc: chrono::NaiveDateTime::parse_from_str("2026-03-24 04:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("time"),
        local: chrono::NaiveDateTime::parse_from_str("2026-03-24 11:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("time"),
    };
    let mut ctx = cf_services::Ctx::new(
        store,
        cf_db::paths::DataPaths::new(dir.path()),
        Box::new(clock),
    );
    let installed = cf_services::quotations::request::install_sample_templates(&mut ctx)
        .expect("install samples");
    assert_eq!(installed.len(), 3);
    assert!(
        cf_services::quotations::request::install_sample_templates(&mut ctx)
            .expect("install samples again")
            .is_empty()
    );
    assert_eq!(
        ctx.store()
            .templates
            .values()
            .filter(|template| template.code.starts_with("SAMPLE-RFQ-"))
            .count(),
        3
    );
    let customer = ctx.store().customers.get(&1).expect("customer");
    let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 24).expect("date");
    assert_eq!(
        cf_services::quotations::request::request_filename(customer, date),
        "YeuCauBaoGia_Đồng_Tâm_20260324.docx"
    );
    let template = ctx.store().templates.get(&installed[0]).expect("template");
    let rendered = cf_services::quotations::request::render_request_docx(
        ctx.paths(),
        template,
        &std::collections::BTreeMap::from([("PARTY_A_NAME".into(), "Khách mẫu".into())]),
        &cf_services::quotations::request::RequestOptions {
            items: vec![cf_services::quotations::request::RequestRow {
                item_no: "1".into(),
                item_name: "Gói thử".into(),
                item_unit: "cơ sở".into(),
                item_qty: "12".into(),
            }],
            features: vec![],
        },
    )
    .expect("render request docx");
    assert!(cf_docx::Package::read(&rendered).is_ok());
}
