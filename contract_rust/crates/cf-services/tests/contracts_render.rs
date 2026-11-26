use std::io::Cursor;

use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    contracts::{
        attachments::{build_bundle_zip, resolve_attachment_templates},
        form08a::render_form_08a,
        render::render_data_for_with_legacy_rows,
    },
};
use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;
use zip::ZipArchive;

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
fn render_data_for_matches_render_context_fixture_case_0() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    let fixture = cf_testkit::fixture("render_context");
    let case = &fixture["cases"][0];
    let c_json = &case["contract"];
    let cust_json = &case["customer"];
    let expected_ctx = case["context"].as_object().expect("expected context");

    let now_utc = ctx.clock().now_utc();
    let store = ctx.store_mut();
    let cust = store.customers.get_mut(&1).expect("cust 1");
    cust.legal_name = cust_json["legal_name"].as_str().unwrap().into();
    cust.short_name = cust_json["short_name"].as_str().map(str::to_owned);
    cust.code = cust_json["code"].as_str().unwrap().into();
    cust.address = cust_json["address"].as_str().map(str::to_owned);
    cust.ward = cust_json["ward"].as_str().map(str::to_owned);
    cust.province = cust_json["province"].as_str().map(str::to_owned);
    cust.representative_name = cust_json["representative_name"].as_str().map(str::to_owned);
    cust.representative_gender = cust_json["representative_gender"]
        .as_str()
        .map(str::to_owned);
    cust.representative_title = cust_json["representative_title"]
        .as_str()
        .map(str::to_owned);
    cust.tax_code = cust_json["tax_code"].as_str().map(str::to_owned);
    cust.bank_account = cust_json["bank_account"].as_str().map(str::to_owned);
    cust.bank_name = cust_json["bank_name"].as_str().map(str::to_owned);
    cust.beneficiary_name = cust_json["beneficiary_name"].as_str().map(str::to_owned);
    cust.budget_relation_code = cust_json["budget_relation_code"]
        .as_str()
        .map(str::to_owned);
    cust.phone = cust_json["phone"].as_str().map(str::to_owned);
    cust.email = cust_json["email"].as_str().map(str::to_owned);
    cust.contact_name = cust_json["contact_name"].as_str().map(str::to_owned);
    cust.establishment_decision = cust_json["establishment_decision"]
        .as_str()
        .map(str::to_owned);
    cust.establishment_date = cust_json["establishment_date"]
        .as_str()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    cust.bill_main_station = cust_json["bill_main_station"].as_bool().unwrap();

    // Sync sub-units to match fixture
    store.units.clear();
    for (idx, u) in cust_json["sub_units"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let uid = (idx + 1) as i64;
        store.units.insert(
            uid,
            cf_db::model::CustomerUnit {
                id: uid,
                customer_id: 1,
                name: u["name"].as_str().unwrap().into(),
                unit_type: u["unit_type"].as_str().unwrap().into(),
                address: None,
                notes: None,
                is_active: u["is_active"].as_bool().unwrap(),
                include_in_billing: u["include_in_billing"].as_bool().unwrap(),
                sort_order: uid,
                created_at: now_utc,
                updated_at: now_utc,
            },
        );
    }

    let prod = store.products.get_mut(&1).expect("prod 1");
    prod.name = c_json["product"]["name"].as_str().unwrap().into();
    prod.code = c_json["product"]["code"].as_str().unwrap().into();

    let contract = store.contracts.get_mut(&1).expect("contract 1");
    contract.contract_number = c_json["contract_number"].as_str().unwrap().into();
    contract.party_b_authorization_number = c_json["party_b_authorization_number"]
        .as_str()
        .map(str::to_owned);
    contract.total_pre_vat = Some(
        c_json["total_pre_vat"]
            .as_str()
            .unwrap()
            .parse::<Decimal>()
            .unwrap(),
    );
    contract.total_vat = Some(
        c_json["total_vat"]
            .as_str()
            .unwrap()
            .parse::<Decimal>()
            .unwrap(),
    );
    contract.sign_date =
        NaiveDate::parse_from_str(c_json["sign_date"].as_str().unwrap(), "%Y-%m-%d").ok();
    contract.start_date =
        NaiveDate::parse_from_str(c_json["start_date"].as_str().unwrap(), "%Y-%m-%d").ok();
    contract.end_date =
        NaiveDate::parse_from_str(c_json["end_date"].as_str().unwrap(), "%Y-%m-%d").ok();
    contract.month_count = c_json["month_count"].as_i64();
    contract.unit_count = c_json["unit_count"].as_i64();
    contract.quantity = Some(
        c_json["quantity"]
            .as_str()
            .unwrap()
            .parse::<Decimal>()
            .unwrap(),
    );
    contract.unit_price_pre_vat = Some(
        c_json["unit_price_pre_vat"]
            .as_str()
            .unwrap()
            .parse::<Decimal>()
            .unwrap(),
    );
    contract.vat_rate = Some(
        c_json["vat_rate"]
            .as_str()
            .unwrap()
            .parse::<Decimal>()
            .unwrap(),
    );
    contract.unit_price_vat = Some(
        c_json["unit_price_vat"]
            .as_str()
            .unwrap()
            .parse::<Decimal>()
            .unwrap(),
    );
    contract.total_amount = Some(
        c_json["total_amount"]
            .as_str()
            .unwrap()
            .parse::<Decimal>()
            .unwrap(),
    );
    contract.pricing_description = c_json["pricing_description"].as_str().map(str::to_owned);
    contract.acceptance_date =
        NaiveDate::parse_from_str(c_json["acceptance_date"].as_str().unwrap(), "%Y-%m-%d").ok();

    let data = render_data_for_with_legacy_rows(store, 1, 1, Some(true)).expect("render data");
    for (key, expected_val) in expected_ctx {
        assert_eq!(
            data.values.get(key).map(String::as_str),
            expected_val.as_str(),
            "mismatch at key {key}"
        );
    }
}

#[test]
fn attachment_templates_reject_wrong_group_and_zip_deduplicates_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    // Set template 2 (Nghiệm thu) to group G2 while contract 1 uses product 1 (group G1)
    ctx.store_mut()
        .templates
        .get_mut(&2)
        .expect("tpl 2")
        .product_group = Some("G2".into());
    let err = resolve_attachment_templates(ctx.store(), &[2], 1).expect_err("wrong group");
    assert!(err.user_message().contains("thuộc nhóm sản phẩm 'G2'"));

    let zip_bytes = build_bundle_zip(&[
        ("a.docx".into(), b"first".to_vec()),
        ("a.docx".into(), b"second".to_vec()),
    ])
    .expect("zip");
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes)).expect("open zip");
    let mut names = Vec::new();
    for i in 0..archive.len() {
        names.push(
            archive
                .by_index(i)
                .expect("entry")
                .name()
                .expect("name")
                .into_owned(),
        );
    }
    assert_eq!(names, vec!["a.docx", "a_2.docx"]);
}

#[test]
fn render_form_08a_includes_uppercase_budget_unit_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    ctx.store_mut()
        .customers
        .get_mut(&1)
        .expect("c1")
        .legal_name = "Trạm Y tế xã Sông Xanh".into();
    let (bytes, filename) = render_form_08a(ctx.store(), 1).expect("render 08a");
    assert!(filename.starts_with("08A_"));
    assert!(filename.ends_with(".docx"));
    let texts = cf_docx::inspect::docx_texts(&bytes).expect("inspect 08a");
    let joined = texts.into_values().flatten().collect::<Vec<_>>().join("\n");
    assert!(joined.contains("TRẠM Y TẾ XÃ SÔNG XANH"));
}
