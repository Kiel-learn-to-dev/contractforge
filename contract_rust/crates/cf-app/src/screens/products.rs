use std::cell::RefCell;

use cf_core::fmt::{fmt_currency, fmt_number};
use cf_services::{
    Ctx, ServiceError,
    products::{
        edit::toggle_active,
        list::{in_service_counts, list_products},
    },
};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    MainWindow, ProductRowData,
    ui_helpers::{ToastKind, toast_from_worker},
    worker::Worker,
};

#[derive(Debug, Clone)]
struct ProductRow {
    id: i64,
    code: String,
    name: String,
    group: Option<String>,
    kind: String,
    product_type: String,
    unit_label: String,
    price: String,
    vat: String,
    duration: String,
    active: bool,
    count: usize,
}

#[derive(Debug)]
struct Loaded {
    rows: Vec<ProductRow>,
    empty_message: String,
}

struct State {
    ui: slint::Weak<MainWindow>,
    worker: Worker<Ctx>,
    show: String,
    rows: Vec<ProductRow>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>) {
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
            show: "active".into(),
            rows: Vec::new(),
        })
    });

    ui.on_product_show_changed(|show| {
        with_state(|state| {
            state.show = match show.as_str() {
                "inactive" => "inactive".into(),
                "all" => "all".into(),
                _ => "active".into(),
            }
        });
        request_load();
    });

    ui.on_product_toggle_active(|id| toggle_product(i64::from(id)));
    request_load();
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|state| state.borrow_mut().as_mut().map(f))
}

fn request_load() {
    let Some((worker, show)) = with_state(|state| (state.worker.clone(), state.show.clone()))
    else {
        return;
    };
    worker.run(move |ctx| load(ctx, &show), apply);
}

pub fn reload() {
    request_load();
}

fn load(ctx: &mut Ctx, show: &str) -> Loaded {
    let products = list_products(ctx.store(), true);
    let counts = in_service_counts(ctx.store());
    let rows = products
        .into_iter()
        .filter(|product| match show {
            "active" => product.is_active,
            "inactive" => !product.is_active,
            _ => true,
        })
        .map(|product| {
            let price = product
                .default_price
                .filter(|value| !value.is_zero())
                .map(|value| fmt_currency(Some(value)))
                .unwrap_or_else(|| "—".into());
            let vat = product
                .default_vat_rate
                .map(|value| format!("{}%", fmt_number(Some(value))))
                .unwrap_or_else(|| "—".into());
            let duration = product
                .default_duration_months
                .filter(|months| *months != 0)
                .map(|months| format!("{months} tháng"))
                .unwrap_or_else(|| "—".into());
            ProductRow {
                id: product.id,
                code: product.code,
                name: product.name,
                group: product.group_code,
                kind: match product.kind.as_str() {
                    "main" => "Gói chính".into(),
                    "addon" => "Gói bổ sung".into(),
                    "storage" => "Gói lưu trữ".into(),
                    other => other.to_owned(),
                },
                product_type: product.product_type,
                unit_label: product
                    .unit_label
                    .filter(|label| !label.trim().is_empty())
                    .unwrap_or_else(|| "—".into()),
                price,
                vat,
                duration,
                active: product.is_active,
                count: counts.get(&product.id).copied().unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    let empty_message = match show {
        "inactive" => "Không có sản phẩm nào đã ngừng bán.",
        "all" => "Chưa có sản phẩm nào.",
        _ => "Chưa có sản phẩm nào đang bán.",
    }
    .to_owned();
    Loaded {
        rows,
        empty_message,
    }
}

fn apply(loaded: Loaded) {
    let Some((weak, rows, empty_message)) = with_state(|state| {
        state.rows = loaded.rows;
        (state.ui.clone(), state.rows.clone(), loaded.empty_message)
    }) else {
        return;
    };
    let Some(ui) = weak.upgrade() else {
        return;
    };
    let mut previous: Option<Option<String>> = None;
    let rows = rows
        .into_iter()
        .map(|row| {
            let show_group = previous.as_ref().is_none_or(|group| group != &row.group);
            previous = Some(row.group.clone());
            ProductRowData {
                id: i32::try_from(row.id).unwrap_or(i32::MAX),
                code: row.code.into(),
                name: row.name.into(),
                group_title: row
                    .group
                    .clone()
                    .unwrap_or_else(|| "Chưa thuộc nhóm nào".into())
                    .into(),
                show_group,
                kind: row.kind.into(),
                product_type: row.product_type.into(),
                unit_label: row.unit_label.into(),
                price: row.price.into(),
                vat: row.vat.into(),
                duration: row.duration.into(),
                active: row.active,
                in_service_count: i32::try_from(row.count).unwrap_or(i32::MAX),
            }
        })
        .collect::<Vec<_>>();
    ui.set_product_rows(ModelRc::new(VecModel::from(rows)));
    ui.set_product_empty_message(empty_message.into());
    let show = with_state(|state| state.show.clone()).unwrap_or_else(|| "active".into());
    ui.set_product_show(show.into());
}

fn toggle_product(id: i64) {
    let Some(worker) = with_state(|state| state.worker.clone()) else {
        return;
    };
    worker.run(
        move |ctx| {
            let name = ctx
                .store()
                .products
                .get(&id)
                .map(|product| product.name.clone())
                .ok_or_else(|| ServiceError::Invalid("Không tìm thấy sản phẩm.".into()))?;
            let now = ctx.clock().now_utc();
            let active = toggle_active(ctx.store_mut(), id, now)?;
            Ok::<_, ServiceError>((name, active))
        },
        |result| match result {
            Ok((name, true)) => {
                toast_from_worker(ToastKind::Success, format!("Đã bán lại {name}"));
                request_load();
            }
            Ok((name, false)) => {
                toast_from_worker(ToastKind::Success, format!("Đã ngừng bán {name}"));
                request_load();
            }
            Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
        },
    );
}
