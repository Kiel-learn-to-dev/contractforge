use cf_db::store::Store;

fn seeded_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
    (dir, Store::load(db).expect("load database"))
}

#[test]
fn customer_list_search_is_accent_insensitive_and_excludes_deleted_rows() {
    let (_dir, mut store) = seeded_store();
    store.customers.get_mut(&1).expect("customer").legal_name = "Sông Xanh".into();
    store
        .customers
        .get_mut(&3)
        .expect("deleted customer")
        .legal_name = "Sông Xanh".into();
    let filters = cf_services::customers::list::CustomerFilters {
        q: "song xanh".into(),
        ..Default::default()
    };
    let (rows, total) =
        cf_services::customers::list::list_customers(&store, &filters).expect("list customers");
    assert_eq!(total, 1);
    assert_eq!(rows[0].0.id, 1);
}

#[test]
fn customer_list_filters_sorts_and_paginates() {
    let (_dir, mut store) = seeded_store();
    let first = store.customers.get_mut(&1).expect("customer");
    first.legal_name = "Beta".into();
    first.ward = Some("Ward B".into());
    let second = store.customers.get_mut(&2).expect("customer");
    second.legal_name = "Alpha".into();
    second.ward = Some("Ward A".into());
    let filters = cf_services::customers::list::CustomerFilters {
        active_only: false,
        sort_by: "legal_name".into(),
        per_page: 1,
        ..Default::default()
    };
    let (page_one, total) =
        cf_services::customers::list::list_customers(&store, &filters).expect("first page");
    assert_eq!(total, 2);
    assert_eq!(page_one[0].0.legal_name, "Alpha");
    let second_filters = cf_services::customers::list::CustomerFilters { page: 2, ..filters };
    let (page_two, _) =
        cf_services::customers::list::list_customers(&store, &second_filters).expect("second page");
    assert_eq!(page_two[0].0.legal_name, "Beta");

    let filtered = cf_services::customers::list::CustomerFilters {
        active_only: false,
        ward: "Ward B".into(),
        customer_type: "Trường học".into(),
        ..Default::default()
    };
    let (rows, total) =
        cf_services::customers::list::list_customers(&store, &filtered).expect("filtered list");
    assert_eq!(total, 1);
    assert_eq!(rows[0].0.id, 1);
    let options = cf_services::customers::list::filter_options(&store);
    assert!(options.wards.contains(&"Ward A".into()));
    assert!(options.wards.contains(&"Ward B".into()));
}
