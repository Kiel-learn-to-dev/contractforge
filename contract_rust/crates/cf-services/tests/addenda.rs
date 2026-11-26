use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use chrono::{NaiveDate, NaiveDateTime};

#[test]
fn replacement_walk_finds_active_product_and_stops_on_cycles() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
    let mut store = Store::load(db).expect("load store");
    store.products.get_mut(&1).expect("old product").is_active = false;
    store
        .products
        .get_mut(&1)
        .expect("old product")
        .replaced_by_id = Some(2);
    store.products.get_mut(&2).expect("replacement").is_active = false;
    store
        .products
        .get_mut(&2)
        .expect("replacement")
        .replaced_by_id = Some(3);
    let current =
        cf_services::addenda::renewal_defaults::current_product(&store, store.products.get(&1));
    assert_eq!(current.map(|product| product.id), Some(3));

    store.products.get_mut(&3).expect("last product").is_active = false;
    store
        .products
        .get_mut(&3)
        .expect("last product")
        .replaced_by_id = Some(1);
    let current =
        cf_services::addenda::renewal_defaults::current_product(&store, store.products.get(&1));
    assert!(current.is_none());
}

#[test]
fn addon_defaults_number_sequentially_and_reject_early_start_and_main_product() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
    let mut store = Store::load(&db).expect("load store");
    store
        .conn
        .execute("UPDATE contracts SET status='PaidActive', start_date='2026-01-01', end_date='2026-12-31', month_count=12 WHERE id=1", [])
        .expect("set eligible status and dates");
    store.reload().expect("reload store");
    let date = NaiveDate::from_ymd_opt(2026, 7, 1).expect("date");
    let first =
        cf_services::addenda::addon::addendum_defaults(&store, 1, "addon", date).expect("defaults");
    assert_eq!(first.number, "PL01/01/G1/2026");
    assert_eq!(
        cf_services::addenda::addon::addendum_defaults(&store, 1, "addon", date)
            .expect("defaults")
            .number,
        first.number
    );
    assert!(first.candidate_product_ids.contains(&2));
    let early = NaiveDate::from_ymd_opt(2025, 12, 31).expect("date");
    assert!(cf_services::addenda::addon::addendum_defaults(&store, 1, "addon", early).is_err());

    let clock = FixedClock {
        date: NaiveDate::from_ymd_opt(2026, 3, 24).expect("date"),
        utc: NaiveDateTime::parse_from_str("2026-03-24 04:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("time"),
        local: NaiveDateTime::parse_from_str("2026-03-24 11:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("time"),
    };
    let store = Store::load(db).expect("reload seeded db");
    let mut ctx = cf_services::Ctx::new(store, DataPaths::new(dir.path()), Box::new(clock));
    let error = cf_services::addenda::addon::create_addendum(
        &mut ctx,
        1,
        "addon",
        cf_services::addenda::addon::AddonForm {
            start_date: date,
            sign_date: None,
            template_id: None,
            items: vec![cf_services::addenda::addon::AddonItem {
                product_id: 1,
                description: None,
                quantity: Some("1".into()),
                unit_price_pre_vat: None,
                vat_rate: None,
                amount_pre_vat_override: None,
                note: String::new(),
            }],
        },
    )
    .expect_err("main product not addon");
    assert!(
        error
            .user_message()
            .contains("chỉ nhận gói bổ sung cùng nhóm")
    );

    let child_id = cf_services::addenda::addon::create_addendum(
        &mut ctx,
        1,
        "addon",
        cf_services::addenda::addon::AddonForm {
            start_date: date,
            sign_date: None,
            template_id: None,
            items: vec![cf_services::addenda::addon::AddonItem {
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
    .expect("create addon");
    let child = ctx
        .store()
        .contracts
        .get(&child_id)
        .expect("child contract");
    assert_eq!(child.contract_number, "PL01/01/G1/2026");
    assert_eq!(child.doc_kind, "addendum_addon");
    assert_eq!(child.parent_contract_id, Some(1));
    let child_events = ctx
        .store()
        .events
        .values()
        .filter(|e| e.contract_id == child_id)
        .map(|e| e.event_type.as_str())
        .collect::<Vec<_>>();
    assert!(child_events.contains(&"created"));
    assert!(child_events.contains(&"addendum"));
    assert!(
        ctx.store()
            .events
            .values()
            .any(|e| e.contract_id == 1 && e.event_type == "addendum")
    );
    let next = cf_services::addenda::addon::addendum_defaults(ctx.store(), 1, "addon", date)
        .expect("next defaults");
    assert_eq!(next.number, "PL02/01/G1/2026");
}

#[test]
fn renewal_appendix_and_contract_link_to_parent_and_change_expiring_status() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
    let mut store = Store::load(&db).expect("load store");
    store.conn.execute("UPDATE contracts SET status='PaidActive', start_date='2026-01-01', end_date='2026-03-31', month_count=3 WHERE id=1", []).expect("prepare parent");
    store.reload().expect("reload parent");
    let clock = FixedClock {
        date: NaiveDate::from_ymd_opt(2026, 3, 24).expect("date"),
        utc: NaiveDateTime::parse_from_str("2026-03-24 04:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("time"),
        local: NaiveDateTime::parse_from_str("2026-03-24 11:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("time"),
    };
    let mut ctx = cf_services::Ctx::new(store, DataPaths::new(dir.path()), Box::new(clock));
    let start = NaiveDate::from_ymd_opt(2026, 4, 1).expect("date");
    let bad = cf_services::addenda::renewal::create_renewal_appendix(
        &mut ctx,
        1,
        cf_services::addenda::addon::AddonForm {
            start_date: NaiveDate::from_ymd_opt(2026, 3, 31).expect("date"),
            sign_date: None,
            template_id: None,
            items: vec![cf_services::addenda::addon::AddonItem {
                product_id: 1,
                description: None,
                quantity: Some("1".into()),
                unit_price_pre_vat: None,
                vat_rate: None,
                amount_pre_vat_override: None,
                note: String::new(),
            }],
        },
        12,
    )
    .expect_err("renewal must start later");
    assert_eq!(
        bad.user_message(),
        "Gia hạn phải bắt đầu sau ngày hết hạn của hợp đồng gốc."
    );
    let appendix = cf_services::addenda::renewal::create_renewal_appendix(
        &mut ctx,
        1,
        cf_services::addenda::addon::AddonForm {
            start_date: start,
            sign_date: None,
            template_id: None,
            items: vec![cf_services::addenda::addon::AddonItem {
                product_id: 1,
                description: None,
                quantity: Some("1".into()),
                unit_price_pre_vat: None,
                vat_rate: None,
                amount_pre_vat_override: None,
                note: String::new(),
            }],
        },
        12,
    )
    .expect("renewal appendix");
    assert_eq!(
        ctx.store().contracts[&appendix].doc_kind,
        "addendum_renewal"
    );
    assert_eq!(ctx.store().contracts[&appendix].parent_contract_id, Some(1));
    ctx.store_mut()
        .conn
        .execute(
            "UPDATE contracts SET status='Signed' WHERE id=?1",
            [appendix],
        )
        .expect("sign renewal appendix");
    ctx.store_mut().reload().expect("reload signed renewal");
    assert!(
        cf_services::contracts::expiring::expiring(
            ctx.store(),
            NaiveDate::from_ymd_opt(2026, 3, 24).expect("date"),
            30
        )
        .iter()
        .all(|row| row.contract.id != 1)
    );

    let new_id = cf_services::addenda::renewal::create_renewal_contract(
        &mut ctx,
        1,
        "06/G1/2026",
        cf_services::addenda::addon::AddonForm {
            start_date: start,
            sign_date: None,
            template_id: None,
            items: vec![cf_services::addenda::addon::AddonItem {
                product_id: 1,
                description: None,
                quantity: Some("1".into()),
                unit_price_pre_vat: None,
                vat_rate: None,
                amount_pre_vat_override: None,
                note: String::new(),
            }],
        },
        12,
    )
    .expect("renewal contract");
    assert_eq!(ctx.store().contracts[&new_id].renewed_from_id, Some(1));
    assert!(
        ctx.store()
            .events
            .values()
            .any(|e| { e.contract_id == new_id && e.event_type == "renewal" })
    );
    assert!(
        ctx.store()
            .events
            .values()
            .any(|e| { e.contract_id == 1 && e.event_type == "renewal" })
    );
}
