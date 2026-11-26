//! S15 — chi tiết hợp đồng: đổi trạng thái, chứng từ, sinh file + kèm theo lẻ, biểu 08a, mở file/ZIP, phụ lục & gia hạn.
//! Trạng thái màn hình chỉ sống trên luồng UI; đọc/ghi Store chạy trên worker.

use std::{cell::RefCell, collections::BTreeSet, path::PathBuf, rc::Rc};

use cf_core::{
    dates::parse_date_input,
    fmt::fmt_currency,
    lifecycle::{ACTIVE_LIFECYCLE, valid_targets},
    template_rules::Role,
};
use cf_services::{
    Ctx, ServiceError,
    addenda::addon::can_add_addendum,
    contracts::{
        attachments::render_attachments,
        delete::delete_contract,
        evidence::{EvidenceKind, delete_evidence, upload_evidence},
        form08a::render_form_08a,
        generate::generate_docx,
        status::change_status,
    },
    templates::list::templates_for,
};
use chrono::NaiveDate;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::{
    ContractEvidenceSlot, ContractHub, ContractItemViewRow, CustomerHub, IdLabel, MainWindow,
    RelatedLink, files, nav,
    ui_helpers::{Feedback, ToastKind, post_to_ui, toast_from_worker},
    view,
    worker::Worker,
};

const KINDS: [EvidenceKind; 3] = [
    EvidenceKind::Scan,
    EvidenceKind::Invoice,
    EvidenceKind::PaymentSlip,
];
const EVIDENCE_LABELS: [&str; 3] = ["Scan hợp đồng đã ký", "Hóa đơn", "Ủy nhiệm chi"];
const ACTOR: &str = "người dùng";

#[derive(Debug)]
struct Detail {
    id: i64,
    title: String,
    info: String,
    status_label: String,
    status_variant: &'static str,
    is_expiring: bool,
    has_scan: bool,
    customer_id: i64,
    customer_name: String,
    customer_addr: String,
    customer_rep: String,
    sign_date: String,
    start_date: String,
    end_date: String,
    months: String,
    product_name: String,
    product_group: String,
    pricing_model: String,
    items: Vec<ContractItemViewRow>,
    pre_vat: String,
    vat: String,
    total: String,
    amount_words: String,
    payment_text: String,
    selected_units: Vec<SharedString>,
    unit_count: String,
    unit_price_pre: String,
    vat_rate: String,
    unit_price_vat: String,
    template_id: i64,
    template_name: String,
    targets: Vec<(String, String)>,
    evidence: [Option<String>; 3],
    evidence_paths: [Option<PathBuf>; 3],
    evidence_slots: [ContractEvidenceSlot; 3],
    invoice_date_input: String,
    payment_date_input: String,
    generated: Option<String>,
    docx_path: Option<PathBuf>,
    bundle_path: Option<PathBuf>,
    can_add_addendum: bool,
    can_renew: bool,
    related: Vec<(i64, String)>,
    attachments: Vec<(i64, String, bool)>,
    events: Vec<String>,
}

struct Screen {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    feedback: Rc<Feedback>,
    contract_id: Option<i64>,
    targets: Vec<(String, String)>,
    docx_path: Option<PathBuf>,
    bundle_path: Option<PathBuf>,
    evidence_paths: [Option<PathBuf>; 3],
    attachments: Vec<(i64, String)>,
    selected_attachments: BTreeSet<i64>,
}

thread_local! {
    static SCREEN: RefCell<Option<Screen>> = const { RefCell::new(None) };
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>, feedback: &Rc<Feedback>) {
    SCREEN.with(|cell| {
        *cell.borrow_mut() = Some(Screen {
            ui: ui.as_weak(),
            worker: worker.clone(),
            feedback: Rc::clone(feedback),
            contract_id: None,
            targets: Vec::new(),
            docx_path: None,
            bundle_path: None,
            evidence_paths: [None, None, None],
            attachments: Vec::new(),
            selected_attachments: BTreeSet::new(),
        });
    });

    ui.on_contracts_open(|id| open(i64::from(id)));
    ui.on_contract_detail_back(close);
    ui.on_contract_detail_target(|index| {
        let code = with_screen(|screen| {
            usize::try_from(index)
                .ok()
                .and_then(|index| screen.targets.get(index))
                .map(|(code, _)| code.clone())
        })
        .flatten();
        if let Some(code) = code {
            change_to(code);
        }
    });
    ui.on_contract_detail_upload(|index| upload(index, None));
    ui.on_contract_detail_remove(ask_remove);
    ui.on_contract_detail_generate(generate);
    ui.on_contract_detail_delete(ask_delete);

    let hub = ui.global::<ContractHub>();
    hub.on_back_to_contract_detail(|| {
        if let Some(ui) = with_screen(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
            ui.global::<ContractHub>()
                .set_detail_submode("detail".into());
        }
    });
    hub.on_open_related(|id| open(i64::from(id)));
    hub.on_open_customer(|cid| {
        if cid <= 0 {
            return;
        }
        if let Some(ui) = with_screen(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
            ui.invoke_navigate(nav::Screen::Customers.index());
            ui.global::<CustomerHub>().invoke_open_detail(cid);
        }
    });
    hub.on_open_template(|tid| {
        if tid <= 0 {
            return;
        }
        if let Some(ui) = with_screen(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
            ui.invoke_navigate(nav::Screen::Templates.index());
            ui.invoke_template_open(tid);
        }
    });
    hub.on_open_docx(|| open_stored_file(false));
    hub.on_open_folder(open_stored_folder);
    hub.on_open_bundle(|| open_stored_file(true));
    hub.on_view_evidence(view_evidence);
    hub.on_upload_evidence_dated(|index, date_raw| {
        let raw = date_raw.trim();
        let formatted = if raw.is_empty() {
            None
        } else {
            match parse_date_input(Some(raw)) {
                Ok(Some(d)) => Some(d.format("%Y-%m-%d").to_string()),
                Ok(None) => None,
                Err(e) => {
                    toast_from_worker(ToastKind::Danger, format!("Ngày không hợp lệ: {e}"));
                    return;
                }
            }
        };
        upload(index, formatted);
    });
    hub.on_confirm_generate(|| {
        let Some(feedback) = with_screen(|s| Rc::clone(&s.feedback)) else {
            return;
        };
        feedback.confirm(
            "Sinh file .docx",
            "Sinh file .docx? File cũ (nếu có) sẽ bị ghi đè.",
            generate,
        );
    });
    hub.on_toggle_regen_attachment(|idx| {
        with_screen(|s| {
            if let Ok(i) = usize::try_from(idx)
                && let Some((id, _)) = s.attachments.get(i)
                && !s.selected_attachments.remove(id)
            {
                s.selected_attachments.insert(*id);
            }
        });
        push_regen_attachments();
    });
    hub.on_download_single_attachment(|tpl_id| download_single_attachment(i64::from(tpl_id)));
    hub.on_download_08a(download_08a);
}

fn with_screen<R>(action: impl FnOnce(&mut Screen) -> R) -> Option<R> {
    SCREEN.with(|cell| cell.borrow_mut().as_mut().map(action))
}

pub fn current_id() -> Option<i64> {
    with_screen(|screen| screen.contract_id).flatten()
}

pub fn open(id: i64) {
    with_screen(|screen| {
        screen.contract_id = Some(id);
        if let Some(ui) = screen.ui.upgrade() {
            ui.global::<ContractHub>()
                .set_detail_submode("detail".into());
        }
    });
    reload();
}

fn close() {
    with_screen(|screen| screen.contract_id = None);
    if let Some(ui) = with_screen(|screen| screen.ui.clone()).and_then(|weak| weak.upgrade()) {
        ui.global::<ContractHub>()
            .set_detail_submode("detail".into());
        ui.set_contract_detail_id(0);
    }
}

fn reload() {
    let Some((worker, id)) =
        with_screen(|screen| screen.contract_id.map(|id| (screen.worker.clone(), id))).flatten()
    else {
        return;
    };
    worker.run(
        move |ctx| load(ctx, id),
        move |result| post_to_ui(move || apply(result)),
    );
}

fn make_evidence_slot(
    ctx: &Ctx,
    rel_path: Option<&str>,
    name: Option<&str>,
    date: Option<NaiveDate>,
    uploaded_at: Option<chrono::NaiveDateTime>,
) -> (Option<PathBuf>, ContractEvidenceSlot) {
    let resolved = rel_path
        .map(|p| ctx.paths().resolve_stored(p))
        .filter(|p| p.is_file());
    let has_file = resolved.is_some();
    let file_name = name.unwrap_or_default().to_owned();
    let is_pdf = file_name.to_ascii_lowercase().ends_with(".pdf");
    let slot = ContractEvidenceSlot {
        has_file,
        file_name: file_name.into(),
        date_label: date
            .map(|d| d.format("%d/%m/%Y").to_string())
            .unwrap_or_default()
            .into(),
        uploaded_at: uploaded_at
            .map(|dt| dt.format("%d/%m/%Y %H:%M").to_string())
            .unwrap_or_default()
            .into(),
        is_pdf,
    };
    (resolved, slot)
}

fn load(ctx: &mut Ctx, id: i64) -> Result<Detail, String> {
    let store = ctx.store();
    let today = ctx.clock().today();
    let contract = store
        .contracts
        .get(&id)
        .ok_or_else(|| "Không tìm thấy hợp đồng".to_owned())?;
    let cust = store.customers.get(&contract.customer_id);
    let customer_name = cust
        .map(|c| c.legal_name.clone())
        .unwrap_or_else(|| "—".into());
    let customer_addr = cust
        .map(|c| {
            [
                c.address.as_deref(),
                c.ward.as_deref(),
                c.province.as_deref(),
            ]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
        })
        .unwrap_or_default();
    let customer_rep = cust
        .and_then(|c| c.representative_name.clone())
        .unwrap_or_default();
    let product = contract
        .product_id
        .and_then(|product| store.products.get(&product));
    let product_name = product
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "—".into());
    let product_group = product
        .and_then(|p| p.group_code.clone())
        .unwrap_or_else(|| "—".into());
    let tpl = contract
        .template_id
        .and_then(|tid| store.templates.get(&tid));
    let template_id = tpl.map(|t| t.id).unwrap_or(0);
    let template_name = tpl.map(|t| t.name.clone()).unwrap_or_default();
    let targets = valid_targets(&contract.status)
        .into_iter()
        .map(|status| (status.as_str().to_owned(), status.label().to_owned()))
        .collect();
    let info = format!(
        "Khách hàng: {customer_name}\nGói chính: {product_name}\nNgày ký: {} · Hiệu lực: {} → {}\nTổng tiền: {} · Số tháng: {} · Số cơ sở: {}",
        view::date_text(contract.sign_date),
        view::date_text(contract.start_date),
        view::date_text(contract.end_date),
        view::money_text(contract.total_amount),
        contract
            .month_count
            .map_or_else(String::new, |m| m.to_string()),
        contract
            .unit_count
            .map_or_else(|| "1".into(), |u| u.to_string()),
    );

    let raw_items = store.indexes.items_of(store, id);
    let items = raw_items
        .into_iter()
        .enumerate()
        .map(|(i, it)| ContractItemViewRow {
            idx: i32::try_from(i + 1).unwrap_or(0),
            description: it.description.as_str().into(),
            unit: it.unit_label.as_deref().unwrap_or_default().into(),
            quantity: it.quantity.normalize().to_string().into(),
            periods: it.period_count.normalize().to_string().into(),
            unit_price: fmt_currency(it.unit_price_pre_vat).into(),
            pre_vat: fmt_currency(Some(it.amount_pre_vat)).into(),
            vat_rate: format!("{}%", it.vat_rate.normalize()).into(),
            vat_amount: fmt_currency(Some(it.vat_amount)).into(),
            total: fmt_currency(Some(it.amount_total)).into(),
        })
        .collect();
    let payment_text = match contract.payment_mode.as_deref() {
        Some("prepaid") => {
            if let Some(cy) = contract.payment_cycle_months {
                format!("Thanh toán: trả trước, chu kỳ {cy} tháng")
            } else {
                "Thanh toán: trả trước".to_owned()
            }
        }
        Some("postpaid") => {
            if let Some(cy) = contract.payment_cycle_months {
                format!("Thanh toán: trả sau, chu kỳ {cy} tháng")
            } else {
                "Thanh toán: trả sau".to_owned()
            }
        }
        _ => String::new(),
    };
    let mut events = store
        .indexes
        .events_of(store, id)
        .into_iter()
        .map(|event| {
            format!(
                "{}  {}  {}",
                event.occurred_at.format("%d/%m/%Y %H:%M"),
                event
                    .description
                    .clone()
                    .unwrap_or_else(|| event.event_type.clone()),
                event.actor.clone().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    events.reverse();

    let docx_path = contract
        .output_file_path
        .as_deref()
        .map(|p| ctx.paths().resolve_stored(p))
        .filter(|p| p.is_file());
    let bundle_dir = ctx.paths().output_bundles.join(id.to_string());
    let bundle_path = std::fs::read_dir(&bundle_dir).ok().and_then(|entries| {
        entries
            .flatten()
            .map(|e| e.path())
            .find(|p| p.is_file() && p.extension().is_some_and(|e| e == "zip"))
    });

    let mut related = Vec::new();
    if let Some(pid) = contract.parent_contract_id
        && let Some(parent) = store.contracts.get(&pid)
    {
        related.push((parent.id, format!("HĐ gốc: {}", parent.contract_number)));
    }
    if let Some(rid) = contract.renewed_from_id
        && let Some(prev) = store.contracts.get(&rid)
    {
        related.push((prev.id, format!("Gia hạn từ: {}", prev.contract_number)));
    }
    for child in store.indexes.children_of(store, id) {
        related.push((child.id, format!("Phụ lục: {}", child.contract_number)));
    }
    for succ in store.indexes.renewals_of(store, id) {
        related.push((succ.id, format!("HĐ gia hạn: {}", succ.contract_number)));
    }

    let root_contract = contract
        .parent_contract_id
        .and_then(|pid| store.contracts.get(&pid))
        .unwrap_or(contract);
    let group = root_contract
        .product_id
        .and_then(|pid| store.products.get(&pid))
        .and_then(|p| p.group_code.as_deref());
    let attachments = templates_for(store, group, Role::Attachment, true)
        .into_iter()
        .map(|t| {
            let preselect = cf_core::template_rules::preselected_attachment(&t.to_info());
            (t.id, format!("{} ({})", t.name, t.contract_type), preselect)
        })
        .collect();

    let add_ok = can_add_addendum(contract);
    let renew_ok = contract.doc_kind == "contract"
        && contract.start_date.is_some()
        && contract.end_date.is_some()
        && cf_core::lifecycle::IN_SERVICE.contains(&contract.status);

    let is_expiring = contract.end_date.is_some_and(|end| {
        let days = (end - today).num_days();
        days > 0 && days <= 60
    }) && ACTIVE_LIFECYCLE.contains(&contract.status);

    let (scan_path, scan_slot) = make_evidence_slot(
        ctx,
        contract.signed_pdf_path.as_deref(),
        contract.signed_pdf_name.as_deref(),
        None,
        contract.signed_pdf_uploaded_at,
    );
    let (inv_path, inv_slot) = make_evidence_slot(
        ctx,
        contract.invoice_pdf_path.as_deref(),
        contract.invoice_pdf_name.as_deref(),
        contract.invoice_date,
        contract.invoice_pdf_uploaded_at,
    );
    let (pay_path, pay_slot) = make_evidence_slot(
        ctx,
        contract.payment_slip_path.as_deref(),
        contract.payment_slip_name.as_deref(),
        contract.payment_date,
        contract.payment_slip_uploaded_at,
    );

    Ok(Detail {
        id,
        title: contract.contract_number.clone(),
        info,
        status_label: contract.status.label().to_owned(),
        status_variant: view::variant_of(contract.status.color()),
        is_expiring,
        has_scan: scan_path.is_some(),
        customer_id: contract.customer_id,
        customer_name,
        customer_addr,
        customer_rep,
        sign_date: view::date_text(contract.sign_date),
        start_date: view::date_text(contract.start_date),
        end_date: view::date_text(contract.end_date),
        months: contract
            .month_count
            .map_or_else(|| "—".into(), |m| format!("{m} tháng")),
        product_name,
        product_group,
        pricing_model: contract.pricing_model.clone(),
        items,
        pre_vat: view::money_text(contract.total_pre_vat),
        vat: view::money_text(contract.total_vat),
        total: view::money_text(contract.total_amount),
        amount_words: contract.amount_text.clone().unwrap_or_default(),
        payment_text,
        selected_units: contract
            .selected_unit_names
            .iter()
            .map(|u| SharedString::from(u.as_str()))
            .collect(),
        unit_count: contract
            .unit_count
            .map_or_else(|| "—".into(), |u| u.to_string()),
        unit_price_pre: view::money_text(contract.unit_price_pre_vat),
        vat_rate: contract
            .vat_rate
            .map_or_else(|| "—".into(), |v| format!("{}%", v.normalize())),
        unit_price_vat: view::money_text(contract.unit_price_vat),
        template_id,
        template_name,
        targets,
        evidence: [
            contract.signed_pdf_name.clone(),
            contract.invoice_pdf_name.clone(),
            contract.payment_slip_name.clone(),
        ],
        evidence_paths: [scan_path, inv_path, pay_path],
        evidence_slots: [scan_slot, inv_slot, pay_slot],
        invoice_date_input: view::date_text(contract.invoice_date),
        payment_date_input: view::date_text(contract.payment_date),
        generated: contract.output_file_name.clone(),
        docx_path,
        bundle_path,
        can_add_addendum: add_ok,
        can_renew: renew_ok,
        related,
        attachments,
        events,
    })
}

fn apply(result: Result<Detail, String>) {
    let Some(ui_weak) = with_screen(|screen| screen.ui.clone()) else {
        return;
    };
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };
    match result {
        Ok(detail) => {
            with_screen(|screen| {
                screen.targets = detail.targets.clone();
                screen.docx_path = detail.docx_path.clone();
                screen.bundle_path = detail.bundle_path.clone();
                screen.evidence_paths = detail.evidence_paths.clone();
                screen.attachments = detail
                    .attachments
                    .iter()
                    .map(|(id, name, _)| (*id, name.clone()))
                    .collect();
                screen.selected_attachments = detail
                    .attachments
                    .iter()
                    .filter(|(_, _, pre)| *pre)
                    .map(|(id, _, _)| *id)
                    .collect();
            });
            ui.set_contract_detail_id(i32::try_from(detail.id).unwrap_or(i32::MAX));
            ui.set_contract_detail_title(detail.title.into());
            ui.set_contract_detail_info(detail.info.into());
            ui.set_contract_detail_status(detail.status_label.into());
            ui.set_contract_detail_status_variant(detail.status_variant.into());
            ui.set_contract_detail_target_names(ModelRc::new(VecModel::from(
                detail
                    .targets
                    .iter()
                    .map(|(_, label)| SharedString::from(label.as_str()))
                    .collect::<Vec<_>>(),
            )));
            ui.set_contract_detail_evidence(ModelRc::new(VecModel::from(
                EVIDENCE_LABELS
                    .iter()
                    .zip(detail.evidence.iter())
                    .map(|(label, name)| {
                        SharedString::from(
                            match name {
                                Some(name) => format!("{label}: {name}"),
                                None => format!("{label}: chưa có"),
                            }
                            .as_str(),
                        )
                    })
                    .collect::<Vec<_>>(),
            )));
            ui.set_contract_detail_generated(
                detail
                    .generated
                    .unwrap_or_else(|| "Chưa có file — nhấn \"Sinh file .docx\"".to_owned())
                    .into(),
            );
            ui.set_contract_detail_events(ModelRc::new(VecModel::from(
                detail
                    .events
                    .iter()
                    .map(|line| SharedString::from(line.as_str()))
                    .collect::<Vec<_>>(),
            )));

            let hub = ui.global::<ContractHub>();
            hub.set_can_add_addendum(detail.can_add_addendum);
            hub.set_can_renew(detail.can_renew);
            hub.set_has_docx(detail.docx_path.is_some());
            hub.set_has_bundle(detail.bundle_path.is_some());
            hub.set_detail_is_expiring(detail.is_expiring);
            hub.set_detail_has_scan(detail.has_scan);
            hub.set_detail_customer_id(i32::try_from(detail.customer_id).unwrap_or(0));
            hub.set_detail_customer_name(detail.customer_name.into());
            hub.set_detail_customer_addr(detail.customer_addr.into());
            hub.set_detail_customer_rep(detail.customer_rep.into());
            hub.set_detail_sign_date(detail.sign_date.into());
            hub.set_detail_start_date(detail.start_date.into());
            hub.set_detail_end_date(detail.end_date.into());
            hub.set_detail_months(detail.months.into());
            hub.set_detail_product_name(detail.product_name.into());
            hub.set_detail_product_group(detail.product_group.into());
            hub.set_detail_pricing_model(detail.pricing_model.into());
            hub.set_detail_items(ModelRc::new(VecModel::from(detail.items)));
            hub.set_detail_pre_vat(detail.pre_vat.into());
            hub.set_detail_vat(detail.vat.into());
            hub.set_detail_total(detail.total.into());
            hub.set_detail_amount_words(detail.amount_words.into());
            hub.set_detail_payment_text(detail.payment_text.into());
            hub.set_detail_selected_units(ModelRc::new(VecModel::from(detail.selected_units)));
            hub.set_detail_unit_count(detail.unit_count.into());
            hub.set_detail_unit_price_pre(detail.unit_price_pre.into());
            hub.set_detail_vat_rate(detail.vat_rate.into());
            hub.set_detail_unit_price_vat(detail.unit_price_vat.into());
            hub.set_detail_template_id(i32::try_from(detail.template_id).unwrap_or(0));
            hub.set_detail_template_name(detail.template_name.into());
            let [scan, inv, pay] = detail.evidence_slots;
            hub.set_detail_scan(scan);
            hub.set_detail_invoice(inv);
            hub.set_detail_payment_slip(pay);
            hub.set_detail_invoice_date_input(detail.invoice_date_input.into());
            hub.set_detail_payment_date_input(detail.payment_date_input.into());
            hub.set_related_links(ModelRc::new(VecModel::from(
                detail
                    .related
                    .into_iter()
                    .map(|(id, label)| RelatedLink {
                        id: i32::try_from(id).unwrap_or(0),
                        label: label.into(),
                    })
                    .collect::<Vec<_>>(),
            )));
            let single_rows = detail
                .attachments
                .iter()
                .map(|(id, name, _)| IdLabel {
                    id: i32::try_from(*id).unwrap_or(0),
                    label: name.clone().into(),
                    checked: false,
                })
                .collect::<Vec<_>>();
            hub.set_single_attachments(ModelRc::new(VecModel::from(single_rows)));
            push_regen_attachments();
        }
        Err(message) => toast_from_worker(ToastKind::Danger, message),
    }
}

fn push_regen_attachments() {
    let payload = with_screen(|s| {
        let rows = s
            .attachments
            .iter()
            .map(|(id, name)| IdLabel {
                id: i32::try_from(*id).unwrap_or(0),
                label: name.clone().into(),
                checked: s.selected_attachments.contains(id),
            })
            .collect::<Vec<_>>();
        (s.ui.clone(), rows)
    });
    if let Some((ui, rows)) = payload
        && let Some(ui) = ui.upgrade()
    {
        ui.global::<ContractHub>()
            .set_regen_attachments(ModelRc::new(VecModel::from(rows)));
    }
}

fn open_stored_file(is_bundle: bool) {
    let path = with_screen(|s| {
        if is_bundle {
            s.bundle_path.clone()
        } else {
            s.docx_path.clone()
        }
    })
    .flatten();
    let Some(path) = path else {
        toast_from_worker(ToastKind::Warning, "File không tồn tại.".into());
        return;
    };
    if let Err(e) = files::open_path(&path) {
        toast_from_worker(ToastKind::Danger, format!("Không mở được file: {e}"));
    }
}

fn open_stored_folder() {
    let Some(path) = with_screen(|s| s.docx_path.clone()).flatten() else {
        return;
    };
    if let Err(e) = files::open_folder(&path) {
        toast_from_worker(ToastKind::Danger, format!("Không mở được thư mục: {e}"));
    }
}

fn view_evidence(index: i32) {
    let path = with_screen(|s| {
        usize::try_from(index)
            .ok()
            .and_then(|i| s.evidence_paths.get(i))
            .cloned()
            .flatten()
    })
    .flatten();
    let Some(path) = path else {
        toast_from_worker(ToastKind::Warning, "File chứng từ không tồn tại.".into());
        return;
    };
    if let Err(e) = files::open_path(&path) {
        toast_from_worker(ToastKind::Danger, format!("Không mở được chứng từ: {e}"));
    }
}

fn download_single_attachment(template_id: i64) {
    let Some((worker, cid)) =
        with_screen(|s| s.contract_id.map(|id| (s.worker.clone(), id))).flatten()
    else {
        return;
    };
    worker.run(
        move |ctx| {
            let tpl = ctx
                .store()
                .templates
                .get(&template_id)
                .cloned()
                .ok_or_else(|| ServiceError::NotFound("Không tìm thấy mẫu.".into()))?;
            let mut items = render_attachments(ctx, cid, &[tpl])?;
            items
                .pop()
                .ok_or_else(|| ServiceError::Invalid("Không sinh được văn bản.".into()))
        },
        |res| {
            post_to_ui(move || match res {
                Ok((filename, bytes)) => {
                    if let Some(path) = files::save_as(&filename, ("Word (.docx)", &["docx"])) {
                        match files::write_chosen(&path, &bytes) {
                            Ok(()) => toast_from_worker(
                                ToastKind::Success,
                                format!("Đã lưu {}", path.display()),
                            ),
                            Err(e) => toast_from_worker(
                                ToastKind::Danger,
                                format!("Không ghi được file: {e}"),
                            ),
                        }
                    }
                }
                Err(e) => toast_from_worker(ToastKind::Danger, e.user_message()),
            });
        },
    );
}

fn download_08a() {
    let Some((worker, cid)) =
        with_screen(|s| s.contract_id.map(|id| (s.worker.clone(), id))).flatten()
    else {
        return;
    };
    worker.run(
        move |ctx| render_form_08a(ctx.store(), cid),
        |res| {
            post_to_ui(move || match res {
                Ok((bytes, filename)) => {
                    if let Some(path) = files::save_as(&filename, ("Word (.docx)", &["docx"])) {
                        match files::write_chosen(&path, &bytes) {
                            Ok(()) => toast_from_worker(
                                ToastKind::Success,
                                format!("Đã lưu {}", path.display()),
                            ),
                            Err(e) => toast_from_worker(
                                ToastKind::Danger,
                                format!("Không ghi được file: {e}"),
                            ),
                        }
                    }
                }
                Err(e) => toast_from_worker(ToastKind::Danger, e.user_message()),
            });
        },
    );
}

fn change_to(target: String) {
    let Some((worker, id)) =
        with_screen(|screen| screen.contract_id.map(|id| (screen.worker.clone(), id))).flatten()
    else {
        return;
    };
    worker.run(
        move |ctx| change_status(ctx, id, &target, ACTOR, ""),
        move |result: Result<(), ServiceError>| {
            match result {
                Ok(()) => {
                    toast_from_worker(ToastKind::Success, "Đã đổi trạng thái hợp đồng.".to_owned())
                }
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
            post_to_ui(reload);
        },
    );
}

fn upload(index: i32, date_iso: Option<String>) {
    let Some(kind) = usize::try_from(index)
        .ok()
        .and_then(|index| KINDS.get(index))
        .copied()
    else {
        return;
    };
    let Some(id) = current_id() else {
        return;
    };
    let filters: &[(&str, &[&str])] = match kind {
        EvidenceKind::Scan | EvidenceKind::Invoice => &[("PDF", &["pdf"])],
        EvidenceKind::PaymentSlip => &[("PDF hoặc ảnh", &["pdf", "png", "jpg", "jpeg"])],
    };
    let Some(path) = files::pick_file(filters) else {
        return;
    };
    send_upload(id, kind, path, date_iso);
}

fn send_upload(id: i64, kind: EvidenceKind, path: PathBuf, date_iso: Option<String>) {
    let Some(worker) = with_screen(|screen| screen.worker.clone()) else {
        return;
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned);
    worker.run(
        move |ctx| {
            let bytes = std::fs::read(&path)
                .map_err(|error| ServiceError::FileMissing(error.to_string()))?;
            upload_evidence(ctx, id, kind, &bytes, name.as_deref(), date_iso.as_deref())
        },
        move |result| {
            match result {
                Ok(outcome) => toast_from_worker(ToastKind::Success, outcome.message),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
            post_to_ui(reload);
        },
    );
}

fn ask_remove(index: i32) {
    let Some(kind) = usize::try_from(index)
        .ok()
        .and_then(|index| KINDS.get(index))
        .copied()
    else {
        return;
    };
    let Some((id, feedback)) = with_screen(|screen| {
        screen
            .contract_id
            .map(|id| (id, Rc::clone(&screen.feedback)))
    })
    .flatten() else {
        return;
    };
    let label = EVIDENCE_LABELS[index as usize];
    feedback.confirm(
        "Xóa chứng từ",
        &format!("Xóa file \"{label}\" của hợp đồng này?"),
        move || remove_evidence(id, kind),
    );
}

fn remove_evidence(id: i64, kind: EvidenceKind) {
    let Some(worker) = with_screen(|screen| screen.worker.clone()) else {
        return;
    };
    worker.run(
        move |ctx| delete_evidence(ctx, id, kind),
        move |result| {
            match result {
                Ok(_) => toast_from_worker(ToastKind::Success, "Đã xóa chứng từ.".to_owned()),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
            post_to_ui(reload);
        },
    );
}

fn generate() {
    let Some((worker, id, attachment_ids)) = with_screen(|screen| {
        screen.contract_id.map(|id| {
            (
                screen.worker.clone(),
                id,
                screen
                    .selected_attachments
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
            )
        })
    })
    .flatten() else {
        return;
    };
    worker.run(
        move |ctx| generate_docx(ctx, id, &attachment_ids),
        move |result| {
            match result {
                Ok(outcome) => {
                    toast_from_worker(ToastKind::Success, format!("Đã sinh {}", outcome.file_name))
                }
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
            post_to_ui(reload);
        },
    );
}

fn ask_delete() {
    let Some((id, feedback)) = with_screen(|screen| {
        screen
            .contract_id
            .map(|id| (id, Rc::clone(&screen.feedback)))
    })
    .flatten() else {
        return;
    };
    feedback.confirm(
        "Xóa hợp đồng",
        "Xóa hợp đồng này? Hợp đồng còn phụ lục sẽ không xóa được. Giấy tờ của khách hàng vẫn giữ nguyên.",
        move || run_delete(id),
    );
}

fn run_delete(id: i64) {
    let Some((worker, weak)) = with_screen(|screen| (screen.worker.clone(), screen.ui.clone()))
    else {
        return;
    };
    worker.run(
        move |ctx| delete_contract(ctx, id),
        move |result| {
            match result {
                Ok(_) => toast_from_worker(ToastKind::Warning, "Đã xóa hợp đồng.".to_owned()),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
            post_to_ui(move || {
                close();
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_contracts_reload();
                }
            });
        },
    );
}
