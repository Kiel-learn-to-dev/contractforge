use std::cell::RefCell;

use cf_core::template_rules::Role;
use cf_services::{
    Ctx,
    products::{
        clean,
        edit::{create_product, update_product},
    },
    templates::list::templates_for,
};
use serde_json::{Value, json};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    MainWindow, ProductFormData,
    ui_helpers::{ToastKind, toast_from_worker},
    worker::Worker,
};

#[derive(Clone)]
struct TemplateChoice {
    id: Option<i64>,
    label: String,
}

struct State {
    ui: slint::Weak<MainWindow>,
    worker: Worker<Ctx>,
    editing_id: Option<i64>,
    templates: Vec<TemplateChoice>,
    replaced_choices: Vec<TemplateChoice>,
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>) {
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
            editing_id: None,
            templates: Vec::new(),
            replaced_choices: Vec::new(),
        })
    });

    let weak = ui.as_weak();
    ui.on_product_add(move || {
        let data = blank_form();
        if let Some(ui) = weak.upgrade() {
            ui.set_product_form_data(data);
            ui.set_product_form_errors("".into());
            ui.set_product_form_visible(true);
        }
        with_state(|state| state.editing_id = None);
        load_templates(String::new(), None, None);
    });

    ui.on_product_edit(|id| open_edit(i64::from(id)));
    ui.on_product_cancel(|| {
        if let Some(ui) = with_state(|state| state.ui.clone()).and_then(|weak| weak.upgrade()) {
            ui.set_product_form_visible(false);
            ui.set_product_form_errors("".into());
        }
    });
    ui.on_product_group_changed(|group| {
        let (selected, replaced) = with_state(|state| {
            state.ui.upgrade().map(|ui| {
                let d = ui.get_product_form_data();
                (
                    d.default_template_id.parse::<i64>().ok(),
                    d.replaced_by_id.parse::<i64>().ok(),
                )
            })
        })
        .flatten()
        .unwrap_or((None, None));
        load_templates(group.to_string(), selected, replaced);
    });
    ui.on_product_template_changed(|label| {
        let choice = with_state(|state| {
            state
                .templates
                .iter()
                .position(|choice| choice.label == label.as_str())
                .map(|index| (index as i32, state.templates[index].id))
        })
        .flatten();
        if let Some(ui) = with_state(|state| state.ui.clone()).and_then(|weak| weak.upgrade()) {
            let mut data = ui.get_product_form_data();
            data.default_template_id = choice
                .and_then(|(_, id)| id)
                .map(|value| value.to_string())
                .unwrap_or_default()
                .into();
            if let Some((index, _)) = choice {
                ui.set_product_template_index(index);
            }
            ui.set_product_form_data(data);
        }
    });
    ui.on_product_replaced_changed(|idx| {
        let picked = with_state(|state| {
            usize::try_from(idx)
                .ok()
                .and_then(|i| state.replaced_choices.get(i))
                .and_then(|c| c.id)
        })
        .flatten();
        if let Some(ui) = with_state(|state| state.ui.clone()).and_then(|weak| weak.upgrade()) {
            let mut data = ui.get_product_form_data();
            data.replaced_by_id = picked.map(|v| v.to_string()).unwrap_or_default().into();
            ui.set_product_replaced_index(idx);
            ui.set_product_form_data(data);
        }
    });
    ui.on_product_save(save);
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|state| state.borrow_mut().as_mut().map(f))
}

fn blank_form() -> ProductFormData {
    ProductFormData {
        code: "".into(),
        name: "".into(),
        product_type: "".into(),
        group_code: "".into(),
        kind: "main".into(),
        unit_label: "".into(),
        default_price: "".into(),
        default_vat_rate: "0".into(),
        price_period_months: "1".into(),
        default_duration_months: "".into(),
        sort_order: "0".into(),
        quantity_source: "units".into(),
        min_quantity: "".into(),
        quantity_step: "".into(),
        account_limit: "".into(),
        default_template_id: "".into(),
        contract_number_format: "".into(),
        replaced_by_id: "".into(),
        feature_text: "".into(),
    }
}

fn open_edit(id: i64) {
    let Some(worker) = with_state(|state| {
        state.editing_id = Some(id);
        state.worker.clone()
    }) else {
        return;
    };
    worker.run(
        move |ctx| {
            let product = ctx.store().products.get(&id).cloned();
            product.map(|product| {
                let template = product.default_template_id;
                let group = product.group_code.clone().unwrap_or_default();
                let feature_text = clean::feature_text(&product);
                let data = ProductFormData {
                    code: product.code.into(),
                    name: product.name.into(),
                    product_type: product.product_type.into(),
                    group_code: group.clone().into(),
                    kind: product.kind.into(),
                    unit_label: product.unit_label.unwrap_or_default().into(),
                    default_price: product
                        .default_price
                        .map(|x| x.to_string())
                        .unwrap_or_default()
                        .into(),
                    default_vat_rate: product
                        .default_vat_rate
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "0".into())
                        .into(),
                    price_period_months: product.price_period_months.to_string().into(),
                    default_duration_months: product
                        .default_duration_months
                        .map(|x| x.to_string())
                        .unwrap_or_default()
                        .into(),
                    sort_order: product.sort_order.to_string().into(),
                    quantity_source: product.quantity_source.into(),
                    min_quantity: product
                        .min_quantity
                        .map(|x| x.to_string())
                        .unwrap_or_default()
                        .into(),
                    quantity_step: product
                        .quantity_step
                        .map(|x| x.to_string())
                        .unwrap_or_default()
                        .into(),
                    account_limit: product
                        .account_limit
                        .map(|x| x.to_string())
                        .unwrap_or_default()
                        .into(),
                    default_template_id: template.map(|x| x.to_string()).unwrap_or_default().into(),
                    contract_number_format: product
                        .contract_number_format
                        .clone()
                        .unwrap_or_default()
                        .into(),
                    replaced_by_id: product
                        .replaced_by_id
                        .map(|x| x.to_string())
                        .unwrap_or_default()
                        .into(),
                    feature_text: feature_text.into(),
                };
                (data, group, template)
            })
        },
        move |result| match result {
            Some((data, group, template)) => {
                let replaced = data.replaced_by_id.parse::<i64>().ok();
                if let Some(ui) =
                    with_state(|state| state.ui.clone()).and_then(|weak| weak.upgrade())
                {
                    ui.set_product_form_data(data);
                    ui.set_product_form_errors("".into());
                    ui.set_product_form_visible(true);
                }
                load_templates(group, template, replaced);
            }
            None => toast_from_worker(ToastKind::Danger, "Không tìm thấy sản phẩm.".into()),
        },
    );
}

fn load_templates(group: String, selected_id: Option<i64>, replaced_id: Option<i64>) {
    let Some((worker, editing_id)) = with_state(|state| (state.worker.clone(), state.editing_id))
    else {
        return;
    };
    worker.run(
        move |ctx| {
            let mut choices = vec![TemplateChoice {
                id: None,
                label: "— Không dùng mẫu mặc định —".into(),
            }];
            choices.extend(
                templates_for(
                    ctx.store(),
                    (!group.trim().is_empty()).then_some(group.as_str()),
                    Role::Contract,
                    true,
                )
                .into_iter()
                .map(|template| TemplateChoice {
                    id: Some(template.id),
                    label: match template.product_group.as_deref() {
                        Some(group) if !group.trim().is_empty() => {
                            format!("{} ({group})", template.name)
                        }
                        _ => format!("{} (Mẫu chung)", template.name),
                    },
                }),
            );
            let mut replaced = vec![TemplateChoice {
                id: None,
                label: "— Không chọn gói thay thế —".into(),
            }];
            let mut prods = ctx
                .store()
                .products
                .values()
                .filter(|p| Some(p.id) != editing_id && p.is_active)
                .map(|p| (p.id, format!("[{}] {}", p.code, p.name)))
                .collect::<Vec<_>>();
            prods.sort_by(|a, b| a.1.cmp(&b.1));
            replaced.extend(prods.into_iter().map(|(id, label)| TemplateChoice {
                id: Some(id),
                label,
            }));
            (choices, replaced)
        },
        move |(choices, replaced)| {
            let Some((weak, index, r_index)) = with_state(|state| {
                state.templates = choices;
                state.replaced_choices = replaced;
                let index = state
                    .templates
                    .iter()
                    .position(|item| item.id == selected_id)
                    .map(|i| i as i32)
                    .unwrap_or(0);
                let r_index = state
                    .replaced_choices
                    .iter()
                    .position(|item| item.id == replaced_id)
                    .map(|i| i as i32)
                    .unwrap_or(0);
                (state.ui.clone(), index, r_index)
            }) else {
                return;
            };
            if let Some(ui) = weak.upgrade() {
                let (labels, r_labels) = with_state(|state| {
                    (
                        state
                            .templates
                            .iter()
                            .map(|item| item.label.clone().into())
                            .collect::<Vec<_>>(),
                        state
                            .replaced_choices
                            .iter()
                            .map(|item| item.label.clone().into())
                            .collect::<Vec<_>>(),
                    )
                })
                .unwrap_or_default();
                ui.set_product_template_options(ModelRc::new(VecModel::from(labels)));
                ui.set_product_template_index(index);
                ui.set_product_replaced_options(ModelRc::new(VecModel::from(r_labels)));
                ui.set_product_replaced_index(r_index);
                let id = with_state(|state| {
                    state.templates.get(index as usize).and_then(|item| item.id)
                })
                .flatten();
                let mut data = ui.get_product_form_data();
                data.default_template_id =
                    id.map(|value| value.to_string()).unwrap_or_default().into();
                ui.set_product_form_data(data);
            }
        },
    );
}

fn to_json(data: &ProductFormData) -> Value {
    json!({
        "code": data.code.to_string(), "name": data.name.to_string(), "product_type": data.product_type.to_string(),
        "group_code": data.group_code.to_string(), "kind": data.kind.to_string(), "unit_label": data.unit_label.to_string(),
        "default_price": data.default_price.to_string(), "default_vat_rate": data.default_vat_rate.to_string(),
        "price_period_months": data.price_period_months.to_string(), "default_duration_months": data.default_duration_months.to_string(),
        "sort_order": data.sort_order.to_string(), "quantity_source": data.quantity_source.to_string(),
        "min_quantity": data.min_quantity.to_string(), "quantity_step": data.quantity_step.to_string(), "account_limit": data.account_limit.to_string(),
        "default_template_id": data.default_template_id.to_string(), "contract_number_format": data.contract_number_format.to_string(),
        "replaced_by_id": data.replaced_by_id.to_string(), "feature_text": data.feature_text.to_string(),
    })
}

fn save(data: ProductFormData) {
    let Some((worker, id)) = with_state(|state| (state.worker.clone(), state.editing_id)) else {
        return;
    };
    let value = to_json(&data);
    worker.run(
        move |ctx| {
            let now = ctx.clock().now_utc();
            if let Some(id) = id {
                update_product(ctx.store_mut(), id, &value, now).map(|_| id)
            } else {
                create_product(ctx.store_mut(), &value, now)
            }
        },
        move |result| match result {
            Ok(_) => {
                if let Some(ui) =
                    with_state(|state| state.ui.clone()).and_then(|weak| weak.upgrade())
                {
                    ui.set_product_form_visible(false);
                    ui.set_product_form_errors("".into());
                }
                toast_from_worker(ToastKind::Success, "Đã lưu sản phẩm.".into());
                crate::screens::products::reload();
            }
            Err(error) => {
                if let Some(ui) =
                    with_state(|state| state.ui.clone()).and_then(|weak| weak.upgrade())
                {
                    ui.set_product_form_errors(error.user_message().into());
                }
                toast_from_worker(ToastKind::Danger, error.user_message());
            }
        },
    );
}
