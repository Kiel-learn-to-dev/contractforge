//! S19 — Báo giá (3 tab: Theo danh mục / Tự nhập / Yêu cầu báo giá).

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use cf_core::{
    dates::parse_date_input, fmt::fmt_currency, pricing::ItemInput, template_rules::Role,
};
use cf_services::{
    Ctx, ServiceError,
    contracts::preview::price_preview,
    quotations::{
        catalog::{
            generate_catalog_quotation, list_quotation_products, list_quotation_templates,
            resolve_quotation_selection,
        },
        issuers::{IssuerInput, list_issuers, save_issuer},
        manual::{LAYOUTS, build_document, build_lines, render_document},
        request::{
            RequestContextInput, build_request_context, build_request_lists,
            install_sample_templates, list_request_templates, render_request_docx,
            request_filename, resolve_request_items, resolve_request_template,
        },
    },
    templates::list::templates_for,
};
use serde_json::json;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::{
    IdLabel, MainWindow, ManualItemRow, QuotationHub, files,
    ui_helpers::{Feedback, ToastKind, toast_from_worker},
    worker::Worker,
};

const BLANK_LAYOUT_BYTES: [(&str, &str); 5] = [
    ("compact", "sample_quote_compact.docx"),
    ("formal", "sample_quote_formal.docx"),
    ("landscape", "sample_quote_landscape.docx"),
    ("features", "sample_quote_features.docx"),
    ("terms", "sample_quote_terms.docx"),
];

#[derive(Clone)]
struct CustomerSnapshot {
    id: i64,
    label: String,
    legal_name: String,
    address: String,
    tax_code: String,
    contact: String,
}

#[derive(Clone)]
struct ProductSnapshot {
    id: i64,
    group_code: Option<String>,
    label: String,
    name: String,
    unit_label: String,
    price: String,
    vat: String,
    duration_months: Option<i64>,
}

#[derive(Clone)]
struct IssuerSnapshot {
    label: String,
    name: String,
    address: String,
    tax_code: String,
    representative: String,
    title: String,
    phone: String,
    email: String,
}

#[derive(Clone)]
struct ItemDraft {
    name: String,
    unit: String,
    quantity: String,
    periods: String,
    price: String,
    vat: String,
}

impl Default for ItemDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            unit: "Đơn vị".into(),
            quantity: "1".into(),
            periods: "12".into(),
            price: "0".into(),
            vat: "8".into(),
        }
    }
}

struct LoadedData {
    customers: Vec<CustomerSnapshot>,
    products: Vec<ProductSnapshot>,
    all_addons: Vec<(i64, Option<String>, String)>,
    catalog_templates: Vec<(i64, String)>,
    issuers: Vec<IssuerSnapshot>,
    request_templates: Vec<(i64, String)>,
    today: String,
}

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    feedback: Rc<Feedback>,
    today: String,
    customers: Vec<CustomerSnapshot>,
    products: Vec<ProductSnapshot>,
    all_addons: Vec<(i64, Option<String>, String)>,
    catalog_addons: Vec<(i64, String)>,
    catalog_checked_addons: BTreeSet<i64>,
    catalog_templates: Vec<(i64, String)>,
    issuers: Vec<IssuerSnapshot>,
    request_templates: Vec<(i64, String)>,
    request_addons: Vec<(i64, String)>,
    request_checked_addons: BTreeSet<i64>,
    manual_items: Vec<ItemDraft>,
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
            today: String::new(),
            customers: Vec::new(),
            products: Vec::new(),
            all_addons: Vec::new(),
            catalog_addons: Vec::new(),
            catalog_checked_addons: BTreeSet::new(),
            catalog_templates: Vec::new(),
            issuers: Vec::new(),
            request_templates: Vec::new(),
            request_addons: Vec::new(),
            request_checked_addons: BTreeSet::new(),
            manual_items: vec![ItemDraft::default()],
        });
    });

    let layout_names = LAYOUTS
        .iter()
        .map(|(_, label, desc)| SharedString::from(format!("{label} — {desc}")))
        .collect::<Vec<_>>();
    let hub = ui.global::<QuotationHub>();
    hub.set_layout_names(ModelRc::new(VecModel::from(layout_names)));
    sync_manual_items();

    hub.on_catalog_customer_changed(|_idx| {
        refresh_catalog_summary();
    });
    hub.on_catalog_product_changed(|prod_idx| {
        on_catalog_product_changed(prod_idx);
    });
    hub.on_catalog_template_changed(|_idx| {
        refresh_catalog_summary();
    });
    hub.on_catalog_params_changed(|_date, _months| {
        refresh_catalog_summary();
    });
    hub.on_catalog_toggle_addon(|idx| {
        STATE.with(|c| {
            let mut g = c.borrow_mut();
            let Some(s) = g.as_mut() else {
                return;
            };
            if let Ok(i) = usize::try_from(idx)
                && let Some((id, _)) = s.catalog_addons.get(i)
                && !s.catalog_checked_addons.remove(id)
            {
                s.catalog_checked_addons.insert(*id);
            }
        });
        sync_catalog_addons();
        refresh_catalog_summary();
    });
    hub.on_catalog_reset(|| {
        let reset_vals = STATE.with(|c| {
            let mut g = c.borrow_mut();
            let s = g.as_mut()?;
            s.catalog_checked_addons.clear();
            let m = s
                .products
                .first()
                .and_then(|p| p.duration_months)
                .filter(|m| *m > 0)
                .unwrap_or(12)
                .to_string();
            Some((s.today.clone(), m))
        });
        if let Some((today, months)) = reset_vals {
            with_hub(|hub| {
                hub.set_customer_index(0);
                hub.set_product_index(0);
                hub.set_catalog_quote_date(today.into());
                hub.set_catalog_months(months.into());
            });
            on_catalog_product_changed(0);
        }
    });
    hub.on_catalog_generate(|cust_idx, prod_idx, tmpl_idx, date_raw, months_raw| {
        catalog_generate(
            cust_idx,
            prod_idx,
            tmpl_idx,
            date_raw.as_str(),
            months_raw.as_str(),
        );
    });

    hub.on_manual_select_issuer(select_issuer);
    hub.on_manual_fill_customer(fill_manual_customer);
    hub.on_manual_add_from_product(add_manual_item_from_product);
    hub.on_manual_save_issuer(save_current_issuer);
    hub.on_manual_add_item(|| {
        STATE.with(|c| {
            if let Some(s) = c.borrow_mut().as_mut() {
                s.manual_items.push(ItemDraft::default());
            }
        });
        sync_manual_items();
    });
    hub.on_manual_remove_item(|idx| {
        STATE.with(|c| {
            if let Some(s) = c.borrow_mut().as_mut()
                && let Ok(i) = usize::try_from(idx)
                && i < s.manual_items.len()
            {
                s.manual_items.remove(i);
            }
        });
        sync_manual_items();
    });
    hub.on_manual_update_item(|idx, name, unit, qty, periods, price, vat| {
        STATE.with(|c| {
            if let Some(s) = c.borrow_mut().as_mut()
                && let Ok(i) = usize::try_from(idx)
                && let Some(row) = s.manual_items.get_mut(i)
            {
                row.name = name.into();
                row.unit = unit.into();
                row.quantity = qty.into();
                row.periods = periods.into();
                row.price = price.into();
                row.vat = vat.into();
            }
        });
        update_manual_totals();
    });
    hub.on_manual_download_blank(download_blank_layout);
    hub.on_manual_generate(generate_manual);

    hub.on_request_product_changed(on_request_product_changed);
    hub.on_request_toggle_addon(|idx| {
        STATE.with(|c| {
            let mut g = c.borrow_mut();
            let Some(s) = g.as_mut() else {
                return;
            };
            if let Ok(i) = usize::try_from(idx)
                && let Some((id, _)) = s.request_addons.get(i)
                && !s.request_checked_addons.remove(id)
            {
                s.request_checked_addons.insert(*id);
            }
        });
        sync_request_addons();
    });
    hub.on_request_install_samples(install_request_samples);
    hub.on_request_generate(
        |cust_idx, prod_idx, tmpl_idx, req_date, deadline, months, addressee, subject, note| {
            generate_request(
                cust_idx,
                prod_idx,
                tmpl_idx,
                req_date.as_str(),
                deadline.as_str(),
                months.as_str(),
                addressee.as_str(),
                subject.as_str(),
                note.as_str(),
            );
        },
    );
}

pub fn reload() {
    let Some(worker) = STATE.with(|c| c.borrow().as_ref().map(|s| s.worker.clone())) else {
        return;
    };
    worker.run(
        |ctx| {
            let store = ctx.store();
            let mut customers = store
                .customers
                .values()
                .filter(|c| c.is_active && !c.is_deleted)
                .map(|c| {
                    let label = c
                        .short_name
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|s| format!("{s} — {}", c.legal_name))
                        .unwrap_or_else(|| c.legal_name.clone());
                    let addr = [
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
                    CustomerSnapshot {
                        id: c.id,
                        label,
                        legal_name: c.legal_name.clone(),
                        address: addr,
                        tax_code: c.tax_code.clone().unwrap_or_default(),
                        contact: c
                            .contact_name
                            .clone()
                            .or_else(|| c.representative_name.clone())
                            .unwrap_or_default(),
                    }
                })
                .collect::<Vec<_>>();
            customers.sort_by_key(|a| a.label.to_lowercase());

            let products = list_quotation_products(store)
                .into_iter()
                .map(|p| ProductSnapshot {
                    id: p.id,
                    group_code: p.group_code.clone(),
                    label: format!("{} ({})", p.name, p.code),
                    name: p.name,
                    unit_label: p.unit_label.unwrap_or_else(|| "Đơn vị".into()),
                    price: p
                        .default_price
                        .map(|v| v.normalize().to_string())
                        .unwrap_or_else(|| "0".into()),
                    vat: p
                        .default_vat_rate
                        .map(|v| v.normalize().to_string())
                        .unwrap_or_else(|| "8".into()),
                    duration_months: p.default_duration_months,
                })
                .collect::<Vec<_>>();

            let mut all_addons = store
                .products
                .values()
                .filter(|p| p.is_active && p.kind == "addon")
                .map(|p| {
                    (
                        p.id,
                        p.group_code.clone(),
                        format!("[{}] {}", p.code, p.name),
                    )
                })
                .collect::<Vec<_>>();
            all_addons.sort_by_key(|a| a.2.to_lowercase());

            let first_group = products.first().and_then(|p| p.group_code.as_deref());
            let mut catalog_templates = templates_for(store, first_group, Role::Quotation, true)
                .into_iter()
                .map(|t| (t.id, format!("{} ({})", t.name, t.code)))
                .collect::<Vec<_>>();
            if catalog_templates.is_empty() {
                catalog_templates = list_quotation_templates(store)
                    .into_iter()
                    .map(|t| (t.id, format!("{} ({})", t.name, t.code)))
                    .collect();
            }

            let issuers = list_issuers(store)
                .into_iter()
                .map(|i| IssuerSnapshot {
                    label: if i.tax_code.trim().is_empty() {
                        i.name.clone()
                    } else {
                        format!("{} ({})", i.name, i.tax_code)
                    },
                    name: i.name,
                    address: i.address,
                    tax_code: i.tax_code,
                    representative: i.representative,
                    title: i.title,
                    phone: i.phone,
                    email: i.email,
                })
                .collect::<Vec<_>>();

            let request_templates = list_request_templates(store, first_group)
                .into_iter()
                .map(|t| (t.id, format!("{} ({})", t.name, t.code)))
                .collect::<Vec<_>>();

            LoadedData {
                customers,
                products,
                all_addons,
                catalog_templates,
                issuers,
                request_templates,
                today: ctx.clock().today().format("%d/%m/%Y").to_string(),
            }
        },
        |loaded| {
            let c_names = loaded
                .customers
                .iter()
                .map(|c| SharedString::from(c.label.as_str()))
                .collect::<Vec<_>>();
            let p_names = loaded
                .products
                .iter()
                .map(|p| SharedString::from(p.label.as_str()))
                .collect::<Vec<_>>();
            let t_names = loaded
                .catalog_templates
                .iter()
                .map(|(_, l)| SharedString::from(l.as_str()))
                .collect::<Vec<_>>();
            let i_names = loaded
                .issuers
                .iter()
                .map(|i| SharedString::from(i.label.as_str()))
                .collect::<Vec<_>>();
            let r_names = loaded
                .request_templates
                .iter()
                .map(|(_, l)| SharedString::from(l.as_str()))
                .collect::<Vec<_>>();

            let first_months = loaded
                .products
                .first()
                .and_then(|p| p.duration_months)
                .filter(|m| *m > 0)
                .map(|m| m.to_string());

            STATE.with(|cell| {
                if let Some(s) = cell.borrow_mut().as_mut() {
                    s.today = loaded.today.clone();
                    s.customers = loaded.customers;
                    s.products = loaded.products;
                    s.all_addons = loaded.all_addons;
                    s.catalog_templates = loaded.catalog_templates;
                    s.issuers = loaded.issuers;
                    s.request_templates = loaded.request_templates;
                    rebuild_addons_for_group(s, true, 0);
                    rebuild_addons_for_group(s, false, 0);
                }
            });

            with_hub(|hub| {
                hub.set_customer_names(ModelRc::new(VecModel::from(c_names)));
                hub.set_product_names(ModelRc::new(VecModel::from(p_names)));
                hub.set_catalog_templates(ModelRc::new(VecModel::from(t_names)));
                hub.set_issuer_names(ModelRc::new(VecModel::from(i_names)));
                hub.set_request_templates(ModelRc::new(VecModel::from(r_names)));
                if hub.get_catalog_quote_date().is_empty() {
                    hub.set_catalog_quote_date(loaded.today.clone().into());
                }
                if let Some(m) = first_months
                    && hub.get_catalog_months().as_str() == "12"
                {
                    hub.set_catalog_months(m.into());
                }
                if hub.get_manual_quote_date().is_empty() {
                    hub.set_manual_quote_date(loaded.today.clone().into());
                }
                if hub.get_request_date().is_empty() {
                    hub.set_request_date(loaded.today.into());
                }
            });
            sync_catalog_addons();
            sync_request_addons();
            refresh_catalog_summary();
        },
    );
}

fn rebuild_addons_for_group(s: &mut State, for_catalog: bool, prod_idx: usize) {
    let group = s
        .products
        .get(prod_idx)
        .and_then(|p| p.group_code.as_deref())
        .map(|g| g.trim().to_ascii_lowercase());
    let filtered = s
        .all_addons
        .iter()
        .filter(|(_, ag, _)| match (&group, ag) {
            (Some(g), Some(a)) => g == &a.trim().to_ascii_lowercase(),
            _ => false,
        })
        .map(|(id, _, label)| (*id, label.clone()))
        .collect::<Vec<_>>();
    if for_catalog {
        s.catalog_addons = filtered;
        s.catalog_checked_addons.clear();
    } else {
        s.request_addons = filtered;
        s.request_checked_addons.clear();
    }
}

fn sync_catalog_addons() {
    let rows = STATE.with(|c| {
        let g = c.borrow();
        let Some(s) = g.as_ref() else {
            return Vec::new();
        };
        s.catalog_addons
            .iter()
            .map(|(id, label)| IdLabel {
                id: *id as i32,
                label: label.as_str().into(),
                checked: s.catalog_checked_addons.contains(id),
            })
            .collect::<Vec<_>>()
    });
    with_hub(|hub| hub.set_catalog_addons(ModelRc::new(VecModel::from(rows))));
}

fn sync_request_addons() {
    let rows = STATE.with(|c| {
        let g = c.borrow();
        let Some(s) = g.as_ref() else {
            return Vec::new();
        };
        s.request_addons
            .iter()
            .map(|(id, label)| IdLabel {
                id: *id as i32,
                label: label.as_str().into(),
                checked: s.request_checked_addons.contains(id),
            })
            .collect::<Vec<_>>()
    });
    with_hub(|hub| hub.set_request_addons(ModelRc::new(VecModel::from(rows))));
}

fn on_catalog_product_changed(prod_idx: i32) {
    let info = STATE.with(|cell| {
        let mut g = cell.borrow_mut();
        let s = g.as_mut()?;
        let i = usize::try_from(prod_idx).ok().unwrap_or(0);
        rebuild_addons_for_group(s, true, i);
        let prod = s.products.get(i);
        let group = prod.and_then(|p| p.group_code.clone());
        let dur = prod.and_then(|p| p.duration_months).filter(|m| *m > 0);
        Some((s.worker.clone(), group, dur))
    });
    sync_catalog_addons();
    let Some((worker, group, dur)) = info else {
        return;
    };
    if let Some(m) = dur {
        with_hub(|hub| hub.set_catalog_months(m.to_string().into()));
    }
    worker.run(
        move |ctx| {
            let mut list = templates_for(ctx.store(), group.as_deref(), Role::Quotation, true)
                .into_iter()
                .map(|t| (t.id, format!("{} ({})", t.name, t.code)))
                .collect::<Vec<_>>();
            if list.is_empty() {
                list = list_quotation_templates(ctx.store())
                    .into_iter()
                    .map(|t| (t.id, format!("{} ({})", t.name, t.code)))
                    .collect();
            }
            list
        },
        |templates| {
            let names = templates
                .iter()
                .map(|(_, l)| SharedString::from(l.as_str()))
                .collect::<Vec<_>>();
            STATE.with(|cell| {
                if let Some(s) = cell.borrow_mut().as_mut() {
                    s.catalog_templates = templates;
                }
            });
            with_hub(|hub| {
                hub.set_catalog_templates(ModelRc::new(VecModel::from(names)));
                hub.set_catalog_template_index(0);
            });
            refresh_catalog_summary();
        },
    );
}

fn on_request_product_changed(prod_idx: i32) {
    let info = STATE.with(|cell| {
        let mut g = cell.borrow_mut();
        let s = g.as_mut()?;
        let i = usize::try_from(prod_idx).ok().unwrap_or(0);
        rebuild_addons_for_group(s, false, i);
        let group = s.products.get(i).and_then(|p| p.group_code.clone());
        Some((s.worker.clone(), group))
    });
    sync_request_addons();
    let Some((worker, group)) = info else {
        return;
    };
    worker.run(
        move |ctx| {
            list_request_templates(ctx.store(), group.as_deref())
                .into_iter()
                .map(|t| (t.id, format!("{} ({})", t.name, t.code)))
                .collect::<Vec<_>>()
        },
        |templates| {
            let names = templates
                .iter()
                .map(|(_, l)| SharedString::from(l.as_str()))
                .collect::<Vec<_>>();
            STATE.with(|cell| {
                if let Some(s) = cell.borrow_mut().as_mut() {
                    s.request_templates = templates;
                }
            });
            with_hub(|hub| {
                hub.set_request_templates(ModelRc::new(VecModel::from(names)));
                hub.set_request_template_index(0);
            });
        },
    );
}

fn catalog_items_list(
    store: &cf_db::store::Store,
    cid: i64,
    pid: i64,
    addons: &[i64],
) -> Vec<ItemInput> {
    let units = store
        .units
        .values()
        .filter(|u| u.customer_id == cid && u.is_active && u.include_in_billing)
        .count();
    let main = usize::from(
        store
            .customers
            .get(&cid)
            .is_some_and(|c| c.bill_main_station),
    );
    let qty = (units + main).max(1).to_string();
    let mut list = vec![ItemInput {
        product_id: pid,
        description: String::new(),
        quantity: qty.clone(),
        unit_price_pre_vat: None,
        vat_rate: None,
        amount_pre_vat_override: None,
        note: String::new(),
    }];
    for aid in addons {
        list.push(ItemInput {
            product_id: *aid,
            description: String::new(),
            quantity: qty.clone(),
            unit_price_pre_vat: None,
            vat_rate: None,
            amount_pre_vat_override: None,
            note: String::new(),
        });
    }
    list
}

fn refresh_catalog_summary() {
    let snapshot = with_hub_ret(|hub| {
        (
            hub.get_customer_index(),
            hub.get_product_index(),
            hub.get_catalog_template_index(),
            hub.get_catalog_quote_date().to_string(),
            hub.get_catalog_months().to_string(),
        )
    });
    let Some((c_idx, p_idx, t_idx, date_raw, months_raw)) = snapshot else {
        return;
    };
    let ids = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        let cid = usize::try_from(c_idx)
            .ok()
            .and_then(|i| s.customers.get(i))
            .map(|c| c.id)?;
        let pid = usize::try_from(p_idx)
            .ok()
            .and_then(|i| s.products.get(i))
            .map(|p| p.id)?;
        let tid = usize::try_from(t_idx)
            .ok()
            .and_then(|i| s.catalog_templates.get(i))
            .map(|(id, _)| *id)?;
        let addons: Vec<i64> = s.catalog_checked_addons.iter().copied().collect();
        Some((s.worker.clone(), cid, pid, tid, addons))
    });
    let Some((worker, cid, pid, tid, addons)) = ids else {
        return;
    };
    let Ok(Some(quote_date)) = parse_date_input(Some(date_raw.as_str())) else {
        return;
    };
    let Ok(months) = months_raw.trim().parse::<i64>() else {
        return;
    };
    worker.run(
        move |ctx| {
            let store = ctx.store();
            if addons.is_empty() {
                resolve_quotation_selection(store, cid, pid, tid, quote_date, months).map(|sel| {
                    sel.context
                        .get("PRICING_DESCRIPTION")
                        .cloned()
                        .unwrap_or_default()
                })
            } else {
                let items = catalog_items_list(store, cid, pid, &addons);
                price_preview(store, &items, months, true).map(|p| {
                    format!(
                        "{} (Tổng sau VAT: {} VNĐ)",
                        p.description,
                        fmt_currency(Some(p.totals.total))
                    )
                })
            }
        },
        |res| {
            let msg = match res {
                Ok(text) => text,
                Err(err) => err.user_message().to_owned(),
            };
            with_hub(|hub| hub.set_catalog_summary(msg.into()));
        },
    );
}

fn catalog_generate(cust_idx: i32, prod_idx: i32, tmpl_idx: i32, date_raw: &str, months_raw: &str) {
    let ids = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        let cid = usize::try_from(cust_idx)
            .ok()
            .and_then(|i| s.customers.get(i))
            .map(|c| c.id);
        let pid = usize::try_from(prod_idx)
            .ok()
            .and_then(|i| s.products.get(i))
            .map(|p| p.id);
        let tid = usize::try_from(tmpl_idx)
            .ok()
            .and_then(|i| s.catalog_templates.get(i))
            .map(|(id, _)| *id);
        let addons: Vec<i64> = s.catalog_checked_addons.iter().copied().collect();
        Some((
            s.worker.clone(),
            Rc::clone(&s.feedback),
            cid,
            pid,
            tid,
            addons,
        ))
    });
    let Some((worker, feedback, cid, pid, tid, addons)) = ids else {
        return;
    };
    let (Some(cid), Some(pid), Some(tid)) = (cid, pid, tid) else {
        feedback.toast(
            ToastKind::Warning,
            "Cần chọn đủ khách hàng, gói chính và mẫu báo giá.",
        );
        return;
    };
    let quote_date = match parse_date_input(Some(date_raw)) {
        Ok(Some(d)) => d,
        _ => {
            feedback.toast(
                ToastKind::Warning,
                "Ngày báo giá không hợp lệ (dd/mm/yyyy).",
            );
            return;
        }
    };
    let months = match months_raw.trim().parse::<i64>() {
        Ok(m) if (1..=120).contains(&m) => m,
        _ => {
            feedback.toast(ToastKind::Warning, "Số tháng phải từ 1 đến 120.");
            return;
        }
    };

    worker.run(
        move |ctx| {
            let items = if addons.is_empty() {
                Vec::new()
            } else {
                catalog_items_list(ctx.store(), cid, pid, &addons)
            };
            generate_catalog_quotation(ctx, cid, pid, tid, quote_date, months, &items)
        },
        |res| match res {
            Ok(doc) => save_docx(&doc.file_name, &doc.bytes),
            Err(error) => toast_from_worker(ToastKind::Danger, error.user_message().to_owned()),
        },
    );
}

fn select_issuer(idx: i32) {
    let issuer = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        usize::try_from(idx)
            .ok()
            .and_then(|i| s.issuers.get(i))
            .cloned()
    });
    let Some(issuer) = issuer else {
        return;
    };
    with_hub(|hub| {
        hub.set_issuer_name(issuer.name.into());
        hub.set_issuer_address(issuer.address.into());
        hub.set_issuer_tax_code(issuer.tax_code.into());
        hub.set_issuer_rep(issuer.representative.into());
        hub.set_issuer_title(issuer.title.into());
        hub.set_issuer_phone(issuer.phone.into());
        hub.set_issuer_email(issuer.email.into());
    });
}

fn fill_manual_customer(idx: i32) {
    let cust = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        usize::try_from(idx)
            .ok()
            .and_then(|i| s.customers.get(i))
            .cloned()
    });
    let Some(cust) = cust else {
        return;
    };
    with_hub(|hub| {
        hub.set_recipient_name(cust.legal_name.into());
        hub.set_recipient_address(cust.address.into());
        hub.set_recipient_tax_code(cust.tax_code.into());
        hub.set_recipient_contact(cust.contact.into());
    });
}

fn add_manual_item_from_product(idx: i32) {
    STATE.with(|c| {
        let mut g = c.borrow_mut();
        let Some(s) = g.as_mut() else {
            return;
        };
        if let Some(p) = usize::try_from(idx)
            .ok()
            .and_then(|i| s.products.get(i))
            .cloned()
        {
            let periods = p
                .duration_months
                .filter(|m| *m > 0)
                .unwrap_or(12)
                .to_string();
            let draft = ItemDraft {
                name: p.name,
                unit: p.unit_label,
                quantity: "1".into(),
                periods,
                price: p.price,
                vat: p.vat,
            };
            if s.manual_items.len() == 1 && s.manual_items[0].name.trim().is_empty() {
                s.manual_items[0] = draft;
            } else {
                s.manual_items.push(draft);
            }
        }
    });
    sync_manual_items();
}

fn save_current_issuer() {
    let input = with_hub_ret(|hub| IssuerInput {
        name: hub.get_issuer_name().into(),
        address: hub.get_issuer_address().into(),
        tax_code: hub.get_issuer_tax_code().into(),
        representative: hub.get_issuer_rep().into(),
        title: hub.get_issuer_title().into(),
        phone: hub.get_issuer_phone().into(),
        email: hub.get_issuer_email().into(),
    });
    let Some(input) = input else {
        return;
    };
    let Some(worker) = STATE.with(|c| c.borrow().as_ref().map(|s| s.worker.clone())) else {
        return;
    };
    worker.run(
        move |ctx| save_issuer(ctx, input),
        |res| match res {
            Ok((_, created)) => {
                let msg = if created {
                    "Đã lưu hồ sơ phát hành mới."
                } else {
                    "Hồ sơ này đã tồn tại trong danh sách."
                };
                toast_from_worker(ToastKind::Success, msg.to_owned());
                reload();
            }
            Err(error) => toast_from_worker(ToastKind::Danger, error.user_message().to_owned()),
        },
    );
}

fn sync_manual_items() {
    let rows = STATE.with(|c| {
        let g = c.borrow();
        let Some(s) = g.as_ref() else {
            return Vec::new();
        };
        s.manual_items
            .iter()
            .map(|r| ManualItemRow {
                name: r.name.as_str().into(),
                unit: r.unit.as_str().into(),
                quantity: r.quantity.as_str().into(),
                periods: r.periods.as_str().into(),
                price: r.price.as_str().into(),
                vat: r.vat.as_str().into(),
            })
            .collect::<Vec<_>>()
    });
    with_hub(|hub| hub.set_manual_items(ModelRc::new(VecModel::from(rows))));
    update_manual_totals();
}

fn update_manual_totals() {
    let items = STATE.with(|c| {
        c.borrow()
            .as_ref()
            .map(|s| s.manual_items.clone())
            .unwrap_or_default()
    });
    let filled: Vec<_> = items
        .iter()
        .filter(|r| !r.name.trim().is_empty())
        .map(|item| {
            json!({
                "name": item.name,
                "unit": item.unit,
                "quantity": item.quantity,
                "periods": item.periods,
                "price": item.price,
                "vat": item.vat,
            })
        })
        .collect();
    let msg = if filled.is_empty() {
        String::new()
    } else {
        let raw = serde_json::Value::Array(filled).to_string();
        match build_lines(&raw) {
            Ok((_, totals)) => format!(
                "Trước thuế: {} VNĐ · Thuế VAT: {} VNĐ · Tổng sau thuế: {} VNĐ",
                fmt_currency(Some(totals.pre_vat)),
                fmt_currency(Some(totals.vat)),
                fmt_currency(Some(totals.total)),
            ),
            Err(_) => String::new(),
        }
    };
    with_hub(|hub| hub.set_manual_totals_text(msg.into()));
}

fn download_blank_layout(layout_idx: i32) {
    let Some(feedback) = STATE.with(|c| c.borrow().as_ref().map(|s| Rc::clone(&s.feedback))) else {
        return;
    };
    let idx = usize::try_from(layout_idx)
        .ok()
        .filter(|i| *i < BLANK_LAYOUT_BYTES.len())
        .unwrap_or(0);
    let (key, filename) = BLANK_LAYOUT_BYTES[idx];
    let Some(bytes) = cf_services::assets::default_template_bytes(filename) else {
        feedback.toast(ToastKind::Danger, "Không tìm thấy mẫu báo giá mặc định.");
        return;
    };
    let Some(path) = files::save_as(&format!("Mau_BaoGia_{key}.docx"), ("Word", &["docx"])) else {
        return;
    };
    match files::write_chosen(&path, &bytes) {
        Ok(()) => {
            let _ = files::open_folder(&path);
            feedback.toast(ToastKind::Success, &format!("Đã lưu {}", path.display()));
        }
        Err(error) => feedback.toast(ToastKind::Danger, &format!("Không lưu được file: {error}")),
    }
}

fn generate_manual(layout_idx: i32) {
    let Some(feedback) = STATE.with(|c| c.borrow().as_ref().map(|s| Rc::clone(&s.feedback))) else {
        return;
    };
    let idx = usize::try_from(layout_idx)
        .ok()
        .filter(|i| *i < BLANK_LAYOUT_BYTES.len())
        .unwrap_or(0);
    let (layout_key, filename) = BLANK_LAYOUT_BYTES[idx];
    let Some(template_bytes) = cf_services::assets::default_template_bytes(filename) else {
        feedback.toast(ToastKind::Danger, "Không tìm thấy mẫu báo giá mặc định.");
        return;
    };
    let items = STATE.with(|c| {
        c.borrow()
            .as_ref()
            .map(|s| s.manual_items.clone())
            .unwrap_or_default()
    });
    let items_json = serde_json::Value::Array(
        items
            .iter()
            .map(|item| {
                json!({
                    "name": item.name,
                    "unit": item.unit,
                    "quantity": item.quantity,
                    "periods": item.periods,
                    "price": item.price,
                    "vat": item.vat,
                })
            })
            .collect(),
    )
    .to_string();

    let Some(mut values) = with_hub_ret(|hub| {
        BTreeMap::from([
            ("layout".into(), layout_key.to_owned()),
            ("issuer_name".into(), hub.get_issuer_name().into()),
            ("issuer_address".into(), hub.get_issuer_address().into()),
            ("issuer_tax_code".into(), hub.get_issuer_tax_code().into()),
            ("issuer_representative".into(), hub.get_issuer_rep().into()),
            ("issuer_title".into(), hub.get_issuer_title().into()),
            ("issuer_phone".into(), hub.get_issuer_phone().into()),
            ("issuer_email".into(), hub.get_issuer_email().into()),
            ("recipient_name".into(), hub.get_recipient_name().into()),
            (
                "recipient_address".into(),
                hub.get_recipient_address().into(),
            ),
            (
                "recipient_tax_code".into(),
                hub.get_recipient_tax_code().into(),
            ),
            (
                "recipient_contact".into(),
                hub.get_recipient_contact().into(),
            ),
            ("quote_number".into(), hub.get_manual_quote_number().into()),
            ("quote_date".into(), hub.get_manual_quote_date().into()),
            ("valid_until".into(), hub.get_manual_valid_until().into()),
            ("note".into(), hub.get_manual_note().into()),
        ])
    }) else {
        return;
    };
    values.insert("items_json".into(), items_json);

    let doc = match build_document(&values, &[]) {
        Ok(d) => d,
        Err(error) => {
            feedback.toast(ToastKind::Danger, &error.user_message());
            return;
        }
    };
    let bytes = match render_document(&template_bytes, &doc) {
        Ok(b) => b,
        Err(error) => {
            feedback.toast(ToastKind::Danger, &error.user_message());
            return;
        }
    };
    save_docx(&doc.file_name, &bytes);
}

fn install_request_samples() {
    let Some(worker) = STATE.with(|c| c.borrow().as_ref().map(|s| s.worker.clone())) else {
        return;
    };
    worker.run(install_sample_templates, |res| match res {
        Ok(ids) => {
            let msg = if ids.is_empty() {
                "Các mẫu yêu cầu báo giá sẵn có đã được cài đủ.".to_owned()
            } else {
                format!("Đã cài {} mẫu yêu cầu báo giá.", ids.len())
            };
            toast_from_worker(ToastKind::Success, msg);
            reload();
        }
        Err(error) => toast_from_worker(ToastKind::Danger, error.user_message().to_owned()),
    });
}

#[allow(clippy::too_many_arguments)]
fn generate_request(
    cust_idx: i32,
    prod_idx: i32,
    tmpl_idx: i32,
    req_date_raw: &str,
    deadline_raw: &str,
    months_raw: &str,
    addressee: &str,
    subject: &str,
    note: &str,
) {
    let (main_name, main_qty) = with_hub_ret(|hub| {
        (
            hub.get_request_main_name().to_string(),
            hub.get_request_main_qty().to_string(),
        )
    })
    .unwrap_or_default();
    let ids = STATE.with(|cell| {
        let g = cell.borrow();
        let s = g.as_ref()?;
        let cid = usize::try_from(cust_idx)
            .ok()
            .and_then(|i| s.customers.get(i))
            .map(|c| c.id);
        let pid = usize::try_from(prod_idx)
            .ok()
            .and_then(|i| s.products.get(i))
            .map(|p| p.id);
        let tid = usize::try_from(tmpl_idx)
            .ok()
            .and_then(|i| s.request_templates.get(i))
            .map(|(id, _)| *id);
        let addons: Vec<i64> = s.request_checked_addons.iter().copied().collect();
        Some((
            s.worker.clone(),
            Rc::clone(&s.feedback),
            cid,
            pid,
            tid,
            addons,
        ))
    });
    let Some((worker, feedback, cid, pid, tid, addons)) = ids else {
        return;
    };
    let (Some(cid), Some(pid), Some(tid)) = (cid, pid, tid) else {
        feedback.toast(
            ToastKind::Warning,
            "Chọn đủ khách hàng, gói chính và mẫu yêu cầu (bấm 'Cài mẫu yêu cầu có sẵn' nếu trống).",
        );
        return;
    };
    let request_date = match parse_date_input(Some(req_date_raw)) {
        Ok(Some(d)) => d,
        _ => {
            feedback.toast(
                ToastKind::Warning,
                "Ngày yêu cầu không hợp lệ (dd/mm/yyyy).",
            );
            return;
        }
    };
    if deadline_raw.trim().is_empty() {
        feedback.toast(
            ToastKind::Warning,
            "Vui lòng nhập hạn phản hồi (dd/mm/yyyy).",
        );
        return;
    }
    let deadline = match parse_date_input(Some(deadline_raw)) {
        Ok(Some(d)) => Some(d),
        _ => {
            feedback.toast(
                ToastKind::Warning,
                "Hạn phản hồi không hợp lệ (dd/mm/yyyy).",
            );
            return;
        }
    };
    let months = match months_raw.trim().parse::<i64>() {
        Ok(m) if (1..=120).contains(&m) => m,
        _ => {
            feedback.toast(ToastKind::Warning, "Số tháng phải từ 1 đến 120.");
            return;
        }
    };
    let addressee = addressee.to_owned();
    let subject = subject.to_owned();
    let note = note.to_owned();

    worker.run(
        move |ctx| -> Result<(String, Vec<u8>), ServiceError> {
            let store = ctx.store();
            let customer = store
                .customers
                .get(&cid)
                .ok_or_else(|| ServiceError::NotFound("Không tìm thấy khách hàng.".into()))?
                .clone();
            let units = store
                .units
                .values()
                .filter(|u| u.customer_id == cid)
                .cloned()
                .collect::<Vec<_>>();
            let addon_rows: Vec<(i64, String, String)> = addons
                .into_iter()
                .map(|id| (id, String::new(), String::new()))
                .collect();
            let (items, features) =
                resolve_request_items(store, cid, pid, months, &main_name, &main_qty, &addon_rows)?;
            let group = store
                .products
                .get(&pid)
                .and_then(|p| p.group_code.as_deref());
            let template = resolve_request_template(store, tid, group)?;
            let context = build_request_context(
                &customer,
                &units,
                RequestContextInput {
                    request_date,
                    deadline,
                    month_count: months,
                    addressee: &addressee,
                    subject: &subject,
                    note: &note,
                },
            );
            let lists = build_request_lists(&items, &features);
            let bytes = render_request_docx(ctx.paths(), &template, &context, &lists)?;
            let file_name = request_filename(&customer, request_date);
            Ok((file_name, bytes))
        },
        |res| match res {
            Ok((name, bytes)) => save_docx(&name, &bytes),
            Err(error) => toast_from_worker(ToastKind::Danger, error.user_message().to_owned()),
        },
    );
}

fn save_docx(default_name: &str, bytes: &[u8]) {
    let Some(feedback) = STATE.with(|c| c.borrow().as_ref().map(|s| Rc::clone(&s.feedback))) else {
        return;
    };
    let Some(path) = files::save_as(default_name, ("Word", &["docx"])) else {
        return;
    };
    match files::write_chosen(&path, bytes) {
        Ok(()) => {
            let _ = files::open_path(&path);
            feedback.toast(ToastKind::Success, &format!("Đã lưu {}", path.display()));
        }
        Err(error) => feedback.toast(ToastKind::Danger, &format!("Không lưu được file: {error}")),
    }
}

fn with_hub<F: FnOnce(&QuotationHub)>(f: F) {
    STATE.with(|cell| {
        if let Some(ui) = cell.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
            f(&ui.global::<QuotationHub>());
        }
    });
}

fn with_hub_ret<R, F: FnOnce(&QuotationHub) -> R>(f: F) -> Option<R> {
    STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|s| s.ui.upgrade())
            .map(|ui| f(&ui.global::<QuotationHub>()))
    })
}
