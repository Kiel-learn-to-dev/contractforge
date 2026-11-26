use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    contracts::create::{ContractForm, CreateOptions, create_draft},
};
use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;

fn context(dir: &tempfile::TempDir) -> Ctx {
    let db = cf_testkit::seeded_db(dir.path()).expect("seed test db");
    let store = Store::load(db).expect("load store");
    let clock = FixedClock {
        date: NaiveDate::from_ymd_opt(2026, 3, 24).expect("valid date"),
        utc: NaiveDateTime::parse_from_str("2026-03-24 04:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("valid timestamp"),
        local: NaiveDateTime::parse_from_str("2026-03-24 11:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("valid timestamp"),
    };
    Ctx::new(store, DataPaths::new(dir.path()), Box::new(clock))
}

#[test]
fn creates_items_contract_and_event_in_single_batch() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut ctx = context(&dir);
    let form = ContractForm {
        contract_number: "06/G1/2026".into(),
        customer_id: 1,
        product_id: Some(1),
        template_id: None,
        start_date: NaiveDate::from_ymd_opt(2026, 1, 1),
        end_date: NaiveDate::from_ymd_opt(2026, 12, 31),
        month_count: Some(12),
        items: vec![cf_core::pricing::ItemInput {
            product_id: 1,
            description: "Gói chính G1".into(),
            quantity: "2".into(),
            unit_price_pre_vat: None,
            vat_rate: None,
            amount_pre_vat_override: None,
            note: String::new(),
        }],
        ..ContractForm::default()
    };
    let id = create_draft(&mut ctx, form, CreateOptions::default()).expect("create draft");
    let contract = ctx.store().contracts.get(&id).expect("contract reloaded");
    assert_eq!(contract.status.as_str(), "Draft");
    assert_eq!(contract.pricing_model, "items");
    assert_eq!(contract.total_amount, Some(Decimal::from(11_639_998)));
    assert_eq!(
        contract.amount_text.as_deref(),
        Some("Mười một triệu, sáu trăm ba mươi chín nghìn, chín trăm chín mươi tám đồng")
    );
    assert_eq!(contract.created_at.to_string(), "2026-03-24 04:40:37");
    assert_eq!(
        ctx.store().indexes.items_by_contract.get(&id).map(Vec::len),
        Some(1)
    );
    assert!(
        ctx.store()
            .events
            .values()
            .any(|event| event.contract_id == id && event.event_type == "created")
    );
}

#[test]
fn item_totals_override_legacy_values_but_keep_user_text_and_clean_units() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut ctx = context(&dir);
    let id = create_draft(
        &mut ctx,
        ContractForm {
            contract_number: "07/G1/2026".into(),
            customer_id: 1,
            start_date: NaiveDate::from_ymd_opt(2026, 1, 1),
            end_date: NaiveDate::from_ymd_opt(2026, 12, 31),
            month_count: Some(12),
            unit_price_pre_vat: Some(Decimal::from(1)),
            vat_rate: Some(Decimal::ZERO),
            quantity: Some(Decimal::ONE),
            total_amount: Some(Decimal::from(999)),
            amount_text: Some("Số tiền do người dùng nhập".into()),
            payment_mode: Some("  PREPAID  ".into()),
            selected_unit_names: vec![" Trạm A ".into(), " ".into(), "Trạm B".into()],
            items: vec![cf_core::pricing::ItemInput {
                product_id: 1,
                description: String::new(),
                quantity: "1".into(),
                unit_price_pre_vat: None,
                vat_rate: None,
                amount_pre_vat_override: None,
                note: String::new(),
            }],
            ..ContractForm::default()
        },
        CreateOptions::default(),
    )
    .expect("create contract");
    let c = &ctx.store().contracts[&id];
    assert_ne!(c.total_amount, Some(Decimal::from(999)));
    assert_eq!(c.amount_text.as_deref(), Some("Số tiền do người dùng nhập"));
    assert_eq!(c.payment_mode.as_deref(), Some("prepaid"));
    assert_eq!(c.selected_unit_names, vec!["Trạm A", "Trạm B"]);
}

#[test]
fn duplicate_contract_number_is_rejected_without_changes() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut ctx = context(&dir);
    let before = ctx.store().contracts.len();
    let error = create_draft(
        &mut ctx,
        ContractForm {
            contract_number: "01/G1/2026".into(),
            customer_id: 1,
            ..ContractForm::default()
        },
        CreateOptions::default(),
    )
    .expect_err("duplicate number");
    assert_eq!(error.user_message(), "Số hợp đồng '01/G1/2026' đã tồn tại.");
    assert_eq!(ctx.store().contracts.len(), before);
}

#[test]
fn legacy_contract_computes_legacy_amounts_and_wrong_template_group_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(&dir);
    let id = create_draft(
        &mut ctx,
        ContractForm {
            contract_number: "08/G1/2026".into(),
            customer_id: 1,
            product_id: Some(1),
            month_count: Some(12),
            unit_count: Some(2),
            unit_price_pre_vat: Some(Decimal::from(440_909)),
            vat_rate: Some(Decimal::from(10)),
            ..ContractForm::default()
        },
        CreateOptions {
            require_main: false,
            ..CreateOptions::default()
        },
    )
    .expect("create legacy");
    let legacy = &ctx.store().contracts[&id];
    assert_eq!(legacy.pricing_model, "legacy");
    assert_eq!(
        legacy.total_amount,
        Some("11639997.6".parse::<Decimal>().unwrap())
    );

    // Template 1 has product_group="G1"; passing template_group=Some("G2") must fail
    let err = create_draft(
        &mut ctx,
        ContractForm {
            contract_number: "09/G2/2026".into(),
            customer_id: 1,
            product_id: Some(3),
            template_id: Some(1),
            ..ContractForm::default()
        },
        CreateOptions {
            require_main: false,
            template_group: Some("G2"),
            ..CreateOptions::default()
        },
    )
    .expect_err("wrong template group");
    assert!(err.user_message().contains("thuộc nhóm sản phẩm 'G1'"));
}
