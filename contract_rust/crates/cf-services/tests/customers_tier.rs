use std::collections::BTreeSet;

use cf_core::lifecycle::ContractStatus;
use cf_db::model::{Contract, Customer};
use cf_services::customers::tier::{TierContract, classify};
use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;
use serde_json::Value;

fn customer() -> Customer {
    let now =
        NaiveDateTime::parse_from_str("2026-03-01 00:00:00", "%Y-%m-%d %H:%M:%S").expect("dt");
    Customer {
        id: 1,
        code: "KH-001".into(),
        legal_name: "Trạm Y tế".into(),
        short_name: None,
        customer_type: "Trạm y tế".into(),
        address: None,
        ward: None,
        district: None,
        province: None,
        tax_code: None,
        representative_name: None,
        representative_gender: None,
        representative_title: None,
        bank_account: None,
        bank_name: None,
        beneficiary_name: None,
        contact_name: None,
        phone: None,
        email: None,
        is_active: true,
        is_deleted: false,
        deleted_at: None,
        bill_main_station: true,
        record_count: None,
        budget_relation_code: None,
        kcb_code: None,
        establishment_decision: None,
        establishment_date: None,
        notes: None,
        created_at: now,
        updated_at: now,
    }
}

fn contract(
    id: i64,
    status: ContractStatus,
    end_date: Option<NaiveDate>,
    created_date: NaiveDate,
    label: Option<&str>,
) -> TierContract {
    let created_at = created_date.and_hms_opt(9, 0, 0).expect("hms");
    TierContract {
        contract: Contract {
            id,
            contract_number: format!("{id:02}/2026"),
            status,
            customer_id: 1,
            product_id: Some(1),
            template_id: Some(1),
            sign_date: Some(created_date),
            start_date: Some(created_date),
            end_date,
            month_count: Some(12),
            sign_location: None,
            unit_count: Some(1),
            quantity: None,
            unit_price_pre_vat: None,
            vat_rate: None,
            unit_price_vat: None,
            total_amount: Some(Decimal::from(1_000_000)),
            amount_text: None,
            pricing_description: None,
            party_b_representative: None,
            party_b_authorization_number: None,
            party_b_authorization_date: None,
            acceptance_date: None,
            liquidation_date: None,
            output_file_path: None,
            output_file_name: None,
            signed_pdf_path: None,
            signed_pdf_name: None,
            signed_pdf_uploaded_at: None,
            invoice_date: None,
            invoice_pdf_path: None,
            invoice_pdf_name: None,
            invoice_pdf_uploaded_at: None,
            payment_date: None,
            payment_slip_path: None,
            payment_slip_name: None,
            payment_slip_uploaded_at: None,
            selected_unit_names: Vec::new(),
            render_data: Value::Null,
            notes: None,
            is_batch: false,
            batch_job_id: None,
            pricing_model: "items".into(),
            total_pre_vat: None,
            total_vat: None,
            payment_mode: None,
            payment_cycle_months: None,
            parent_contract_id: None,
            renewed_from_id: None,
            doc_kind: "contract".into(),
            created_at,
            updated_at: created_at,
        },
        product_label: label.map(str::to_owned),
    }
}

#[test]
fn classify_covers_tiers_1_through_5_including_subcases() {
    let c = customer();
    let today = NaiveDate::from_ymd_opt(2026, 3, 24).expect("today");
    let catalog = BTreeSet::from(["G1".to_string(), "G2".to_string()]);

    // Tier 1: active expiring in <= 30 days
    let t1 = classify(
        &c,
        &[],
        &[contract(
            1,
            ContractStatus::Active,
            NaiveDate::from_ymd_opt(2026, 4, 10),
            today,
            Some("G1"),
        )],
        &catalog,
        today,
    );
    assert_eq!(t1.tier, 1);

    // Tier 2a: active expiring in 31..=60 days
    let t2a = classify(
        &c,
        &[],
        &[contract(
            2,
            ContractStatus::PaidActive,
            NaiveDate::from_ymd_opt(2026, 5, 10),
            today,
            Some("G1"),
        )],
        &catalog,
        today,
    );
    assert_eq!(t2a.tier, 2);
    assert!(t2a.action_text.contains("Chuẩn bị gia hạn"));

    // Tier 2b: no active, has expired/terminated
    let t2b = classify(
        &c,
        &[],
        &[contract(
            3,
            ContractStatus::Expired,
            NaiveDate::from_ymd_opt(2026, 1, 10),
            today,
            Some("G1"),
        )],
        &catalog,
        today,
    );
    assert_eq!(t2b.tier, 2);
    assert!(t2b.action_text.contains("chưa ký mới"));

    // Tier 2c: no active, old draft (> 14 days)
    let old_day = NaiveDate::from_ymd_opt(2026, 3, 1).expect("old");
    let t2c = classify(
        &c,
        &[],
        &[contract(
            4,
            ContractStatus::Draft,
            NaiveDate::from_ymd_opt(2026, 12, 31),
            old_day,
            Some("G1"),
        )],
        &catalog,
        today,
    );
    assert_eq!(t2c.tier, 2);
    assert!(t2c.action_text.contains("chưa ký"));

    // Tier 3: active far expiry, missing catalog product
    let t3 = classify(
        &c,
        &[],
        &[contract(
            5,
            ContractStatus::Invoiced,
            NaiveDate::from_ymd_opt(2026, 12, 31),
            today,
            Some("G1"),
        )],
        &catalog,
        today,
    );
    assert_eq!(t3.tier, 3);
    assert!(t3.action_text.contains("Upsell G2"));

    // Tier 4: active far expiry, owns all catalog products
    let t4 = classify(
        &c,
        &[],
        &[
            contract(
                6,
                ContractStatus::Active,
                NaiveDate::from_ymd_opt(2026, 12, 31),
                today,
                Some("G1"),
            ),
            contract(
                7,
                ContractStatus::Active,
                NaiveDate::from_ymd_opt(2026, 12, 31),
                today,
                Some("G2"),
            ),
        ],
        &catalog,
        today,
    );
    assert_eq!(t4.tier, 4);

    // Tier 5a: young draft (<= 14 days), no inactive
    let t5a = classify(
        &c,
        &[],
        &[contract(
            8,
            ContractStatus::Generated,
            NaiveDate::from_ymd_opt(2026, 12, 31),
            today,
            Some("G1"),
        )],
        &catalog,
        today,
    );
    assert_eq!(t5a.tier, 5);
    assert!(t5a.action_text.contains("HĐ nháp"));

    // Tier 5b: no contracts
    let t5b = classify(&c, &[], &[], &catalog, today);
    assert_eq!(t5b.tier, 5);
    assert!(t5b.action_text.contains("Chưa có hợp đồng"));
}
