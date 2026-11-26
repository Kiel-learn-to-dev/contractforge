use cf_db::store::Store;

#[test]
fn invalid_addendum_format_is_rejected_without_saving_and_profile_completion_is_checked() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
    let mut store = Store::load(db).expect("load database");
    assert!(!cf_services::settings::is_organization_profile_complete(
        &store
    ));

    let error = cf_services::settings::save_addendum_format(&mut store, "PL{seq}")
        .expect_err("format needs parent");
    assert_eq!(
        error.user_message(),
        "Khuôn phụ lục phải chứa {seq} và {parent}, nếu không các phụ lục sẽ trùng số."
    );
    assert_eq!(
        cf_services::settings::get_setting(&store, "addendum_number_format", ""),
        ""
    );

    cf_services::settings::save_addendum_format(&mut store, " PL{seq}/{parent} ")
        .expect("save valid format");
    assert_eq!(
        cf_services::settings::addendum_format(&store),
        "PL{seq}/{parent}"
    );
    cf_services::settings::save_party_b(
        &mut store,
        [
            ("party_b_name".into(), "  Tổ chức mẫu  ".into()),
            ("party_b_representative".into(), " Người đại diện ".into()),
            ("untrusted_key".into(), "ignored".into()),
        ],
    )
    .expect("save profile");
    assert!(cf_services::settings::is_organization_profile_complete(
        &store
    ));
    let profile = cf_services::settings::get_party_b_settings(&store);
    assert_eq!(profile.name, "Tổ chức mẫu");
    assert_eq!(profile.representative, "Người đại diện");
    assert!(!store.settings.contains_key("untrusted_key"));
}
