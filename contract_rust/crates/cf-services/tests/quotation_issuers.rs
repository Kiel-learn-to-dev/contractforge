use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use chrono::{NaiveDate, NaiveDateTime};

#[test]
fn issuer_fingerprint_matches_python_json_serialization() {
    let fields = std::collections::BTreeMap::from([
        ("name", "Công ty Mẫu".to_owned()),
        ("address", "Số 1".to_owned()),
        ("tax_code", String::new()),
        ("representative", String::new()),
        ("title", String::new()),
        ("phone", String::new()),
        ("email", String::new()),
    ]);
    assert_eq!(
        cf_services::quotations::issuers::fingerprint(&fields),
        "7041804701cc1d7a46517cb558d235a5f7106847666a7b01c08f253b990beb2b"
    );
}

#[test]
fn saving_identical_issuer_twice_returns_one_record() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
    let store = Store::load(db).expect("load database");
    let clock = FixedClock {
        date: NaiveDate::from_ymd_opt(2026, 3, 24).expect("date"),
        utc: NaiveDateTime::parse_from_str("2026-03-24 04:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("time"),
        local: NaiveDateTime::parse_from_str("2026-03-24 11:40:37", "%Y-%m-%d %H:%M:%S")
            .expect("time"),
    };
    let mut ctx = cf_services::Ctx::new(store, DataPaths::new(dir.path()), Box::new(clock));
    let input = cf_services::quotations::issuers::IssuerInput {
        name: " Công ty Mẫu ".into(),
        address: "Số 1".into(),
        ..Default::default()
    };
    let (first, created) =
        cf_services::quotations::issuers::save_issuer(&mut ctx, input.clone()).expect("save first");
    assert!(created);
    let (again, created) =
        cf_services::quotations::issuers::save_issuer(&mut ctx, input).expect("save duplicate");
    assert!(!created);
    assert_eq!(again.id, first.id);
    assert_eq!(ctx.store().issuers.len(), 1);
}
