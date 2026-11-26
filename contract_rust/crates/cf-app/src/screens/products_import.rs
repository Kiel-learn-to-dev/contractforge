use std::{cell::RefCell, path::PathBuf};

use cf_services::{
    Ctx, ServiceError,
    products::{
        catalog_export::make_catalog_xlsx,
        catalog_import::{CatalogAction, CatalogRow, apply_catalog_rows, parse_catalog_xlsx},
    },
};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{
    MainWindow, ProductImportRowData, files,
    ui_helpers::{ToastKind, toast_from_worker},
    worker::Worker,
};

struct State {
    ui: slint::Weak<MainWindow>,
    worker: Worker<Ctx>,
    rows: Vec<CatalogRow>,
    busy: bool,
    generation: u64,
}

thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>) {
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
            rows: Vec::new(),
            busy: false,
            generation: 0,
        })
    });

    ui.on_product_import_open(|| {
        with_state(|state| state.rows.clear());
        if let Some(ui) = main_ui() {
            ui.set_product_import_visible(true);
            ui.set_product_import_rows(ModelRc::new(VecModel::from(
                Vec::<ProductImportRowData>::new(),
            )));
            ui.set_product_import_summary("Chưa chọn file.".into());
            ui.set_product_import_can_apply(false);
        }
    });
    ui.on_product_import_close(|| {
        let can_close = with_state(|state| !state.busy).unwrap_or(false);
        if can_close && let Some(ui) = main_ui() {
            ui.set_product_import_visible(false);
        }
    });
    ui.on_product_import_choose(choose_import);
    ui.on_product_import_template(|| export_file(false));
    ui.on_product_import_export(|| export_file(true));
    ui.on_product_import_apply(apply_preview);
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|state| state.borrow_mut().as_mut().map(f))
}
fn main_ui() -> Option<MainWindow> {
    with_state(|state| state.ui.clone()).and_then(|weak| weak.upgrade())
}

fn choose_import() {
    let Some(path) = files::pick_file(&[("Excel", &["xlsx"])]) else {
        return;
    };
    let Some((worker, generation)) = with_state(|state| {
        if state.busy {
            return None;
        }
        state.busy = true;
        state.generation += 1;
        Some((state.worker.clone(), state.generation))
    })
    .flatten() else {
        return;
    };
    if let Some(ui) = main_ui() {
        ui.set_product_import_busy(true);
        ui.set_product_import_can_apply(false);
    }
    worker.run(
        move |ctx| {
            let bytes = std::fs::read(path)
                .map_err(|error| ServiceError::Invalid(format!("Không đọc được file: {error}")))?;
            parse_catalog_xlsx(ctx.store(), &bytes)
        },
        move |result| {
            if !finish(generation) {
                return;
            }
            if let Some(ui) = main_ui() {
                ui.set_product_import_busy(false);
            }
            match result {
                Ok(rows) => show_preview(rows),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
        },
    );
}

fn show_preview(rows: Vec<CatalogRow>) {
    let created = rows
        .iter()
        .filter(|row| row.action == CatalogAction::Create)
        .count();
    let updated = rows
        .iter()
        .filter(|row| row.action == CatalogAction::Update)
        .count();
    let errors = rows
        .iter()
        .filter(|row| row.action == CatalogAction::Error)
        .count();
    let can_apply = !rows.is_empty() && errors == 0;
    let summary = format!(
        "{} dòng: {created} tạo, {updated} cập nhật, {errors} lỗi",
        rows.len()
    );
    let ui_rows = rows
        .iter()
        .map(|row| ProductImportRowData {
            row: i32::try_from(row.row).unwrap_or(i32::MAX),
            action: match row.action {
                CatalogAction::Create => "Tạo",
                CatalogAction::Update => "Cập nhật",
                CatalogAction::Error => "Lỗi",
            }
            .into(),
            code: row.code.clone().into(),
            name: row.name.clone().into(),
            error: row.error.clone().into(),
        })
        .collect::<Vec<_>>();
    with_state(|state| state.rows = rows);
    if let Some(ui) = main_ui() {
        ui.set_product_import_rows(ModelRc::new(VecModel::from(ui_rows)));
        ui.set_product_import_summary(summary.into());
        ui.set_product_import_can_apply(can_apply);
    }
}

fn apply_preview() {
    let Some((worker, rows, generation)) = with_state(|state| {
        if state.busy
            || state.rows.is_empty()
            || state
                .rows
                .iter()
                .any(|row| row.action == CatalogAction::Error)
        {
            return None;
        }
        state.busy = true;
        state.generation += 1;
        Some((state.worker.clone(), state.rows.clone(), state.generation))
    })
    .flatten() else {
        return;
    };
    if let Some(ui) = main_ui() {
        ui.set_product_import_busy(true);
    }
    worker.run(
        move |ctx| apply_catalog_rows(ctx, &rows),
        move |result| {
            if !finish(generation) {
                return;
            }
            if let Some(ui) = main_ui() {
                ui.set_product_import_busy(false);
            }
            match result {
                Ok((created, updated)) => {
                    with_state(|state| state.rows.clear());
                    if let Some(ui) = main_ui() {
                        ui.set_product_import_visible(false);
                        ui.set_product_import_rows(ModelRc::new(VecModel::from(Vec::<
                            ProductImportRowData,
                        >::new(
                        ))));
                    }
                    toast_from_worker(
                        ToastKind::Success,
                        format!("Đã tạo {created} sản phẩm, cập nhật {updated} sản phẩm."),
                    );
                    crate::screens::products::reload();
                }
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
        },
    );
}

fn export_file(with_data: bool) {
    let Some((worker, generation)) = with_state(|state| {
        if state.busy {
            return None;
        }
        state.busy = true;
        state.generation += 1;
        Some((state.worker.clone(), state.generation))
    })
    .flatten() else {
        return;
    };
    if let Some(ui) = main_ui() {
        ui.set_product_import_busy(true);
    }
    worker.run(
        move |ctx| make_catalog_xlsx(with_data.then_some(ctx.store()), with_data),
        move |result| {
            if !finish(generation) {
                return;
            }
            if let Some(ui) = main_ui() {
                ui.set_product_import_busy(false);
            }
            match result {
                Ok(bytes) => write_export(bytes, with_data),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
        },
    );
}

fn finish(generation: u64) -> bool {
    with_state(|state| {
        if state.generation != generation {
            return false;
        }
        state.busy = false;
        true
    })
    .unwrap_or(false)
}

fn write_export(bytes: Vec<u8>, with_data: bool) {
    let suggested = if with_data {
        "product_catalog.xlsx"
    } else {
        "product_template.xlsx"
    };
    let Some(path): Option<PathBuf> = files::save_as(suggested, ("Excel", &["xlsx"])) else {
        return;
    };
    match files::write_chosen(&path, &bytes) {
        Ok(()) => toast_from_worker(ToastKind::Success, format!("Đã lưu {}", path.display())),
        Err(error) => toast_from_worker(ToastKind::Danger, format!("Không lưu được file: {error}")),
    }
}
