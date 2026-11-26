use std::{cell::RefCell, collections::BTreeMap, path::PathBuf, rc::Rc};

use cf_core::{render_context::ALL_CANONICAL_FIELDS, template_rules::CONTRACT_TYPES};
use cf_services::{
    Ctx, ServiceError,
    templates::{
        create::{TemplateDraft, create_template},
        edit::{inspect_analysis, reanalyze, update_mapping, update_metadata},
    },
};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    MainWindow, TemplateFormData, TemplateMappingRowData, files,
    ui_helpers::{Feedback, ToastKind, toast_from_worker},
    worker::Worker,
};

struct State {
    ui: slint::Weak<MainWindow>,
    worker: Worker<Ctx>,
    feedback: Rc<Feedback>,
    editing_id: Option<i64>,
    detail_id: Option<i64>,
    selected_file: Option<PathBuf>,
    mapping: BTreeMap<String, String>,
    canonical: Vec<String>,
    busy: bool,
}
struct Detail {
    id: i64,
    code: String,
    name: String,
    summary: String,
    flags: String,
    legacy: bool,
    rows: Vec<TemplateMappingRowData>,
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>, feedback: &Rc<Feedback>) {
    let mut canonical = vec!["— chưa map —".to_owned()];
    canonical.extend(ALL_CANONICAL_FIELDS.iter().map(|field| (*field).to_owned()));
    canonical.push("Tự nhập".into());
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
            feedback: Rc::clone(feedback),
            editing_id: None,
            detail_id: None,
            selected_file: None,
            mapping: BTreeMap::new(),
            canonical,
            busy: false,
        })
    });
    ui.set_template_type_suggestions(ModelRc::new(VecModel::from(
        CONTRACT_TYPES
            .iter()
            .map(|kind| (*kind).into())
            .collect::<Vec<_>>(),
    )));
    let weak = ui.as_weak();
    ui.on_template_create(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_template_form_data(blank_form());
            ui.set_template_form_error("".into());
            ui.set_template_selected_file("".into());
            ui.set_template_mode("form".into());
        }
        with_state(|state| {
            state.editing_id = None;
            state.selected_file = None;
            state.busy = false;
        });
    });
    ui.on_template_open(|id| load_detail(i64::from(id)));
    ui.on_template_edit(|| {
        if let Some(id) = with_state(|s| s.detail_id).flatten() {
            open_edit_by_id(id);
        }
    });
    ui.on_template_edit_id(|id| open_edit_by_id(i64::from(id)));
    ui.on_template_detail_download(|| {
        if let Some(id) = with_state(|s| s.detail_id).flatten() {
            crate::screens::templates_list::download_template(id);
        }
    });
    ui.on_template_form_cancel(|| {
        if !busy()
            && let Some(ui) = main_ui()
        {
            ui.set_template_mode("list".into());
            ui.set_template_form_error("".into());
        }
    });
    ui.on_template_file_pick(|| {
        if busy() {
            return;
        }
        let Some(path) = files::pick_file(&[("Word", &["docx"])]) else {
            return;
        };
        let label = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        with_state(|state| state.selected_file = Some(path));
        if let Some(ui) = main_ui() {
            ui.set_template_selected_file(label.into());
        }
    });
    ui.on_template_form_save(save_form);
    ui.on_template_mapping_change(|placeholder, field| {
        mapping_select(placeholder.to_string(), field.to_string())
    });
    ui.on_template_mapping_custom(|placeholder, field| {
        mapping_custom(placeholder.to_string(), field.to_string())
    });
    ui.on_template_mapping_save(save_mapping);
    ui.on_template_detail_back(|| {
        if let Some(ui) = main_ui() {
            ui.set_template_mode("list".into());
        }
    });
    ui.on_template_reanalyze(confirm_reanalyze);
}

fn blank_form() -> TemplateFormData {
    TemplateFormData {
        id: "".into(),
        code: "".into(),
        name: "".into(),
        kind: "Hợp đồng".into(),
        group: "".into(),
        description: "".into(),
        filename: "".into(),
    }
}

fn open_edit_by_id(id: i64) {
    let Some(worker) = with_state(|state| {
        if state.busy {
            return None;
        }
        state.busy = true;
        Some(state.worker.clone())
    })
    .flatten() else {
        return;
    };
    worker.run(
        move |ctx| ctx.store().templates.get(&id).cloned(),
        move |template| {
            with_state(|state| state.busy = false);
            match template {
                Some(template) => {
                    let form = TemplateFormData {
                        id: template.id.to_string().into(),
                        code: template.code.into(),
                        name: template.name.into(),
                        kind: template.contract_type.into(),
                        group: template.product_group.unwrap_or_default().into(),
                        description: template.description.unwrap_or_default().into(),
                        filename: template.file_name.unwrap_or_default().into(),
                    };
                    with_state(|state| {
                        state.editing_id = Some(id);
                        state.selected_file = None;
                    });
                    if let Some(ui) = main_ui() {
                        ui.set_template_form_data(form);
                        ui.set_template_selected_file(
                            "File hiện tại được giữ nguyên khi sửa.".into(),
                        );
                        ui.set_template_form_error("".into());
                        ui.set_template_mode("form".into());
                    }
                }
                None => toast_from_worker(ToastKind::Danger, "Không tìm thấy template.".into()),
            }
        },
    );
}
fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|state| state.borrow_mut().as_mut().map(f))
}
fn busy() -> bool {
    with_state(|state| state.busy).unwrap_or(true)
}
fn main_ui() -> Option<MainWindow> {
    with_state(|state| state.ui.clone()).and_then(|weak| weak.upgrade())
}

fn save_form(form: TemplateFormData) {
    let Some((worker, id, path)) = with_state(|state| {
        if state.busy {
            return None;
        }
        if state.editing_id.is_none() && state.selected_file.is_none() {
            return None;
        }
        state.busy = true;
        Some((
            state.worker.clone(),
            state.editing_id,
            state.selected_file.clone(),
        ))
    })
    .flatten() else {
        if let Some(ui) = main_ui() {
            ui.set_template_form_error(
                if with_state(|state| state.selected_file.is_none() && state.editing_id.is_none())
                    .unwrap_or(false)
                {
                    "Vui lòng chọn file .docx.".into()
                } else {
                    "Đang xử lý yêu cầu trước đó.".into()
                },
            );
        }
        return;
    };
    let form_values = (
        form.code.to_string(),
        form.name.to_string(),
        form.kind.to_string(),
        form.group.to_string(),
        form.description.to_string(),
    );
    if let Some(ui) = main_ui() {
        ui.set_template_form_error("".into());
    }
    worker.run(
        move |ctx| {
            let (code, name, kind, group, description) = form_values;
            let id = if let Some(id) = id {
                update_metadata(ctx, id, &name, &kind, &description, &group)?;
                id
            } else {
                let path =
                    path.ok_or_else(|| ServiceError::Invalid("Vui lòng chọn file .docx.".into()))?;
                let bytes = std::fs::read(&path).map_err(|error| {
                    ServiceError::Invalid(format!("Không đọc được file: {error}"))
                })?;
                let filename = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_owned();
                create_template(
                    ctx,
                    TemplateDraft {
                        code: &code,
                        name: &name,
                        contract_type: &kind,
                        file_bytes: &bytes,
                        original_filename: &filename,
                        description: &description,
                        mode: "placeholder",
                        product_group: &group,
                    },
                )?
            };
            Ok::<_, ServiceError>(id)
        },
        move |result| {
            with_state(|state| state.busy = false);
            match result {
                Ok(id) => {
                    crate::screens::templates_list::reload();
                    toast_from_worker(ToastKind::Success, "Đã lưu template.".into());
                    load_detail(id);
                }
                Err(error) => {
                    if let Some(ui) = main_ui() {
                        ui.set_template_form_error(error.user_message().into());
                    }
                }
            }
        },
    );
}

fn load_detail(id: i64) {
    let Some(worker) = with_state(|state| {
        if state.busy {
            return None;
        }
        state.busy = true;
        state.detail_id = Some(id);
        Some(state.worker.clone())
    })
    .flatten() else {
        return;
    };
    worker.run(
        move |ctx| {
            let template = ctx.store().templates.get(&id).cloned().ok_or_else(|| {
                ServiceError::NotFound(format!("Không tìm thấy template id={id}"))
            })?;
            let analysis = inspect_analysis(ctx, id);
            let (summary, flags, legacy) = match analysis {
                Ok(analysis) => {
                    let summary = format!(
                        "{} placeholder; {} nhóm lặp; {} khóa tùy chọn.",
                        analysis.placeholders.len(),
                        analysis.repeat_groups.len(),
                        analysis.optional_keys.len()
                    );
                    let mut flags = Vec::new();
                    if !analysis.repeat_groups.is_empty() {
                        flags.push(format!("Nhóm lặp: {}", analysis.repeat_groups.join(", ")));
                    }
                    if !analysis.row_warnings.is_empty() {
                        flags.push(format!(
                            "Cảnh báo dòng: {}",
                            analysis.row_warnings.join("; ")
                        ));
                    }
                    if !analysis.split_run_warnings.is_empty() {
                        flags.push(format!(
                            "Cảnh báo tách run: {}",
                            analysis.split_run_warnings.join("; ")
                        ));
                    }
                    flags.push(format!(
                        "Nguồn: {}",
                        analysis
                            .source_counts
                            .iter()
                            .map(|(key, count)| format!("{key}: {count}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    (summary, flags.join("\n"), analysis.uses_legacy_party_a)
                }
                Err(error) => (
                    "Không đọc được phân tích mới từ file.".into(),
                    error.user_message(),
                    false,
                ),
            };
            let rows = template
                .detected_placeholders
                .iter()
                .map(|placeholder| {
                    let field = template
                        .field_mapping
                        .get(placeholder)
                        .cloned()
                        .unwrap_or_default();
                    let custom =
                        !field.is_empty() && !ALL_CANONICAL_FIELDS.contains(&field.as_str());
                    TemplateMappingRowData {
                        placeholder: placeholder.clone().into(),
                        field: field.into(),
                        custom,
                    }
                })
                .collect::<Vec<_>>();
            Ok::<_, ServiceError>(Detail {
                id,
                code: template.code,
                name: template.name,
                summary,
                flags,
                legacy,
                rows,
            })
        },
        |result| {
            with_state(|state| state.busy = false);
            match result {
                Ok(detail) => show_detail(detail),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
        },
    );
}

fn show_detail(detail: Detail) {
    let Some((weak, canonical)) = with_state(|state| {
        state.detail_id = Some(detail.id);
        state.mapping = detail
            .rows
            .iter()
            .map(|row| (row.placeholder.to_string(), row.field.to_string()))
            .collect();
        (state.ui.clone(), state.canonical.clone())
    }) else {
        return;
    };
    if let Some(ui) = weak.upgrade() {
        ui.set_template_detail_code(detail.code.into());
        ui.set_template_detail_name(detail.name.into());
        ui.set_template_detail_summary(detail.summary.into());
        ui.set_template_detail_flags(detail.flags.into());
        ui.set_template_detail_legacy(detail.legacy);
        ui.set_template_mapping_rows(ModelRc::new(VecModel::from(detail.rows)));
        ui.set_template_canonical_fields(ModelRc::new(VecModel::from(
            canonical.into_iter().map(Into::into).collect::<Vec<_>>(),
        )));
        ui.set_template_mode("detail".into());
    }
}

fn update_mapping_rows() {
    let Some((weak, rows)) = with_state(|state| {
        let rows = state
            .mapping
            .iter()
            .map(|(placeholder, field)| TemplateMappingRowData {
                placeholder: placeholder.clone().into(),
                field: field.clone().into(),
                custom: !field.is_empty() && !ALL_CANONICAL_FIELDS.contains(&field.as_str()),
            })
            .collect::<Vec<_>>();
        (state.ui.clone(), rows)
    }) else {
        return;
    };
    if let Some(ui) = weak.upgrade() {
        ui.set_template_mapping_rows(ModelRc::new(VecModel::from(rows)));
    }
}

fn mapping_select(placeholder: String, field: String) {
    if field == "Tự nhập" {
        return;
    }
    let val = if field == "— chưa map —" {
        String::new()
    } else {
        field
    };
    with_state(|state| {
        if state.mapping.contains_key(&placeholder) {
            state.mapping.insert(placeholder, val);
        }
    });
    update_mapping_rows();
}
fn mapping_custom(placeholder: String, field: String) {
    with_state(|state| {
        if state.mapping.contains_key(&placeholder) {
            state.mapping.insert(placeholder, field);
        }
    });
}

fn save_mapping() {
    let Some((worker, id, mapping)) = with_state(|state| {
        if state.busy {
            return None;
        }
        let id = state.detail_id?;
        state.busy = true;
        Some((state.worker.clone(), id, state.mapping.clone()))
    })
    .flatten() else {
        return;
    };
    worker.run(
        move |ctx| {
            let now = ctx.clock().now_utc();
            update_mapping(ctx.store_mut(), id, &mapping, now)
        },
        |result| {
            with_state(|state| state.busy = false);
            match result {
                Ok(()) => {
                    toast_from_worker(ToastKind::Success, "Đã lưu ánh xạ placeholder.".into())
                }
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
        },
    );
}

fn confirm_reanalyze() {
    if busy() {
        return;
    }
    let Some((feedback, worker, id)) = with_state(|state| {
        Some((
            Rc::clone(&state.feedback),
            state.worker.clone(),
            state.detail_id?,
        ))
    })
    .flatten() else {
        return;
    };
    feedback.confirm("Phân tích lại template", "Phân tích lại file và cập nhật danh sách placeholder? Mapping hiện có được giữ lại cho các khóa còn tồn tại.", move || {
        if with_state(|state| state.busy).unwrap_or(true) { return; }
        with_state(|state| state.busy = true);
        worker.run(move |ctx| reanalyze(ctx, id), move |result| {
            with_state(|state| state.busy = false);
            match result {
                Ok(analysis) => {
                    toast_from_worker(ToastKind::Success, format!("Phân tích lại thành công — {} placeholder", analysis.placeholders.len()));
                    load_detail(id); crate::screens::templates_list::reload();
                }
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
        });
    });
}

#[allow(dead_code)]
fn _selected_file_path() -> Option<PathBuf> {
    with_state(|state| state.selected_file.clone()).flatten()
}
