//! Màn hình thùng rác khách hàng.

use std::cell::RefCell;

use cf_services::{
    Ctx,
    customers::state::{list_deleted_customers, restore_customer},
};
use slint::{ComponentHandle, ModelRc, VecModel, Weak};

use crate::{
    CustomerHub, CustomerTrashRowData, MainWindow,
    ui_helpers::{ToastKind, post_to_ui, toast_from_worker},
    worker::Worker,
};

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>) {
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
        });
    });
    let hub = ui.global::<CustomerHub>();
    hub.on_open_trash(open);
    hub.on_trash_restore(|id| restore(i64::from(id)));
}

pub fn open() {
    let Some(worker) = STATE.with(|c| c.borrow().as_ref().map(|s| s.worker.clone())) else {
        return;
    };
    worker.run(
        |ctx| {
            list_deleted_customers(ctx.store())
                .into_iter()
                .map(|c| CustomerTrashRowData {
                    id: i32::try_from(c.id).unwrap_or(0),
                    code: c.code.into(),
                    name: c.legal_name.into(),
                    customer_type: c.customer_type.into(),
                    deleted_at: c
                        .deleted_at
                        .map(|d| d.format("%d/%m/%Y %H:%M").to_string())
                        .unwrap_or_default()
                        .into(),
                })
                .collect::<Vec<_>>()
        },
        |rows| {
            post_to_ui(move || {
                STATE.with(|c| {
                    if let Some(ui) = c.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
                        let hub = ui.global::<CustomerHub>();
                        hub.set_trash_rows(ModelRc::new(VecModel::from(rows)));
                        hub.set_mode("trash".into());
                    }
                });
            });
        },
    );
}

fn restore(id: i64) {
    let Some((worker, ui)) = STATE.with(|c| {
        c.borrow()
            .as_ref()
            .map(|s| (s.worker.clone(), s.ui.clone()))
    }) else {
        return;
    };
    worker.run(
        move |ctx| {
            let now = ctx.clock().now_utc();
            restore_customer(ctx.store_mut(), id, now)
        },
        move |res| {
            post_to_ui(move || match res {
                Ok(()) => {
                    toast_from_worker(ToastKind::Success, "Đã khôi phục khách hàng.".into());
                    if let Some(ui) = ui.upgrade() {
                        ui.invoke_customer_reload();
                    }
                    open();
                }
                Err(e) => toast_from_worker(ToastKind::Danger, e.user_message()),
            });
        },
    );
}
