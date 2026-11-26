//! S16 — phụ lục bổ sung gói giữa kỳ.

use std::{cell::RefCell, collections::BTreeSet};

use cf_core::{dates::parse_date_input, template_rules::Role};
use cf_services::{
    Ctx,
    addenda::addon::{AddonForm, AddonItem, KIND_ADDON, addendum_defaults, create_addendum},
    contracts::generate::generate_docx,
    templates::list::templates_for,
};
use chrono::NaiveDate;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::{
    ContractHub, IdLabel, MainWindow,
    ui_helpers::{ToastKind, post_to_ui, toast_from_worker},
    view,
    worker::Worker,
};

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    parent_id: Option<i64>,
    candidates: Vec<(i64, String)>,
    selected_products: BTreeSet<i64>,
    templates: Vec<(i64, String)>,
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
            candidates: Vec::new(),
            selected_products: BTreeSet::new(),
            templates: Vec::new(),
            template_index: 0,
        });
    });

    let hub = ui.global::<ContractHub>();
    hub.on_open_addendum(|| {
        if let Some(pid) = crate::screens::contract_detail::current_id() {
            open(pid);
        }
    });
    hub.on_addon_start_changed(|raw| {
        if let Ok(Some(date)) = parse_date_input(Some(raw.as_str())) {
            refresh_defaults(date, false);
        }
    });
    hub.on_addon_toggle_product(|idx| {
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
    hub.on_addon_template_changed(|idx| {
        STATE.with(|c| {
            if let Some(s) = c.borrow_mut().as_mut()
                && let Ok(i) = usize::try_from(idx)
            {
                s.template_index = i;
            }
        });
    });
    hub.on_addon_save(|start_raw, sign_raw, and_gen| {
        save(start_raw.as_str(), sign_raw.as_str(), and_gen);
    });
}

pub fn open(parent_id: i64) {
    let today = chrono::Local::now().date_naive();
    STATE.with(|c| {
        if let Some(s) = c.borrow_mut().as_mut() {
            s.parent_id = Some(parent_id);
            s.selected_products.clear();
            s.template_index = 0;
        }
    });
    refresh_defaults(today, true);
}

fn refresh_defaults(start_date: NaiveDate, reset_selections: bool) {
    let Some((worker, parent_id)) = STATE.with(|c| {
        c.borrow()
            .as_ref()
            .and_then(|s| s.parent_id.map(|id| (s.worker.clone(), id)))
    }) else {
        return;
    };
    worker.run(
        move |ctx| load_defaults(ctx, parent_id, start_date),
        move |res| post_to_ui(move || apply_defaults(res, reset_selections)),
    );
}

struct Loaded {
    parent_title: String,
    parent_customer: String,
    parent_product: String,
    parent_dates: String,
    number: String,
    start_date: String,
    end_date: String,
    months: String,
    sign_date: String,
    candidates: Vec<(i64, String)>,
    templates: Vec<(i64, String)>,
}

fn load_defaults(ctx: &Ctx, parent_id: i64, start_date: NaiveDate) -> Result<Loaded, String> {
    let store = ctx.store();
    let parent = store
        .contracts
        .get(&parent_id)
        .ok_or_else(|| "Không tìm thấy hợp đồng gốc.".to_owned())?;
    let clamped = match (parent.start_date, parent.end_date) {
        (Some(s), Some(e)) => start_date.clamp(s, e),
        _ => start_date,
    };
    let defs =
        addendum_defaults(store, parent_id, KIND_ADDON, clamped).map_err(|e| e.user_message())?;
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
    let templates = templates_for(store, group, Role::Addon, true)
        .into_iter()
        .map(|t| (t.id, t.name.clone()))
        .collect::<Vec<_>>();
    let parent_customer = store
        .customers
        .get(&parent.customer_id)
        .map(|c| c.legal_name.clone())
        .unwrap_or_default();
    let parent_product = parent
        .product_id
        .and_then(|id| store.products.get(&id))
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let parent_dates = format!(
        "{} → {}",
        view::date_text(parent.start_date),
        view::date_text(parent.end_date)
    );
    Ok(Loaded {
        parent_title: parent.contract_number.clone(),
        parent_customer,
        parent_product,
        parent_dates,
        number: defs.number,
        start_date: view::date_text(Some(defs.start_date)),
        end_date: view::date_text(Some(defs.end_date)),
        months: defs.month_count.to_string(),
        sign_date: view::date_text(Some(ctx.clock().today())),
        candidates,
        templates,
    })
}

fn apply_defaults(res: Result<Loaded, String>, reset_selections: bool) {
    match res {
        Ok(loaded) => {
            let ui = STATE.with(|c| {
                let mut g = c.borrow_mut();
                let s = g.as_mut()?;
                s.candidates = loaded.candidates;
                s.templates = loaded.templates;
                if reset_selections {
                    s.template_index = if s.templates.is_empty() { 0 } else { 1 };
                }
                s.ui.upgrade()
            });
            if let Some(ui) = ui {
                let hub = ui.global::<ContractHub>();
                hub.set_addon_parent_title(loaded.parent_title.into());
                hub.set_addon_parent_customer(loaded.parent_customer.into());
                hub.set_addon_parent_product(loaded.parent_product.into());
                hub.set_addon_parent_dates(loaded.parent_dates.into());
                hub.set_addon_number(loaded.number.into());
                if reset_selections || hub.get_addon_start_date().is_empty() {
                    hub.set_addon_start_date(loaded.start_date.into());
                }
                hub.set_addon_end_date(loaded.end_date.into());
                hub.set_addon_months(loaded.months.into());
                if reset_selections || hub.get_addon_sign_date().is_empty() {
                    hub.set_addon_sign_date(loaded.sign_date.into());
                }
                let (tpl_names, tpl_idx) = STATE.with(|c| {
                    c.borrow()
                        .as_ref()
                        .map(|s| {
                            let mut names = vec![SharedString::from("— Không chọn —")];
                            names.extend(
                                s.templates
                                    .iter()
                                    .map(|(_, n)| SharedString::from(n.as_str())),
                            );
                            (names, i32::try_from(s.template_index).unwrap_or(0))
                        })
                        .unwrap_or_default()
                });
                hub.set_addon_templates(ModelRc::new(VecModel::from(tpl_names)));
                hub.set_addon_template_index(tpl_idx);
                hub.set_addon_error("".into());
                hub.set_detail_submode("addendum".into());
            }
            push_products();
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
            .set_addon_products(ModelRc::new(VecModel::from(rows)));
    }
}

fn save(start_raw: &str, sign_raw: &str, and_gen: bool) {
    let Some((worker, parent_id, selected, template_id, ui)) = STATE.with(|c| {
        let g = c.borrow();
        let s = g.as_ref()?;
        let pid = s.parent_id?;
        let tpl = s
            .template_index
            .checked_sub(1)
            .and_then(|idx| s.templates.get(idx))
            .map(|(id, _)| *id);
        Some((
            s.worker.clone(),
            pid,
            s.selected_products.iter().copied().collect::<Vec<_>>(),
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
    let items = selected
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

    worker.run(
        move |ctx| -> Result<i64, String> {
            let new_id = create_addendum(
                ctx,
                parent_id,
                KIND_ADDON,
                AddonForm {
                    start_date,
                    sign_date,
                    template_id,
                    items,
                },
            )
            .map_err(|e| e.user_message())?;
            if and_gen {
                generate_docx(ctx, new_id, &[]).map_err(|e| e.user_message())?;
            }
            Ok(new_id)
        },
        move |res| {
            post_to_ui(move || match res {
                Ok(new_id) => {
                    toast_from_worker(ToastKind::Success, "Đã tạo phụ lục bổ sung.".into());
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
        ui.global::<ContractHub>().set_addon_error(msg.into());
    }
}
