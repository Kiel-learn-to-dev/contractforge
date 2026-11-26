use std::{cell::RefCell, collections::BTreeSet, path::PathBuf, rc::Rc};

use cf_db::{model::ContractTemplate, paths::resolve_stored};
use cf_services::{
    Ctx, ServiceError,
    templates::{
        bulk::bulk_upload, cleanup::cleanup_orphan_template_files, edit::toggle_active,
        list::list_templates,
    },
};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    MainWindow, TemplateRowData, files,
    ui_helpers::{Feedback, ToastKind, toast_from_worker},
    worker::Worker,
};

struct State {
    ui: slint::Weak<MainWindow>,
    worker: Worker<Ctx>,
    type_filter: String,
    group_filter: String,
    active_only: bool,
    all_types: Vec<String>,
    all_groups: Vec<String>,
    busy: bool,
}
struct Loaded {
    rows: Vec<TemplateRowData>,
    types: Vec<String>,
    groups: Vec<String>,
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>, feedback: &Rc<Feedback>) {
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
            type_filter: "Tất cả loại".into(),
            group_filter: "Tất cả nhóm".into(),
            active_only: false,
            all_types: Vec::new(),
            all_groups: Vec::new(),
            busy: false,
        })
    });
    ui.on_template_filter_changed(|kind, group, active| {
        with_state(|state| {
            state.type_filter = kind.to_string();
            state.group_filter = group.to_string();
            state.active_only = active;
        });
        reload();
    });
    ui.on_template_toggle(|id| toggle_template(i64::from(id)));
    ui.on_template_upload(upload_files);
    ui.on_template_download(|id| download_template(i64::from(id)));
    let confirm_feedback = Rc::clone(feedback);
    let worker = worker.clone();
    ui.on_template_cleanup(move || {
        if busy() {
            return;
        }
        let confirm_worker = worker.clone();
        confirm_feedback.confirm(
            "Dọn file mẫu thừa",
            "Xóa các file mẫu không còn được bản ghi nào sử dụng?",
            move || {
                set_busy(true);
                confirm_worker.run(
                    |ctx| cleanup_orphan_template_files(ctx),
                    |result| {
                        set_busy(false);
                        match result {
                            Ok(count) => {
                                set_message(format!("Đã xóa {count} file mẫu thừa."));
                                toast_from_worker(
                                    ToastKind::Success,
                                    format!("Đã xóa {count} file mẫu thừa."),
                                );
                                reload();
                            }
                            Err(error) => {
                                toast_from_worker(ToastKind::Danger, error.user_message())
                            }
                        }
                    },
                );
            },
        );
    });
    reload();
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|state| state.borrow_mut().as_mut().map(f))
}
fn busy() -> bool {
    with_state(|state| state.busy).unwrap_or(true)
}
fn set_busy(value: bool) {
    with_state(|state| state.busy = value);
}
fn set_message(message: String) {
    if let Some(ui) = with_state(|state| state.ui.clone()).and_then(|weak| weak.upgrade()) {
        ui.set_template_operation_message(message.into());
    }
}

pub fn reload() {
    let Some((worker, kind, group, active)) = with_state(|state| {
        (
            state.worker.clone(),
            state.type_filter.clone(),
            state.group_filter.clone(),
            state.active_only,
        )
    }) else {
        return;
    };
    worker.run(
        move |ctx| {
            let templates = list_templates(ctx.store(), false);
            let types = templates
                .iter()
                .map(|template| template.contract_type.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let groups = templates
                .iter()
                .filter_map(|template| template.product_group.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let rows = templates
                .into_iter()
                .filter(|template| {
                    (!active || template.is_active)
                        && (kind == "Tất cả loại" || template.contract_type == kind)
                        && (group == "Tất cả nhóm"
                            || template.product_group.as_deref() == Some(group.as_str()))
                })
                .map(to_row)
                .collect();
            Loaded {
                rows,
                types,
                groups,
            }
        },
        apply_loaded,
    );
}

fn to_row(template: ContractTemplate) -> TemplateRowData {
    let mapped = template
        .detected_placeholders
        .iter()
        .filter(|placeholder| {
            template
                .field_mapping
                .get(*placeholder)
                .is_some_and(|value| !value.trim().is_empty())
        })
        .count();
    TemplateRowData {
        id: i32::try_from(template.id).unwrap_or(i32::MAX),
        code: template.code.into(),
        name: template.name.into(),
        kind: template.contract_type.into(),
        group: template
            .product_group
            .unwrap_or_else(|| "Mẫu chung".into())
            .into(),
        status: if template.is_active {
            "Đang bật"
        } else {
            "Đang tạm ngưng"
        }
        .into(),
        placeholders: i32::try_from(template.detected_placeholders.len()).unwrap_or(i32::MAX),
        mapped: i32::try_from(mapped).unwrap_or(i32::MAX),
    }
}

fn apply_loaded(loaded: Loaded) {
    let Some((weak, all_types, all_groups)) = with_state(|state| {
        state.all_types = loaded.types.clone();
        state.all_groups = loaded.groups.clone();
        (
            state.ui.clone(),
            state.all_types.clone(),
            state.all_groups.clone(),
        )
    }) else {
        return;
    };
    if let Some(ui) = weak.upgrade() {
        let mut types = vec!["Tất cả loại".to_owned()];
        types.extend(all_types);
        let mut groups = vec!["Tất cả nhóm".to_owned()];
        groups.extend(all_groups);
        ui.set_template_type_options(ModelRc::new(VecModel::from(
            types.into_iter().map(Into::into).collect::<Vec<_>>(),
        )));
        ui.set_template_group_options(ModelRc::new(VecModel::from(
            groups.into_iter().map(Into::into).collect::<Vec<_>>(),
        )));
        ui.set_template_rows(ModelRc::new(VecModel::from(loaded.rows)));
        let (kind, group, active) = with_state(|state| {
            (
                state.type_filter.clone(),
                state.group_filter.clone(),
                state.active_only,
            )
        })
        .unwrap_or_default();
        ui.set_template_type_filter(kind.into());
        ui.set_template_group_filter(group.into());
        ui.set_template_active_only(active);
    }
}

fn toggle_template(id: i64) {
    if busy() {
        return;
    }
    let Some(worker) = with_state(|state| state.worker.clone()) else {
        return;
    };
    set_busy(true);
    worker.run(
        move |ctx| {
            let code = ctx
                .store()
                .templates
                .get(&id)
                .map(|template| template.code.clone())
                .ok_or_else(|| {
                    ServiceError::NotFound(format!("Không tìm thấy template id={id}"))
                })?;
            let now = ctx.clock().now_utc();
            let active = toggle_active(ctx.store_mut(), id, now)?;
            Ok::<_, ServiceError>((code, active))
        },
        |result| {
            set_busy(false);
            match result {
                Ok((code, true)) => {
                    let message = format!("Đã kích hoạt template {code}");
                    set_message(message.clone());
                    toast_from_worker(ToastKind::Success, message);
                    reload();
                }
                Ok((code, false)) => {
                    let message = format!("Đã tạm ngưng template {code}");
                    set_message(message.clone());
                    toast_from_worker(ToastKind::Success, message);
                    reload();
                }
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
        },
    );
}

fn upload_files() {
    if busy() {
        return;
    }
    let paths = files::pick_files(&[("Word", &["docx"])]);
    if paths.is_empty() {
        return;
    }
    let Some(worker) = with_state(|state| state.worker.clone()) else {
        return;
    };
    set_busy(true);
    worker.run(
        move |ctx| bulk_upload(ctx, &paths),
        |results| {
            set_busy(false);
            let ok = results
                .iter()
                .filter(|result| result.error.is_none())
                .count();
            let failed = results.len() - ok;
            let details = results
                .iter()
                .filter_map(|result| {
                    result
                        .error
                        .as_ref()
                        .map(|error| format!("{}: {error}", result.filename))
                })
                .collect::<Vec<_>>();
            let message = if details.is_empty() {
                format!("Đã tải lên {ok} template.")
            } else {
                format!(
                    "Đã tải lên {ok} template, {failed} file lỗi: {}",
                    details.join("; ")
                )
            };
            set_message(message.clone());
            toast_from_worker(
                if failed == 0 {
                    ToastKind::Success
                } else {
                    ToastKind::Warning
                },
                message,
            );
            reload();
        },
    );
}

pub fn download_template(id: i64) {
    if busy() {
        return;
    }
    let Some(worker) = with_state(|state| state.worker.clone()) else {
        return;
    };
    set_busy(true);
    worker.run(
        move |ctx| {
            let template = ctx.store().templates.get(&id).ok_or_else(|| {
                ServiceError::NotFound(format!("Không tìm thấy template id={id}"))
            })?;
            let raw = template
                .file_path
                .as_deref()
                .ok_or_else(|| ServiceError::FileMissing("File không tồn tại.".into()))?;
            let path = resolve_stored(&ctx.paths().root, raw);
            let bytes = std::fs::read(&path).map_err(|_| {
                ServiceError::FileMissing(format!("File không tồn tại: {}", path.display()))
            })?;
            let name = template
                .file_name
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("template.docx")
                        .to_owned()
                });
            Ok::<_, ServiceError>((name, bytes))
        },
        |result| {
            set_busy(false);
            match result {
                Ok((name, bytes)) => write_download(name, bytes),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
        },
    );
}

fn write_download(name: String, bytes: Vec<u8>) {
    let Some(path): Option<PathBuf> = files::save_as(&name, ("Word", &["docx"])) else {
        return;
    };
    match files::write_chosen(&path, &bytes) {
        Ok(()) => toast_from_worker(ToastKind::Success, format!("Đã lưu {}", path.display())),
        Err(error) => toast_from_worker(ToastKind::Danger, format!("Không lưu được file: {error}")),
    }
}
