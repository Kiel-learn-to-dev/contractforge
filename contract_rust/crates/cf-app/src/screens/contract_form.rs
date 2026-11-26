//! S14 — form sinh hợp đồng: lọc khách không dấu, cơ sở tính phí, gói chính + bổ sung, mẫu + kèm theo, ngày, tiền; lưu nháp và sinh file.
//! Mọi tính toán nghiệp vụ nằm ở cf-services; màn hình chỉ giữ dữ liệu nhập và hiển thị kết quả.

use std::{cell::RefCell, collections::BTreeSet, rc::Rc, str::FromStr};

use cf_core::{
    dates::{compute_end_date, parse_date_input},
    fmt::fmt_currency,
    pricing::ItemInput,
    text::search_key,
    words::amount_to_words_dec,
};
use cf_db::model::Product;
use cf_services::{
    Ctx, ServiceError,
    contracts::{
        create::{ContractForm, CreateOptions, create_draft},
        generate::generate_docx,
        options::{ProductBrief, TemplateOption, product_options},
        preview::price_preview,
        suggest::{Facility, customer_facilities, suggest_number},
    },
    settings::{get_party_b_settings, is_organization_profile_complete},
};
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::{
    ContractHub, ContractPriceLineRow, FormAddonRow, IdLabel, MainWindow, nav,
    ui_helpers::{Feedback, ToastKind, post_to_ui, toast_from_worker},
    view,
    worker::Worker,
};

#[derive(Debug, Clone)]
struct Input {
    customer_id: Option<i64>,
    product_id: Option<i64>,
    template_id: Option<i64>,
    addons: BTreeSet<i64>,
    attachments: BTreeSet<i64>,
    number: String,
    sign_date: String,
    start_date: String,
    end_date: String,
    months: String,
    unit_count: String,
    payment_mode: String,
    payment_cycle_months: String,
    acceptance_date: String,
    liquidation_date: String,
    location: String,
    total: String,
    amount_text: String,
    pricing_description: String,
    party_b_rep: String,
    party_b_auth_num: String,
    party_b_auth_date: String,
    notes: String,
    manual_number: bool,
    manual_location: bool,
    manual_start: bool,
    manual_end: bool,
    manual_months: bool,
    manual_unit_count: bool,
    manual_acceptance: bool,
    manual_liquidation: bool,
    manual_amount_text: bool,
    manual_pricing_desc: bool,
    party_b_seeded: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            customer_id: None,
            product_id: None,
            template_id: None,
            addons: BTreeSet::new(),
            attachments: BTreeSet::new(),
            number: String::new(),
            sign_date: String::new(),
            start_date: String::new(),
            end_date: String::new(),
            months: "12".into(),
            unit_count: "1".into(),
            payment_mode: String::new(),
            payment_cycle_months: String::new(),
            acceptance_date: String::new(),
            liquidation_date: String::new(),
            location: String::new(),
            total: String::new(),
            amount_text: String::new(),
            pricing_description: String::new(),
            party_b_rep: String::new(),
            party_b_auth_num: String::new(),
            party_b_auth_date: String::new(),
            notes: String::new(),
            manual_number: false,
            manual_location: false,
            manual_start: false,
            manual_end: false,
            manual_months: false,
            manual_unit_count: false,
            manual_acceptance: false,
            manual_liquidation: false,
            manual_amount_text: false,
            manual_pricing_desc: false,
            party_b_seeded: false,
        }
    }
}

#[derive(Debug, Clone)]
struct Refresh {
    customer_id: Option<i64>,
    product_id: Option<i64>,
    addons: Vec<i64>,
    months: String,
    sign_year: i32,
    suggest_number: bool,
    unit_count: i64,
    manual_unit_override: Option<i64>,
    reload_facilities: bool,
}

#[derive(Debug)]
struct PreviewComputed {
    summary: String,
    lines: Vec<ContractPriceLineRow>,
    pre_vat: String,
    vat: String,
    total: String,
    amount_words: String,
    description: String,
}

#[derive(Debug)]
struct Computed {
    customers: Vec<(i64, String, String)>, // (id, legal_name, search_key)
    mains: Vec<(i64, String)>,
    addons: Vec<(i64, String)>,
    templates: Vec<TemplateOption>,
    attachments: Vec<TemplateOption>,
    default_template: Option<i64>,
    default_months: Option<i64>,
    customer_legal_name: Option<String>,
    customer_info_text: String,
    suggested: Option<String>,
    facilities: Option<Vec<Facility>>,
    preview: Result<PreviewComputed, String>,
    main_group: Option<String>,
    party_b_incomplete: bool,
    party_b_defaults: (String, String, String),
}

struct Screen {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    input: Input,
    all_customers: Vec<(i64, String, String)>,
    customers: Vec<(i64, String)>,
    customer_filter: String,
    mains: Vec<(i64, String)>,
    addons: Vec<(i64, String)>,
    templates: Vec<TemplateOption>,
    attachments: Vec<TemplateOption>,
    facilities: Vec<Facility>,
    main_group: Option<String>,
    summary: String,
    error: String,
    busy: bool,
}

thread_local! {
    static SCREEN: RefCell<Option<Screen>> = const { RefCell::new(None) };
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>, _feedback: &Rc<Feedback>) {
    SCREEN.with(|cell| {
        *cell.borrow_mut() = Some(Screen {
            ui: ui.as_weak(),
            worker: worker.clone(),
            input: Input::default(),
            all_customers: Vec::new(),
            customers: Vec::new(),
            customer_filter: String::new(),
            mains: Vec::new(),
            addons: Vec::new(),
            templates: Vec::new(),
            attachments: Vec::new(),
            facilities: Vec::new(),
            main_group: None,
            summary: String::new(),
            error: String::new(),
            busy: false,
        });
    });

    ui.on_contract_customer_changed(|index| {
        let changed = with_screen(|screen| {
            screen.input.customer_id = pick(&screen.customers, index);
            screen.input.addons.clear();
            if !screen.input.manual_location
                && let Some(cid) = screen.input.customer_id
                && let Some((_, name, _)) =
                    screen.all_customers.iter().find(|(id, _, _)| *id == cid)
            {
                screen.input.location = lowercase_first(name);
            }
        });
        if changed.is_some() {
            refresh_ex(true, true);
        }
    });

    ui.on_contract_product_changed(|index| {
        let changed = with_screen(|screen| {
            screen.input.product_id = pick(&screen.mains, index);
            screen.input.addons.clear();
            screen.input.template_id = None;
            screen.input.attachments.clear();
        });
        if changed.is_some() {
            refresh_ex(true, false);
        }
    });

    ui.on_contract_template_changed(|index| {
        with_screen(|screen| {
            screen.input.template_id = pick(&screen.templates_as_pairs(), index);
        });
    });

    ui.on_contract_addon_toggled(|index| {
        let toggled = with_screen(|screen| {
            let id = pick(&screen.addons, index)?;
            if !screen.input.addons.remove(&id) {
                screen.input.addons.insert(id);
            }
            Some(())
        })
        .flatten();
        if toggled.is_some() {
            refresh(false);
        }
    });

    ui.on_contract_field_changed(|key, value| {
        let changed = with_screen(|screen| {
            let value = value.to_string();
            let mut suggest_num = false;
            match key.as_str() {
                "number" => {
                    screen.input.number = value;
                    screen.input.manual_number = true;
                }
                "sign_date" => {
                    screen.input.sign_date = value.clone();
                    if !screen.input.manual_start {
                        screen.input.start_date = value;
                    }
                    auto_fill_dates(&mut screen.input);
                    suggest_num = !screen.input.manual_number;
                    push_dates(screen);
                }
                "start_date" => {
                    screen.input.manual_start = !value.trim().is_empty();
                    screen.input.start_date = value;
                    auto_fill_dates(&mut screen.input);
                    push_dates(screen);
                }
                "end_date" => {
                    screen.input.manual_end = !value.trim().is_empty();
                    screen.input.end_date = value;
                    if !screen.input.manual_liquidation && !screen.input.end_date.trim().is_empty()
                    {
                        screen.input.liquidation_date = screen.input.end_date.clone();
                    }
                    push_dates(screen);
                }
                "months" => {
                    screen.input.manual_months = !value.trim().is_empty();
                    screen.input.months = value;
                    auto_fill_dates(&mut screen.input);
                    push_dates(screen);
                }
                "unit_count" => {
                    screen.input.manual_unit_count = !value.trim().is_empty();
                    screen.input.unit_count = value;
                    suggest_num = false;
                    // trigger price refresh below
                }
                "payment_cycle_months" => screen.input.payment_cycle_months = value,
                "acceptance_date" => {
                    screen.input.manual_acceptance = !value.trim().is_empty();
                    screen.input.acceptance_date = value;
                    push_dates(screen);
                }
                "liquidation_date" => {
                    screen.input.manual_liquidation = !value.trim().is_empty();
                    screen.input.liquidation_date = value;
                    push_dates(screen);
                }
                "location" => {
                    screen.input.manual_location = !value.trim().is_empty();
                    screen.input.location = value;
                }
                "total" => screen.input.total = value,
                "amount_text" => {
                    screen.input.manual_amount_text = !value.trim().is_empty();
                    screen.input.amount_text = value;
                }
                "pricing_description" => {
                    screen.input.manual_pricing_desc = !value.trim().is_empty();
                    screen.input.pricing_description = value;
                }
                "party_b_rep" => screen.input.party_b_rep = value,
                "party_b_auth_num" => screen.input.party_b_auth_num = value,
                "party_b_auth_date" => screen.input.party_b_auth_date = value,
                "notes" => screen.input.notes = value,
                _ => {}
            }
            (suggest_num, matches!(key.as_str(), "unit_count" | "months"))
        });
        if let Some((suggest_num, needs_price)) = changed
            && (suggest_num || needs_price)
        {
            refresh(suggest_num);
        }
    });

    ui.on_contract_save(save);

    let hub = ui.global::<ContractHub>();
    hub.on_open_settings(|| {
        if let Some(ui) = with_screen(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
            ui.invoke_navigate(nav::Screen::Settings.index());
        }
    });
    hub.on_payment_mode_changed(|idx| {
        with_screen(|screen| {
            screen.input.payment_mode = match idx {
                1 => "prepaid".into(),
                2 => "postpaid".into(),
                _ => String::new(),
            };
        });
    });
    hub.on_cancel_form(|| {
        if let Some(ui) = with_screen(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
            ui.invoke_navigate(nav::Screen::Contracts.index());
        }
    });
    hub.on_filter_customers(|query| {
        let auto_selected = with_screen(|screen| {
            screen.customer_filter = query.to_string();
            apply_customer_filter(screen);
            let single = (screen.customers.len() == 1).then(|| screen.customers[0].0);
            if let Some(cid) = single
                && screen.input.customer_id != Some(cid)
            {
                screen.input.customer_id = Some(cid);
                return true;
            }
            false
        });
        push_customer_list_and_facilities();
        if auto_selected == Some(true) {
            refresh_ex(true, true);
        }
    });
    hub.on_toggle_facility(|idx| {
        with_screen(|s| {
            if let Ok(i) = usize::try_from(idx)
                && let Some(fac) = s.facilities.get_mut(i)
            {
                fac.checked = !fac.checked;
                s.input.manual_unit_count = false;
                s.input.unit_count = checked_unit_count(&s.facilities).to_string();
            }
        });
        push_customer_list_and_facilities();
        refresh(false);
    });
    hub.on_select_all_facilities(|checked| {
        with_screen(|s| {
            for fac in &mut s.facilities {
                fac.checked = checked;
            }
            s.input.manual_unit_count = false;
            s.input.unit_count = checked_unit_count(&s.facilities).to_string();
        });
        push_customer_list_and_facilities();
        refresh(false);
    });
    hub.on_toggle_form_attachment(|idx| {
        with_screen(|s| {
            if let Ok(i) = usize::try_from(idx)
                && let Some(att) = s.attachments.get(i)
                && !s.input.attachments.remove(&att.id)
            {
                s.input.attachments.insert(att.id);
            }
        });
        push_form_attachments();
    });

    refresh_ex(true, true);
}

pub fn select_customer(customer_id: i64) {
    let changed = with_screen(|screen| {
        screen.input.customer_id = Some(customer_id);
        screen.input.addons.clear();
        screen.input.manual_number = false;
        screen.input.manual_location = false;
        screen.input.manual_unit_count = false;
        if let Some((_, name, _)) = screen
            .all_customers
            .iter()
            .find(|(id, _, _)| *id == customer_id)
        {
            screen.input.location = lowercase_first(name);
        }
    });
    if changed.is_some() {
        refresh_ex(true, true);
    }
}

impl Screen {
    fn templates_as_pairs(&self) -> Vec<(i64, String)> {
        self.templates
            .iter()
            .map(|option| (option.id, option.name.clone()))
            .collect()
    }
}

fn with_screen<R>(action: impl FnOnce(&mut Screen) -> R) -> Option<R> {
    SCREEN.with(|cell| cell.borrow_mut().as_mut().map(action))
}

fn pick(items: &[(i64, String)], index: i32) -> Option<i64> {
    usize::try_from(index)
        .ok()
        .and_then(|index| items.get(index))
        .map(|(id, _)| *id)
}

fn lowercase_first(s: &str) -> String {
    let mut chars = s.trim().chars();
    match chars.next() {
        Some(first) => first.to_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn auto_fill_dates(input: &mut Input) {
    let start = parse_date(&input.start_date).ok().flatten();
    let months = input.months.trim().parse::<i32>().ok().filter(|m| *m > 0);
    if let Some(s) = start {
        if !input.manual_acceptance {
            input.acceptance_date = view::date_text(Some(s + chrono::Duration::days(2)));
        }
        if let Some(m) = months {
            let end_str = view::date_text(Some(compute_end_date(s, m)));
            if !input.manual_end {
                input.end_date = end_str.clone();
            }
            if !input.manual_liquidation {
                input.liquidation_date = if input.manual_end && !input.end_date.trim().is_empty() {
                    input.end_date.clone()
                } else {
                    end_str
                };
            }
        }
    }
}

fn date_warnings_text(input: &Input) -> String {
    let sign = parse_date(&input.sign_date).ok().flatten();
    let start = parse_date(&input.start_date).ok().flatten();
    let months = input.months.trim().parse::<i32>().ok().filter(|m| *m > 0);
    let end = if input.manual_end {
        parse_date(&input.end_date).ok().flatten()
    } else {
        start.zip(months).map(|(s, m)| compute_end_date(s, m))
    };
    let acc = parse_date(&input.acceptance_date).ok().flatten();
    let liq = parse_date(&input.liquidation_date).ok().flatten();
    let mut warnings = Vec::new();
    if let (Some(a), Some(s)) = (acc, start)
        && a < s
    {
        warnings.push("Ngày nghiệm thu sớm hơn ngày hiệu lực.");
    }
    if let (Some(l), Some(e)) = (liq, end)
        && l < e
    {
        warnings.push("Ngày thanh lý sớm hơn ngày kết thúc.");
    }
    if let (Some(l), Some(a)) = (liq, acc)
        && l < a
    {
        warnings.push("Ngày thanh lý sớm hơn ngày nghiệm thu.");
    }
    if let (Some(sg), Some(s)) = (sign, start)
        && sg > s
    {
        warnings.push("Ngày ký sau ngày hiệu lực.");
    }
    warnings.join(" ")
}

fn apply_customer_filter(screen: &mut Screen) {
    let q = search_key(screen.customer_filter.trim());
    screen.customers = screen
        .all_customers
        .iter()
        .filter(|(_, _, key)| q.is_empty() || key.contains(&q))
        .map(|(id, name, _)| (*id, name.clone()))
        .collect();
}

fn checked_unit_count(facilities: &[Facility]) -> i64 {
    let count = facilities.iter().filter(|f| f.checked).count();
    i64::try_from(count.max(1)).unwrap_or(1)
}

fn refresh(suggest: bool) {
    refresh_ex(suggest, false);
}

fn refresh_ex(suggest: bool, reload_facilities: bool) {
    let Some((worker, request)) = with_screen(|screen| {
        let sign_year = parse_date(&screen.input.sign_date)
            .ok()
            .flatten()
            .map_or(chrono::Local::now().year(), |date| date.year());
        let unit_count = if screen.input.manual_unit_count {
            screen
                .input
                .unit_count
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|v| *v >= 1)
                .unwrap_or_else(|| checked_unit_count(&screen.facilities))
        } else {
            checked_unit_count(&screen.facilities)
        };
        let manual_unit_override = if screen.input.manual_unit_count {
            screen
                .input
                .unit_count
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|v| *v >= 1)
        } else {
            None
        };
        (
            screen.worker.clone(),
            Refresh {
                customer_id: screen.input.customer_id,
                product_id: screen.input.product_id,
                addons: screen.input.addons.iter().copied().collect(),
                months: screen.input.months.clone(),
                sign_year,
                suggest_number: suggest && !screen.input.manual_number,
                unit_count,
                manual_unit_override,
                reload_facilities,
            },
        )
    }) else {
        return;
    };
    worker.run(
        move |ctx| compute(ctx, &request),
        move |computed| post_to_ui(move || apply(computed)),
    );
}

fn compute(ctx: &mut Ctx, request: &Refresh) -> Computed {
    let store = ctx.store();
    let mut customers: Vec<(i64, String, String)> = store
        .customers
        .values()
        .filter(|customer| !customer.is_deleted && customer.is_active)
        .map(|customer| {
            let key = search_key(&format!(
                "{} {} {}",
                customer.code,
                customer.legal_name,
                customer.short_name.as_deref().unwrap_or_default()
            ));
            (customer.id, customer.legal_name.clone(), key)
        })
        .collect();
    customers.sort_by(|a, b| a.1.cmp(&b.1));
    let mut mains: Vec<(i64, String)> = store
        .products
        .values()
        .filter(|product: &&Product| product.is_active && product.kind == "main")
        .map(|product| (product.id, product.name.clone()))
        .collect();
    mains.sort_by(|a, b| a.1.cmp(&b.1));

    let facilities = if request.reload_facilities {
        Some(customer_facilities(store, request.customer_id).facilities)
    } else {
        None
    };
    let effective_units = request.manual_unit_override.unwrap_or_else(|| {
        facilities
            .as_deref()
            .map(checked_unit_count)
            .unwrap_or(request.unit_count)
    });

    let (addons, templates, attachments, default_template) =
        match (request.customer_id, request.product_id) {
            (Some(customer), Some(product)) => {
                let options = product_options(store, product, customer, effective_units);
                let addons: Vec<(i64, String)> = options
                    .addons
                    .iter()
                    .map(|brief: &ProductBrief| (brief.id, brief.name.clone()))
                    .collect();
                (
                    addons,
                    options.templates.contract,
                    options.templates.attachments,
                    options.templates.default_id,
                )
            }
            _ => (Vec::new(), Vec::new(), Vec::new(), None),
        };

    let suggested = match (request.customer_id, request.suggest_number) {
        (Some(customer), true) => {
            suggest_number(store, customer, request.product_id, request.sign_year).ok()
        }
        _ => None,
    };

    let preview = match request.product_id {
        Some(product) if !request.months.trim().is_empty() => {
            let months = request
                .months
                .trim()
                .parse::<i64>()
                .map_err(|_| "Số tháng không hợp lệ.".to_owned());
            match months {
                Ok(months) => {
                    let mut ids = vec![product];
                    ids.extend(request.addons.iter().copied());
                    let items: Vec<ItemInput> = ids
                        .into_iter()
                        .map(|id| item_with_qty(id, effective_units))
                        .collect();
                    price_preview(store, &items, months, true)
                        .map(|p| {
                            let lines = p
                                .lines
                                .iter()
                                .map(|ln| ContractPriceLineRow {
                                    name: ln.description.as_str().into(),
                                    unit: ln.unit_label.as_str().into(),
                                    qty: ln.quantity.normalize().to_string().into(),
                                    price: fmt_currency(Some(ln.unit_price_pre_vat)).into(),
                                    vat_rate: format!("{}%", ln.vat_rate.normalize()).into(),
                                    pre_vat: fmt_currency(Some(ln.amount_pre_vat)).into(),
                                    vat_amount: fmt_currency(Some(ln.vat_amount)).into(),
                                    total: fmt_currency(Some(ln.amount_total)).into(),
                                })
                                .collect();
                            let words = amount_to_words_dec(p.totals.total).unwrap_or_default();
                            let summary = format!(
                                "Tổng tiền: {} (trước VAT {}, VAT {})",
                                view::money_text(Some(p.totals.total)),
                                view::money_text(Some(p.totals.pre_vat)),
                                view::money_text(Some(p.totals.vat)),
                            );
                            PreviewComputed {
                                summary,
                                lines,
                                pre_vat: view::money_text(Some(p.totals.pre_vat)),
                                vat: view::money_text(Some(p.totals.vat)),
                                total: view::money_text(Some(p.totals.total)),
                                amount_words: words,
                                description: p.description,
                            }
                        })
                        .map_err(|error| error.user_message())
                }
                Err(message) => Err(message),
            }
        }
        _ => Ok(PreviewComputed {
            summary: String::new(),
            lines: Vec::new(),
            pre_vat: String::new(),
            vat: String::new(),
            total: String::new(),
            amount_words: String::new(),
            description: String::new(),
        }),
    };

    let main_product = request.product_id.and_then(|id| store.products.get(&id));
    let main_group = main_product.and_then(|product| product.group_code.clone());
    let default_months = main_product
        .and_then(|product| product.default_duration_months)
        .filter(|m| *m > 0);
    let cust = request.customer_id.and_then(|id| store.customers.get(&id));
    let customer_legal_name = cust.map(|c| c.legal_name.clone());
    let customer_info_text = cust
        .map(|c| {
            let mut parts = Vec::new();
            if let Some(rep) = c
                .representative_name
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                let title = c.representative_title.as_deref().unwrap_or("");
                if title.is_empty() {
                    parts.push(format!("Đại diện: {rep}"));
                } else {
                    parts.push(format!("Đại diện: {rep} ({title})"));
                }
            }
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
            if !addr.is_empty() {
                parts.push(format!("Địa chỉ: {addr}"));
            }
            parts.join(" · ")
        })
        .unwrap_or_default();
    let party_b_incomplete = !is_organization_profile_complete(store);
    let pbd = get_party_b_settings(store);

    Computed {
        customers,
        mains,
        addons,
        templates,
        attachments,
        default_template,
        default_months,
        customer_legal_name,
        customer_info_text,
        suggested,
        facilities,
        preview,
        main_group,
        party_b_incomplete,
        party_b_defaults: (
            pbd.representative,
            pbd.authorization_number,
            pbd.authorization_date,
        ),
    }
}

fn apply(computed: Computed) {
    let snapshot = with_screen(|screen| {
        screen.all_customers = computed.customers;
        apply_customer_filter(screen);
        screen.mains = computed.mains;
        screen.addons = computed.addons;
        screen.templates = computed.templates;
        if screen.attachments.is_empty() && !computed.attachments.is_empty() {
            screen.input.attachments = computed
                .attachments
                .iter()
                .filter(|a| a.preselect)
                .map(|a| a.id)
                .collect();
        }
        screen.attachments = computed.attachments;
        if let Some(facs) = computed.facilities {
            screen.facilities = facs;
            if !screen.input.manual_unit_count {
                screen.input.unit_count = checked_unit_count(&screen.facilities).to_string();
            }
        }
        screen.main_group = computed.main_group;
        if screen.input.template_id.is_none() {
            screen.input.template_id = computed.default_template;
        }
        if !screen.input.manual_months
            && let Some(m) = computed.default_months
        {
            screen.input.months = m.to_string();
            auto_fill_dates(&mut screen.input);
        }
        if !screen.input.manual_location
            && let Some(name) = computed.customer_legal_name
        {
            screen.input.location = lowercase_first(&name);
        }
        if !screen.input.party_b_seeded {
            screen.input.party_b_seeded = true;
            if screen.input.party_b_rep.is_empty() {
                screen.input.party_b_rep = computed.party_b_defaults.0;
            }
            if screen.input.party_b_auth_num.is_empty() {
                screen.input.party_b_auth_num = computed.party_b_defaults.1;
            }
            if screen.input.party_b_auth_date.is_empty() {
                screen.input.party_b_auth_date = computed.party_b_defaults.2;
            }
        }
        if let Some(number) = computed.suggested {
            screen.input.number = number;
        }
        let (price_lines, price_pre_vat, price_vat, price_total) = match computed.preview {
            Ok(p) => {
                screen.summary = p.summary;
                screen.error = String::new();
                if !screen.input.manual_amount_text && !p.amount_words.is_empty() {
                    screen.input.amount_text = p.amount_words;
                }
                if !screen.input.manual_pricing_desc && !p.description.is_empty() {
                    screen.input.pricing_description = p.description;
                }
                (p.lines, p.pre_vat, p.vat, p.total)
            }
            Err(message) => {
                screen.summary = String::new();
                screen.error = message;
                (Vec::new(), String::new(), String::new(), String::new())
            }
        };
        Snapshot {
            ui: screen.ui.clone(),
            customers: names(&screen.customers),
            mains: names(&screen.mains),
            customer_index: index_of(&screen.customers, screen.input.customer_id),
            main_index: index_of(&screen.mains, screen.input.product_id),
            template_names: template_names(&screen.templates),
            template_index: index_of_template(&screen.templates, screen.input.template_id),
            addon_rows: screen
                .addons
                .iter()
                .map(|(id, name)| FormAddonRow {
                    id: i32::try_from(*id).unwrap_or(i32::MAX),
                    name: SharedString::from(name.as_str()),
                    checked: screen.input.addons.contains(id),
                })
                .collect(),
            number: screen.input.number.clone(),
            end_date: if screen.input.manual_end && !screen.input.end_date.is_empty() {
                screen.input.end_date.clone()
            } else {
                end_date_text(&screen.input)
            },
            start_date: screen.input.start_date.clone(),
            months: screen.input.months.clone(),
            unit_count: screen.input.unit_count.clone(),
            location: screen.input.location.clone(),
            acceptance_date: screen.input.acceptance_date.clone(),
            liquidation_date: screen.input.liquidation_date.clone(),
            amount_words: screen.input.amount_text.clone(),
            pricing_desc: screen.input.pricing_description.clone(),
            party_b_rep: screen.input.party_b_rep.clone(),
            party_b_auth_num: screen.input.party_b_auth_num.clone(),
            party_b_auth_date: screen.input.party_b_auth_date.clone(),
            customer_info_text: computed.customer_info_text,
            price_lines,
            price_pre_vat,
            price_vat,
            price_total,
            date_warnings: date_warnings_text(&screen.input),
            summary: screen.summary.clone(),
            error: screen.error.clone(),
            party_b_incomplete: computed.party_b_incomplete,
        }
    });
    let Some(snapshot) = snapshot else {
        return;
    };
    let Some(ui) = snapshot.ui.upgrade() else {
        return;
    };
    ui.set_contract_customer_names(string_model(snapshot.customers));
    ui.set_contract_customer_index(snapshot.customer_index);
    ui.set_contract_main_names(string_model(snapshot.mains));
    ui.set_contract_main_index(snapshot.main_index);
    ui.set_contract_template_names(string_model(snapshot.template_names));
    ui.set_contract_template_index(snapshot.template_index);
    ui.set_contract_addon_rows(ModelRc::new(VecModel::from(snapshot.addon_rows)));
    ui.set_contract_number_text(snapshot.number.into());
    ui.set_contract_end_date_text(snapshot.end_date.clone().into());
    ui.set_form_summary_text(snapshot.summary.into());
    ui.set_form_error_text(snapshot.error.into());

    let hub = ui.global::<ContractHub>();
    hub.set_party_b_incomplete(snapshot.party_b_incomplete);
    hub.set_customer_info_text(snapshot.customer_info_text.into());
    hub.set_price_lines(ModelRc::new(VecModel::from(snapshot.price_lines)));
    hub.set_price_pre_vat(snapshot.price_pre_vat.into());
    hub.set_price_vat(snapshot.price_vat.into());
    hub.set_price_total(snapshot.price_total.into());
    if hub.get_start_date().as_str() != snapshot.start_date {
        hub.set_start_date(snapshot.start_date.into());
    }
    if hub.get_end_date_input().as_str() != snapshot.end_date {
        hub.set_end_date_input(snapshot.end_date.into());
    }
    if hub.get_months().as_str() != snapshot.months {
        hub.set_months(snapshot.months.into());
    }
    if hub.get_unit_count_input().as_str() != snapshot.unit_count {
        hub.set_unit_count_input(snapshot.unit_count.into());
    }
    if hub.get_location().as_str() != snapshot.location {
        hub.set_location(snapshot.location.into());
    }
    if hub.get_acceptance_date().as_str() != snapshot.acceptance_date {
        hub.set_acceptance_date(snapshot.acceptance_date.into());
    }
    if hub.get_liquidation_date().as_str() != snapshot.liquidation_date {
        hub.set_liquidation_date(snapshot.liquidation_date.into());
    }
    if hub.get_amount_words().as_str() != snapshot.amount_words {
        hub.set_amount_words(snapshot.amount_words.into());
    }
    if hub.get_pricing_desc().as_str() != snapshot.pricing_desc {
        hub.set_pricing_desc(snapshot.pricing_desc.into());
    }
    if hub.get_party_b_rep().as_str() != snapshot.party_b_rep {
        hub.set_party_b_rep(snapshot.party_b_rep.into());
    }
    if hub.get_party_b_auth_num().as_str() != snapshot.party_b_auth_num {
        hub.set_party_b_auth_num(snapshot.party_b_auth_num.into());
    }
    if hub.get_party_b_auth_date().as_str() != snapshot.party_b_auth_date {
        hub.set_party_b_auth_date(snapshot.party_b_auth_date.into());
    }
    hub.set_date_warnings(snapshot.date_warnings.into());
    push_customer_list_and_facilities();
    push_form_attachments();
}

fn push_dates(screen: &Screen) {
    let Some(ui) = screen.ui.upgrade() else {
        return;
    };
    let end_str = if screen.input.manual_end && !screen.input.end_date.is_empty() {
        screen.input.end_date.clone()
    } else {
        end_date_text(&screen.input)
    };
    ui.set_contract_end_date_text(end_str.clone().into());
    let hub = ui.global::<ContractHub>();
    if hub.get_start_date().as_str() != screen.input.start_date {
        hub.set_start_date(screen.input.start_date.clone().into());
    }
    if hub.get_end_date_input().as_str() != end_str {
        hub.set_end_date_input(end_str.into());
    }
    if hub.get_months().as_str() != screen.input.months {
        hub.set_months(screen.input.months.clone().into());
    }
    if hub.get_acceptance_date().as_str() != screen.input.acceptance_date {
        hub.set_acceptance_date(screen.input.acceptance_date.clone().into());
    }
    if hub.get_liquidation_date().as_str() != screen.input.liquidation_date {
        hub.set_liquidation_date(screen.input.liquidation_date.clone().into());
    }
    hub.set_date_warnings(date_warnings_text(&screen.input).into());
}

fn push_customer_list_and_facilities() {
    let payload = with_screen(|screen| {
        let cust_names = names(&screen.customers);
        let cust_idx = index_of(&screen.customers, screen.input.customer_id);
        let fac_rows = screen
            .facilities
            .iter()
            .enumerate()
            .map(|(i, f)| IdLabel {
                id: i32::try_from(i).unwrap_or(0),
                label: format!("{} ({})", f.name, f.unit_type).into(),
                checked: f.checked,
            })
            .collect::<Vec<_>>();
        let unit_count = if screen.input.manual_unit_count {
            screen
                .input
                .unit_count
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|v| *v >= 1)
                .unwrap_or_else(|| checked_unit_count(&screen.facilities))
        } else {
            checked_unit_count(&screen.facilities)
        };
        (
            screen.ui.clone(),
            cust_names,
            cust_idx,
            fac_rows,
            unit_count,
        )
    });
    if let Some((ui, cust_names, cust_idx, fac_rows, unit_count)) = payload
        && let Some(ui) = ui.upgrade()
    {
        ui.set_contract_customer_names(string_model(cust_names));
        ui.set_contract_customer_index(cust_idx);
        let hub = ui.global::<ContractHub>();
        hub.set_facilities(ModelRc::new(VecModel::from(fac_rows)));
        hub.set_unit_count_text(format!("{unit_count} cơ sở").into());
    }
}

fn push_form_attachments() {
    let payload = with_screen(|screen| {
        let count = screen.input.attachments.len();
        let rows = screen
            .attachments
            .iter()
            .map(|att| IdLabel {
                id: i32::try_from(att.id).unwrap_or(0),
                label: format!("{} ({})", att.name, att.kind.as_deref().unwrap_or("")).into(),
                checked: screen.input.attachments.contains(&att.id),
            })
            .collect::<Vec<_>>();
        (screen.ui.clone(), rows, count)
    });
    if let Some((ui, rows, count)) = payload
        && let Some(ui) = ui.upgrade()
    {
        let hub = ui.global::<ContractHub>();
        hub.set_form_attachments(ModelRc::new(VecModel::from(rows)));
        hub.set_selected_att_count(i32::try_from(count).unwrap_or(0));
    }
}

struct Snapshot {
    ui: Weak<MainWindow>,
    customers: Vec<SharedString>,
    mains: Vec<SharedString>,
    customer_index: i32,
    main_index: i32,
    template_names: Vec<SharedString>,
    template_index: i32,
    addon_rows: Vec<FormAddonRow>,
    number: String,
    end_date: String,
    start_date: String,
    months: String,
    unit_count: String,
    location: String,
    acceptance_date: String,
    liquidation_date: String,
    amount_words: String,
    pricing_desc: String,
    party_b_rep: String,
    party_b_auth_num: String,
    party_b_auth_date: String,
    customer_info_text: String,
    price_lines: Vec<ContractPriceLineRow>,
    price_pre_vat: String,
    price_vat: String,
    price_total: String,
    date_warnings: String,
    summary: String,
    error: String,
    party_b_incomplete: bool,
}

fn names(items: &[(i64, String)]) -> Vec<SharedString> {
    items
        .iter()
        .map(|(_, name)| SharedString::from(name.as_str()))
        .collect()
}

fn template_names(templates: &[TemplateOption]) -> Vec<SharedString> {
    templates
        .iter()
        .map(|option| SharedString::from(option.name.as_str()))
        .collect()
}

fn string_model(values: Vec<SharedString>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(values))
}

fn index_of(items: &[(i64, String)], id: Option<i64>) -> i32 {
    id.and_then(|id| items.iter().position(|(candidate, _)| *candidate == id))
        .and_then(|position| i32::try_from(position).ok())
        .unwrap_or(-1)
}

fn index_of_template(templates: &[TemplateOption], id: Option<i64>) -> i32 {
    id.and_then(|id| templates.iter().position(|option| option.id == id))
        .and_then(|position| i32::try_from(position).ok())
        .unwrap_or(-1)
}

fn end_date_text(input: &Input) -> String {
    let start = parse_date(&input.start_date).ok().flatten();
    let months = input.months.trim().parse::<i32>().ok();
    match (start, months) {
        (Some(start), Some(months)) if months > 0 => {
            view::date_text(Some(compute_end_date(start, months)))
        }
        _ => String::new(),
    }
}

fn parse_date(raw: &str) -> Result<Option<NaiveDate>, String> {
    let raw = raw.trim();
    parse_date_input(if raw.is_empty() { None } else { Some(raw) })
        .map_err(|error| error.to_string())
}

fn save(generate: bool) {
    let Some((worker, built, attachment_ids, ui)) = with_screen(|screen| {
        if screen.busy {
            return None;
        }
        let unit_names: Vec<String> = screen
            .facilities
            .iter()
            .filter(|f| f.checked)
            .map(|f| f.name.clone())
            .collect();
        let unit_count = if screen.input.manual_unit_count {
            screen
                .input
                .unit_count
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|v| *v >= 1)
                .unwrap_or_else(|| checked_unit_count(&screen.facilities))
        } else {
            checked_unit_count(&screen.facilities)
        };
        let atts: Vec<i64> = screen.input.attachments.iter().copied().collect();
        Some((
            screen.worker.clone(),
            build_form_ex(
                &screen.input,
                &screen.addons,
                screen.main_group.clone(),
                unit_count,
                unit_names,
            ),
            atts,
            screen.ui.clone(),
        ))
    })
    .flatten() else {
        return;
    };
    let (form, group) = match built {
        Ok(built) => built,
        Err(message) => {
            toast_from_worker(ToastKind::Danger, message.clone());
            with_screen(|screen| screen.error = message);
            push_error();
            return;
        }
    };
    with_screen(|screen| screen.busy = true);
    worker.run(
        move |ctx| -> Result<(i64, String), ServiceError> {
            let id = create_draft(
                ctx,
                form,
                CreateOptions {
                    template_group: group.as_deref(),
                    ..CreateOptions::default()
                },
            )?;
            if generate {
                let result = generate_docx(ctx, id, &attachment_ids)?;
                Ok((id, format!("Đã sinh {}", result.file_name)))
            } else {
                Ok((id, format!("Đã lưu nháp hợp đồng id={id}")))
            }
        },
        move |result| {
            with_screen(|screen| screen.busy = false);
            match result {
                Ok((id, message)) => {
                    toast_from_worker(ToastKind::Success, message);
                    post_to_ui(move || {
                        if let Some(ui) = ui.upgrade() {
                            ui.invoke_contracts_reload();
                            ui.invoke_navigate(nav::Screen::Contracts.index());
                            ui.invoke_contracts_open(i32::try_from(id).unwrap_or(0));
                        }
                    });
                }
                Err(error) => {
                    let msg = error.user_message();
                    toast_from_worker(ToastKind::Danger, msg.clone());
                    with_screen(|screen| screen.error = msg);
                    post_to_ui(push_error);
                }
            }
        },
    );
}

fn push_error() {
    let Some((weak, error)) = with_screen(|screen| (screen.ui.clone(), screen.error.clone()))
    else {
        return;
    };
    if let Some(ui) = weak.upgrade() {
        ui.set_form_error_text(error.into());
    }
}

#[allow(dead_code)]
fn build_form(
    input: &Input,
    addons: &[(i64, String)],
    main_group: Option<String>,
) -> Result<(ContractForm, Option<String>), String> {
    build_form_ex(input, addons, main_group, 1, Vec::new())
}

fn build_form_ex(
    input: &Input,
    addons: &[(i64, String)],
    main_group: Option<String>,
    unit_count: i64,
    selected_unit_names: Vec<String>,
) -> Result<(ContractForm, Option<String>), String> {
    let customer_id = input.customer_id.ok_or("Chưa chọn khách hàng.")?;
    let number = input.number.trim().to_owned();
    if number.is_empty() {
        return Err("Chưa có số hợp đồng.".into());
    }
    let sign_date = parse_date(&input.sign_date).map_err(|e| format!("Ngày ký: {e}"))?;
    let start_date = parse_date(&input.start_date).map_err(|e| format!("Ngày bắt đầu: {e}"))?;
    let acceptance_date =
        parse_date(&input.acceptance_date).map_err(|e| format!("Ngày nghiệm thu: {e}"))?;
    let liquidation_date =
        parse_date(&input.liquidation_date).map_err(|e| format!("Ngày thanh lý: {e}"))?;
    let months = input.months.trim().parse::<i64>().ok();
    let end_date = if input.manual_end && !input.end_date.trim().is_empty() {
        Some(
            parse_date(&input.end_date)
                .map_err(|e| format!("Ngày kết thúc: {e}"))?
                .ok_or_else(|| "Ngày kết thúc không hợp lệ.".to_owned())?,
        )
    } else {
        match (start_date, months) {
            (Some(start), Some(months)) if months > 0 => {
                Some(compute_end_date(start, i32::try_from(months).unwrap_or(0)))
            }
            _ => None,
        }
    };
    let payment_mode = non_empty(&input.payment_mode);
    let payment_cycle_months = input.payment_cycle_months.trim().parse::<i64>().ok();
    let total = match input.total.trim() {
        "" => None,
        raw => Some(
            Decimal::from_str(&raw.replace('.', ""))
                .map_err(|_| "Tổng tiền không hợp lệ.".to_owned())?,
        ),
    };
    let mut items = Vec::new();
    if let Some(main) = input.product_id {
        items.push(item_with_qty(main, unit_count));
    }
    for (addon, _) in addons.iter().filter(|(id, _)| input.addons.contains(id)) {
        items.push(item_with_qty(*addon, unit_count));
    }
    Ok((
        ContractForm {
            contract_number: number,
            customer_id,
            product_id: input.product_id,
            template_id: input.template_id,
            sign_date,
            start_date,
            end_date,
            month_count: months,
            sign_location: non_empty(&input.location),
            unit_count: Some(unit_count),
            unit_price_pre_vat: None,
            vat_rate: None,
            unit_price_vat: None,
            quantity: None,
            total_amount: total,
            amount_text: non_empty(&input.amount_text),
            pricing_description: non_empty(&input.pricing_description),
            party_b_representative: non_empty(&input.party_b_rep),
            party_b_authorization_number: non_empty(&input.party_b_auth_num),
            party_b_authorization_date: non_empty(&input.party_b_auth_date),
            acceptance_date,
            liquidation_date,
            notes: non_empty(&input.notes),
            selected_unit_names,
            items,
            payment_mode,
            payment_cycle_months,
        },
        main_group,
    ))
}

fn item_with_qty(product_id: i64, unit_count: i64) -> ItemInput {
    ItemInput {
        product_id,
        description: String::new(),
        quantity: unit_count.max(1).to_string(),
        unit_price_pre_vat: None,
        vat_rate: None,
        amount_pre_vat_override: None,
        note: String::new(),
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{Input, build_form};

    fn filled() -> Input {
        Input {
            customer_id: Some(1),
            product_id: Some(10),
            addons: [20].into_iter().collect(),
            number: "01/CF/2026".into(),
            sign_date: "09/03/2026".into(),
            start_date: "01/04/2026".into(),
            months: "12".into(),
            total: "1.250.000".into(),
            ..Input::default()
        }
    }

    #[test]
    fn form_needs_a_customer_and_a_number() {
        let mut input = filled();
        input.customer_id = None;
        assert!(build_form(&input, &[], None).is_err());

        let mut input = filled();
        input.number = "  ".into();
        assert_eq!(
            build_form(&input, &[], None).err().as_deref(),
            Some("Chưa có số hợp đồng.")
        );
    }

    #[test]
    fn bad_dates_are_reported_with_their_field() {
        let mut input = filled();
        input.sign_date = "khong-phai-ngay".into();
        let message = build_form(&input, &[], None).err().unwrap_or_default();
        assert!(message.starts_with("Ngày ký:"), "{message}");
    }

    #[test]
    fn main_and_selected_addons_become_items_and_end_date_is_computed() {
        let input = filled();
        let addons = vec![(20, "Gói A".to_owned()), (21, "Gói B".to_owned())];
        let (form, group) = build_form(&input, &addons, Some("NHOM".into())).expect("form");
        assert_eq!(group.as_deref(), Some("NHOM"));
        assert_eq!(form.items.len(), 2);
        assert_eq!(form.items[0].product_id, 10);
        assert_eq!(form.items[1].product_id, 20);
        assert_eq!(
            form.end_date.map(|d| d.to_string()),
            Some("2027-03-31".into())
        );
    }

    #[test]
    fn sign_date_auto_fills_start_end_acceptance_and_liquidation_dates() {
        let mut input = Input {
            sign_date: "15/01/2026".into(),
            ..Input::default()
        };
        input.start_date = input.sign_date.clone();
        super::auto_fill_dates(&mut input);
        assert_eq!(input.months, "12");
        assert_eq!(super::end_date_text(&input), "14/01/2027");
        assert_eq!(input.acceptance_date, "17/01/2026");
        assert_eq!(input.liquidation_date, "14/01/2027");
    }
}
