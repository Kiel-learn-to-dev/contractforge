//! S04 — form tạo/sửa khách hàng, đơn vị con và cơ sở tính phí.

use std::{cell::RefCell, collections::BTreeMap};

use cf_services::{
    Ctx,
    customers::{
        edit::{create_customer, generate_next_code, update_customer},
        units::{UnitInput, save_billing, save_sub_units},
    },
};
use serde_json::Value as Json;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::{
    CustomerFormData, CustomerHub, CustomerUnitRowData, MainWindow, dia_chinh,
    ui_helpers::{ToastKind, post_to_ui, toast_from_worker},
    worker::Worker,
};

const CUSTOMER_TYPES: &[&str] = &["Trạm y tế", "TTYT", "Đơn vị quản lý", "Bệnh viện", "Khác"];
const UNIT_TYPES: &[&str] = &[
    "Điểm trạm",
    "Phòng khám",
    "Cơ sở",
    "Chi nhánh",
    "Kho",
    "Khác",
];

#[derive(Debug, Clone)]
struct UnitDraft {
    id: Option<i64>,
    name: String,
    unit_type: String,
    address: String,
    notes: String,
    include_in_billing: bool,
}

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    editing_id: Option<i64>,
    bill_main: bool,
    units: Vec<UnitDraft>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>) {
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
            editing_id: None,
            bill_main: true,
            units: Vec::new(),
        });
    });

    let hub = ui.global::<CustomerHub>();
    hub.set_customer_types(string_model(CUSTOMER_TYPES.iter().copied()));
    hub.set_unit_types(string_model(UNIT_TYPES.iter().copied()));
    let mut provs = vec![String::new()];
    provs.extend(dia_chinh::provinces().iter().cloned());
    hub.set_province_options(ModelRc::new(VecModel::from(
        provs
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    )));

    hub.on_open_add(|| open(None));
    hub.on_open_edit(|id| open(Some(i64::from(id))));
    hub.on_form_province_changed(|province| {
        let mut wards = vec![SharedString::from("")];
        wards.extend(
            dia_chinh::wards_of(province.as_str())
                .iter()
                .map(SharedString::from),
        );
        if let Some(ui) = with_state(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
            ui.global::<CustomerHub>()
                .set_ward_options(ModelRc::new(VecModel::from(wards)));
        }
    });
    hub.on_form_add_unit(|| {
        with_state(|s| {
            s.units.push(UnitDraft {
                id: None,
                name: String::new(),
                unit_type: "Điểm trạm".into(),
                address: String::new(),
                notes: String::new(),
                include_in_billing: true,
            });
        });
        push_units();
    });
    hub.on_form_remove_unit(|index| {
        with_state(|s| {
            if let Ok(i) = usize::try_from(index)
                && i < s.units.len()
            {
                s.units.remove(i);
            }
        });
        push_units();
    });
    hub.on_form_update_unit(|index, name, unit_type, address, notes| {
        with_state(|s| {
            if let Ok(i) = usize::try_from(index)
                && let Some(row) = s.units.get_mut(i)
            {
                row.name = name.to_string();
                row.unit_type = unit_type.to_string();
                row.address = address.to_string();
                row.notes = notes.to_string();
            }
        });
    });
    hub.on_form_toggle_bill_main(|checked| {
        with_state(|s| s.bill_main = checked);
        push_billed_badge();
    });
    hub.on_form_toggle_unit_billing(|index| {
        with_state(|s| {
            if let Ok(i) = usize::try_from(index)
                && let Some(row) = s.units.get_mut(i)
            {
                row.include_in_billing = !row.include_in_billing;
            }
        });
        push_units();
    });
    hub.on_form_save(save);
    hub.on_form_cancel(|| {
        let editing = with_state(|s| (s.ui.clone(), s.editing_id));
        if let Some((ui, eid)) = editing {
            if let Some(id) = eid {
                crate::screens::customer_detail::open(id);
            } else if let Some(ui) = ui.upgrade() {
                ui.global::<CustomerHub>().set_mode("list".into());
            }
        }
    });
}

pub fn open(customer_id: Option<i64>) {
    let Some(worker) = with_state(|s| {
        s.editing_id = customer_id;
        s.worker.clone()
    }) else {
        return;
    };
    worker.run(
        move |ctx| load_form(ctx, customer_id),
        move |res| post_to_ui(move || apply_loaded(res)),
    );
}

struct Loaded {
    title: String,
    data: CustomerFormData,
    bill_main: bool,
    units: Vec<UnitDraft>,
}

fn load_form(ctx: &Ctx, customer_id: Option<i64>) -> Result<Loaded, String> {
    let Some(id) = customer_id else {
        let next = generate_next_code(ctx.store());
        return Ok(Loaded {
            title: "Thêm khách hàng mới".into(),
            data: CustomerFormData {
                code: next.into(),
                customer_type: "Trạm y tế".into(),
                bill_main_station: true,
                ..CustomerFormData::default()
            },
            bill_main: true,
            units: Vec::new(),
        });
    };
    let c = ctx
        .store()
        .customers
        .get(&id)
        .ok_or_else(|| format!("Không tìm thấy khách hàng id={id}"))?;
    let mut unit_rows = ctx
        .store()
        .units
        .values()
        .filter(|u| u.customer_id == id && u.is_active)
        .cloned()
        .collect::<Vec<_>>();
    unit_rows.sort_by_key(|u| (u.sort_order, u.id));
    let units = unit_rows
        .into_iter()
        .map(|u| UnitDraft {
            id: Some(u.id),
            name: u.name,
            unit_type: u.unit_type,
            address: u.address.unwrap_or_default(),
            notes: u.notes.unwrap_or_default(),
            include_in_billing: u.include_in_billing,
        })
        .collect();
    let est_date = c
        .establishment_date
        .map(|d| d.format("%d/%m/%Y").to_string())
        .unwrap_or_default();
    Ok(Loaded {
        title: format!("Sửa khách hàng: {}", c.code),
        bill_main: c.bill_main_station,
        data: CustomerFormData {
            code: c.code.clone().into(),
            legal_name: c.legal_name.clone().into(),
            short_name: c.short_name.clone().unwrap_or_default().into(),
            customer_type: c.customer_type.clone().into(),
            address: c.address.clone().unwrap_or_default().into(),
            province: c.province.clone().unwrap_or_default().into(),
            ward: c.ward.clone().unwrap_or_default().into(),
            tax_code: c.tax_code.clone().unwrap_or_default().into(),
            budget_relation_code: c.budget_relation_code.clone().unwrap_or_default().into(),
            kcb_code: c.kcb_code.clone().unwrap_or_default().into(),
            establishment_decision: c.establishment_decision.clone().unwrap_or_default().into(),
            establishment_date: est_date.into(),
            representative_name: c.representative_name.clone().unwrap_or_default().into(),
            representative_gender: c.representative_gender.clone().unwrap_or_default().into(),
            representative_title: c.representative_title.clone().unwrap_or_default().into(),
            bank_account: c.bank_account.clone().unwrap_or_default().into(),
            bank_name: c.bank_name.clone().unwrap_or_default().into(),
            beneficiary_name: c.beneficiary_name.clone().unwrap_or_default().into(),
            contact_name: c.contact_name.clone().unwrap_or_default().into(),
            phone: c.phone.clone().unwrap_or_default().into(),
            email: c.email.clone().unwrap_or_default().into(),
            notes: c.notes.clone().unwrap_or_default().into(),
            bill_main_station: c.bill_main_station,
        },
        units,
    })
}

fn apply_loaded(res: Result<Loaded, String>) {
    match res {
        Ok(loaded) => {
            let province = loaded.data.province.to_string();
            with_state(|s| {
                s.bill_main = loaded.bill_main;
                s.units = loaded.units;
            });
            if let Some(ui) = with_state(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
                let hub = ui.global::<CustomerHub>();
                let mut wards = vec![SharedString::from("")];
                wards.extend(
                    dia_chinh::wards_of(&province)
                        .iter()
                        .map(SharedString::from),
                );
                hub.set_ward_options(ModelRc::new(VecModel::from(wards)));
                hub.set_form_title(loaded.title.into());
                hub.set_form_data(loaded.data);
                hub.set_form_error("".into());
                hub.set_mode("form".into());
            }
            push_units();
        }
        Err(msg) => toast_from_worker(ToastKind::Danger, msg),
    }
}

fn save(data: CustomerFormData) {
    let Some((worker, editing_id, units)) =
        with_state(|s| (s.worker.clone(), s.editing_id, s.units.clone()))
    else {
        return;
    };
    let map = form_to_map(&data);
    let bill_main = data.bill_main_station;
    let unit_inputs: Vec<UnitInput> = units
        .into_iter()
        .map(|u| UnitInput {
            id: u.id,
            name: u.name,
            unit_type: u.unit_type,
            address: u.address,
            notes: u.notes,
            include_in_billing: Some(u.include_in_billing),
        })
        .collect();

    worker.run(
        move |ctx| -> Result<i64, String> {
            let now = ctx.clock().now_utc();
            let id = match editing_id {
                Some(id) => {
                    update_customer(ctx.store_mut(), id, &map, now)
                        .map_err(|e| e.user_message())?;
                    id
                }
                None => {
                    create_customer(ctx.store_mut(), &map, now).map_err(|e| e.user_message())?
                }
            };
            save_sub_units(ctx.store_mut(), id, &unit_inputs, now).map_err(|e| e.user_message())?;
            let included_ids = ctx
                .store()
                .units
                .values()
                .filter(|u| u.customer_id == id && u.is_active && u.include_in_billing)
                .map(|u| u.id)
                .collect::<Vec<_>>();
            save_billing(ctx.store_mut(), id, bill_main, &included_ids, now)
                .map_err(|e| e.user_message())?;
            Ok(id)
        },
        move |res| {
            post_to_ui(move || match res {
                Ok(id) => {
                    toast_from_worker(ToastKind::Success, "Đã lưu thông tin khách hàng.".into());
                    if let Some(ui) = with_state(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
                        ui.invoke_customer_reload();
                        crate::screens::customer_detail::open(id);
                    }
                }
                Err(err) => {
                    if let Some(ui) = with_state(|s| s.ui.clone()).and_then(|w| w.upgrade()) {
                        ui.global::<CustomerHub>().set_form_error(err.into());
                    }
                }
            });
        },
    );
}

fn form_to_map(data: &CustomerFormData) -> BTreeMap<String, Json> {
    [
        ("code", data.code.as_str()),
        ("legal_name", data.legal_name.as_str()),
        ("short_name", data.short_name.as_str()),
        ("customer_type", data.customer_type.as_str()),
        ("address", data.address.as_str()),
        ("province", data.province.as_str()),
        ("ward", data.ward.as_str()),
        ("tax_code", data.tax_code.as_str()),
        ("budget_relation_code", data.budget_relation_code.as_str()),
        ("kcb_code", data.kcb_code.as_str()),
        (
            "establishment_decision",
            data.establishment_decision.as_str(),
        ),
        ("establishment_date", data.establishment_date.as_str()),
        ("representative_name", data.representative_name.as_str()),
        ("representative_gender", data.representative_gender.as_str()),
        ("representative_title", data.representative_title.as_str()),
        ("bank_account", data.bank_account.as_str()),
        ("bank_name", data.bank_name.as_str()),
        ("beneficiary_name", data.beneficiary_name.as_str()),
        ("contact_name", data.contact_name.as_str()),
        ("phone", data.phone.as_str()),
        ("email", data.email.as_str()),
        ("notes", data.notes.as_str()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), Json::String(v.to_owned())))
    .collect()
}

fn push_units() {
    let Some((ui, rows)) = with_state(|s| {
        let rows = s
            .units
            .iter()
            .map(|u| CustomerUnitRowData {
                id: u.id.and_then(|v| i32::try_from(v).ok()).unwrap_or(0),
                name: u.name.clone().into(),
                unit_type: u.unit_type.clone().into(),
                address: u.address.clone().into(),
                notes: u.notes.clone().into(),
                checked: u.include_in_billing,
                active: true,
            })
            .collect::<Vec<_>>();
        (s.ui.clone(), rows)
    }) else {
        return;
    };
    if let Some(ui) = ui.upgrade() {
        ui.global::<CustomerHub>()
            .set_form_units(ModelRc::new(VecModel::from(rows)));
    }
    push_billed_badge();
}

fn push_billed_badge() {
    let Some((ui, count)) = with_state(|s| {
        let sub = s.units.iter().filter(|u| u.include_in_billing).count();
        let total = (usize::from(s.bill_main) + sub).max(1);
        (s.ui.clone(), total)
    }) else {
        return;
    };
    if let Some(ui) = ui.upgrade() {
        ui.global::<CustomerHub>()
            .set_form_billed_badge(format!("{count} cơ sở tính phí").into());
    }
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|cell| cell.borrow_mut().as_mut().map(f))
}

fn string_model<'a>(items: impl Iterator<Item = &'a str>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        items.map(SharedString::from).collect::<Vec<_>>(),
    ))
}
