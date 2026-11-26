use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    contracts::{
        expiry::expire_due,
        status::{bulk_change_status, change_status},
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
fn valid_status_change_records_event_and_missing_invoice_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());

    // Draft (id=1) -> Generated
    change_status(&mut ctx, 1, "Generated", "user", "test").expect("draft -> generated");
    assert_eq!(ctx.store().contracts[&1].status.as_str(), "Generated");
    assert_eq!(
        ctx.store()
            .events
            .values()
            .filter(|e| e.contract_id == 1 && e.event_type == "status_changed")
            .count(),
        1
    );

    // Signed (id=3) -> Invoiced without invoice file must fail
    let err = change_status(&mut ctx, 3, "Invoiced", "user", "").expect_err("missing invoice");
    assert!(!err.user_message().is_empty());
    assert_eq!(ctx.store().contracts[&3].status.as_str(), "Signed");
}

#[test]
fn bulk_change_status_is_all_or_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    // id=1 is Draft (can -> Generated), id=5 is Expired (cannot -> Generated)
    let err =
        bulk_change_status(&mut ctx, &[1, 5], "Generated").expect_err("one invalid fails all");
    assert!(err.user_message().contains("Không cập nhật hợp đồng nào"));
    assert_eq!(ctx.store().contracts[&1].status.as_str(), "Draft");
    assert_eq!(ctx.store().contracts[&5].status.as_str(), "Expired");

    // Both id=1 (Draft) and id=2 (Generated) can -> Sent? Let's check Draft->Sent vs Generated->Sent:
    // Actually Draft can -> Generated, or let's change id=1 to Generated first, then bulk Sent.
    change_status(&mut ctx, 1, "Generated", "user", "").expect("1 -> Generated");
    let count = bulk_change_status(&mut ctx, &[1, 2], "Sent").expect("bulk Sent");
    assert_eq!(count, 2);
    assert_eq!(ctx.store().contracts[&1].status.as_str(), "Sent");
    assert_eq!(ctx.store().contracts[&2].status.as_str(), "Sent");
}

#[test]
fn expire_due_marks_yesterday_signed_contract_as_expired_with_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    // Set id=3 (Signed) end_date = 2026-03-23 (yesterday relative to FixedClock 2026-03-24)
    ctx.store_mut()
        .conn
        .execute(
            "UPDATE contracts SET end_date = '2026-03-23' WHERE id = 3",
            [],
        )
        .expect("set end_date");
    ctx.store_mut().reload().expect("reload");

    let count = expire_due(&mut ctx).expect("expire_due");
    assert_eq!(count, 1);
    assert_eq!(ctx.store().contracts[&3].status.as_str(), "Expired");
    assert!(
        ctx.store()
            .events
            .values()
            .any(|e| e.contract_id == 3 && e.event_type == "expired")
    );
}
