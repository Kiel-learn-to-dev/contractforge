//! Tìm kiếm nhanh toàn cục ở thanh trên cùng (hợp đồng + khách hàng không dấu).

use std::cell::RefCell;

use cf_services::{Ctx, dashboard::search::api_search};
use chrono::{Datelike, Local};
use slint::{ComponentHandle, Model, ModelRc, VecModel, Weak};

use crate::{CustomerHub, MainWindow, SearchHub, SearchResultRow, nav::Screen, worker::Worker};

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    request_id: u64,
    first: Option<(String, i64)>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>) {
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
            request_id: 0,
            first: None,
        });
    });

    let hub = ui.global::<SearchHub>();
    hub.on_search_changed(|query| {
        run_search(query.as_str());
    });
    hub.on_submit_first(|| {
        let first = STATE.with(|cell| cell.borrow().as_ref().and_then(|s| s.first.clone()));
        if let Some((kind, id)) = first {
            open_target(&kind, id);
        }
    });
    hub.on_pick_result(|kind, id| {
        open_target(kind.as_str(), i64::from(id));
    });
    hub.on_close_popup(|| {
        with_hub(|hub| hub.set_open(false));
    });
    hub.on_filter_model(|query, model| {
        let query = cf_core::text::search_key(query.as_str());
        let indices = model
            .iter()
            .enumerate()
            .filter(|(_, item)| cf_core::text::search_key(item.as_str()).contains(&query))
            .map(|(index, _)| index as i32)
            .collect::<Vec<_>>();
        ModelRc::new(VecModel::from(indices))
    });
    hub.on_month_days(|year, month| {
        let mut cells = vec![0_i32; 42];
        if let Some(first) = chrono::NaiveDate::from_ymd_opt(year, month as u32, 1) {
            let offset = first.weekday().num_days_from_monday() as usize;
            let mut day = first;
            while day.month() == first.month() {
                cells[offset + day.day() as usize - 1] = day.day() as i32;
                match day.succ_opt() {
                    Some(next) => day = next,
                    None => break,
                }
            }
        }
        ModelRc::new(VecModel::from(cells))
    });
    hub.on_date_from_text(|text| {
        let parsed = cf_core::dates::parse_date_input(Some(text.as_str()))
            .ok()
            .flatten()
            .unwrap_or_else(|| Local::now().date_naive());
        crate::Date {
            year: parsed.year(),
            month: parsed.month() as i32,
            day: parsed.day() as i32,
        }
    });
}

fn run_search(raw_query: &str) {
    let trimmed = raw_query.trim();
    if trimmed.chars().count() < 2 {
        STATE.with(|cell| {
            if let Some(s) = cell.borrow_mut().as_mut() {
                s.request_id = s.request_id.wrapping_add(1);
                s.first = None;
            }
        });
        with_hub(|hub| {
            hub.set_open(false);
            hub.set_results(ModelRc::new(VecModel::from(Vec::<SearchResultRow>::new())));
        });
        return;
    }

    let query = trimmed.to_owned();
    let Some((worker, req_id)) = STATE.with(|cell| {
        let mut g = cell.borrow_mut();
        let s = g.as_mut()?;
        s.request_id = s.request_id.wrapping_add(1);
        Some((s.worker.clone(), s.request_id))
    }) else {
        return;
    };

    worker.run(
        move |ctx| api_search(ctx.store(), &query, Some(10)),
        move |items| {
            let stale = STATE.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .is_none_or(|s| s.request_id != req_id)
            });
            if stale {
                return;
            }
            let first = items.first().map(|r| (r.kind.to_owned(), r.id));
            let rows = items
                .into_iter()
                .map(|r| SearchResultRow {
                    kind: r.kind.into(),
                    id: r.id as i32,
                    label: r.label.into(),
                    sub: r.sub.into(),
                    status: r.status.unwrap_or_default().into(),
                })
                .collect::<Vec<_>>();
            STATE.with(|cell| {
                if let Some(s) = cell.borrow_mut().as_mut() {
                    s.first = first;
                }
            });
            with_hub(|hub| {
                hub.set_results(ModelRc::new(VecModel::from(rows)));
                hub.set_open(true);
            });
        },
    );
}

fn open_target(kind: &str, id: i64) {
    with_hub(|hub| {
        hub.set_open(false);
        hub.set_query("".into());
    });
    STATE.with(|cell| {
        let Some(ui) = cell.borrow().as_ref().and_then(|s| s.ui.upgrade()) else {
            return;
        };
        match kind {
            "contract" => {
                ui.invoke_navigate(Screen::Contracts.index());
                ui.invoke_contracts_open(id as i32);
            }
            "customer" => {
                ui.invoke_navigate(Screen::Customers.index());
                ui.global::<CustomerHub>().invoke_open_detail(id as i32);
            }
            _ => {}
        }
    });
}

fn with_hub<F: FnOnce(&SearchHub)>(f: F) {
    STATE.with(|cell| {
        if let Some(ui) = cell.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
            f(&ui.global::<SearchHub>());
        }
    });
}
