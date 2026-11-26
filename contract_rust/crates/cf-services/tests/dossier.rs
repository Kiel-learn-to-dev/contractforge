use std::fs;

use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    contracts::evidence::{EvidenceKind, upload_evidence},
    customers::docs::upload_doc,
    dossier::{build::materialize, plan::build_plan},
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
fn dossier_plan_tracks_two_contracts_two_products_and_missing_docs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());

    // Customer 1 has contracts 1 (Draft), 2 (Generated), 4 (PaidActive, product 1).
    // Give contract 2 status Signed and product 3 (G2-MAIN) so customer 1 has 2 eligible contracts on 2 products.
    ctx.store_mut()
        .conn
        .execute(
            "UPDATE contracts SET status='Signed', product_id=3 WHERE id=2",
            [],
        )
        .expect("update c2");
    ctx.store_mut().reload().expect("reload");

    upload_doc(&mut ctx, 1, "cccd_front", "", b"%PDF front", "front.pdf").expect("upload front");
    upload_evidence(
        &mut ctx,
        4,
        EvidenceKind::Scan,
        b"%PDF scan4",
        Some("s4.pdf"),
        None,
    )
    .expect("scan 4");

    let plan = build_plan(ctx.store(), ctx.paths(), 1).expect("build plan");
    // 3 fixed docs + 2 contracts * 2 slots (signed_pdf, invoice_pdf) = 7 slots
    assert_eq!(plan.contracts.len(), 2);
    assert_eq!(plan.slots.len(), 7);
    assert!(!plan.is_complete());
    assert!(
        plan.missing_required()
            .iter()
            .any(|s| s.slot.key == "cccd_back")
    );
    let ready_signed = plan
        .slots
        .iter()
        .find(|s| s.slot.key == "contract_4_signed_pdf")
        .expect("c4 signed slot");
    assert!(ready_signed.ok());
    assert!(ready_signed.dest_name.contains("Hop_dong_G1-MAIN.pdf"));
}

#[test]
fn materialize_twice_cleans_stale_managed_files_and_preserves_user_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());
    upload_doc(&mut ctx, 1, "cccd_front", "", b"%PDF front", "front.pdf").expect("upload front");
    let other_id = upload_doc(&mut ctx, 1, "other", "Giay Phep", b"%PDF other", "gp.pdf")
        .expect("upload other");

    let plan1 = build_plan(ctx.store(), ctx.paths(), 1).expect("plan 1");
    let res1 = materialize(&ctx, &plan1).expect("materialize 1");
    assert_eq!(res1.copied, 2);
    let user_file = res1.dest_dir.join("tai_lieu_rieng_nguoi_dung.txt");
    fs::write(&user_file, b"keep me").expect("write foreign file");

    // Remove the "other" document, then materialize again -> old managed file removed, user file preserved
    cf_services::customers::docs::delete_doc(&mut ctx, 1, other_id).expect("delete other");
    let plan2 = build_plan(ctx.store(), ctx.paths(), 1).expect("plan 2");
    let res2 = materialize(&ctx, &plan2).expect("materialize 2");
    assert_eq!(res2.copied, 1);
    assert!(res2.removed >= 2);
    assert!(user_file.is_file());
    assert!(!res2.dest_dir.join("90_Giay_Phep.pdf").exists());
}
