//! S05 — chi tiết khách hàng: bố cục 2 cột, hồ sơ pháp lý, 3 slot giấy tờ + khác, hợp đồng, cơ sở tính phí, xuất thông tin/gói ZIP.

use std::{cell::RefCell, rc::Rc};

use cf_core::fmt::fmt_currency;
use cf_services::{
    Ctx,
    customers::{
        docs::{delete_doc, doc_path, upload_doc},
        info::{download_bundle, export_info},
        state::toggle_active,
        units::save_billing,
    },
};
use slint::{ComponentHandle, ModelRc, VecModel, Weak};

use crate::{
    CustomerContractRowData, CustomerDetailDocSlot, CustomerDetailProfile, CustomerDocRowData,
    CustomerHub, CustomerUnitRowData, MainWindow, files,
    nav::Screen,
    ui_helpers::{Feedback, ToastKind, post_to_ui, toast_from_worker},
    view,
    worker::Worker,
};

struct UnitBilling {
    id: i64,
    name: String,
    unit_type: String,
    active: bool,
    checked: bool,
}

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    feedback: Rc<Feedback>,
    customer_id: Option<i64>,
    is_active: bool,
    bill_main: bool,
    units: Vec<UnitBilling>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>, feedback: &Rc<Feedback>) {
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
            feedback: Rc::clone(feedback),
            customer_id: None,
            is_active: true,
            bill_main: true,
            units: Vec::new(),
        });
    });

    let hub = ui.global::<CustomerHub>();
    hub.on_open_detail(|id| open(i64::from(id)));
    hub.on_back_to_list(|| {
        if let Some(ui) = with_state(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
            ui.global::<CustomerHub>().set_mode("list".into());
        }
    });
    hub.on_detail_toggle_active(ask_toggle);
    hub.on_detail_toggle_bill_main(|checked| {
        with_state(|s| s.bill_main = checked);
    });
    hub.on_detail_toggle_billing_unit(|index| {
        with_state(|s| {
            if let Ok(i) = usize::try_from(index)
                && let Some(u) = s.units.get_mut(i)
            {
                u.checked = !u.checked;
            }
        });
        push_billing_units();
    });
    hub.on_detail_save_billing(save_customer_billing);
    hub.on_detail_upload_doc(|doc_type, label| {
        upload_customer_doc(doc_type.as_str(), label.as_str())
    });
    hub.on_detail_open_doc(|doc_id| open_customer_doc(i64::from(doc_id)));
    hub.on_detail_download_doc(|doc_id| download_customer_doc(i64::from(doc_id)));
    hub.on_detail_delete_doc(|doc_id| ask_delete_doc(i64::from(doc_id)));
    hub.on_detail_export_info(export_customer_info);
    hub.on_detail_download_bundle(download_customer_bundle);
    hub.on_detail_open_contract(|contract_id| {
        if let Some(ui) = with_state(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
            ui.invoke_navigate(Screen::Contracts.index());
            ui.invoke_contracts_open(contract_id);
        }
    });
    hub.on_detail_create_contract(|| {
        let Some((ui, cid)) = with_state(|s| (s.ui.clone(), s.customer_id)) else {
            return;
        };
        if let (Some(ui), Some(cid)) = (ui.upgrade(), cid) {
            ui.invoke_navigate(Screen::Generate.index());
            crate::screens::contract_form::select_customer(cid);
        }
    });
    hub.on_open_dossier(|| {
        if let Some(cid) = current_customer_id() {
            crate::screens::dossier::open(cid);
        }
    });

    hub.on_open_edit(|id| {
        let target = if id > 0 {
            Some(i64::from(id))
        } else {
            current_customer_id()
        };
        crate::screens::customer_form::open(target);
    });
}

pub fn current_customer_id() -> Option<i64> {
    with_state(|s| s.customer_id).flatten()
}

pub fn open(customer_id: i64) {
    with_state(|s| s.customer_id = Some(customer_id));
    reload();
}

pub fn reload() {
    let Some((worker, cid)) =
        with_state(|s| s.customer_id.map(|id| (s.worker.clone(), id))).flatten()
    else {
        return;
    };
    worker.run(
        move |ctx| load_detail(ctx, cid),
        move |res| post_to_ui(move || apply_detail(res)),
    );
}

struct DetailLoaded {
    profile: CustomerDetailProfile,
    slot_cccd_front: CustomerDetailDocSlot,
    slot_cccd_back: CustomerDetailDocSlot,
    slot_establishment: CustomerDetailDocSlot,
    is_active: bool,
    bill_main: bool,
    units: Vec<UnitBilling>,
    other_docs: Vec<CustomerDocRowData>,
    contracts: Vec<CustomerContractRowData>,
}

fn empty_slot() -> CustomerDetailDocSlot {
    CustomerDetailDocSlot {
        has_file: false,
        doc_id: 0,
        file_name: "".into(),
        uploaded_at: "".into(),
        is_pdf: false,
    }
}

fn load_detail(ctx: &Ctx, cid: i64) -> Result<DetailLoaded, String> {
    let store = ctx.store();
    let c = store
        .customers
        .get(&cid)
        .ok_or_else(|| format!("Không tìm thấy khách hàng id={cid}"))?;

    let mut raw_units = store
        .units
        .values()
        .filter(|u| u.customer_id == cid && u.is_active)
        .cloned()
        .collect::<Vec<_>>();
    raw_units.sort_by_key(|u| (u.sort_order, u.id));
    let billed_sub_count = raw_units.iter().filter(|u| u.include_in_billing).count();
    let total_units = usize::from(c.bill_main_station) + billed_sub_count;
    let total_units_display = total_units.max(1);

    let full_address_preview = [
        c.address.as_deref(),
        c.ward.as_deref(),
        c.province.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(", ");

    let units = raw_units
        .into_iter()
        .map(|u| UnitBilling {
            id: u.id,
            name: u.name,
            unit_type: u.unit_type,
            active: u.is_active,
            checked: u.include_in_billing,
        })
        .collect();

    let mut raw_docs = store
        .documents
        .values()
        .filter(|d| d.customer_id == cid)
        .cloned()
        .collect::<Vec<_>>();
    raw_docs.sort_by_key(|d| (d.doc_type.clone(), d.id));
    let doc_count = raw_docs.len();

    let mut slot_cccd_front = empty_slot();
    let mut slot_cccd_back = empty_slot();
    let mut slot_establishment = empty_slot();
    let mut other_docs = Vec::new();

    for d in raw_docs {
        let is_pdf = d.file_name.to_ascii_lowercase().ends_with(".pdf")
            || d.file_path.to_ascii_lowercase().ends_with(".pdf");
        let slot = CustomerDetailDocSlot {
            has_file: true,
            doc_id: i32::try_from(d.id).unwrap_or(0),
            file_name: d.file_name.clone().into(),
            uploaded_at: d.uploaded_at.format("%d/%m/%Y %H:%M").to_string().into(),
            is_pdf,
        };
        match d.doc_type.as_str() {
            "cccd_front" => slot_cccd_front = slot,
            "cccd_back" => slot_cccd_back = slot,
            "establishment" => slot_establishment = slot,
            _ => {
                other_docs.push(CustomerDocRowData {
                    id: i32::try_from(d.id).unwrap_or(0),
                    doc_type: d.doc_type.into(),
                    label: d.label.unwrap_or_else(|| "Giấy tờ khác".into()).into(),
                    file_name: d.file_name.into(),
                    uploaded_at: d.uploaded_at.format("%d/%m/%Y %H:%M").to_string().into(),
                    is_pdf,
                });
            }
        }
    }

    let mut raw_contracts = store
        .contracts
        .values()
        .filter(|ct| ct.customer_id == cid)
        .cloned()
        .collect::<Vec<_>>();
    raw_contracts.sort_by(|a, b| b.sign_date.cmp(&a.sign_date).then_with(|| b.id.cmp(&a.id)));
    let contracts = raw_contracts
        .into_iter()
        .map(|ct| {
            let product = ct
                .product_id
                .and_then(|id| store.products.get(&id))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "—".into());
            let badge = view::status_badge(&ct.status);
            CustomerContractRowData {
                id: i32::try_from(ct.id).unwrap_or(0),
                number: ct.contract_number.into(),
                product: product.into(),
                sign_date: view::date_text(ct.sign_date).into(),
                end_date: view::date_text(ct.end_date).into(),
                total: fmt_currency(ct.total_amount).into(),
                status_label: badge.label.into(),
                status_variant: badge.variant.into(),
            }
        })
        .collect();

    let main_name = c
        .short_name
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&c.legal_name);

    let profile = CustomerDetailProfile {
        code: c.code.clone().into(),
        legal_name: c.legal_name.clone().into(),
        short_name: c.short_name.clone().unwrap_or_default().into(),
        customer_type: c.customer_type.clone().into(),
        status_label: if c.is_active {
            "Đang hoạt động"
        } else {
            "Ngưng hoạt động"
        }
        .into(),
        status_variant: if c.is_active { "success" } else { "secondary" }.into(),
        is_active: c.is_active,
        doc_count_text: format!("{doc_count} file").into(),
        tax_code: c.tax_code.clone().unwrap_or_default().into(),
        budget_relation_code: c.budget_relation_code.clone().unwrap_or_default().into(),
        kcb_code: c.kcb_code.clone().unwrap_or_default().into(),
        decision_number: c.establishment_decision.clone().unwrap_or_default().into(),
        decision_date: c
            .establishment_date
            .map(|d| d.format("%d/%m/%Y").to_string())
            .unwrap_or_default()
            .into(),
        representative_name: c.representative_name.clone().unwrap_or_default().into(),
        representative_title: c.representative_title.clone().unwrap_or_default().into(),
        representative_gender: c.representative_gender.clone().unwrap_or_default().into(),
        address: c.address.clone().unwrap_or_default().into(),
        ward: c.ward.clone().unwrap_or_default().into(),
        province: c.province.clone().unwrap_or_default().into(),
        full_address_preview: full_address_preview.into(),
        contact_name: c.contact_name.clone().unwrap_or_default().into(),
        phone: c.phone.clone().unwrap_or_default().into(),
        email: c.email.clone().unwrap_or_default().into(),
        bank_account: c.bank_account.clone().unwrap_or_default().into(),
        bank_name: c.bank_name.clone().unwrap_or_default().into(),
        beneficiary_name: c.beneficiary_name.clone().unwrap_or_default().into(),
        notes: c.notes.clone().unwrap_or_default().into(),
        created_at: c.created_at.format("%d/%m/%Y %H:%M").to_string().into(),
        updated_at: c.updated_at.format("%d/%m/%Y %H:%M").to_string().into(),
        billed_units_badge: format!("{total_units_display} cơ sở").into(),
        main_station_label: format!("{main_name} — Trạm chính (đại diện ký hợp đồng)").into(),
    };

    Ok(DetailLoaded {
        profile,
        slot_cccd_front,
        slot_cccd_back,
        slot_establishment,
        is_active: c.is_active,
        bill_main: c.bill_main_station,
        units,
        other_docs,
        contracts,
    })
}

fn apply_detail(res: Result<DetailLoaded, String>) {
    match res {
        Ok(loaded) => {
            with_state(|s| {
                s.is_active = loaded.is_active;
                s.bill_main = loaded.bill_main;
                s.units = loaded.units;
            });
            if let Some(ui) = with_state(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
                let hub = ui.global::<CustomerHub>();
                hub.set_detail_profile(loaded.profile);
                hub.set_detail_slot_cccd_front(loaded.slot_cccd_front);
                hub.set_detail_slot_cccd_back(loaded.slot_cccd_back);
                hub.set_detail_slot_establishment(loaded.slot_establishment);
                hub.set_detail_bill_main(loaded.bill_main);
                hub.set_detail_docs(ModelRc::new(VecModel::from(loaded.other_docs)));
                hub.set_detail_contracts(ModelRc::new(VecModel::from(loaded.contracts)));
                hub.set_mode("detail".into());
            }
            push_billing_units();
        }
        Err(msg) => toast_from_worker(ToastKind::Danger, msg),
    }
}

fn push_billing_units() {
    let Some((ui, rows)) = with_state(|s| {
        let rows = s
            .units
            .iter()
            .map(|u| CustomerUnitRowData {
                id: i32::try_from(u.id).unwrap_or(0),
                name: u.name.clone().into(),
                unit_type: u.unit_type.clone().into(),
                address: "".into(),
                notes: "".into(),
                checked: u.checked,
                active: u.active,
            })
            .collect::<Vec<_>>();
        (s.ui.clone(), rows)
    }) else {
        return;
    };
    if let Some(ui) = ui.upgrade() {
        ui.global::<CustomerHub>()
            .set_detail_billing_units(ModelRc::new(VecModel::from(rows)));
    }
}

fn ask_toggle() {
    let Some((active, feedback)) = with_state(|s| (s.is_active, Rc::clone(&s.feedback))) else {
        return;
    };
    let (title, msg) = if active {
        (
            "Tạm ngưng khách hàng",
            "Tạm ngưng hoạt động khách hàng này?",
        )
    } else {
        ("Kích hoạt khách hàng", "Kích hoạt lại khách hàng này?")
    };
    feedback.confirm(title, msg, toggle);
}

fn toggle() {
    let Some((worker, cid, ui)) =
        with_state(|s| s.customer_id.map(|id| (s.worker.clone(), id, s.ui.clone()))).flatten()
    else {
        return;
    };
    worker.run(
        move |ctx| {
            let now = ctx.clock().now_utc();
            toggle_active(ctx.store_mut(), cid, now)
        },
        move |res| {
            post_to_ui(move || match res {
                Ok(active) => {
                    toast_from_worker(
                        ToastKind::Success,
                        if active {
                            "Đã kích hoạt khách hàng."
                        } else {
                            "Đã tạm ngưng khách hàng."
                        }
                        .into(),
                    );
                    if let Some(ui) = ui.upgrade() {
                        ui.invoke_customer_reload();
                    }
                    reload();
                }
                Err(e) => toast_from_worker(ToastKind::Danger, e.user_message()),
            });
        },
    );
}

fn save_customer_billing() {
    let Some((worker, cid, bill_main, included)) = with_state(|s| {
        s.customer_id.map(|id| {
            let included: Vec<i64> = s.units.iter().filter(|u| u.checked).map(|u| u.id).collect();
            (s.worker.clone(), id, s.bill_main, included)
        })
    })
    .flatten() else {
        return;
    };
    worker.run(
        move |ctx| {
            let now = ctx.clock().now_utc();
            save_billing(ctx.store_mut(), cid, bill_main, &included, now)
        },
        move |res| {
            post_to_ui(move || match res {
                Ok(()) => {
                    toast_from_worker(ToastKind::Success, "Đã lưu cơ sở tính phí.".into());
                    reload();
                }
                Err(e) => toast_from_worker(ToastKind::Danger, e.user_message()),
            });
        },
    );
}

fn upload_customer_doc(doc_type: &str, label: &str) {
    if doc_type == "other" && label.trim().is_empty() {
        toast_from_worker(
            ToastKind::Warning,
            "Vui lòng nhập tên giấy tờ khác trước khi chọn file.".into(),
        );
        return;
    }
    let Some((worker, cid, ui)) =
        with_state(|s| s.customer_id.map(|id| (s.worker.clone(), id, s.ui.clone()))).flatten()
    else {
        return;
    };
    let Some(path) =
        files::pick_file(&[("Giấy tờ (PDF, JPG, PNG)", &["pdf", "jpg", "jpeg", "png"])])
    else {
        return;
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            toast_from_worker(ToastKind::Danger, format!("Không đọc được file: {e}"));
            return;
        }
    };
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_owned();
    let doc_type = doc_type.to_owned();
    let label = label.trim().to_owned();
    worker.run(
        move |ctx| upload_doc(ctx, cid, &doc_type, &label, &bytes, &filename),
        move |res| {
            post_to_ui(move || match res {
                Ok(_) => {
                    toast_from_worker(ToastKind::Success, "Đã tải giấy tờ lên.".into());
                    if let Some(ui) = ui.upgrade() {
                        ui.global::<CustomerHub>()
                            .set_detail_other_doc_label("".into());
                    }
                    reload();
                }
                Err(e) => toast_from_worker(ToastKind::Danger, e.user_message()),
            });
        },
    );
}

fn open_customer_doc(doc_id: i64) {
    let Some((worker, cid)) =
        with_state(|s| s.customer_id.map(|id| (s.worker.clone(), id))).flatten()
    else {
        return;
    };
    worker.run(
        move |ctx| doc_path(ctx.store(), &ctx.paths().root, cid, doc_id),
        move |res| {
            post_to_ui(move || match res {
                Ok(path) => {
                    if let Err(e) = files::open_path(&path) {
                        toast_from_worker(ToastKind::Danger, format!("Không mở được file: {e}"));
                    }
                }
                Err(e) => toast_from_worker(ToastKind::Danger, e.user_message()),
            });
        },
    );
}

fn download_customer_doc(doc_id: i64) {
    let Some((worker, cid)) =
        with_state(|s| s.customer_id.map(|id| (s.worker.clone(), id))).flatten()
    else {
        return;
    };
    worker.run(
        move |ctx| {
            let path = doc_path(ctx.store(), &ctx.paths().root, cid, doc_id)?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("giay-to")
                .to_owned();
            let bytes = std::fs::read(&path)?;
            Ok::<_, cf_services::ServiceError>((name, bytes))
        },
        move |res| {
            post_to_ui(move || match res {
                Ok((name, bytes)) => {
                    let ext = std::path::Path::new(&name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("pdf");
                    if let Some(dest) = files::save_as(&name, ("Tệp", &[ext])) {
                        match files::write_chosen(&dest, &bytes) {
                            Ok(()) => toast_from_worker(
                                ToastKind::Success,
                                format!("Đã lưu {}", dest.display()),
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

fn ask_delete_doc(doc_id: i64) {
    let Some((worker, cid, feedback)) = with_state(|s| {
        s.customer_id
            .map(|id| (s.worker.clone(), id, Rc::clone(&s.feedback)))
    })
    .flatten() else {
        return;
    };
    feedback.confirm(
        "Xóa giấy tờ",
        "Xóa giấy tờ này khỏi hồ sơ khách hàng?",
        move || {
            worker.run(
                move |ctx| delete_doc(ctx, cid, doc_id),
                move |res| {
                    post_to_ui(move || match res {
                        Ok(_) => {
                            toast_from_worker(ToastKind::Warning, "Đã xóa giấy tờ.".into());
                            reload();
                        }
                        Err(e) => toast_from_worker(ToastKind::Danger, e.user_message()),
                    });
                },
            );
        },
    );
}

fn export_customer_info() {
    let Some((worker, cid)) =
        with_state(|s| s.customer_id.map(|id| (s.worker.clone(), id))).flatten()
    else {
        return;
    };
    worker.run(
        move |ctx| export_info(ctx.store(), cid, ctx.clock().now_local()),
        move |res| {
            post_to_ui(move || match res {
                Ok(exported) => {
                    if let Some(path) = files::save_as(&exported.file_name, ("Markdown", &["md"])) {
                        match files::write_chosen(&path, &exported.content) {
                            Ok(()) => toast_from_worker(
                                ToastKind::Success,
                                format!("Đã xuất {}", path.display()),
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

fn download_customer_bundle() {
    let Some((worker, cid)) =
        with_state(|s| s.customer_id.map(|id| (s.worker.clone(), id))).flatten()
    else {
        return;
    };
    worker.run(
        move |ctx| download_bundle(ctx, cid),
        move |res| {
            post_to_ui(move || match res {
                Ok(bundle) => {
                    if let Some(path) = files::save_as(&bundle.file_name, ("ZIP", &["zip"])) {
                        match files::write_chosen(&path, &bundle.content) {
                            Ok(()) => toast_from_worker(
                                ToastKind::Success,
                                format!("Đã lưu gói hồ sơ {}", path.display()),
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

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|cell| cell.borrow_mut().as_mut().map(f))
}
