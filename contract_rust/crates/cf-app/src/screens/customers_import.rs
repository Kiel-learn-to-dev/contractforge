//! S07 — nhập khách hàng từ Excel và xuất báo cáo 4 sheet.

use std::cell::RefCell;

use cf_services::{
    Ctx,
    customers::{
        export_list::make_customer_import_template_xlsx, import_flat::import_flat,
        import_grouped::import_grouped_from_excel, report::export_report,
    },
};
use slint::{ComponentHandle, ModelRc, VecModel, Weak};

use crate::{
    CustomerHub, CustomerImportRowData, MainWindow, files,
    ui_helpers::{ToastKind, post_to_ui, toast_from_worker},
    worker::Worker,
};

type ImportMessage = (usize, String, bool);
type ImportResult = Result<(String, Vec<ImportMessage>), String>;

struct State {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn install(ui: &MainWindow, worker: &Worker<Ctx>) {
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            ui: ui.as_weak(),
            worker: worker.clone(),
        });
    });

    let hub = ui.global::<CustomerHub>();
    hub.on_open_import(|| {
        STATE.with(|c| {
            if let Some(ui) = c.borrow().as_ref().and_then(|s| s.ui.upgrade()) {
                let hub = ui.global::<CustomerHub>();
                hub.set_import_summary("".into());
                hub.set_import_filename("".into());
                hub.set_import_errors(ModelRc::new(VecModel::default()));
                hub.set_mode("import".into());
            }
        });
    });
    hub.on_import_download_template(download_template);
    hub.on_import_run(|mode, sheet| run_import(mode.as_str(), sheet.as_str()));
    hub.on_export_report(run_export_report);
}

fn download_template() {
    match make_customer_import_template_xlsx() {
        Ok(bytes) => {
            if let Some(path) = files::save_as("mau-nhap-khach-hang.xlsx", ("Excel", &["xlsx"])) {
                match files::write_chosen(&path, &bytes) {
                    Ok(()) => toast_from_worker(
                        ToastKind::Success,
                        format!("Đã lưu file mẫu {}", path.display()),
                    ),
                    Err(e) => {
                        toast_from_worker(ToastKind::Danger, format!("Không ghi được file: {e}"))
                    }
                }
            }
        }
        Err(e) => toast_from_worker(ToastKind::Danger, e.user_message()),
    }
}

fn run_import(mode: &str, sheet: &str) {
    let Some((worker, ui)) = STATE.with(|c| {
        c.borrow()
            .as_ref()
            .map(|s| (s.worker.clone(), s.ui.clone()))
    }) else {
        return;
    };
    let Some(path) = files::pick_file(&[("Excel (.xlsx, .xls)", &["xlsx", "xls"])]) else {
        return;
    };
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            toast_from_worker(ToastKind::Danger, format!("Không đọc được file: {e}"));
            return;
        }
    };
    let grouped = mode == "grouped";
    let sheet_opt = (!sheet.trim().is_empty()).then(|| sheet.trim().to_owned());

    worker.run(
        move |ctx| -> ImportResult {
            if grouped {
                let res = import_grouped_from_excel(ctx, &bytes, sheet_opt.as_deref())
                    .map_err(|e| e.user_message())?;
                let summary = format!(
                    "Đã xử lý {} trạm ({} điểm trạm): tạo mới {}, cập nhật {}, bỏ qua {}.",
                    res.total_customers, res.total_units, res.created, res.updated, res.skipped
                );
                let mut messages = res
                    .errors
                    .into_iter()
                    .map(|(r, m)| (r, m, false))
                    .collect::<Vec<_>>();
                messages.extend(res.warnings.into_iter().map(|(r, m)| (r, m, true)));
                Ok((summary, messages))
            } else {
                let res = import_flat(ctx, &bytes).map_err(|e| e.user_message())?;
                let summary = format!(
                    "Tổng {} dòng: tạo mới {}, cập nhật {}, bỏ qua {}.",
                    res.total_rows, res.created, res.updated, res.skipped
                );
                Ok((
                    summary,
                    res.errors.into_iter().map(|(r, m)| (r, m, false)).collect(),
                ))
            }
        },
        move |res| {
            post_to_ui(move || match res {
                Ok((summary, errors)) => {
                    let rows = errors
                        .into_iter()
                        .map(|(row, msg, is_warning)| CustomerImportRowData {
                            row: i32::try_from(row).unwrap_or(0),
                            message: msg.into(),
                            is_warning,
                        })
                        .collect::<Vec<_>>();
                    if let Some(ui) = ui.upgrade() {
                        let hub = ui.global::<CustomerHub>();
                        hub.set_import_filename(filename.into());
                        hub.set_import_summary(summary.clone().into());
                        hub.set_import_errors(ModelRc::new(VecModel::from(rows)));
                        ui.invoke_customer_reload();
                    }
                    toast_from_worker(ToastKind::Success, summary);
                }
                Err(e) => toast_from_worker(ToastKind::Danger, e),
            });
        },
    );
}

fn run_export_report() {
    let Some(worker) = STATE.with(|c| c.borrow().as_ref().map(|s| s.worker.clone())) else {
        return;
    };
    worker.run(
        |ctx| {
            let today = ctx.clock().now_local().date();
            let bytes = export_report(ctx.store(), today)?;
            Ok::<_, cf_services::ServiceError>((
                format!("bao-cao-khach-hang-{}.xlsx", today.format("%Y%m%d")),
                bytes,
            ))
        },
        |res| {
            post_to_ui(move || match res {
                Ok((suggested, bytes)) => {
                    if let Some(path) = files::save_as(&suggested, ("Excel", &["xlsx"])) {
                        match files::write_chosen(&path, &bytes) {
                            Ok(()) => toast_from_worker(
                                ToastKind::Success,
                                format!("Đã xuất báo cáo {}", path.display()),
                            ),
                            Err(e) => toast_from_worker(
                                ToastKind::Danger,
                                format!("Không ghi được file: {e}"),
                            ),
                        }
                    }
                }
                Err(e) => toast_from_worker(ToastKind::Danger, e.user_message()),
            });
        },
    );
}
