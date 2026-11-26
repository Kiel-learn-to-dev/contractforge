use cf_db::store::Store;
use cf_services::customers::state::{
    bulk_delete_customers, delete_customer, list_deleted_customers, restore_customer, toggle_active,
};
use chrono::NaiveDateTime;

fn seeded_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed");
    (dir, Store::load(db).expect("load"))
}

fn dt(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").expect("dt")
}

#[test]
fn toggle_delete_trash_order_and_restore_work() {
    let (_dir, mut store) = seeded_store();
    let active = toggle_active(&mut store, 1, dt("2026-03-24 09:00:00")).expect("toggle");
    assert!(!active);
    assert!(!store.customers[&1].is_active);

    assert!(delete_customer(&mut store, 1, dt("2026-03-24 10:00:00")).expect("delete 1"));
    assert!(!delete_customer(&mut store, 1, dt("2026-03-24 10:01:00")).expect("delete 1 again"));
    let bulk = bulk_delete_customers(&mut store, &[2, 999], dt("2026-03-24 11:00:00"))
        .expect("bulk delete");
    assert_eq!(bulk, 1);

    let trash = list_deleted_customers(&store);
    assert_eq!(trash.len(), 3);
    // Mới nhất trước: id=2 (11:00), id=1 (10:00), id=3 (seeded deleted_at=None)
    assert_eq!(trash[0].id, 2);
    assert_eq!(trash[1].id, 1);
    assert_eq!(trash[2].id, 3);

    restore_customer(&mut store, 1, dt("2026-03-24 12:00:00")).expect("restore 1");
    assert!(!store.customers[&1].is_deleted);
    assert_eq!(store.customers[&1].deleted_at, None);
    let trash_after = list_deleted_customers(&store);
    assert_eq!(
        trash_after.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![2, 3]
    );
}
