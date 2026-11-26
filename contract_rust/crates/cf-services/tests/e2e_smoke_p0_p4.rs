//! Smoke test End-to-End xuyên suốt từ P0 -> P4 qua tầng cf-services và cf-db.
//! Kiểm tra toàn bộ vòng đời dữ liệu: Cài đặt -> Khách hàng / Cơ sở / Giấy tờ / Hồ sơ / Thùng rác
//! -> Sản phẩm / Excel -> Mẫu hợp đồng -> Hợp đồng / Sinh .docx / ZIP / 08a / Chứng từ / Trạng thái
//! -> Phụ lục / Gia hạn -> Sinh hàng loạt -> Báo giá (Danh mục / Tự nhập / Yêu cầu)
//! -> Dashboard / Sắp hết hạn / Tìm kiếm toàn cục -> Nạp lại DB từ đĩa.

use std::{collections::BTreeMap, io::Cursor, path::PathBuf, sync::atomic::AtomicBool};

use calamine::{Reader, Xlsx, open_workbook_from_rs};
use cf_core::{dates::compute_end_date, pricing::ItemInput, template_rules::Role};
use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
use cf_services::{
    Ctx,
    addenda::{
        addon::{AddonForm, AddonItem, KIND_ADDON, addendum_defaults, create_addendum},
        renewal::{create_renewal_appendix, create_renewal_contract},
        renewal_defaults::renewal_defaults,
    },
    batch::{
        excel::{import_excel_mapping, make_batch_template_xlsx},
        row::form_from_params,
        run::{BatchJob, run_batch},
    },
    contracts::{
        create::{ContractForm, CreateOptions, create_draft},
        evidence::{EvidenceKind, upload_evidence},
        export::export_contracts,
        form08a::render_form_08a,
        generate::generate_docx,
        list::{ContractFilters, list_contracts},
        preview::price_preview,
        status::change_status,
        suggest::suggest_number,
    },
    customers::{
        docs::upload_doc,
        edit::{create_customer, generate_next_code},
        info::{download_bundle, export_info},
        report::export_report,
        state::{delete_customer, list_deleted_customers, restore_customer},
        units::{UnitInput, save_billing, save_sub_units},
    },
    dashboard::{
        data::get_dashboard_data,
        expiring_page::{export_expiring_csv, get_expiring_contracts},
        search::api_search,
    },
    dossier::{build::materialize, plan::build_plan},
    products::{
        catalog_export::make_catalog_xlsx,
        catalog_import::{CatalogAction, parse_catalog_xlsx},
        edit::create_product,
    },
    quotations::{
        catalog::generate_catalog_quotation,
        issuers::{IssuerInput, save_issuer},
        manual::{build_document, render_document},
        request::{
            RequestContextInput, build_request_context, build_request_lists,
            install_sample_templates, list_request_templates, render_request_docx,
            request_filename, resolve_request_items, resolve_request_template,
        },
    },
    settings,
    templates::{bundled::seed_from_project_files, list::templates_for},
};
use chrono::{NaiveDate, NaiveDateTime};
use rust_xlsxwriter::Workbook;
use serde_json::{Value as Json, json};
use zip::ZipArchive;

fn project_assets_templates() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("ContractForge")
        .join("assets")
        .join("default_templates")
}

#[test]
fn full_p0_to_p4_end_to_end_smoke() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let paths = DataPaths::new(&root);
    paths.ensure_dirs().expect("ensure dirs");

    let db_path = cf_testkit::seeded_db(&root).expect("seed db");
    let store = Store::load(&db_path).expect("load store");
    let today = NaiveDate::from_ymd_opt(2026, 3, 24).expect("date");
    let now_utc =
        NaiveDateTime::parse_from_str("2026-03-24 04:30:00", "%Y-%m-%d %H:%M:%S").expect("utc");
    let now_local =
        NaiveDateTime::parse_from_str("2026-03-24 11:30:00", "%Y-%m-%d %H:%M:%S").expect("local");
    let clock = FixedClock {
        date: today,
        utc: now_utc,
        local: now_local,
    };
    let mut ctx = Ctx::new(store, paths, Box::new(clock));

    // ── 1. Settings (Bên B + Khuôn phụ lục) ───────────────────────────────
    settings::save_addendum_format(ctx.store_mut(), "PL{seq}/{parent}").expect("save format");
    settings::save_party_b(
        ctx.store_mut(),
        [
            ("party_b_name".into(), "Viettel Chi nhánh E2E".into()),
            ("party_b_address".into(), "Số 1 Đại lộ Thăng Long".into()),
            ("party_b_representative".into(), "Nguyễn Văn B".into()),
            ("party_b_title".into(), "Giám đốc".into()),
        ],
    )
    .expect("save party B");
    assert!(settings::is_organization_profile_complete(ctx.store()));

    // ── 2. Customers + Units + Docs + Dossier + Export + Trash ────────────
    let code = generate_next_code(ctx.store());
    let cid = create_customer(
        ctx.store_mut(),
        &BTreeMap::from([
            ("code".into(), Json::String(code.clone())),
            (
                "legal_name".into(),
                Json::String("Bệnh viện Đa khoa Smoke".into()),
            ),
            ("short_name".into(), Json::String("BV Smoke".into())),
            ("customer_type".into(), Json::String("Bệnh viện".into())),
            ("province".into(), Json::String("Hà Nội".into())),
            ("ward".into(), Json::String("Phường Dịch Vọng".into())),
            ("address".into(), Json::String("Số 10 Cầu Giấy".into())),
            ("tax_code".into(), Json::String("0101234567".into())),
            (
                "representative_name".into(),
                Json::String("Trần Văn A".into()),
            ),
            (
                "representative_title".into(),
                Json::String("Giám đốc".into()),
            ),
        ]),
        now_utc,
    )
    .expect("create customer");

    save_sub_units(
        ctx.store_mut(),
        cid,
        &[
            UnitInput {
                id: None,
                name: "Điểm khám A".into(),
                unit_type: "Điểm trạm".into(),
                address: "Cơ sở 1".into(),
                notes: "".into(),
                include_in_billing: Some(true),
            },
            UnitInput {
                id: None,
                name: "Điểm khám B".into(),
                unit_type: "Điểm trạm".into(),
                address: "Cơ sở 2".into(),
                notes: "".into(),
                include_in_billing: Some(false),
            },
        ],
        now_utc,
    )
    .expect("save units");
    let first_unit_id = ctx
        .store()
        .units
        .values()
        .find(|u| u.customer_id == cid && u.name == "Điểm khám A")
        .expect("unit A")
        .id;
    save_billing(ctx.store_mut(), cid, true, &[first_unit_id], now_utc).expect("save billing");

    upload_doc(
        &mut ctx,
        cid,
        "cccd_front",
        "",
        b"%PDF-1.4 front",
        "front.pdf",
    )
    .expect("front");
    upload_doc(&mut ctx, cid, "cccd_back", "", b"%PDF-1.4 back", "back.pdf").expect("back");
    upload_doc(
        &mut ctx,
        cid,
        "establishment",
        "",
        b"%PDF-1.4 est",
        "est.pdf",
    )
    .expect("est");

    let info_md = export_info(ctx.store(), cid, now_local).expect("info md");
    assert!(String::from_utf8_lossy(&info_md.content).contains("Bệnh viện Đa khoa Smoke"));
    let bundle_cust = download_bundle(&ctx, cid).expect("customer bundle");
    assert!(!bundle_cust.content.is_empty());

    let report_xlsx = export_report(ctx.store(), today).expect("customer report");
    let report_wb: Xlsx<_> = open_workbook_from_rs(Cursor::new(report_xlsx)).expect("open report");
    assert_eq!(report_wb.sheet_names().len(), 4);

    // ── 3. Products + Catalog Excel Round-Trip ────────────────────────────
    let pid_new = create_product(
        ctx.store_mut(),
        &json!({
            "code": "G1-SMOKE",
            "name": "Gói Smoke cùng nhóm G1",
            "product_type": "Phần mềm",
            "kind": "addon",
            "group_code": "G1",
            "unit_label": "Cơ sở",
            "default_price": "200000",
            "default_vat_rate": "8",
            "price_period_months": "1",
            "default_duration_months": "12",
            "quantity_source": "units"
        }),
        now_utc,
    )
    .expect("create addon product");
    let catalog_bytes = make_catalog_xlsx(Some(ctx.store()), true).expect("catalog xlsx");
    let parsed_catalog = parse_catalog_xlsx(ctx.store(), &catalog_bytes).expect("parse catalog");
    assert!(
        parsed_catalog
            .iter()
            .all(|r| r.action != CatalogAction::Error)
    );

    // ── 4. Templates (seed + lookup) ──────────────────────────────────────
    let seeded_tpls =
        seed_from_project_files(&mut ctx, &project_assets_templates()).expect("seed bundled");
    assert!(seeded_tpls > 0);

    // ── 5. Contracts: Suggest -> Preview -> Draft -> Generate -> 08a -> Evidence -> Status -> Addenda -> Renewal ──
    let main_pid = 1_i64; // Seeded G1-MAIN
    let items = vec![
        ItemInput {
            product_id: main_pid,
            description: String::new(),
            quantity: "2".into(),
            unit_price_pre_vat: None,
            vat_rate: None,
            amount_pre_vat_override: None,
            note: String::new(),
        },
        ItemInput {
            product_id: pid_new,
            description: String::new(),
            quantity: "2".into(),
            unit_price_pre_vat: None,
            vat_rate: None,
            amount_pre_vat_override: None,
            note: String::new(),
        },
    ];
    let preview = price_preview(ctx.store(), &items, 12, true).expect("price preview");
    assert_eq!(preview.lines.len(), 2);

    let suggested_num =
        suggest_number(ctx.store(), cid, Some(main_pid), 2026).expect("suggest num");
    let start = NaiveDate::from_ymd_opt(2026, 4, 1).expect("start");
    let end = compute_end_date(start, 12);
    let contract_id = create_draft(
        &mut ctx,
        ContractForm {
            contract_number: suggested_num.clone(),
            customer_id: cid,
            product_id: Some(main_pid),
            template_id: Some(1),
            sign_date: Some(today),
            start_date: Some(start),
            end_date: Some(end),
            month_count: Some(12),
            sign_location: Some("bệnh viện Đa khoa Smoke".into()),
            unit_count: Some(2),
            selected_unit_names: vec!["BV Smoke".into(), "Điểm khám A".into()],
            items,
            payment_mode: Some("postpaid".into()),
            payment_cycle_months: Some(6),
            party_b_representative: Some("Nguyễn Văn B".into()),
            ..ContractForm::default()
        },
        CreateOptions {
            template_group: Some("G1"),
            ..CreateOptions::default()
        },
    )
    .expect("create contract");

    let att_ids: Vec<i64> = templates_for(ctx.store(), Some("G1"), Role::Attachment, true)
        .into_iter()
        .map(|t| t.id)
        .take(1)
        .collect();
    let generated = generate_docx(&mut ctx, contract_id, &att_ids).expect("generate docx");
    assert!(generated.file_path.is_file());
    assert_eq!(
        ctx.store().contracts[&contract_id].status.as_str(),
        "Generated"
    );
    if let Some(zip_path) = &generated.bundle_path {
        assert!(zip_path.is_file());
    }

    let (form08a_bytes, form08a_name) = render_form_08a(ctx.store(), contract_id).expect("form08a");
    assert!(!form08a_bytes.is_empty());
    assert!(form08a_name.ends_with(".docx"));

    change_status(&mut ctx, contract_id, "Sent", "e2e", "").expect("to Sent");
    assert_eq!(ctx.store().contracts[&contract_id].status.as_str(), "Sent");
    upload_evidence(
        &mut ctx,
        contract_id,
        EvidenceKind::Scan,
        b"%PDF-1.4 scan",
        Some("signed_scan.pdf"),
        None,
    )
    .expect("upload scan");
    assert_eq!(
        ctx.store().contracts[&contract_id].status.as_str(),
        "Signed"
    );
    upload_evidence(
        &mut ctx,
        contract_id,
        EvidenceKind::Invoice,
        b"%PDF-1.4 inv",
        Some("invoice.pdf"),
        Some("2026-04-02"),
    )
    .expect("upload invoice");
    assert_eq!(
        ctx.store().contracts[&contract_id].status.as_str(),
        "Invoiced"
    );
    upload_evidence(
        &mut ctx,
        contract_id,
        EvidenceKind::PaymentSlip,
        b"%PDF-1.4 slip",
        Some("payment.pdf"),
        Some("2026-04-03"),
    )
    .expect("upload slip");
    assert_eq!(
        ctx.store().contracts[&contract_id].status.as_str(),
        "PaidActive"
    );

    // Dossier materialization with live contract docs + evidence
    let plan = build_plan(ctx.store(), ctx.paths(), cid).expect("dossier plan");
    assert!(!plan.contracts.is_empty());
    let mat = materialize(&ctx, &plan).expect("materialize dossier");
    assert!(mat.dest_dir.is_dir());
    assert!(mat.copied >= 4);

    // Addendum & Renewal on the PaidActive contract
    let add_start = NaiveDate::from_ymd_opt(2026, 7, 1).expect("add start");
    let _addon_defs =
        addendum_defaults(ctx.store(), contract_id, KIND_ADDON, add_start).expect("addon defs");
    let addon_id = create_addendum(
        &mut ctx,
        contract_id,
        KIND_ADDON,
        AddonForm {
            start_date: add_start,
            sign_date: Some(today),
            template_id: None,
            items: vec![AddonItem {
                product_id: pid_new,
                description: None,
                quantity: Some("1".into()),
                unit_price_pre_vat: None,
                vat_rate: None,
                amount_pre_vat_override: None,
                note: String::new(),
            }],
        },
    )
    .expect("create addendum");
    assert_eq!(
        ctx.store().contracts[&addon_id].parent_contract_id,
        Some(contract_id)
    );

    let ren_defs = renewal_defaults(ctx.store(), contract_id).expect("renewal defs");
    let renewal_app_id = create_renewal_appendix(
        &mut ctx,
        contract_id,
        AddonForm {
            start_date: ren_defs.start_date,
            sign_date: Some(today),
            template_id: None,
            items: vec![AddonItem {
                product_id: main_pid,
                description: None,
                quantity: None,
                unit_price_pre_vat: None,
                vat_rate: None,
                amount_pre_vat_override: None,
                note: String::new(),
            }],
        },
        12,
    )
    .expect("create renewal appendix");
    assert_eq!(
        ctx.store().contracts[&renewal_app_id].parent_contract_id,
        Some(contract_id)
    );

    let renewal_num = suggest_number(ctx.store(), cid, Some(main_pid), 2027).expect("suggest 2027");
    let renewal_ct_id = create_renewal_contract(
        &mut ctx,
        contract_id,
        &renewal_num,
        AddonForm {
            start_date: ren_defs.start_date,
            sign_date: Some(today),
            template_id: Some(1),
            items: vec![AddonItem {
                product_id: main_pid,
                description: None,
                quantity: None,
                unit_price_pre_vat: None,
                vat_rate: None,
                amount_pre_vat_override: None,
                note: String::new(),
            }],
        },
        12,
    )
    .expect("create renewal contract");
    assert_eq!(
        ctx.store().contracts[&renewal_ct_id].renewed_from_id,
        Some(contract_id)
    );

    let list_res = list_contracts(ctx.store(), &ContractFilters::default(), today);
    assert!(list_res.total >= 3);
    let exp_contracts = export_contracts(ctx.store(), &ContractFilters::default(), today)
        .expect("export contracts");
    assert!(!exp_contracts.bytes.is_empty());

    // ── 6. Batch (Form + Excel Import + Run + ZIP) ────────────────────────
    let _batch_tpl_xlsx = make_batch_template_xlsx().expect("batch tpl xlsx");
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (col, h) in [
        "Mã khách hàng",
        "Số hợp đồng",
        "Ngày ký",
        "Ngày bắt đầu",
        "Số tháng",
        "Mã gói chính",
    ]
    .iter()
    .enumerate()
    {
        ws.write_string(0, col as u16, *h).expect("header");
    }
    ws.write_string(1, 0, &code).expect("c1");
    ws.write_string(1, 1, "BATCH-E2E-01/2026").expect("n1");
    ws.write_string(1, 2, "24/03/2026").expect("sd1");
    ws.write_string(1, 3, "01/04/2026").expect("st1");
    ws.write_string(1, 4, "12").expect("m1");
    ws.write_string(1, 5, "G1-MAIN").expect("p1");
    let batch_xlsx_bytes = wb.save_to_buffer().expect("save batch xlsx");
    let lookup = BTreeMap::from([(code.clone(), cid)]);
    let parsed_batch = import_excel_mapping(&batch_xlsx_bytes, &lookup);
    assert_eq!(parsed_batch.rows.len(), 1);
    assert!(parsed_batch.errors.is_empty());

    let batch_form =
        form_from_params(ctx.store(), cid, 1, &parsed_batch.rows[0]).expect("form from params");
    let cancel = AtomicBool::new(false);
    let batch_res = run_batch(
        &mut ctx,
        BatchJob {
            id: "E2E-JOB-1".into(),
            template_id: 1,
            year: 2026,
            rows: vec![batch_form],
        },
        |_, _| {},
        &cancel,
    )
    .expect("run batch");
    assert_eq!(batch_res.succeeded, 1);
    assert_eq!(batch_res.failed, 0);
    let batch_zip = batch_res.zip_path.expect("batch zip");
    let zip_bytes = std::fs::read(&batch_zip).expect("read batch zip");
    let archive = ZipArchive::new(Cursor::new(zip_bytes)).expect("open batch zip");
    assert_eq!(archive.len(), 1);

    // ── 7. Quotations (Catalog / Manual / Request) ────────────────────────
    let cat_quote = generate_catalog_quotation(&mut ctx, cid, main_pid, 4, today, 12, &[])
        .expect("catalog quotation");
    assert!(!cat_quote.bytes.is_empty());

    save_issuer(
        &mut ctx,
        IssuerInput {
            name: "Viettel E2E".into(),
            address: "Hà Nội".into(),
            tax_code: "0100109106".into(),
            representative: "Nguyễn Văn B".into(),
            title: "Giám đốc".into(),
            phone: "0988000111".into(),
            email: "e2e@viettel.vn".into(),
        },
    )
    .expect("save issuer");

    let manual_doc = build_document(
        &BTreeMap::from([
            ("layout".into(), "compact".into()),
            ("issuer_name".into(), "Viettel E2E".into()),
            ("recipient_name".into(), "BV Smoke".into()),
            ("quote_date".into(), "24/03/2026".into()),
            (
                "items_json".into(),
                json!([{"name": "Dịch vụ E2E", "unit": "Tháng", "quantity": "1", "periods": "12", "price": "500000", "vat": "8"}]).to_string(),
            ),
        ]),
        &[],
    )
    .expect("build manual quote");
    let compact_bytes = std::fs::read(project_assets_templates().join("sample_quote_compact.docx"))
        .expect("read sample compact quote");
    let rendered_manual =
        render_document(&compact_bytes, &manual_doc).expect("render manual quote");
    assert!(!rendered_manual.is_empty());

    install_sample_templates(&mut ctx).expect("install req samples");
    let req_tpls = list_request_templates(ctx.store(), Some("G1"));
    let req_tpl_id = req_tpls.first().expect("at least one request template").id;
    let (req_items, req_features) = resolve_request_items(
        ctx.store(),
        cid,
        main_pid,
        12,
        "",
        "2",
        &[(pid_new, String::new(), String::new())],
    )
    .expect("resolve req items");
    let req_tpl =
        resolve_request_template(ctx.store(), req_tpl_id, Some("G1")).expect("resolve req tpl");
    let customer_clone = ctx.store().customers[&cid].clone();
    let units_clone: Vec<_> = ctx
        .store()
        .units
        .values()
        .filter(|u| u.customer_id == cid)
        .cloned()
        .collect();
    let req_ctx = build_request_context(
        &customer_clone,
        &units_clone,
        RequestContextInput {
            request_date: today,
            deadline: NaiveDate::from_ymd_opt(2026, 4, 10),
            month_count: 12,
            addressee: "Ban Giám đốc",
            subject: "Yêu cầu báo giá E2E",
            note: "Smoke note",
        },
    );
    let req_lists = build_request_lists(&req_items, &req_features);
    let req_bytes =
        render_request_docx(ctx.paths(), &req_tpl, &req_ctx, &req_lists).expect("render req docx");
    assert!(!req_bytes.is_empty());
    let _req_name = request_filename(&customer_clone, today);

    // ── 8. Dashboard, Expiring, Search, Customer Soft-Delete/Restore ──────
    let dash = get_dashboard_data(ctx.store(), today, now_utc);
    assert!(dash.summary.summary.total_contracts >= 5);
    let expiring = get_expiring_contracts(ctx.store(), today, 90);
    let (_csv_name, csv_bytes) = export_expiring_csv(&expiring, 90);
    assert!(!csv_bytes.is_empty());

    let hits = api_search(ctx.store(), "Smoke", None);
    assert!(hits.iter().any(|h| h.kind == "customer" && h.id == cid));
    assert!(hits.iter().any(|h| h.kind == "contract"));

    // Soft-delete customer -> appears in trash -> restore back
    assert!(delete_customer(ctx.store_mut(), cid, now_utc).expect("soft delete"));
    assert!(
        list_deleted_customers(ctx.store())
            .iter()
            .any(|c| c.id == cid)
    );
    restore_customer(ctx.store_mut(), cid, now_utc).expect("restore customer");
    assert!(!ctx.store().customers[&cid].is_deleted);

    // ── 9. Reload SQLite from disk to verify persistence ──────────────────
    let reloaded = Store::load(&db_path).expect("reload store from disk");
    assert!(reloaded.customers.contains_key(&cid));
    assert!(reloaded.contracts.contains_key(&contract_id));
    assert!(reloaded.contracts.contains_key(&addon_id));
    assert!(reloaded.contracts.contains_key(&renewal_app_id));
    assert!(reloaded.contracts.contains_key(&renewal_ct_id));
    assert!(settings::is_organization_profile_complete(&reloaded));
}
