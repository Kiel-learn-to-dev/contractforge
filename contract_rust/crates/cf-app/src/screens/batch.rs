//! S18 — Sinh hợp đồng hàng loạt (form 2 cột + xem trước sửa số HĐ, chạy nền có tiến độ & hủy, nhập Excel).

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use cf_core::{
    dates::parse_date_input,
    fmt::fmt_currency,
    pricing::ItemInput,
    template_rules::{self, Role},
    text::search_key,
};
use cf_services::{
    Ctx, ServiceError,
    batch::{
        excel::{import_excel_mapping, make_batch_template_xlsx},
        row::form_from_params,
        run::{BatchJob, BatchResult, run_batch},
    },
    contracts::{create::ContractForm, preview::price_preview, suggest::suggest_number},
    settings::get_party_b_settings,
};
use chrono::Datelike;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::{
    BatchHub, BatchImportPreviewRow, BatchOutcomeRow, BatchPreviewRow, IdLabel, MainWindow, files,
    ui_helpers::{Feedback, ToastKind, toast_from_worker},
    worker::{Progress, Worker},
};

type PreviewRow = (i64, String, String, String, String, String, i64, String);

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    feedback: Rc<Feedback>,
    templates: Vec<(i64, String)>,
    /// (id, code, label, group_code)
    products: Vec<(i64, String, String, Option<String>)>,
    /// (id, code, label, group_code)
    all_addons: Vec<(i64, String, String, Option<String>)>,
    group_addons: Vec<(i64, String, String)>,
    checked_addons: BTreeSet<i64>,
    /// (id, label, search_key)
    all_customers: Vec<(i64, String, String)>,
    customers: Vec<(i64, String)>,
    customer_search: String,
    checked_customers: BTreeSet<i64>,
    preview_rows: Vec<PreviewRow>,
    preview_numbers: BTreeMap<i64, String>,
    template_index: usize,
    product_index: usize,
    party_b_seeded: bool,
    cancel_flag: Option<Arc<AtomicBool>>,
    last_zip_path: Option<PathBuf>,
    output_batch_dir: Option<PathBuf>,
    excel_rows: Vec<BTreeMap<String, String>>,
}

struct LoadedCatalog {
    templates: Vec<(i64, String)>,
    products: Vec<(i64, String, String, Option<String>)>,
    addons: Vec<(i64, String, String, Option<String>)>,
    customers: Vec<(i64, String, String)>,
    today: String,
    output_batch_dir: PathBuf,
    party_b_defaults: (String, String, String),
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
            templates: Vec::new(),
            products: Vec::new(),
            all_addons: Vec::new(),
            group_addons: Vec::new(),
            checked_addons: BTreeSet::new(),
            all_customers: Vec::new(),
            customers: Vec::new(),
            customer_search: String::new(),
            checked_customers: BTreeSet::new(),
            preview_rows: Vec::new(),
            preview_numbers: BTreeMap::new(),
            template_index: 0,
            product_index: 0,
            party_b_seeded: false,
            cancel_flag: None,
            last_zip_path: None,
            output_batch_dir: None,
            excel_rows: Vec::new(),
        });
    });

    let hub = ui.global::<BatchHub>();
    hub.on_open_import(|| {
        with_hub(|hub| {
            hub.set_mode("import".into());
        });
    });
    hub.on_back_to_form(|| {
        with_hub(|hub| {
            hub.set_mode("form".into());
        });
    });
    hub.on_template_changed(|idx| {
        STATE.with(|cell| {
            if let Some(s) = cell.borrow_mut().as_mut()
                && let Ok(i) = usize::try_from(idx)
            {
                s.template_index = i;
            }
        });
    });
    hub.on_product_changed(|idx| {
        STATE.with(|cell| {
            if let Some(s) = cell.borrow_mut().as_mut()
                && let Ok(i) = usize::try_from(idx)
            {
                s.product_index = i;
                rebuild_group_addons(s);
            }
        });
        push_group_addons();
    });
    hub.on_toggle_addon(|idx| {
        STATE.with(|cell| {
            let mut g = cell.borrow_mut();
            let Some(s) = g.as_mut() else {
                return;
            };
            if let Ok(i) = usize::try_from(idx)
                && let Some((id, _, _)) = s.group_addons.get(i)
                && !s.checked_addons.remove(id)
            {
                s.checked_addons.insert(*id);
            }
        });
        push_group_addons();
    });
    hub.on_customer_search_changed(|q| {
        STATE.with(|cell| {
            if let Some(s) = cell.borrow_mut().as_mut() {
                s.customer_search = q.to_string();
                apply_customer_filter(s);
            }
        });
        push_customers();
    });
    hub.on_toggle_customer(|idx| {
        STATE.with(|cell| {
            let mut g = cell.borrow_mut();
            let Some(s) = g.as_mut() else {
                return;
            };
            if let Ok(i) = usize::try_from(idx)
                && let Some((id, _)) = s.customers.get(i)
                && !s.checked_customers.remove(id)
            {
                s.checked_customers.insert(*id);
            }
        });
        push_customers();
    });
    hub.on_select_all_customers(|all| {
        STATE.with(|cell| {
            let mut g = cell.borrow_mut();
            let Some(s) = g.as_mut() else {
                return;
            };
            if all {
                for (id, _) in &s.customers {
                    s.checked_customers.insert(*id);
                }
            } else {
                for (id, _) in &s.customers {
                    s.checked_customers.remove(id);
                }
            }
        });
        push_customers();
    });
    hub.on_refresh_preview(|sign_date, start_date, months| {
        refresh_preview(sign_date.as_str(), start_date.as_str(), months.as_str());
    });
    hub.on_preview_number_edited(|idx, val| {
        STATE.with(|cell| {
            let mut g = cell.borrow_mut();
            let Some(s) = g.as_mut() else {
                return;
            };
            if let Ok(i) = usize::try_from(idx)
                && let Some(row) = s.preview_rows.get_mut(i)
            {
                let v = val.to_string();
                s.preview_numbers.insert(row.0, v.clone());
                row.2 = v;
            }
        });
    });
    hub.on_run_batch(|sign_date, start_date, months| {
        run_from_form(sign_date.as_str(), start_date.as_str(), months.as_str());
    });
    hub.on_cancel_batch(|| {
        STATE.with(|cell| {
            if let Some(s) = cell.borrow().as_ref()
                && let Some(flag) = &s.cancel_flag
            {
                flag.store(true, Ordering::Relaxed);
            }
        });
        with_hub(|hub| {
            hub.set_progress_label("Đang dừng sau hợp đồng hiện tại...".into());
        });
    });
    hub.on_save_zip(save_zip);
    hub.on_open_batch_folder(open_batch_folder);
    hub.on_download_excel_template(download_excel_template);
    hub.on_pick_excel(pick_excel);
    hub.on_run_excel_batch(run_excel_batch);
}

pub fn reload() {
    let Some(worker) = STATE.with(|c| c.borrow().as_ref().map(|s| s.worker.clone())) else {
        return;
    };
    worker.run(
        |ctx| {
            let mut templates = ctx
                .store()
                .templates
                .values()
                .filter(|t| {
                    t.is_active && template_rules::has_role(&t.contract_type, Role::Contract)
                })
                .map(|t| (t.id, format!("{} ({})", t.name, t.code)))
                .collect::<Vec<_>>();
            templates.sort_by_key(|a| a.1.to_lowercase());

            let mut products = ctx
                .store()
                .products
                .values()
                .filter(|p| p.is_active && p.kind == "main")
                .map(|p| {
                    (
                        p.id,
                        p.code.clone(),
                        format!("{} ({})", p.name, p.code),
                        p.group_code.clone(),
                    )
                })
                .collect::<Vec<_>>();
            products.sort_by_key(|a| a.2.to_lowercase());

            let mut addons = ctx
                .store()
                .products
                .values()
                .filter(|p| p.is_active && p.kind == "addon")
                .map(|p| {
                    (
                        p.id,
                        p.code.clone(),
                        format!("[{}] {}", p.code, p.name),
                        p.group_code.clone(),
                    )
                })
                .collect::<Vec<_>>();
            addons.sort_by_key(|a| a.2.to_lowercase());

            let mut customers = ctx
                .store()
                .customers
                .values()
                .filter(|c| c.is_active && !c.is_deleted)
                .map(|c| {
                    let short = c
                        .short_name
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&c.legal_name);
                    let key = search_key(&format!("{short} {} {}", c.code, c.legal_name));
                    (c.id, format!("{short} [{}]", c.code), key)
                })
                .collect::<Vec<_>>();
            customers.sort_by_key(|a| a.1.to_lowercase());

            let pbd = get_party_b_settings(ctx.store());
            LoadedCatalog {
                templates,
                products,
                addons,
                customers,
                today: ctx.clock().today().format("%d/%m/%Y").to_string(),
                output_batch_dir: ctx.paths().output_batch.clone(),
                party_b_defaults: (
                    pbd.representative,
                    pbd.authorization_number,
                    pbd.authorization_date,
                ),
            }
        },
        |loaded| {
            let mut seeded_party_b = None;
            STATE.with(|cell| {
                let mut g = cell.borrow_mut();
                let Some(s) = g.as_mut() else {
                    return;
                };
                s.templates = loaded.templates;
                s.products = loaded.products;
                s.all_addons = loaded.addons;
                s.all_customers = loaded.customers;
                s.output_batch_dir = Some(loaded.output_batch_dir);
                if s.template_index >= s.templates.len() {
                    s.template_index = 0;
                }
                if s.product_index >= s.products.len() {
                    s.product_index = 0;
                }
                rebuild_group_addons(s);
                apply_customer_filter(s);
                if !s.party_b_seeded {
                    s.party_b_seeded = true;
                    seeded_party_b = Some(loaded.party_b_defaults);
                }
            });
            with_hub(|hub| {
                if hub.get_sign_date().is_empty() {
                    hub.set_sign_date(loaded.today.clone().into());
                }
                if hub.get_start_date().is_empty() {
                    hub.set_start_date(loaded.today.into());
                }
                if let Some((rep, auth_num, auth_date)) = seeded_party_b {
                    if hub.get_party_b_rep().is_empty() {
                        hub.set_party_b_rep(rep.into());
                    }
                    if hub.get_party_b_auth_num().is_empty() {
                        hub.set_party_b_auth_num(auth_num.into());
                    }
                    if hub.get_party_b_auth_date().is_empty() {
                        hub.set_party_b_auth_date(auth_date.into());
                    }
                }
            });
            push_catalog();
        },
    );
}

fn rebuild_group_addons(s: &mut State) {
    let group = s
        .products
        .get(s.product_index)
        .and_then(|(_, _, _, g)| g.as_deref())
        .map(|g| g.trim().to_ascii_lowercase());
    s.group_addons = s
        .all_addons
        .iter()
        .filter(|(_, _, _, ag)| match (&group, ag) {
            (Some(g), Some(a)) => g == &a.trim().to_ascii_lowercase(),
            _ => false,
        })
        .map(|(id, code, label, _)| (*id, code.clone(), label.clone()))
        .collect();
    let valid_ids: BTreeSet<i64> = s.group_addons.iter().map(|(id, _, _)| *id).collect();
    s.checked_addons.retain(|id| valid_ids.contains(id));
}

fn apply_customer_filter(s: &mut State) {
    let q = search_key(s.customer_search.trim());
    s.customers = s
        .all_customers
        .iter()
        .filter(|(_, _, key)| q.is_empty() || key.contains(&q))
        .map(|(id, label, _)| (*id, label.clone()))
        .collect();
}

fn push_catalog() {
    let snapshot = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        let t_names = s
            .templates
            .iter()
            .map(|(_, l)| SharedString::from(l.as_str()))
            .collect::<Vec<_>>();
        let p_names = s
            .products
            .iter()
            .map(|(_, _, l, _)| SharedString::from(l.as_str()))
            .collect::<Vec<_>>();
        Some((
            t_names,
            p_names,
            s.template_index as i32,
            s.product_index as i32,
        ))
    });
    if let Some((t_names, p_names, t_idx, p_idx)) = snapshot {
        with_hub(|hub| {
            hub.set_template_names(ModelRc::new(VecModel::from(t_names)));
            hub.set_product_names(ModelRc::new(VecModel::from(p_names)));
            hub.set_template_index(t_idx);
            hub.set_product_index(p_idx);
        });
    }
    push_group_addons();
    push_customers();
}

fn push_group_addons() {
    let rows = STATE.with(|cell| {
        let g = cell.borrow();
        let Some(s) = g.as_ref() else {
            return Vec::new();
        };
        s.group_addons
            .iter()
            .map(|(id, _, label)| IdLabel {
                id: *id as i32,
                label: label.as_str().into(),
                checked: s.checked_addons.contains(id),
            })
            .collect::<Vec<_>>()
    });
    with_hub(|hub| {
        hub.set_group_addons(ModelRc::new(VecModel::from(rows)));
    });
}

fn push_customers() {
    let rows = STATE.with(|cell| {
        let g = cell.borrow();
        let Some(s) = g.as_ref() else {
            return Vec::new();
        };
        s.customers
            .iter()
            .map(|(id, label)| IdLabel {
                id: *id as i32,
                label: label.as_str().into(),
                checked: s.checked_customers.contains(id),
            })
            .collect::<Vec<_>>()
    });
    with_hub(|hub| {
        hub.set_customers(ModelRc::new(VecModel::from(rows)));
    });
}

fn refresh_preview(sign_date_raw: &str, start_date_raw: &str, months_raw: &str) {
    let params = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        let product_id = s.products.get(s.product_index).map(|(id, _, _, _)| *id);
        let addons: Vec<i64> = s.checked_addons.iter().copied().collect();
        let selected = s
            .all_customers
            .iter()
            .filter(|(id, _, _)| s.checked_customers.contains(id))
            .map(|(id, name, _)| (*id, name.clone()))
            .collect::<Vec<_>>();
        Some((
            s.worker.clone(),
            Rc::clone(&s.feedback),
            product_id,
            addons,
            selected,
        ))
    });
    let Some((worker, feedback, product_id, addons, selected)) = params else {
        return;
    };
    if selected.is_empty() {
        feedback.toast(ToastKind::Warning, "Hãy chọn ít nhất một khách hàng.");
        return;
    }
    let sign_date = sign_date_raw.to_owned();
    let start_date = start_date_raw.to_owned();
    let months_str = months_raw.to_owned();
    let month_count = months_raw.trim().parse::<i64>().unwrap_or(12).max(1);
    worker.run(
        move |ctx| {
            let store = ctx.store();
            let year = parse_date_input(Some(sign_date.as_str()))
                .ok()
                .flatten()
                .unwrap_or_else(|| ctx.clock().today())
                .year();
            selected
                .into_iter()
                .map(|(cid, name)| {
                    let num = suggest_number(store, cid, product_id, year)
                        .unwrap_or_else(|e| format!("Lỗi: {}", e.user_message()));
                    let billing_units = {
                        let c = store.customers.get(&cid);
                        let sub = store
                            .units
                            .values()
                            .filter(|u| u.customer_id == cid && u.is_active && u.include_in_billing)
                            .count();
                        let main = usize::from(c.is_some_and(|c| c.bill_main_station));
                        (main + sub).max(1) as i64
                    };
                    let amount_text = if let Some(pid) = product_id {
                        let mut items = vec![ItemInput {
                            product_id: pid,
                            description: String::new(),
                            quantity: billing_units.to_string(),
                            unit_price_pre_vat: None,
                            vat_rate: None,
                            amount_pre_vat_override: None,
                            note: String::new(),
                        }];
                        for aid in &addons {
                            items.push(ItemInput {
                                product_id: *aid,
                                description: String::new(),
                                quantity: billing_units.to_string(),
                                unit_price_pre_vat: None,
                                vat_rate: None,
                                amount_pre_vat_override: None,
                                note: String::new(),
                            });
                        }
                        price_preview(store, &items, month_count, true)
                            .map(|p| format!("{} VNĐ", fmt_currency(Some(p.totals.total))))
                            .unwrap_or_else(|_| "—".into())
                    } else {
                        "—".into()
                    };
                    (
                        cid,
                        name,
                        num,
                        sign_date.clone(),
                        start_date.clone(),
                        months_str.clone(),
                        billing_units,
                        amount_text,
                    )
                })
                .collect::<Vec<_>>()
        },
        |items| {
            STATE.with(|cell| {
                if let Some(s) = cell.borrow_mut().as_mut() {
                    s.preview_numbers.clear();
                    for (cid, _, num, _, _, _, _, _) in &items {
                        s.preview_numbers.insert(*cid, num.clone());
                    }
                    s.preview_rows = items.clone();
                }
            });
            let rows = items
                .into_iter()
                .map(
                    |(
                        cid,
                        customer_name,
                        contract_number,
                        sign_date,
                        start_date,
                        months,
                        units,
                        amount_text,
                    )| BatchPreviewRow {
                        customer_id: cid as i32,
                        customer_name: customer_name.into(),
                        contract_number: contract_number.into(),
                        sign_date: sign_date.into(),
                        start_date: start_date.into(),
                        months: months.into(),
                        units: units as i32,
                        amount_text: amount_text.into(),
                    },
                )
                .collect::<Vec<_>>();
            with_hub(|hub| {
                hub.set_preview_rows(ModelRc::new(VecModel::from(rows)));
            });
        },
    );
}

fn run_from_form(sign_date_raw: &str, start_date_raw: &str, months_raw: &str) {
    let mut party_b = (String::new(), String::new(), String::new());
    with_hub(|hub| {
        party_b = (
            hub.get_party_b_rep().trim().to_owned(),
            hub.get_party_b_auth_num().trim().to_owned(),
            hub.get_party_b_auth_date().trim().to_owned(),
        );
    });
    let setup = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        let template_id = s.templates.get(s.template_index).map(|(id, _)| *id);
        let main_code = s
            .products
            .get(s.product_index)
            .map(|(_, code, _, _)| code.clone());
        let addon_codes = s
            .group_addons
            .iter()
            .filter(|(id, _, _)| s.checked_addons.contains(id))
            .map(|(_, code, _)| code.clone())
            .collect::<Vec<_>>()
            .join(",");
        let selected = s
            .all_customers
            .iter()
            .filter(|(id, _, _)| s.checked_customers.contains(id))
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>();
        Some((
            s.worker.clone(),
            Rc::clone(&s.feedback),
            template_id,
            main_code,
            addon_codes,
            s.preview_numbers.clone(),
            selected,
        ))
    });
    let Some((worker, feedback, template_id, main_code, addon_codes, preview_numbers, selected)) =
        setup
    else {
        return;
    };
    let Some(template_id) = template_id else {
        feedback.toast(ToastKind::Warning, "Chưa có mẫu hợp đồng khả dụng.");
        return;
    };
    let Some(main_code) = main_code else {
        feedback.toast(ToastKind::Warning, "Chưa có gói sản phẩm chính.");
        return;
    };
    if selected.is_empty() {
        feedback.toast(ToastKind::Warning, "Hãy chọn ít nhất một khách hàng.");
        return;
    }
    let sign_date = sign_date_raw.trim().to_owned();
    let start_date = start_date_raw.trim().to_owned();
    let months = months_raw.trim().to_owned();
    if sign_date.is_empty() || start_date.is_empty() {
        feedback.toast(ToastKind::Warning, "Nhập ngày ký và ngày bắt đầu.");
        return;
    }
    let total = selected.len();
    let specs = selected
        .into_iter()
        .map(|cid| {
            let mut params = BTreeMap::new();
            params.insert("main_code".into(), main_code.clone());
            if !addon_codes.is_empty() {
                params.insert("addon_codes".into(), addon_codes.clone());
            }
            if let Some(num) = preview_numbers.get(&cid).filter(|s| !s.trim().is_empty()) {
                params.insert("contract_number".into(), num.clone());
            }
            params.insert("sign_date".into(), sign_date.clone());
            params.insert("start_date".into(), start_date.clone());
            params.insert("month_count".into(), months.clone());
            if !party_b.0.is_empty() {
                params.insert("party_b_representative".into(), party_b.0.clone());
            }
            if !party_b.1.is_empty() {
                params.insert("party_b_authorization_number".into(), party_b.1.clone());
            }
            if !party_b.2.is_empty() {
                params.insert("party_b_authorization_date".into(), party_b.2.clone());
            }
            (cid, params)
        })
        .collect::<Vec<_>>();
    let year_hint = parse_date_input(Some(&sign_date))
        .ok()
        .flatten()
        .map(|d| d.year());
    launch_batch_task(&worker, template_id, year_hint, specs, total);
}

fn run_excel_batch() {
    let mut prefix = String::new();
    with_hub(|hub| {
        prefix = hub.get_import_prefix().trim().to_owned();
    });
    let setup = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        let template_id = s.templates.get(s.template_index).map(|(id, _)| *id);
        Some((
            s.worker.clone(),
            Rc::clone(&s.feedback),
            template_id,
            s.excel_rows.clone(),
        ))
    });
    let Some((worker, feedback, template_id, excel_rows)) = setup else {
        return;
    };
    let Some(template_id) = template_id else {
        feedback.toast(ToastKind::Warning, "Chưa chọn mẫu hợp đồng.");
        return;
    };
    if excel_rows.is_empty() {
        feedback.toast(ToastKind::Warning, "Chưa có dòng dữ liệu hợp lệ từ Excel.");
        return;
    }
    let mut specs = Vec::with_capacity(excel_rows.len());
    for (idx, mut row) in excel_rows.into_iter().enumerate() {
        let Ok(cid) = row
            .get("customer_id")
            .map(String::as_str)
            .unwrap_or_default()
            .parse::<i64>()
        else {
            feedback.toast(
                ToastKind::Danger,
                "Dữ liệu Excel thiếu mã khách hàng hợp lệ.",
            );
            return;
        };
        if row
            .get("contract_number")
            .is_none_or(|v| v.trim().is_empty())
            && !prefix.is_empty()
        {
            row.insert("contract_number".into(), format!("{prefix}-{:02}", idx + 1));
        }
        specs.push((cid, row));
    }
    let total = specs.len();
    launch_batch_task(&worker, template_id, None, specs, total);
}

fn launch_batch_task(
    worker: &Worker<Ctx>,
    template_id: i64,
    year_hint: Option<i32>,
    specs: Vec<(i64, BTreeMap<String, String>)>,
    total: usize,
) {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_worker = Arc::clone(&cancel);
    STATE.with(|cell| {
        if let Some(s) = cell.borrow_mut().as_mut() {
            s.cancel_flag = Some(Arc::clone(&cancel));
            s.last_zip_path = None;
        }
    });
    with_hub(|hub| {
        hub.set_mode("result".into());
        hub.set_running(true);
        hub.set_progress_done(0);
        hub.set_progress_total(total as i32);
        hub.set_progress_label("Đang chuẩn bị chạy lô...".into());
        hub.set_result_summary("".into());
        hub.set_has_zip(false);
        hub.set_outcome_rows(ModelRc::new(VecModel::from(Vec::<BatchOutcomeRow>::new())));
    });

    worker.run_task(
        cancel,
        on_progress,
        move |ctx, task| -> Result<(PathBuf, BatchResult), ServiceError> {
            let mut rows: Vec<ContractForm> = Vec::with_capacity(specs.len());
            for (cid, params) in specs {
                rows.push(form_from_params(ctx.store(), cid, template_id, &params)?);
            }
            let year = year_hint.unwrap_or_else(|| ctx.clock().today().year());
            let job_id = ctx.clock().now_local().format("%Y%m%d%H%M%S").to_string();
            let result = run_batch(
                ctx,
                BatchJob {
                    id: job_id,
                    template_id,
                    year,
                    rows,
                },
                |done, total| task.report(done, total, "Đang sinh hợp đồng..."),
                &cancel_worker,
            )?;
            Ok((ctx.paths().output_batch.clone(), result))
        },
        on_batch_done,
    );
}

fn on_progress(progress: Progress) {
    with_hub(|hub| {
        hub.set_progress_done(progress.done as i32);
        hub.set_progress_total(progress.total as i32);
        hub.set_progress_label(progress.label.into());
    });
}

fn on_batch_done(outcome: Result<(PathBuf, BatchResult), ServiceError>) {
    match outcome {
        Ok((dir, result)) => {
            STATE.with(|cell| {
                if let Some(s) = cell.borrow_mut().as_mut() {
                    s.cancel_flag = None;
                    s.last_zip_path = result.zip_path.clone();
                    s.output_batch_dir = Some(dir);
                }
            });
            let summary = if result.cancelled {
                format!(
                    "Đã hủy giữa chừng: thành công {}/{}, lỗi {}.",
                    result.succeeded, result.total, result.failed
                )
            } else {
                format!(
                    "Hoàn tất lô {}: thành công {}/{}, lỗi {}.",
                    result.job_id, result.succeeded, result.total, result.failed
                )
            };
            let rows = result
                .rows
                .into_iter()
                .map(|row| {
                    let ok = row.error.is_none();
                    let status = if ok { "Thành công" } else { "Lỗi" };
                    let message = if let Some(generated) = row.generated {
                        generated.file_name
                    } else {
                        row.error.unwrap_or_default()
                    };
                    let number = if row.contract_number.trim().is_empty() {
                        format!("KH #{}", row.customer_id)
                    } else {
                        row.contract_number
                    };
                    BatchOutcomeRow {
                        contract_number: number.into(),
                        status: status.into(),
                        message: message.into(),
                        ok,
                    }
                })
                .collect::<Vec<_>>();
            with_hub(|hub| {
                hub.set_running(false);
                hub.set_progress_done(result.total as i32);
                hub.set_progress_total(result.total as i32);
                hub.set_result_summary(summary.clone().into());
                hub.set_has_zip(result.zip_path.is_some());
                hub.set_outcome_rows(ModelRc::new(VecModel::from(rows)));
            });
            crate::screens::contracts_list::reload();
            let kind = if result.failed > 0 || result.cancelled {
                ToastKind::Warning
            } else {
                ToastKind::Success
            };
            toast_from_worker(kind, summary);
        }
        Err(error) => {
            STATE.with(|cell| {
                if let Some(s) = cell.borrow_mut().as_mut() {
                    s.cancel_flag = None;
                }
            });
            let msg = error.user_message().to_owned();
            with_hub(|hub| {
                hub.set_running(false);
                hub.set_result_summary(format!("Lỗi chạy lô: {msg}").into());
            });
            toast_from_worker(ToastKind::Danger, msg);
        }
    }
}

fn save_zip() {
    let info = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        Some((Rc::clone(&s.feedback), s.last_zip_path.clone()))
    });
    let Some((feedback, Some(zip_path))) = info else {
        return;
    };
    let Ok(bytes) = std::fs::read(&zip_path) else {
        feedback.toast(ToastKind::Danger, "Không đọc được file ZIP kết quả.");
        return;
    };
    let name = zip_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("batch_contracts.zip");
    let Some(dest) = files::save_as(name, ("ZIP", &["zip"])) else {
        return;
    };
    match files::write_chosen(&dest, &bytes) {
        Ok(()) => {
            let _ = files::open_folder(&dest);
            feedback.toast(ToastKind::Success, &format!("Đã lưu {}", dest.display()));
        }
        Err(error) => feedback.toast(ToastKind::Danger, &format!("Không lưu được ZIP: {error}")),
    }
}

fn open_batch_folder() {
    let info = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        Some((Rc::clone(&s.feedback), s.output_batch_dir.clone()))
    });
    let Some((feedback, Some(dir))) = info else {
        return;
    };
    if let Err(error) =
        std::fs::create_dir_all(&dir).and_then(|()| open::that(&dir).map_err(std::io::Error::other))
    {
        feedback.toast(
            ToastKind::Danger,
            &format!("Không mở được thư mục: {error}"),
        );
    }
}

fn download_excel_template() {
    let Some(feedback) = STATE.with(|c| c.borrow().as_ref().map(|s| Rc::clone(&s.feedback))) else {
        return;
    };
    let bytes = match make_batch_template_xlsx() {
        Ok(b) => b,
        Err(error) => {
            feedback.toast(ToastKind::Danger, &error.user_message());
            return;
        }
    };
    let Some(dest) = files::save_as("Mau_SinhHangLoat.xlsx", ("Excel", &["xlsx"])) else {
        return;
    };
    match files::write_chosen(&dest, &bytes) {
        Ok(()) => {
            let _ = files::open_folder(&dest);
            feedback.toast(ToastKind::Success, &format!("Đã lưu {}", dest.display()));
        }
        Err(error) => feedback.toast(ToastKind::Danger, &format!("Không lưu được file: {error}")),
    }
}

fn pick_excel() {
    let info = STATE.with(|c| {
        let g = c.borrow();
        let s = g.as_ref()?;
        Some((s.worker.clone(), Rc::clone(&s.feedback)))
    });
    let Some((worker, feedback)) = info else {
        return;
    };
    let Some(path) = files::pick_file(&[("Excel", &["xlsx", "xls"])]) else {
        return;
    };
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(error) => {
            feedback.toast(ToastKind::Danger, &format!("Không đọc được file: {error}"));
            return;
        }
    };
    worker.run(
        move |ctx| {
            let mut lookup = BTreeMap::new();
            for customer in ctx
                .store()
                .customers
                .values()
                .filter(|c| c.is_active && !c.is_deleted)
            {
                lookup.insert(customer.code.clone(), customer.id);
                lookup.insert(customer.legal_name.clone(), customer.id);
                if let Some(short) = customer
                    .short_name
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                {
                    lookup.insert(short.to_owned(), customer.id);
                }
            }
            (filename, import_excel_mapping(&bytes, &lookup))
        },
        |(filename, parsed)| {
            let ready = parsed.rows.len();
            let err_count = parsed.errors.len();
            let summary = format!(
                "Tổng {} dòng: {} hợp lệ, {} lỗi.",
                parsed.total_rows, ready, err_count
            );
            let errors = parsed
                .errors
                .into_iter()
                .map(|(row, msg)| {
                    if row == 0 {
                        SharedString::from(msg)
                    } else {
                        SharedString::from(format!("Dòng {row}: {msg}"))
                    }
                })
                .collect::<Vec<_>>();
            let preview_rows = parsed
                .rows
                .iter()
                .map(|row| BatchImportPreviewRow {
                    customer_code: row
                        .get("customer_code")
                        .or_else(|| row.get("customer_id"))
                        .map_or("—", String::as_str)
                        .into(),
                    contract_number: row
                        .get("contract_number")
                        .filter(|s| !s.trim().is_empty())
                        .map_or("—", String::as_str)
                        .into(),
                    sign_date: row
                        .get("sign_date")
                        .filter(|s| !s.trim().is_empty())
                        .map_or("—", String::as_str)
                        .into(),
                    total_amount: row
                        .get("total_amount")
                        .filter(|s| !s.trim().is_empty())
                        .map_or("—", String::as_str)
                        .into(),
                })
                .collect::<Vec<_>>();
            STATE.with(|cell| {
                if let Some(s) = cell.borrow_mut().as_mut() {
                    s.excel_rows = parsed.rows;
                }
            });
            with_hub(|hub| {
                hub.set_import_filename(filename.into());
                hub.set_import_ready_count(ready as i32);
                hub.set_import_summary(summary.into());
                hub.set_import_errors(ModelRc::new(VecModel::from(errors)));
                hub.set_import_preview_rows(ModelRc::new(VecModel::from(preview_rows)));
            });
        },
    );
}

fn with_hub<F: FnOnce(&BatchHub)>(f: F) {
    STATE.with(|cell| {
        if let Some(ui) = cell.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
            f(&ui.global::<BatchHub>());
        }
    });
}
