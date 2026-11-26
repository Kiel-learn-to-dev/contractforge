//! S13 — danh sách hợp đồng: bộ lọc, bảng, chọn nhiều, đổi trạng thái hàng loạt, xuất Excel.
//! Trạng thái màn hình chỉ sống trên luồng UI; việc đọc/ghi Store chạy trên worker.

use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use cf_core::lifecycle::{self, ContractStatus};
use cf_services::{
    Ctx, ServiceError,
    contracts::{
        delete::bulk_delete,
        export::export_contracts,
        list::{ContractFilters, ListResult, list_contracts},
        status::bulk_change_status,
    },
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::{
    ContractHub, ContractRowData, MainWindow, files, nav,
    ui_helpers::{Feedback, ToastKind, post_to_ui, toast_from_worker},
    view,
    worker::Worker,
};

const PAGE_SIZE: usize = 25;
const ALL_STATUSES: &str = "Tất cả trạng thái";
const ALL_YEARS: &str = "Tất cả năm";
const ALL_CUSTOMERS: &str = "Tất cả khách hàng";
const ALL_PRODUCTS: &str = "Tất cả sản phẩm";
const DATE_FIELDS: [(&str, &str); 2] = [("Ngày ký", "sign_date"), ("Ngày hết hạn", "end_date")];

/// Bộ lọc cảnh báo: nhãn hiển thị và giá trị mà `list_contracts` hiểu.
const ALERT_FILTERS: [(&str, &str); 5] = [
    ("Mọi cảnh báo", ""),
    ("Chưa gửi quá 10 ngày", "unsent_10d"),
    ("Sắp hết hạn trong 60 ngày", "expiring_60"),
    ("Đã sinh file", "generated"),
    ("Đang hiệu lực", "all_active"),
];

/// Một dòng đã chuyển sang chuỗi, không còn tham chiếu tới Store.
#[derive(Debug, Clone)]
struct RowSnapshot {
    id: i64,
    number: String,
    customer: String,
    status_label: String,
    status_variant: &'static str,
    total: Option<Decimal>,
    sign_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    flag_unsent: bool,
    flag_expiring: bool,
}

/// Kết quả một lần tải, gửi từ worker về UI.
#[derive(Debug)]
struct Loaded {
    rows: Vec<RowSnapshot>,
    total: usize,
    sum_total: Decimal,
    years: Vec<i32>,
    cnt_unsent: usize,
    cnt_expiring: usize,
    customers: Vec<(i64, String)>,
    products: Vec<(i64, String)>,
}

struct Screen {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    feedback: Rc<Feedback>,
    filters: ContractFilters,
    selected: BTreeSet<i64>,
    rows: Vec<RowSnapshot>,
    total: usize,
    sum_total: Decimal,
    years: Vec<i32>,
    cnt_unsent: usize,
    cnt_expiring: usize,
    statuses: Vec<ContractStatus>,
    customers: Vec<(i64, String)>,
    products: Vec<(i64, String)>,
    request_id: u64,
    busy: bool,
}

thread_local! {
    static SCREEN: RefCell<Option<Screen>> = const { RefCell::new(None) };
}

/// Gắn màn hình hợp đồng vào cửa sổ và tải dữ liệu lần đầu.
pub fn install(ui: &MainWindow, worker: &Worker<Ctx>, feedback: &Rc<Feedback>) {
    SCREEN.with(|cell| {
        *cell.borrow_mut() = Some(Screen {
            ui: ui.as_weak(),
            worker: worker.clone(),
            feedback: Rc::clone(feedback),
            filters: ContractFilters::default(),
            selected: BTreeSet::new(),
            rows: Vec::new(),
            total: 0,
            sum_total: Decimal::ZERO,
            years: Vec::new(),
            cnt_unsent: 0,
            cnt_expiring: 0,
            statuses: lifecycle::SELECTABLE.to_vec(),
            customers: Vec::new(),
            products: Vec::new(),
            request_id: 0,
            busy: false,
        });
    });

    ui.on_contracts_search(|text| {
        let changed = with_screen(|screen| {
            screen.filters.q = text.to_string();
            screen.filters.page = 1;
            screen.selected.clear();
        });
        if changed.is_some() {
            reload();
        }
    });

    ui.on_contracts_filter(
        |status_index,
         year_index,
         alert_index,
         customer_index,
         product_index,
         date_field_index,
         date_from,
         date_to| {
            let changed = with_screen(|screen| {
                screen.filters.status = status_code_at(&screen.statuses, status_index);
                screen.filters.year = year_at(&screen.years, year_index);
                screen.filters.alert_filter = ALERT_FILTERS
                    .get(usize::try_from(alert_index).unwrap_or(0))
                    .map_or_else(String::new, |(_, value)| (*value).to_owned());
                screen.filters.customer_id = id_at(&screen.customers, customer_index);
                screen.filters.product_id = id_at(&screen.products, product_index);
                screen.filters.date_field = DATE_FIELDS
                    .get(usize::try_from(date_field_index).unwrap_or(0))
                    .map_or("sign_date", |(_, value)| *value)
                    .into();
                screen.filters.date_from = parse_filter_date(&date_from);
                screen.filters.date_to = parse_filter_date(&date_to);
                screen.filters.page = 1;
                screen.selected.clear();
            });
            if changed.is_some() {
                reload();
            }
        },
    );

    ui.on_contracts_sort(|field| {
        let changed = with_screen(|screen| {
            if screen.filters.sort_by == field.as_str() {
                screen.filters.sort_dir = if screen.filters.sort_dir == "asc" {
                    "desc"
                } else {
                    "asc"
                }
                .into();
            } else {
                screen.filters.sort_by = field.to_string();
                screen.filters.sort_dir = "asc".into();
            }
            screen.filters.page = 1;
            screen.selected.clear();
        });
        if changed.is_some() {
            reload();
        }
    });
    ui.on_contracts_alert_unsent(|| set_alert_filter("unsent_10d"));
    ui.on_contracts_alert_expiring(|| set_alert_filter("expiring_60"));

    ui.on_contracts_toggle(|index| {
        let toggled = with_screen(|screen| {
            let id = usize::try_from(index)
                .ok()
                .and_then(|index| screen.rows.get(index))
                .map(|row| row.id)?;
            if !screen.selected.remove(&id) {
                screen.selected.insert(id);
            }
            Some(())
        });
        if toggled.flatten().is_some() {
            push_to_ui();
        }
    });

    ui.on_contracts_prev(|| {
        let moved = with_screen(|screen| {
            if screen.filters.page > 1 {
                screen.filters.page -= 1;
                true
            } else {
                false
            }
        });
        if moved == Some(true) {
            reload();
        }
    });

    ui.on_contracts_next(|| {
        let moved = with_screen(|screen| {
            if screen.filters.page < page_count(screen.total) {
                screen.filters.page += 1;
                true
            } else {
                false
            }
        });
        if moved == Some(true) {
            reload();
        }
    });

    ui.on_contracts_bulk_status(ask_bulk_status);
    ui.on_contracts_bulk_delete(ask_bulk_delete);
    ui.on_contracts_export(export);
    ui.on_contracts_reload(reload);

    let hub = ui.global::<ContractHub>();
    hub.on_list_toggle_all(|| {
        let changed = with_screen(|screen| {
            let all_checked = !screen.rows.is_empty()
                && screen.rows.iter().all(|r| screen.selected.contains(&r.id));
            if all_checked {
                for r in &screen.rows {
                    screen.selected.remove(&r.id);
                }
            } else {
                for r in &screen.rows {
                    screen.selected.insert(r.id);
                }
            }
        });
        if changed.is_some() {
            push_to_ui();
        }
    });
    hub.on_list_reset_filters(|| {
        let weak = with_screen(|screen| {
            screen.filters = ContractFilters::default();
            screen.selected.clear();
            screen.ui.clone()
        });
        if let Some(Some(ui)) = weak.map(|w| w.upgrade()) {
            ui.global::<ContractHub>().set_list_search_text("".into());
            ui.set_contract_list_date_from("".into());
            ui.set_contract_list_date_to("".into());
        }
        reload();
    });
    hub.on_list_create_new(|| {
        if let Some(Some(ui)) = with_screen(|s| s.ui.clone()).map(|w| w.upgrade()) {
            ui.invoke_navigate(nav::Screen::Generate.index());
        }
    });

    reload();
}

/// Đọc hoặc sửa trạng thái màn hình. Trả None nếu màn hình chưa được gắn.
fn with_screen<R>(action: impl FnOnce(&mut Screen) -> R) -> Option<R> {
    SCREEN.with(|cell| cell.borrow_mut().as_mut().map(action))
}

fn page_count(total: usize) -> usize {
    total.div_ceil(PAGE_SIZE).max(1)
}

fn parse_filter_date(raw: &str) -> Option<NaiveDate> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()
}

fn id_at(options: &[(i64, String)], index: i32) -> Option<i64> {
    usize::try_from(index)
        .ok()
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| options.get(index))
        .map(|(id, _)| *id)
}

fn set_alert_filter(filter: &str) {
    with_screen(|screen| {
        screen.filters.alert_filter = filter.into();
        screen.filters.page = 1;
        screen.selected.clear();
    });
    reload();
}

/// Mở danh sách hợp đồng với bộ lọc trạng thái đặt sẵn (dùng từ Dashboard).
pub fn open_with_status(status: &str) {
    let target_index = with_screen(|screen| {
        screen.filters = ContractFilters {
            status: status.to_owned(),
            ..ContractFilters::default()
        };
        screen.selected.clear();
        let idx = screen
            .statuses
            .iter()
            .position(|s| s.as_str() == status)
            .map(|p| (p + 1) as i32)
            .unwrap_or(0);
        (screen.ui.clone(), idx)
    });
    if let Some((ui, idx)) = target_index {
        if let Some(ui) = ui.upgrade() {
            ui.set_contract_status_index(idx);
            ui.set_contract_year_index(0);
            ui.set_contract_alert_index(0);
            ui.set_contract_list_customer_index(0);
            ui.set_contract_list_product_index(0);
            ui.set_contract_list_date_from("".into());
            ui.set_contract_list_date_to("".into());
        }
        reload();
    }
}

/// Mở danh sách hợp đồng với bộ lọc cảnh báo đặt sẵn (dùng từ Dashboard).
pub fn open_with_alert(alert: &str) {
    let target_index = with_screen(|screen| {
        screen.filters = ContractFilters {
            alert_filter: alert.to_owned(),
            ..ContractFilters::default()
        };
        screen.selected.clear();
        let idx = ALERT_FILTERS
            .iter()
            .position(|(_, code)| *code == alert)
            .map(|p| p as i32)
            .unwrap_or(0);
        (screen.ui.clone(), idx)
    });
    if let Some((ui, idx)) = target_index {
        if let Some(ui) = ui.upgrade() {
            ui.set_contract_status_index(0);
            ui.set_contract_year_index(0);
            ui.set_contract_alert_index(idx);
            ui.set_contract_list_customer_index(0);
            ui.set_contract_list_product_index(0);
            ui.set_contract_list_date_from("".into());
            ui.set_contract_list_date_to("".into());
        }
        reload();
    }
}

/// Mã trạng thái theo vị trí trong ComboBox; 0 nghĩa là không lọc theo trạng thái.
fn status_code_at(statuses: &[ContractStatus], index: i32) -> String {
    usize::try_from(index)
        .ok()
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| statuses.get(index))
        .map(|status| status.as_str().to_owned())
        .unwrap_or_default()
}

/// Năm theo vị trí trong ComboBox; 0 nghĩa là mọi năm.
fn year_at(years: &[i32], index: i32) -> i32 {
    usize::try_from(index)
        .ok()
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| years.get(index))
        .copied()
        .unwrap_or(0)
}

/// Tải lại danh sách trên worker rồi cập nhật UI.
pub fn reload() {
    let started = std::time::Instant::now();
    let Some((worker, filters, request_id)) = with_screen(|screen| {
        screen.request_id = screen.request_id.wrapping_add(1);
        (
            screen.worker.clone(),
            screen.filters.clone(),
            screen.request_id,
        )
    }) else {
        return;
    };
    worker.run(
        move |ctx| load(ctx, &filters),
        move |result| post_to_ui(move || apply(request_id, result, started)),
    );
}

/// Chạy trên worker: lọc, phân trang và tổng hợp.
fn load(ctx: &mut Ctx, filters: &ContractFilters) -> Result<Loaded, String> {
    let today = ctx.clock().today();
    let ListResult {
        rows,
        total,
        sum_total,
        years,
        cnt_unsent,
        cnt_expiring,
    } = list_contracts(ctx.store(), filters, today);
    let customers = ctx
        .store()
        .customers
        .values()
        .filter(|customer| !customer.is_deleted)
        .map(|customer| {
            (
                customer.id,
                customer
                    .short_name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .unwrap_or(&customer.legal_name)
                    .to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let mut customers = customers;
    customers.sort_by(|a, b| a.1.cmp(&b.1));
    let mut products = ctx
        .store()
        .products
        .values()
        .map(|product| (product.id, product.name.clone()))
        .collect::<Vec<_>>();
    products.sort_by(|a, b| a.1.cmp(&b.1));
    let rows = rows
        .into_iter()
        .map(|row| {
            let status = &row.contract.status;
            RowSnapshot {
                id: row.contract.id,
                number: row.contract.contract_number,
                customer: row.customer_name,
                status_label: status.label().to_owned(),
                status_variant: view::variant_of(status.color()),
                total: row.contract.total_amount,
                sign_date: row.contract.sign_date,
                end_date: row.contract.end_date,
                flag_unsent: row.flag_unsent,
                flag_expiring: row.flag_expiring,
            }
        })
        .collect();
    Ok(Loaded {
        rows,
        total,
        sum_total,
        years,
        cnt_unsent,
        cnt_expiring,
        customers,
        products,
    })
}

/// Trên luồng UI: nhận kết quả, cập nhật trạng thái rồi vẽ lại.
fn apply(request_id: u64, result: Result<Loaded, String>, started: std::time::Instant) {
    if with_screen(|screen| screen.request_id != request_id).unwrap_or(true) {
        return;
    }
    let mut reload_again = false;
    let failure = with_screen(|screen| match result {
        Ok(loaded) => {
            screen.rows = loaded.rows;
            screen.total = loaded.total;
            screen.sum_total = loaded.sum_total;
            screen.years = loaded.years;
            screen.cnt_unsent = loaded.cnt_unsent;
            screen.cnt_expiring = loaded.cnt_expiring;
            screen.customers = loaded.customers;
            screen.products = loaded.products;
            let pages = page_count(screen.total);
            if screen.filters.page > pages {
                screen.filters.page = pages;
                reload_again = true;
            }
            None
        }
        Err(message) => Some(message),
    })
    .flatten();
    if let Some(message) = failure {
        toast_from_worker(ToastKind::Danger, message);
    }
    if reload_again {
        reload();
    }
    push_to_ui();
    tracing::info!(
        "cập nhật danh sách hợp đồng trong {} ms",
        started.elapsed().as_millis()
    );
}

/// Hỏi xác nhận trước khi đổi trạng thái các hợp đồng đã chọn.
fn ask_bulk_status(target_index: i32) {
    let Some((ids, target, feedback)) = with_screen(|screen| {
        if screen.busy {
            return None;
        }
        let ids: Vec<i64> = screen.selected.iter().copied().collect();
        let target = screen
            .statuses
            .get(usize::try_from(target_index).ok()?)
            .cloned();
        Some((ids, target, Rc::clone(&screen.feedback)))
    })
    .flatten() else {
        return;
    };
    let Some(target) = target else {
        return;
    };
    if ids.is_empty() {
        toast_from_worker(ToastKind::Warning, "Chưa chọn hợp đồng nào".to_owned());
        return;
    }
    let message = format!(
        "Đổi trạng thái {} hợp đồng đã chọn thành \"{}\"? Nếu có một dòng không hợp lệ thì không đổi dòng nào.",
        ids.len(),
        target.label()
    );
    let code = target.as_str().to_owned();
    feedback.confirm("Đổi trạng thái hàng loạt", &message, move || {
        run_bulk_status(ids, code)
    });
}

fn ask_bulk_delete() {
    let Some((ids, feedback)) = with_screen(|screen| {
        if screen.busy {
            return None;
        }
        Some((
            screen.selected.iter().copied().collect::<Vec<_>>(),
            Rc::clone(&screen.feedback),
        ))
    })
    .flatten() else {
        return;
    };
    if ids.is_empty() {
        toast_from_worker(ToastKind::Warning, "Chưa chọn hợp đồng nào".to_owned());
        return;
    }
    let message = format!(
        "Xóa {} hợp đồng đã chọn? Không thể hoàn tác. Nếu có một hợp đồng không thể xóa thì không hợp đồng nào bị xóa.",
        ids.len()
    );
    feedback.confirm("Xóa hợp đồng hàng loạt", &message, move || {
        run_bulk_delete(ids)
    });
}

fn run_bulk_status(ids: Vec<i64>, code: String) {
    let Some((worker, weak)) = with_screen(|screen| {
        if screen.busy {
            return None;
        }
        screen.busy = true;
        screen.selected.clear();
        Some((screen.worker.clone(), screen.ui.clone()))
    })
    .flatten() else {
        return;
    };
    worker.run(
        move |ctx| bulk_change_status(ctx, &ids, &code),
        move |result: Result<usize, ServiceError>| {
            with_screen(|screen| screen.busy = false);
            match result {
                Ok(count) => toast_from_worker(
                    ToastKind::Success,
                    format!("Đã đổi trạng thái {count} hợp đồng."),
                ),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
            post_to_ui(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_contracts_reload();
                }
            });
        },
    );
}

fn run_bulk_delete(ids: Vec<i64>) {
    let Some((worker, weak)) = with_screen(|screen| {
        if screen.busy {
            return None;
        }
        screen.busy = true;
        screen.selected.clear();
        Some((screen.worker.clone(), screen.ui.clone()))
    })
    .flatten() else {
        return;
    };
    worker.run(
        move |ctx| bulk_delete(ctx, &ids),
        move |result| {
            with_screen(|screen| screen.busy = false);
            match result {
                Ok(count) => {
                    toast_from_worker(ToastKind::Success, format!("Đã xóa {count} hợp đồng."))
                }
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
            post_to_ui(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_contracts_reload();
                }
            });
        },
    );
}

/// Xuất toàn bộ hợp đồng khớp bộ lọc. Hộp thoại Lưu mở trên luồng UI sau khi worker xong.
fn export() {
    let Some((worker, filters)) =
        with_screen(|screen| (screen.worker.clone(), screen.filters.clone()))
    else {
        return;
    };
    worker.run(
        move |ctx| {
            let today = ctx.clock().today();
            export_contracts(ctx.store(), &filters, today)
        },
        move |result| {
            post_to_ui(move || match result {
                Ok(export) => save_export(&export.file_name, &export.bytes),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            });
        },
    );
}

fn save_export(suggested: &str, bytes: &[u8]) {
    let Some(path) = files::save_as(suggested, ("Excel", &["xlsx"])) else {
        return;
    };
    match files::write_chosen(&path, bytes) {
        Ok(()) => toast_from_worker(ToastKind::Success, format!("Đã xuất {}", path.display())),
        Err(error) => toast_from_worker(ToastKind::Danger, format!("Không ghi được file: {error}")),
    }
}

/// Vẽ lại bảng, bộ lọc và chân trang lên cửa sổ.
fn push_to_ui() {
    let Some(snapshot) = with_screen(|screen| {
        let rows: Vec<ContractRowData> = screen
            .rows
            .iter()
            .map(|row| ContractRowData {
                id: i32::try_from(row.id).unwrap_or(i32::MAX),
                number: SharedString::from(row.number.as_str()),
                customer: SharedString::from(row.customer.as_str()),
                status_label: SharedString::from(row.status_label.as_str()),
                status_variant: SharedString::from(row.status_variant),
                amount: SharedString::from(view::money_text(row.total).as_str()),
                sign_date: SharedString::from(view::date_text(row.sign_date).as_str()),
                end_date: SharedString::from(view::date_text(row.end_date).as_str()),
                flags: SharedString::from(flags_text(row).as_str()),
                selected: screen.selected.contains(&row.id),
            })
            .collect();
        let mut statuses = vec![SharedString::from(ALL_STATUSES)];
        statuses.extend(
            screen
                .statuses
                .iter()
                .map(|status| SharedString::from(status.label())),
        );
        let targets: Vec<SharedString> = screen
            .statuses
            .iter()
            .map(|status| SharedString::from(status.label()))
            .collect();
        let mut years = vec![SharedString::from(ALL_YEARS)];
        years.extend(
            screen
                .years
                .iter()
                .map(|year| SharedString::from(year.to_string().as_str())),
        );
        let mut customers = vec![SharedString::from(ALL_CUSTOMERS)];
        customers.extend(
            screen
                .customers
                .iter()
                .map(|(_, name)| SharedString::from(name.as_str())),
        );
        let mut products = vec![SharedString::from(ALL_PRODUCTS)];
        products.extend(
            screen
                .products
                .iter()
                .map(|(_, name)| SharedString::from(name.as_str())),
        );
        let date_fields = DATE_FIELDS
            .iter()
            .map(|(label, _)| SharedString::from(*label))
            .collect::<Vec<_>>();
        let pages = page_count(screen.total);
        let all_selected =
            !rows.is_empty() && screen.rows.iter().all(|r| screen.selected.contains(&r.id));
        Pushed {
            ui: screen.ui.clone(),
            rows,
            statuses,
            targets,
            years,
            customers,
            products,
            date_fields,
            cnt_unsent: screen.cnt_unsent as i32,
            cnt_expiring: screen.cnt_expiring as i32,
            all_selected,
            status_index: status_index_of(&screen.statuses, &screen.filters.status),
            year_index: screen
                .years
                .iter()
                .position(|year| *year == screen.filters.year)
                .and_then(|position| i32::try_from(position + 1).ok())
                .unwrap_or(0),
            alert_index: ALERT_FILTERS
                .iter()
                .position(|(_, value)| *value == screen.filters.alert_filter)
                .and_then(|position| i32::try_from(position).ok())
                .unwrap_or(0),
            customer_index: screen
                .filters
                .customer_id
                .and_then(|id| {
                    screen
                        .customers
                        .iter()
                        .position(|(candidate, _)| *candidate == id)
                })
                .and_then(|position| i32::try_from(position + 1).ok())
                .unwrap_or(0),
            product_index: screen
                .filters
                .product_id
                .and_then(|id| {
                    screen
                        .products
                        .iter()
                        .position(|(candidate, _)| *candidate == id)
                })
                .and_then(|position| i32::try_from(position + 1).ok())
                .unwrap_or(0),
            date_field_index: DATE_FIELDS
                .iter()
                .position(|(_, value)| *value == screen.filters.date_field)
                .and_then(|position| i32::try_from(position).ok())
                .unwrap_or(0),
            date_from: screen
                .filters
                .date_from
                .map(|date| date.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
            date_to: screen
                .filters
                .date_to
                .map(|date| date.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
            summary: format!(
                "{} hợp đồng · tổng tiền {}",
                screen.total,
                view::money_text(Some(screen.sum_total))
            ),
            page_text: format!("Trang {}/{}", screen.filters.page, pages),
            has_prev: screen.filters.page > 1,
            has_next: screen.filters.page < pages,
            selected_count: i32::try_from(screen.selected.len()).unwrap_or(i32::MAX),
        }
    }) else {
        return;
    };
    let Some(ui) = snapshot.ui.upgrade() else {
        return;
    };
    ui.set_contract_rows(ModelRc::new(VecModel::from(snapshot.rows)));
    ui.set_contract_status_options(ModelRc::new(VecModel::from(snapshot.statuses)));
    ui.set_contract_year_options(ModelRc::new(VecModel::from(snapshot.years)));
    ui.set_contract_list_customer_options(ModelRc::new(VecModel::from(snapshot.customers)));
    ui.set_contract_list_product_options(ModelRc::new(VecModel::from(snapshot.products)));
    ui.set_contract_list_date_field_options(ModelRc::new(VecModel::from(snapshot.date_fields)));
    ui.set_contract_target_options(ModelRc::new(VecModel::from(snapshot.targets)));
    ui.set_contract_alert_options(ModelRc::new(VecModel::from(
        ALERT_FILTERS
            .iter()
            .map(|(label, _)| SharedString::from(*label))
            .collect::<Vec<_>>(),
    )));
    ui.set_contract_status_index(snapshot.status_index);
    ui.set_contract_year_index(snapshot.year_index);
    ui.set_contract_alert_index(snapshot.alert_index);
    ui.set_contract_list_customer_index(snapshot.customer_index);
    ui.set_contract_list_product_index(snapshot.product_index);
    ui.set_contract_list_date_field_index(snapshot.date_field_index);
    ui.set_contract_list_date_from(snapshot.date_from.into());
    ui.set_contract_list_date_to(snapshot.date_to.into());
    ui.set_contract_summary_text(snapshot.summary.into());
    ui.set_contract_page_text(snapshot.page_text.into());
    ui.set_contract_has_prev(snapshot.has_prev);
    ui.set_contract_has_next(snapshot.has_next);
    ui.set_contract_selected_count(snapshot.selected_count);
    let hub = ui.global::<ContractHub>();
    hub.set_list_cnt_unsent(snapshot.cnt_unsent);
    hub.set_list_cnt_expiring(snapshot.cnt_expiring);
    hub.set_list_all_selected(snapshot.all_selected);
    if snapshot.selected_count == 0 {
        hub.set_list_target_index(-1);
    }
}

/// Dữ liệu đã dựng xong để gắn lên cửa sổ.
struct Pushed {
    ui: Weak<MainWindow>,
    rows: Vec<ContractRowData>,
    statuses: Vec<SharedString>,
    targets: Vec<SharedString>,
    years: Vec<SharedString>,
    customers: Vec<SharedString>,
    products: Vec<SharedString>,
    date_fields: Vec<SharedString>,
    cnt_unsent: i32,
    cnt_expiring: i32,
    all_selected: bool,
    status_index: i32,
    year_index: i32,
    alert_index: i32,
    customer_index: i32,
    product_index: i32,
    date_field_index: i32,
    date_from: String,
    date_to: String,
    summary: String,
    page_text: String,
    has_prev: bool,
    has_next: bool,
    selected_count: i32,
}

fn flags_text(row: &RowSnapshot) -> String {
    let mut flags = Vec::new();
    if row.flag_unsent {
        flags.push("Chưa gửi");
    }
    if row.flag_expiring {
        flags.push("Sắp hết hạn");
    }
    flags.join(" · ")
}

/// Vị trí (cộng 1, vì 0 là "Tất cả") của mã trạng thái đang lọc; không lọc thì 0.
fn status_index_of(statuses: &[ContractStatus], code: &str) -> i32 {
    if code.is_empty() {
        return 0;
    }
    statuses
        .iter()
        .position(|status| status.as_str() == code)
        .and_then(|position| i32::try_from(position + 1).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{Loaded, load};
    use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
    use cf_services::{Ctx, contracts::list::ContractFilters};
    use chrono::NaiveDate;

    fn seeded_ctx() -> (tempfile::TempDir, Ctx) {
        let dir = tempfile::tempdir().expect("temp dir");
        let db = cf_testkit::seeded_db(dir.path()).expect("seed database");
        let store = Store::load(db).expect("load database");
        let date = NaiveDate::from_ymd_opt(2026, 3, 24).expect("date");
        let now = date.and_hms_opt(9, 0, 0).expect("time");
        let clock = FixedClock {
            date,
            utc: now,
            local: now,
        };
        let root = dir.path().to_path_buf();
        (dir, Ctx::new(store, DataPaths::new(root), Box::new(clock)))
    }

    #[test]
    fn load_maps_rows_and_totals_from_the_store() {
        let (_dir, mut ctx) = seeded_ctx();
        let Loaded { rows, total, .. } = load(&mut ctx, &ContractFilters::default()).expect("load");
        assert!(total >= rows.len());
        for row in &rows {
            assert!(!row.number.is_empty());
        }
    }

    #[test]
    fn status_filter_returns_only_that_status() {
        let (_dir, mut ctx) = seeded_ctx();
        let filters = ContractFilters {
            status: "Draft".into(),
            ..ContractFilters::default()
        };
        let loaded = load(&mut ctx, &filters).expect("load");
        assert!(loaded.rows.iter().all(|row| row.status_label == "Nháp"));
    }
}
