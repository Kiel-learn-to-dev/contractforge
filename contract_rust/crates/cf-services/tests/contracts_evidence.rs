use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    contracts::evidence::{EvidenceKind, delete_evidence, upload_evidence},
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
fn upload_evidence_transitions_generated_to_signed_to_invoiced_to_paid_active_and_rejects_6mb() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut ctx = context(dir.path());

    // Oversized file (6 MB) rejected
    let big = [b"%PDF".as_slice(), &vec![b'a'; 6 * 1024 * 1024]].concat();
    let err = upload_evidence(&mut ctx, 2, EvidenceKind::Scan, &big, Some("big.pdf"), None)
        .expect_err("6mb rejected");
    assert!(err.user_message().contains("File quá lớn"));

    // Contract 2 starts as Generated
    let scan = upload_evidence(
        &mut ctx,
        2,
        EvidenceKind::Scan,
        b"%PDF scan",
        Some("scan.pdf"),
        None,
    )
    .expect("upload scan");
    assert!(scan.message.contains("Đã ký"));
    assert_eq!(ctx.store().contracts[&2].status.as_str(), "Signed");

    let inv = upload_evidence(
        &mut ctx,
        2,
        EvidenceKind::Invoice,
        b"%PDF invoice",
        Some("inv.pdf"),
        Some("2026-03-24"),
    )
    .expect("upload invoice");
    assert!(inv.message.contains("Đã xuất hóa đơn"));
    assert_eq!(ctx.store().contracts[&2].status.as_str(), "Invoiced");

    let png = b"\x89PNG\r\n\x1a\npayment";
    let slip = upload_evidence(
        &mut ctx,
        2,
        EvidenceKind::PaymentSlip,
        png,
        Some("unc.png"),
        Some("2026-03-24"),
    )
    .expect("upload png slip");
    assert!(slip.message.contains("Đã thanh toán"));
    assert_eq!(ctx.store().contracts[&2].status.as_str(), "PaidActive");
    assert!(slip.file_path.is_file());

    // Delete payment slip: file removed, 3 columns NULL, event recorded, status unchanged
    let deleted = delete_evidence(&mut ctx, 2, EvidenceKind::PaymentSlip).expect("delete slip");
    assert!(deleted);
    assert!(!slip.file_path.exists());
    let c = &ctx.store().contracts[&2];
    assert_eq!(c.payment_slip_path, None);
    assert_eq!(c.payment_slip_name, None);
    assert_eq!(c.payment_slip_uploaded_at, None);
    assert_eq!(c.status.as_str(), "PaidActive");
    assert!(
        ctx.store()
            .events
            .values()
            .any(|e| e.contract_id == 2 && e.event_type == "payment_slip_deleted")
    );
}
