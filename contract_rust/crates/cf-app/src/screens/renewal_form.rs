//! S17 — gia hạn hợp đồng (Phụ lục gia hạn hoặc Hợp đồng mới).

use std::{cell::RefCell, collections::BTreeSet};

use cf_core::{dates::parse_date_input, fmt::fmt_currency, template_rules::Role};
use cf_services::{
    Ctx,
    addenda::{
        addon::{AddonForm, AddonItem},
        renewal::{create_renewal_appendix, create_renewal_contract},
        renewal_defaults::renewal_defaults,
    },
    contracts::{generate::generate_docx, suggest::suggest_number},
    templates::list::templates_for,
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::{
    ContractHub, IdLabel, MainWindow, RenewLineRow,
    ui_helpers::{ToastKind, post_to_ui, toast_from_worker},
    view,
    worker::Worker,
};

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    parent_id: Option<i64>,
    mode: String,
    candidates: Vec<(i64, String)>,
    lines: Vec<(String, Option<i64>)>,
    selected_products: BTreeSet<i64>,
    templates_appendix: Vec<(i64, String)>,
    templates_contract: Vec<(i64, String)>,
    template_index: usize,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>) {
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
            parent_id: None,
            mode: "appendix".into(),
            candidates: Vec::new(),
            lines: Vec::new(),
            selected_products: BTreeSet::new(),
            templates_appendix: Vec::new(),
            templates_contract: Vec::new(),
            template_index: 0,
        });
    });

    let hub = ui.global::<ContractHub>();
    hub.on_open_renewal(|| {
        if let Some(pid) = crate::screens::contract_detail::current_id() {
            open(pid);
        }
    });
    hub.on_renew_mode_changed(|mode| {
        let mode = mode.to_string();
        STATE.with(|c| {
            if let Some(s) = c.borrow_mut().as_mut() {
                s.mode = mode;
                let list = if s.mode == "contract" {
                    &s.templates_contract
                } else {
                    &s.templates_appendix
                };
                s.template_index = if list.is_empty() { 0 } else { 1 };
            }
        });
        push_templates();
    });
    hub.on_renew_toggle_product(|idx| {
        STATE.with(|c| {
            let mut g = c.borrow_mut();
            let Some(s) = g.as_mut() else {
                return;
            };
            if let Ok(i) = usize::try_from(idx)
                && let Some((id, _)) = s.candidates.get(i)
                && !s.selected_products.remove(id)
            {
                s.selected_products.insert(*id);
            }
        });
        push_products();
    });
    hub.on_renew_line_changed(|line_idx, opt_idx| {
        STATE.with(|c| {
            let mut g = c.borrow_mut();
            let Some(s) = g.as_mut() else {
                return;
            };
            if let Ok(li) = usize::try_from(line_idx)
                && let Some(line) = s.lines.get_mut(li)
            {
                line.1 = usize::try_from(opt_idx)
                    .ok()
                    .and_then(|oi| oi.checked_sub(1))
                    .and_then(|ci| s.candidates.get(ci))
                    .map(|(id, _)| *id);
            }
        });
        push_lines();
    });
    hub.on_renew_template_changed(|idx| {
        STATE.with(|c| {
            if let Some(s) = c.borrow_mut().as_mut()
                && let Ok(i) = usize::try_from(idx)
            {
                s.template_index = i;
            }
        });
    });
    hub.on_renew_save(|number, start_raw, months_raw, sign_raw, and_gen| {
        save(
            number.as_str(),
            start_raw.as_str(),
            months_raw.as_str(),
            sign_raw.as_str(),
            and_gen,
        );
    });
}

pub fn open(parent_id: i64) {
    let Some(worker) = STATE.with(|c| {
        let mut g = c.borrow_mut();
        let s = g.as_mut()?;
        s.parent_id = Some(parent_id);
        s.mode = "appendix".into();
        s.template_index = 0;
        Some(s.worker.clone())
    }) else {
        return;
    };
    worker.run(
        move |ctx| load(ctx, parent_id),
        |res| post_to_ui(move || apply(res)),
    );
}

struct Loaded {
    parent_title: String,
    suggested_contract_number: String,
    start_date: String,
    months: String,
    sign_date: String,
    old_monthly: String,
    new_monthly: String,
    diff_monthly: String,
    comparison: String,
    candidates: Vec<(i64, String)>,
    lines: Vec<(String, Option<i64>)>,
    selected: BTreeSet<i64>,
    templates_appendix: Vec<(i64, String)>,
    templates_contract: Vec<(i64, String)>,
}

fn load(ctx: &Ctx, parent_id: i64) -> Result<Loaded, String> {
    let store = ctx.store();
    let parent = store
        .contracts
        .get(&parent_id)
        .ok_or_else(|| "Không tìm thấy hợp đồng gốc.".to_owned())?;
    let defs = renewal_defaults(store, parent_id).map_err(|e| e.user_message())?;
    let selected: BTreeSet<i64> = defs.lines.iter().filter_map(|l| l.product_id).collect();
    let lines = defs
        .lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let old_name = l
                .old_product_id
                .and_then(|id| store.products.get(&id))
                .map(|p| format!("[{}] {}", p.code, p.name))
                .unwrap_or_else(|| "(chưa gắn gói)".to_owned());
            (format!("Dòng {} — gói cũ: {old_name}", i + 1), l.product_id)
        })
        .collect::<Vec<_>>();
    let candidates = defs
        .candidate_product_ids
        .into_iter()
        .filter_map(|id| {
            store
                .products
                .get(&id)
                .map(|p| (p.id, format!("[{}] {}", p.code, p.name)))
        })
        .collect::<Vec<_>>();
    let group = parent
        .product_id
        .and_then(|id| store.products.get(&id))
        .and_then(|p| p.group_code.as_deref());
    let templates_appendix = templates_for(store, group, Role::Renewal, true)
        .into_iter()
        .map(|t| (t.id, t.name.clone()))
        .collect();
    let templates_contract = templates_for(store, group, Role::Contract, true)
        .into_iter()
        .map(|t| (t.id, t.name.clone()))
        .collect();
    let year = chrono::Datelike::year(&defs.start_date);
    let suggested =
        suggest_number(store, parent.customer_id, parent.product_id, year).unwrap_or_default();
    let old_monthly = format!(
        "{} VNĐ/tháng",
        fmt_currency(Some(defs.comparison.old_per_month))
    );
    let new_monthly = format!(
        "{} VNĐ/tháng",
        fmt_currency(Some(defs.comparison.new_per_month))
    );
    let diff_monthly = format!("{} VNĐ/tháng", fmt_currency(Some(defs.comparison.diff)));
    let comparison = format!(
        "So sánh giá/tháng: Cũ {} VNĐ → Mới {} VNĐ (chênh lệch {} VNĐ)",
        fmt_currency(Some(defs.comparison.old_per_month)),
        fmt_currency(Some(defs.comparison.new_per_month)),
        fmt_currency(Some(defs.comparison.diff)),
    );
    Ok(Loaded {
        parent_title: parent.contract_number.clone(),
        suggested_contract_number: suggested,
        start_date: view::date_text(Some(defs.start_date)),
        months: defs.month_count.to_string(),
        sign_date: view::date_text(Some(ctx.clock().today())),
        old_monthly,
        new_monthly,
        diff_monthly,
        comparison,
        candidates,
        lines,
        selected,
        templates_appendix,
        templates_contract,
    })
}

fn apply(res: Result<Loaded, String>) {
    match res {
        Ok(loaded) => {
            let ui = STATE.with(|c| {
                let mut g = c.borrow_mut();
                let s = g.as_mut()?;
                s.candidates = loaded.candidates;
                s.lines = loaded.lines;
                s.selected_products = loaded.selected;
                s.templates_appendix = loaded.templates_appendix;
                s.templates_contract = loaded.templates_contract;
                s.template_index = if s.templates_appendix.is_empty() {
                    0
                } else {
                    1
                };
                s.ui.upgrade()
            });
            if let Some(ui) = ui {
                let hub = ui.global::<ContractHub>();
                hub.set_renew_parent_title(loaded.parent_title.into());
                hub.set_renew_mode("appendix".into());
                hub.set_renew_number(loaded.suggested_contract_number.into());
                hub.set_renew_start_date(loaded.start_date.into());
                hub.set_renew_months(loaded.months.into());
                hub.set_renew_sign_date(loaded.sign_date.into());
                hub.set_renew_old_monthly(loaded.old_monthly.into());
                hub.set_renew_new_monthly(loaded.new_monthly.into());
                hub.set_renew_diff_monthly(loaded.diff_monthly.into());
                hub.set_renew_comparison_text(loaded.comparison.into());
                hub.set_renew_error("".into());
                hub.set_detail_submode("renewal".into());
            }
            push_products();
            push_lines();
            push_templates();
        }
        Err(e) => toast_from_worker(ToastKind::Danger, e),
    }
}

fn push_products() {
    let payload = STATE.with(|c| {
        let g = c.borrow();
        let s = g.as_ref()?;
        let rows = s
            .candidates
            .iter()
            .map(|(id, name)| IdLabel {
                id: i32::try_from(*id).unwrap_or(0),
                label: name.clone().into(),
                checked: s.selected_products.contains(id),
            })
            .collect::<Vec<_>>();
        Some((s.ui.clone(), rows))
    });
    if let Some((ui, rows)) = payload
        && let Some(ui) = ui.upgrade()
    {
        ui.global::<ContractHub>()
            .set_renew_products(ModelRc::new(VecModel::from(rows)));
    }
}

fn push_lines() {
    let payload = STATE.with(|c| {
        let g = c.borrow();
        let s = g.as_ref()?;
        let mut opts = vec![SharedString::from("— Chọn gói —")];
        opts.extend(
            s.candidates
                .iter()
                .map(|(_, n)| SharedString::from(n.as_str())),
        );
        let rows = s
            .lines
            .iter()
            .enumerate()
            .map(|(idx, (old_label, picked))| {
                let selected_index = picked
                    .and_then(|pid| s.candidates.iter().position(|(id, _)| *id == pid))
                    .map(|pos| i32::try_from(pos + 1).unwrap_or(0))
                    .unwrap_or(0);
                RenewLineRow {
                    idx: i32::try_from(idx).unwrap_or(0),
                    old_label: old_label.as_str().into(),
                    selected_index,
                }
            })
            .collect::<Vec<_>>();
        Some((s.ui.clone(), opts, rows))
    });
    if let Some((ui, opts, rows)) = payload
        && let Some(ui) = ui.upgrade()
    {
        let hub = ui.global::<ContractHub>();
        hub.set_renew_candidate_names(ModelRc::new(VecModel::from(opts)));
        hub.set_renew_lines(ModelRc::new(VecModel::from(rows)));
    }
}

fn push_templates() {
    let payload = STATE.with(|c| {
        let g = c.borrow();
        let s = g.as_ref()?;
        let list = if s.mode == "contract" {
            &s.templates_contract
        } else {
            &s.templates_appendix
        };
        let mut names = vec![SharedString::from("— Không chọn —")];
        names.extend(list.iter().map(|(_, n)| SharedString::from(n.as_str())));
        let idx = i32::try_from(s.template_index).unwrap_or(0);
        Some((s.ui.clone(), names, idx))
    });
    if let Some((ui, names, idx)) = payload
        && let Some(ui) = ui.upgrade()
    {
        let hub = ui.global::<ContractHub>();
        hub.set_renew_templates(ModelRc::new(VecModel::from(names)));
        hub.set_renew_template_index(idx);
    }
}

fn save(number: &str, start_raw: &str, months_raw: &str, sign_raw: &str, and_gen: bool) {
    let Some((worker, parent_id, mode, picked_ids, template_id, ui)) = STATE.with(|c| {
        let g = c.borrow();
        let s = g.as_ref()?;
        let pid = s.parent_id?;
        let list = if s.mode == "contract" {
            &s.templates_contract
        } else {
            &s.templates_appendix
        };
        let tpl = s
            .template_index
            .checked_sub(1)
            .and_then(|idx| list.get(idx))
            .map(|(id, _)| *id);
        let picked_ids: Vec<i64> = if !s.lines.is_empty() {
            s.lines.iter().filter_map(|(_, id)| *id).collect()
        } else {
            s.selected_products.iter().copied().collect()
        };
        Some((
            s.worker.clone(),
            pid,
            s.mode.clone(),
            picked_ids,
            tpl,
            s.ui.clone(),
        ))
    }) else {
        return;
    };

    let start_date = match parse_date_input(Some(start_raw)) {
        Ok(Some(d)) => d,
        Ok(None) => {
            set_error(&ui, "Vui lòng nhập ngày bắt đầu.");
            return;
        }
        Err(e) => {
            set_error(&ui, &e.to_string());
            return;
        }
    };
    let sign_date = match parse_date_input(if sign_raw.trim().is_empty() {
        None
    } else {
        Some(sign_raw)
    }) {
        Ok(d) => d,
        Err(e) => {
            set_error(&ui, &e.to_string());
            return;
        }
    };
    let month_count = months_raw.trim().parse::<i64>().unwrap_or(0);
    let items = picked_ids
        .into_iter()
        .map(|product_id| AddonItem {
            product_id,
            description: None,
            quantity: None,
            unit_price_pre_vat: None,
            vat_rate: None,
            amount_pre_vat_override: None,
            note: String::new(),
        })
        .collect();
    let form = AddonForm {
        start_date,
        sign_date,
        template_id,
        items,
    };
    let number = number.to_owned();

    worker.run(
        move |ctx| -> Result<i64, String> {
            let new_id = if mode == "contract" {
                create_renewal_contract(ctx, parent_id, &number, form, month_count)
            } else {
                create_renewal_appendix(ctx, parent_id, form, month_count)
            }
            .map_err(|e| e.user_message())?;
            if and_gen {
                generate_docx(ctx, new_id, &[]).map_err(|e| e.user_message())?;
            }
            Ok(new_id)
        },
        move |res| {
            post_to_ui(move || match res {
                Ok(new_id) => {
                    toast_from_worker(ToastKind::Success, "Đã tạo gia hạn hợp đồng.".into());
                    if let Some(ui) = ui.upgrade() {
                        ui.global::<ContractHub>()
                            .set_detail_submode("detail".into());
                        ui.invoke_contracts_reload();
                        ui.invoke_contracts_open(i32::try_from(new_id).unwrap_or(0));
                    }
                }
                Err(e) => set_error(&ui, &e),
            });
        },
    );
}

fn set_error(ui: &Weak<MainWindow>, msg: &str) {
    if let Some(ui) = ui.upgrade() {
        ui.global::<ContractHub>().set_renew_error(msg.into());
    }
}
