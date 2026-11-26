use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    addenda::addon::{AddonForm, AddonItem, create_addendum},
    contracts::{delete::delete_contract, generate::generate_docx},
    customers::docs::{doc_path, upload_doc},
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
fn delete_contract_refuses_when_addenda_exist_and_keeps_customer_docs_intact() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());

    // Upload a customer doc for customer 1
    let doc_id =
        upload_doc(&mut ctx, 1, "cccd_front", "", b"%PDF doc", "cccd.pdf").expect("upload doc");
    let cdoc_path = doc_path(ctx.store(), dir.path(), 1, doc_id).expect("doc path");
    assert!(cdoc_path.is_file());

    // Generate contract 1 with attachment bundle
    let generated = generate_docx(&mut ctx, 1, &[2]).expect("generate");
    assert!(generated.file_path.is_file());
    let bundle_path = generated.bundle_path.expect("bundle");
    assert!(bundle_path.is_file());

    // Make contract 1 PaidActive + add an addendum -> deletion of contract 1 must be refused
    ctx.store_mut()
        .conn
        .execute(
            "UPDATE contracts SET status='PaidActive', start_date='2026-01-01', end_date='2026-12-31', month_count=12 WHERE id=1",
            [],
        )
        .expect("prepare parent");
    ctx.store_mut().reload().expect("reload");

    let child_id = create_addendum(
        &mut ctx,
        1,
        "addon",
        AddonForm {
            start_date: NaiveDate::from_ymd_opt(2026, 5, 1).expect("date"),
            sign_date: None,
            template_id: None,
            items: vec![AddonItem {
                product_id: 2,
                description: None,
                quantity: Some("1".into()),
                unit_price_pre_vat: None,
                vat_rate: None,
                amount_pre_vat_override: None,
                note: String::new(),
            }],
        },
    )
    .expect("create addendum");

    let refused = delete_contract(&mut ctx, 1).expect_err("refuse delete with addendum");
    assert!(refused.user_message().contains("còn 1 phụ lục"));

    // Delete child first, then parent -> owned files disappear, customer doc stays intact
    assert!(delete_contract(&mut ctx, child_id).expect("delete child"));
    assert!(delete_contract(&mut ctx, 1).expect("delete parent"));
    assert!(!ctx.store().contracts.contains_key(&1));
    assert!(!generated.file_path.exists());
    assert!(!bundle_path.exists());
    assert!(cdoc_path.is_file());
}
