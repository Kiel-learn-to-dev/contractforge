use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    addenda::{addon::AddonForm, addon::AddonItem, renewal::create_renewal_appendix},
    contracts::expiring::{expiring, not_renewed},
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
fn expiring_excludes_signed_renewal_appendix_but_keeps_draft_renewal_appendix() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    ctx.store_mut()
        .conn
        .execute(
            "UPDATE contracts SET status='PaidActive', start_date='2026-01-01', end_date='2026-04-10', month_count=4 WHERE id=4",
            [],
        )
        .expect("prepare parent");
    ctx.store_mut().reload().expect("reload");

    let today = NaiveDate::from_ymd_opt(2026, 3, 24).expect("today");
    assert!(not_renewed(ctx.store(), 4));
    assert!(
        expiring(ctx.store(), today, 30)
            .iter()
            .any(|r| r.contract.id == 4)
    );

    // Create draft renewal appendix -> parent still appears in expiring
    let appendix_id = create_renewal_appendix(
        &mut ctx,
        4,
        AddonForm {
            start_date: NaiveDate::from_ymd_opt(2026, 4, 11).expect("start"),
            sign_date: None,
            template_id: None,
            items: vec![AddonItem {
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
    .expect("create draft renewal");
    assert!(not_renewed(ctx.store(), 4));
    assert!(
        expiring(ctx.store(), today, 30)
            .iter()
            .any(|r| r.contract.id == 4)
    );

    // Update renewal appendix to Signed -> parent disappears from expiring
    ctx.store_mut()
        .conn
        .execute(
            "UPDATE contracts SET status='Signed' WHERE id=?1",
            [appendix_id],
        )
        .expect("sign appendix");
    ctx.store_mut().reload().expect("reload");
    assert!(!not_renewed(ctx.store(), 4));
    assert!(
        expiring(ctx.store(), today, 30)
            .iter()
            .all(|r| r.contract.id != 4)
    );
}
