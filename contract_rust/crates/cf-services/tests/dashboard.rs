use cf_db::store::Store;

fn seeded_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("temporary directory");
    let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
    (dir, Store::load(db).expect("load database"))
}

#[test]
fn global_search_finds_accented_customer_and_related_contract() {
    let (_dir, mut store) = seeded_store();
    let customer = store.customers.get_mut(&1).expect("customer");
    customer.legal_name = "Sông Xanh".into();
    customer.short_name = Some("Sông Xanh".into());
    let results = cf_services::dashboard::search::api_search(&store, " song xanh ", None);
    assert!(
        results
            .iter()
            .any(|result| result.kind == "customer" && result.id == 1)
    );
    assert!(results.iter().any(|result| result.kind == "contract"));
}

#[test]
fn global_search_applies_one_total_limit_after_contracts_and_customers() {
    let (_dir, mut store) = seeded_store();
    store.customers.get_mut(&1).expect("customer").legal_name = "Công ty Thử nghiệm".into();
    let customer = store.customers.get(&1).expect("customer").clone();
    let mut newer = store.contracts.values().next().expect("contract").clone();
    newer.id = 500;
    newer.contract_number = "TEST-NEW".into();
    newer.created_at += chrono::Duration::days(2);
    store.contracts.insert(newer.id, newer);
    let mut older = store.contracts.get(&1).expect("seed contract").clone();
    older.contract_number = "TEST-OLD".into();
    older.created_at -= chrono::Duration::days(2);
    store.contracts.insert(older.id, older);
    store.customers.insert(customer.id, customer);

    let one = cf_services::dashboard::search::api_search(&store, "test", Some(1));
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].label, "TEST-NEW");
    let two = cf_services::dashboard::search::api_search(&store, "test", Some(2));
    assert_eq!(two.len(), 2);
    assert!(two.iter().all(|result| result.kind == "contract"));
    assert_eq!(two[1].label, "TEST-OLD");
}

#[test]
fn global_search_ignores_short_query_and_inactive_or_deleted_customers() {
    let (_dir, mut store) = seeded_store();
    assert!(cf_services::dashboard::search::api_search(&store, " x ", None).is_empty());
    let customer = store.customers.get_mut(&1).expect("customer");
    customer.legal_name = "Sông Xanh".into();
    customer.is_active = false;
    let deleted = store.customers.get_mut(&2).expect("customer");
    deleted.legal_name = "Sông Xanh".into();
    deleted.is_deleted = true;
    let results = cf_services::dashboard::search::api_search(&store, "song xanh", Some(10));
    assert!(results.iter().all(|result| result.kind != "customer"));
}

#[test]
fn summary_and_product_breakdown_match_store_counts_and_uncategorized_bucket() {
    let (_dir, mut store) = seeded_store();
    // Seeded contracts: 1=Draft, 2=Generated, 3=Signed, 4=PaidActive, 5=Expired.
    // Set contract 3 (Signed, ACTIVE_LIFECYCLE) product_id=None and total_amount=500_000,
    // and contract 4 (PaidActive, ACTIVE_LIFECYCLE) total_amount=1_500_000.
    store
        .conn
        .execute(
            "UPDATE contracts SET product_id=NULL, total_amount=500000 WHERE id=3",
            [],
        )
        .expect("update c3");
    store
        .conn
        .execute("UPDATE contracts SET total_amount=1500000 WHERE id=4", [])
        .expect("update c4");
    store.reload().expect("reload");

    let today = chrono::NaiveDate::from_ymd_opt(2026, 3, 24).expect("today");
    let sum_data = cf_services::dashboard::summary::get_summary(&store, today);
    assert_eq!(sum_data.summary.total_contracts, 5);
    assert_eq!(sum_data.summary.in_force, 2); // Signed + PaidActive
    assert_eq!(
        sum_data.summary.total_value_active,
        rust_decimal::Decimal::from(2_000_000)
    );

    let breakdown = cf_services::dashboard::breakdown::contracts_by_product(&store);
    assert!(
        breakdown
            .iter()
            .any(|b| b.name == "Không phân loại" && b.count == 1)
    );
    let total_breakdown: rust_decimal::Decimal = breakdown.iter().map(|b| b.total_value).sum();
    assert_eq!(total_breakdown, sum_data.summary.total_value_active);
}

#[test]
fn trends_january_compares_december_prev_year_and_forecast_has_12_months() {
    let (_dir, mut store) = seeded_store();
    store
        .conn
        .execute(
            "UPDATE contracts SET created_at='2025-12-15 10:00:00', sign_date='2025-12-15', status='Signed' WHERE id=3",
            [],
        )
        .expect("dec 2025");
    store
        .conn
        .execute(
            "UPDATE contracts SET created_at='2026-01-10 10:00:00', sign_date='2026-01-10', status='PaidActive' WHERE id=4",
            [],
        )
        .expect("jan 2026");
    store.reload().expect("reload");

    let jan = chrono::NaiveDate::from_ymd_opt(2026, 1, 20).expect("jan");
    let stats = cf_services::dashboard::trends::monthly_stats(&store, jan);
    assert_eq!(stats.month_label, "Tháng 1/2026");
    assert_eq!(stats.new_contracts, 1);
    assert_eq!(stats.vs_new, 0); // 1 in Jan 2026 - 1 in Dec 2025
    assert_eq!(stats.signed, 1);
    assert_eq!(stats.vs_signed, 0);

    let forecast = cf_services::dashboard::trends::monthly_expiry_forecast(&store, jan, 12);
    assert_eq!(forecast.len(), 12);
    assert_eq!(forecast[0].month, "2026-01");
    assert_eq!(forecast[11].month, "2026-12");
}

#[test]
fn action_items_cover_all_five_cases_and_expiring_page_clamps_and_exports_csv() {
    let (_dir, mut store) = seeded_store();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 3, 24).expect("today");
    let now_utc = chrono::NaiveDateTime::parse_from_str("2026-03-24 10:00:00", "%Y-%m-%d %H:%M:%S")
        .expect("dt");

    // 1. Critical <=7d (id=4 PaidActive ending 2026-03-28)
    // 2. Draft without file (id=1 Draft)
    // 3. Generated > 14 days stale (id=2 Generated updated 2026-03-01)
    // 4. Signed > 7 days stale (id=3 Signed sign_date 2026-03-10)
    // 5. No active templates
    store
        .conn
        .execute("UPDATE contracts SET end_date='2026-03-28' WHERE id=4", [])
        .expect("c4");
    store
        .conn
        .execute(
            "UPDATE contracts SET updated_at='2026-03-01 00:00:00' WHERE id=2",
            [],
        )
        .expect("c2");
    store
        .conn
        .execute("UPDATE contracts SET sign_date='2026-03-10' WHERE id=3", [])
        .expect("c3");
    store
        .conn
        .execute("UPDATE contract_templates SET is_active=0", [])
        .expect("disable templates");
    store.reload().expect("reload");

    let items = cf_services::dashboard::actions::get_action_items(&store, today, now_utc);
    assert_eq!(items.len(), 5);
    assert!(items[0].message.contains("hết hạn trong 7 ngày"));
    assert!(
        items[1]
            .message
            .contains("hợp đồng nháp chưa được sinh file")
    );
    assert!(items[2].message.contains("chưa gửi KH (>14 ngày)"));
    assert!(items[3].message.contains("chưa chuyển hiệu lực (>7 ngày)"));
    assert_eq!(items[4].message, "Chưa có mẫu hợp đồng nào đang hoạt động");

    // Expiring page: days=999 clamped to 365, days=0 clamped to 1
    let rows = cf_services::dashboard::expiring_page::get_expiring_contracts(&store, today, 999);
    assert_eq!(rows.len(), 1);
    let (fname, csv_bytes) = cf_services::dashboard::expiring_page::export_expiring_csv(&rows, 999);
    assert_eq!(fname, "sap_het_han_365ngay.csv");
    let csv = String::from_utf8(csv_bytes).expect("utf8");
    assert!(
        csv.contains("Số hợp đồng,Khách hàng,Ngày hết hạn,Còn lại (ngày),Trạng thái,Giá trị (VNĐ)")
    );
}
