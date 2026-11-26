//! S02 — Danh sách hợp đồng sắp hết hạn (lọc theo số ngày, màu mức độ khẩn, xuất CSV).

use std::{cell::RefCell, rc::Rc};

use cf_core::{fmt::fmt_currency, lifecycle::status_label};
use cf_services::{
    Ctx,
    dashboard::expiring_page::{ExpiringContract, export_expiring_csv, get_expiring_contracts},
};
use slint::{ComponentHandle, ModelRc, VecModel, Weak};

use crate::{
    CustomerHub, DashboardHub, ExpiringHub, ExpiringRowData, MainWindow, files,
    nav::Screen,
    ui_helpers::{Feedback, ToastKind},
    worker::Worker,
};

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    feedback: Rc<Feedback>,
    days: i32,
    cached: Vec<ExpiringContract>,
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
            days: 30,
            cached: Vec::new(),
        });
    });

    let hub = ui.global::<ExpiringHub>();
    hub.on_load_days(|days| {
        load_days(days);
    });
    hub.on_export_csv(|| {
        export_csv();
    });
    hub.on_open_contract(|id| {
        STATE.with(|cell| {
            if let Some(ui) = cell.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
                ui.invoke_navigate(Screen::Contracts.index());
                ui.invoke_contracts_open(id);
            }
        });
    });
    hub.on_open_customer(|cid| {
        if cid <= 0 {
            return;
        }
        STATE.with(|cell| {
            if let Some(ui) = cell.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
                ui.invoke_navigate(Screen::Customers.index());
                ui.global::<CustomerHub>().invoke_open_detail(cid);
            }
        });
    });
    hub.on_back(|| {
        STATE.with(|cell| {
            if let Some(ui) = cell.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
                ui.global::<DashboardHub>().set_submode("overview".into());
            }
        });
    });
}

pub fn open(days: i32) {
    STATE.with(|cell| {
        if let Some(ui) = cell.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
            ui.global::<DashboardHub>().set_submode("expiring".into());
        }
    });
    load_days(days);
}

pub fn load_days(days: i32) {
    let clamped = days.clamp(1, 365);
    let Some(worker) = STATE.with(|cell| {
        let mut g = cell.borrow_mut();
        let s = g.as_mut()?;
        s.days = clamped;
        Some(s.worker.clone())
    }) else {
        return;
    };
    worker.run(
        move |ctx| {
            let today = ctx.clock().now_local().date();
            get_expiring_contracts(ctx.store(), today, i64::from(clamped))
        },
        move |rows| {
            let ui_rows = rows
                .iter()
                .map(|r| ExpiringRowData {
                    contract_id: r.contract_id as i32,
                    customer_id: r.customer_id.unwrap_or(0) as i32,
                    contract_number: r.contract_number.as_str().into(),
                    customer_name: r.customer_name.as_str().into(),
                    end_date: r.end_date.format("%d/%m/%Y").to_string().into(),
                    days_left: r.days_left as i32,
                    status_label: status_label(&r.status).into(),
                    total_text: fmt_currency(r.total_amount).into(),
                    urgency: r.urgency.into(),
                })
                .collect::<Vec<_>>();
            let summary = format!(
                "Có {} hợp đồng hết hạn trong {clamped} ngày tới.",
                ui_rows.len()
            );
            STATE.with(|cell| {
                if let Some(s) = cell.borrow_mut().as_mut() {
                    s.cached = rows;
                }
            });
            with_hub(|hub| {
                hub.set_days(clamped);
                hub.set_rows(ModelRc::new(VecModel::from(ui_rows)));
                hub.set_summary_text(summary.into());
            });
        },
    );
}

fn export_csv() {
    let info = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        Some((Rc::clone(&s.feedback), s.cached.clone(), s.days))
    });
    let Some((feedback, rows, days)) = info else {
        return;
    };
    let (default_name, bytes) = export_expiring_csv(&rows, i64::from(days));
    let Some(path) = files::save_as(&default_name, ("CSV", &["csv"])) else {
        return;
    };
    match files::write_chosen(&path, &bytes) {
        Ok(()) => {
            let _ = files::open_folder(&path);
            feedback.toast(ToastKind::Success, &format!("Đã xuất {}", path.display()));
        }
        Err(error) => feedback.toast(ToastKind::Danger, &format!("Không xuất được CSV: {error}")),
    }
}

fn with_hub<F: FnOnce(&ExpiringHub)>(f: F) {
    STATE.with(|cell| {
        if let Some(ui) = cell.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
            f(&ui.global::<ExpiringHub>());
        }
    });
}
