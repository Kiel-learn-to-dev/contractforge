//! S06 — hồ sơ đấu nối của khách hàng.

use std::{cell::RefCell, path::PathBuf};

use cf_services::{
    Ctx,
    dossier::{
        build::{is_stale, materialize, open_in_file_manager},
        plan::build_plan,
    },
};
use slint::{ComponentHandle, ModelRc, VecModel, Weak};

use crate::{
    CustomerHub, DossierContractRowData, DossierSlotRowData, MainWindow,
    ui_helpers::{ToastKind, post_to_ui, toast_from_worker},
    view,
    worker::Worker,
};

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    customer_id: Option<i64>,
    dest_dir: Option<PathBuf>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>) {
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
            customer_id: None,
            dest_dir: None,
        });
    });

    let hub = ui.global::<CustomerHub>();
    hub.on_back_to_detail(|| {
        STATE.with(|c| {
            if let Some(ui) = c.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
                ui.global::<CustomerHub>().set_mode("detail".into());
            }
        });
    });
    hub.on_dossier_materialize(|| run_materialize(true));
    hub.on_dossier_materialize_only(|| run_materialize(false));
    hub.on_dossier_open_folder(open_folder);
}

pub fn open(customer_id: i64) {
    let Some(worker) = STATE.with(|c| {
        let mut guard = c.borrow_mut();
        let state = guard.as_mut()?;
        state.customer_id = Some(customer_id);
        Some(state.worker.clone())
    }) else {
        return;
    };
    worker.run(
        move |ctx| load_plan(ctx, customer_id),
        |res| post_to_ui(move || apply_plan(res)),
    );
}

struct LoadedPlan {
    customer_name: String,
    dest_dir: PathBuf,
    dir_exists: bool,
    summary: String,
    missing_text: String,
    stale: bool,
    slots: Vec<DossierSlotRowData>,
    contracts: Vec<DossierContractRowData>,
}

fn load_plan(ctx: &Ctx, cid: i64) -> Result<LoadedPlan, String> {
    let plan = build_plan(ctx.store(), ctx.paths(), cid).map_err(|e| e.user_message())?;
    let stale = is_stale(&plan);
    let ready = plan.ready_count();
    let total = plan.slots.len();
    let missing_vec = plan.missing_required();
    let missing = missing_vec.len();
    let missing_text = missing_vec
        .iter()
        .map(|s| s.slot.label.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let summary = format!("Đủ {ready}/{total} mục · Thiếu bắt buộc: {missing}");
    let dir_exists = plan.dest_dir.is_dir();

    let contracts = plan
        .contracts
        .iter()
        .map(|ct| {
            let badge = view::status_badge(&ct.status);
            let product = ct
                .product_id
                .and_then(|pid| plan.product_labels.get(&pid).map(|(_, name)| name.clone()))
                .unwrap_or_else(|| "—".into());
            DossierContractRowData {
                id: i32::try_from(ct.id).unwrap_or(0),
                number: ct.contract_number.as_str().into(),
                product: product.into(),
                status_label: badge.label.into(),
                status_variant: badge.variant.into(),
                has_scan: ct
                    .signed_pdf_path
                    .as_deref()
                    .is_some_and(|s| !s.trim().is_empty()),
            }
        })
        .collect::<Vec<_>>();

    let slots = plan
        .slots
        .into_iter()
        .map(|slot| DossierSlotRowData {
            order: i32::try_from(slot.order).unwrap_or(0),
            label: slot.slot.label.into(),
            source: slot.slot.source.into(),
            dest_name: slot.dest_name.into(),
            size_text: if slot.size > 0 {
                format!("{} KB", (slot.size / 1024).max(1))
            } else {
                String::new()
            }
            .into(),
            ok: slot.source_path.is_some(),
            required: slot.slot.required,
            note: slot.note.into(),
        })
        .collect();
    let name = plan
        .customer
        .short_name
        .filter(|s| !s.is_empty())
        .unwrap_or(plan.customer.legal_name);
    Ok(LoadedPlan {
        customer_name: name,
        dest_dir: plan.dest_dir,
        dir_exists,
        summary,
        missing_text,
        stale,
        slots,
        contracts,
    })
}

fn apply_plan(res: Result<LoadedPlan, String>) {
    match res {
        Ok(loaded) => {
            let dir_str = loaded.dest_dir.to_string_lossy().into_owned();
            let ui = STATE.with(|c| {
                let mut guard = c.borrow_mut();
                let state = guard.as_mut()?;
                state.dest_dir = Some(loaded.dest_dir);
                state.ui.upgrade()
            });
            if let Some(ui) = ui {
                let hub = ui.global::<CustomerHub>();
                hub.set_dossier_customer_name(loaded.customer_name.into());
                hub.set_dossier_dest_dir(dir_str.into());
                hub.set_dossier_dir_exists(loaded.dir_exists);
                hub.set_dossier_summary(loaded.summary.into());
                hub.set_dossier_missing_text(loaded.missing_text.into());
                hub.set_dossier_stale(loaded.stale);
                hub.set_dossier_slots(ModelRc::new(VecModel::from(loaded.slots)));
                hub.set_dossier_contracts(ModelRc::new(VecModel::from(loaded.contracts)));
                hub.set_mode("dossier".into());
            }
        }
        Err(e) => toast_from_worker(ToastKind::Danger, e),
    }
}

fn run_materialize(and_open: bool) {
    let Some((worker, cid)) = STATE.with(|c| {
        c.borrow()
            .as_ref()
            .and_then(|s| s.customer_id.map(|id| (s.worker.clone(), id)))
    }) else {
        return;
    };
    worker.run(
        move |ctx| -> Result<String, String> {
            let plan = build_plan(ctx.store(), ctx.paths(), cid).map_err(|e| e.user_message())?;
            let res = materialize(ctx, &plan).map_err(|e| e.user_message())?;
            if and_open {
                let _ = open_in_file_manager(ctx.paths(), &plan.dest_dir);
            }
            let msg = format!(
                "Đã tạo hồ sơ đấu nối ({} file, thiếu bắt buộc: {}).",
                res.copied, res.missing_required
            );
            Ok(msg)
        },
        move |res| {
            post_to_ui(move || match res {
                Ok(msg) => {
                    toast_from_worker(ToastKind::Success, msg);
                    open(cid);
                }
                Err(e) => toast_from_worker(ToastKind::Danger, e),
            });
        },
    );
}

fn open_folder() {
    let Some((worker, dir)) = STATE.with(|c| {
        c.borrow()
            .as_ref()
            .and_then(|s| s.dest_dir.clone().map(|d| (s.worker.clone(), d)))
    }) else {
        return;
    };
    worker.run(
        move |ctx| open_in_file_manager(ctx.paths(), &dir).map_err(|e| e.user_message()),
        |res| {
            if let Err(e) = res {
                toast_from_worker(ToastKind::Danger, e);
            }
        },
    );
}
