//! S03 — danh sách khách hàng. Trạng thái màn hình chỉ sống trên luồng UI; dữ liệu nặng đi qua worker.
//! Kết quả từ worker quay về UI bằng `post_to_ui`, chỉ mang theo dữ liệu thuần (Send).

use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use cf_services::{
    Ctx, ServiceError,
    customers::{
        list::{CustomerFilterOptions, CustomerFilters, filter_options, list_customers},
        state::bulk_delete_customers,
    },
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::{
    CustomerHub, CustomerRowData, MainWindow,
    ui_helpers::{Feedback, ToastKind, post_to_ui, toast_from_worker},
    worker::Worker,
};

const ALL_WARDS: &str = "Tất cả phường/xã";
const ALL_TYPES: &str = "Tất cả loại";

/// Một dòng đã chuyển sang chuỗi, không còn tham chiếu tới Store.
#[derive(Debug, Clone)]
struct RowSnapshot {
    id: i64,
    code: String,
    name: String,
    ward: String,
    customer_type: String,
    representative: String,
    units: usize,
    contracts: usize,
    active: bool,
}

/// Kết quả một lần tải, gửi từ worker về UI.
#[derive(Debug)]
struct Loaded {
    rows: Vec<RowSnapshot>,
    total: usize,
    options: CustomerFilterOptions,
}

struct Screen {
    ui: Weak<MainWindow>,
    worker: Worker<Ctx>,
    feedback: Rc<Feedback>,
    filters: CustomerFilters,
    selected: BTreeSet<i64>,
    rows: Vec<RowSnapshot>,
    total: usize,
    options: CustomerFilterOptions,
}

thread_local! {
    static SCREEN: RefCell<Option<Screen>> = const { RefCell::new(None) };
}

/// Gắn màn hình khách hàng vào cửa sổ. Gọi một lần khi khởi động, sau `Feedback::install`.
pub fn install(ui: &MainWindow, worker: &Worker<Ctx>, feedback: &Rc<Feedback>) {
    SCREEN.with(|cell| {
        *cell.borrow_mut() = Some(Screen {
            ui: ui.as_weak(),
            worker: worker.clone(),
            feedback: Rc::clone(feedback),
            filters: CustomerFilters::default(),
            selected: BTreeSet::new(),
            rows: Vec::new(),
            total: 0,
            options: CustomerFilterOptions {
                wards: Vec::new(),
                customer_types: Vec::new(),
            },
        });
    });

    ui.on_customer_search(|text| {
        let changed = with_screen(|screen| {
            screen.filters.q = text.to_string();
            screen.filters.page = 1;
            screen.selected.clear();
        });
        if changed.is_some() {
            request_load();
        }
    });

    ui.on_customer_filter(|ward_index, type_index, active_only| {
        let changed = with_screen(|screen| {
            screen.filters.ward = option_at(&screen.options.wards, ward_index);
            screen.filters.customer_type = option_at(&screen.options.customer_types, type_index);
            screen.filters.active_only = active_only;
            screen.filters.page = 1;
            screen.selected.clear();
        });
        if changed.is_some() {
            request_load();
        }
    });

    ui.on_customer_toggle(|index| {
        with_screen(|screen| {
            let Some(id) = usize::try_from(index)
                .ok()
                .and_then(|index| screen.rows.get(index))
                .map(|row| row.id)
            else {
                return;
            };
            if !screen.selected.remove(&id) {
                screen.selected.insert(id);
            }
        });
        push_to_ui();
    });

    ui.on_customer_prev(|| {
        let moved = with_screen(|screen| {
            if screen.filters.page > 1 {
                screen.filters.page -= 1;
                true
            } else {
                false
            }
        });
        if moved == Some(true) {
            request_load();
        }
    });

    ui.on_customer_next(|| {
        let moved = with_screen(|screen| {
            if screen.filters.page < page_count(screen.total, screen.filters.per_page) {
                screen.filters.page += 1;
                true
            } else {
                false
            }
        });
        if moved == Some(true) {
            request_load();
        }
    });

    ui.on_customer_delete(ask_delete_selected);

    ui.on_customer_reload(request_load);

    let hub = ui.global::<CustomerHub>();
    hub.on_list_toggle_all(|all| {
        with_screen(|screen| {
            if all {
                for row in &screen.rows {
                    screen.selected.insert(row.id);
                }
            } else {
                for row in &screen.rows {
                    screen.selected.remove(&row.id);
                }
            }
        });
        push_to_ui();
    });
    hub.on_list_reset_filters(|| {
        let ui_ref = with_screen(|screen| {
            screen.filters = CustomerFilters::default();
            screen.selected.clear();
            screen.ui.clone()
        });
        if let Some(ui) = ui_ref.and_then(|w| w.upgrade()) {
            ui.global::<CustomerHub>().set_list_search_text("".into());
        }
        request_load();
    });

    request_load();
}

/// Đọc hoặc sửa trạng thái màn hình. Trả None nếu màn hình chưa được gắn.
fn with_screen<R>(action: impl FnOnce(&mut Screen) -> R) -> Option<R> {
    SCREEN.with(|cell| cell.borrow_mut().as_mut().map(action))
}

/// Phần tử `index - 1` của danh sách lựa chọn; 0 nghĩa là "Tất cả", tức không lọc.
fn option_at(options: &[String], index: i32) -> String {
    usize::try_from(index)
        .ok()
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| options.get(index))
        .cloned()
        .unwrap_or_default()
}

fn page_count(total: usize, per_page: usize) -> usize {
    total.div_ceil(per_page.max(1)).max(1)
}

/// Tải lại danh sách trên worker rồi cập nhật UI.
pub fn request_load() {
    let Some((worker, filters)) =
        with_screen(|screen| (screen.worker.clone(), screen.filters.clone()))
    else {
        return;
    };
    worker.run(
        move |ctx| load(ctx, &filters),
        move |result| post_to_ui(move || apply(result)),
    );
}

/// Chạy trên worker: lọc, phân trang, và đếm.
fn load(ctx: &mut Ctx, filters: &CustomerFilters) -> Result<Loaded, String> {
    let (rows, total) =
        list_customers(ctx.store(), filters).map_err(|error| error.user_message())?;
    let rows = rows
        .into_iter()
        .map(|(customer, contracts)| {
            let units = usize::from(customer.bill_main_station)
                + ctx
                    .store()
                    .indexes
                    .units_of(ctx.store(), customer.id)
                    .into_iter()
                    .filter(|u| u.is_active && u.include_in_billing)
                    .count();
            RowSnapshot {
                id: customer.id,
                code: customer.code,
                name: customer.legal_name,
                ward: customer.ward.unwrap_or_default(),
                customer_type: customer.customer_type,
                representative: customer.representative_name.unwrap_or_default(),
                units,
                contracts,
                active: customer.is_active,
            }
        })
        .collect();
    Ok(Loaded {
        rows,
        total,
        options: filter_options(ctx.store()),
    })
}

/// Trên luồng UI: nhận kết quả tải, cập nhật trạng thái rồi vẽ lại.
fn apply(result: Result<Loaded, String>) {
    let mut reload_again = false;
    let outcome = with_screen(|screen| match result {
        Ok(loaded) => {
            screen.rows = loaded.rows;
            screen.total = loaded.total;
            screen.options = loaded.options;
            let pages = page_count(screen.total, screen.filters.per_page);
            if screen.filters.page > pages {
                screen.filters.page = pages;
                reload_again = true;
            }
            None
        }
        Err(message) => Some(message),
    });
    if let Some(Some(message)) = outcome {
        toast_from_worker(ToastKind::Danger, message);
    }
    if reload_again {
        request_load();
    }
    push_to_ui();
}

/// Trên luồng UI: hỏi xác nhận trước khi chuyển các khách đã chọn vào thùng rác.
fn ask_delete_selected() {
    let Some((ids, feedback)) = with_screen(|screen| {
        let ids: Vec<i64> = screen.selected.iter().copied().collect();
        (ids, Rc::clone(&screen.feedback))
    }) else {
        return;
    };
    if ids.is_empty() {
        toast_from_worker(ToastKind::Warning, "Chưa chọn khách hàng nào".to_owned());
        return;
    }
    let message = format!(
        "Chuyển {} khách hàng đã chọn vào thùng rác? Có thể khôi phục trong Thùng rác.",
        ids.len()
    );
    feedback.confirm("Xóa khách hàng", &message, move || run_delete(ids));
}

fn run_delete(ids: Vec<i64>) {
    let Some((worker, ui)) = with_screen(|screen| {
        screen.selected.clear();
        (screen.worker.clone(), screen.ui.clone())
    }) else {
        return;
    };
    worker.run(
        move |ctx| {
            let now = ctx.clock().now_utc();
            bulk_delete_customers(ctx.store_mut(), &ids, now)
        },
        move |result: Result<usize, ServiceError>| {
            match result {
                Ok(count) => toast_from_worker(
                    ToastKind::Warning,
                    format!(
                        "Đã chuyển {count} khách hàng vào thùng rác — khôi phục được trong Thùng rác"
                    ),
                ),
                Err(error) => toast_from_worker(ToastKind::Danger, error.user_message()),
            }
            post_to_ui(move || {
                if let Some(ui) = ui.upgrade() {
                    ui.invoke_customer_reload();
                }
            });
        },
    );
}

/// Vẽ lại mọi thuộc tính của màn hình lên cửa sổ.
fn push_to_ui() {
    let Some(snapshot) = with_screen(|screen| {
        let rows: Vec<CustomerRowData> = screen
            .rows
            .iter()
            .map(|row| CustomerRowData {
                id: i32::try_from(row.id).unwrap_or(i32::MAX),
                code: SharedString::from(row.code.as_str()),
                name: SharedString::from(row.name.as_str()),
                ward: SharedString::from(row.ward.as_str()),
                customer_type: SharedString::from(row.customer_type.as_str()),
                representative: SharedString::from(row.representative.as_str()),
                units: i32::try_from(row.units).unwrap_or(i32::MAX),
                contracts: i32::try_from(row.contracts).unwrap_or(i32::MAX),
                selected: screen.selected.contains(&row.id),
                status_label: SharedString::from(if row.active {
                    "Đang hoạt động"
                } else {
                    "Tạm ngưng"
                }),
                status_variant: SharedString::from(if row.active {
                    "success"
                } else {
                    "secondary"
                }),
            })
            .collect();
        let wards = labelled(ALL_WARDS, &screen.options.wards);
        let types = labelled(ALL_TYPES, &screen.options.customer_types);
        let pages = page_count(screen.total, screen.filters.per_page);
        Pushed {
            ui: screen.ui.clone(),
            rows,
            ward_index: index_of(&screen.options.wards, &screen.filters.ward),
            type_index: index_of(
                &screen.options.customer_types,
                &screen.filters.customer_type,
            ),
            wards,
            types,
            active_only: screen.filters.active_only,
            summary: format!("{} khách hàng", screen.total),
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
    let all_selected = !snapshot.rows.is_empty() && snapshot.rows.iter().all(|r| r.selected);
    ui.global::<CustomerHub>()
        .set_list_all_selected(all_selected);
    ui.set_customer_rows(ModelRc::new(VecModel::from(snapshot.rows)));
    ui.set_customer_ward_options(string_model(snapshot.wards));
    ui.set_customer_type_options(string_model(snapshot.types));
    ui.set_customer_ward_index(snapshot.ward_index);
    ui.set_customer_type_index(snapshot.type_index);
    ui.set_customer_active_only(snapshot.active_only);
    ui.set_customer_summary_text(snapshot.summary.into());
    ui.set_customer_page_text(snapshot.page_text.into());
    ui.set_customer_has_prev(snapshot.has_prev);
    ui.set_customer_has_next(snapshot.has_next);
    ui.set_customer_selected_count(snapshot.selected_count);
}

/// Dữ liệu đã dựng xong để gắn lên cửa sổ (không chứa model Slint).
struct Pushed {
    ui: Weak<MainWindow>,
    rows: Vec<CustomerRowData>,
    ward_index: i32,
    type_index: i32,
    wards: Vec<SharedString>,
    types: Vec<SharedString>,
    active_only: bool,
    summary: String,
    page_text: String,
    has_prev: bool,
    has_next: bool,
    selected_count: i32,
}

fn labelled(all: &str, values: &[String]) -> Vec<SharedString> {
    std::iter::once(SharedString::from(all))
        .chain(
            values
                .iter()
                .map(|value| SharedString::from(value.as_str())),
        )
        .collect()
}

fn string_model(values: Vec<SharedString>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(values))
}

/// Vị trí của `value` trong danh sách, cộng 1 để tính cả mục "Tất cả" ở đầu; không có thì 0.
fn index_of(values: &[String], value: &str) -> i32 {
    if value.is_empty() {
        return 0;
    }
    values
        .iter()
        .position(|candidate| candidate == value)
        .and_then(|position| i32::try_from(position + 1).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{index_of, option_at, page_count};

    fn names() -> Vec<String> {
        vec!["Phường A".to_owned(), "Phường B".to_owned()]
    }

    #[test]
    fn index_zero_means_no_filter() {
        assert_eq!(option_at(&names(), 0), "");
        assert_eq!(option_at(&names(), 2), "Phường B");
        assert_eq!(option_at(&names(), 9), "");
        assert_eq!(option_at(&names(), -1), "");
    }

    #[test]
    fn selected_value_maps_back_to_its_index() {
        assert_eq!(index_of(&names(), ""), 0);
        assert_eq!(index_of(&names(), "Phường A"), 1);
        assert_eq!(index_of(&names(), "Phường B"), 2);
        assert_eq!(index_of(&names(), "Không có"), 0);
    }

    #[test]
    fn page_count_is_at_least_one() {
        assert_eq!(page_count(0, 25), 1);
        assert_eq!(page_count(25, 25), 1);
        assert_eq!(page_count(26, 25), 2);
        assert_eq!(page_count(10, 0), 10);
    }
}

#[cfg(test)]
mod load_tests {
    use super::{Ctx, CustomerFilters, bulk_delete_customers, load};
    use cf_db::{clock::FixedClock, paths::DataPaths, store::Store};
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
        let ctx = Ctx::new(store, DataPaths::new(dir.path()), Box::new(clock));
        (dir, ctx)
    }

    #[test]
    fn load_returns_rows_and_options_from_the_store() {
        let (_dir, mut ctx) = seeded_ctx();
        let loaded = load(&mut ctx, &CustomerFilters::default()).expect("load");
        assert!(loaded.total > 0);
        assert_eq!(loaded.rows.len(), loaded.total.min(25));
        assert!(loaded.rows.iter().all(|row| row.active));
        assert!(!loaded.options.wards.is_empty() || loaded.options.customer_types.is_empty());
    }

    #[test]
    fn bulk_delete_then_reload_drops_the_row() {
        let (_dir, mut ctx) = seeded_ctx();
        let before = load(&mut ctx, &CustomerFilters::default()).expect("load before");
        let victim = before.rows[0].id;
        let now = ctx.clock().now_utc();
        let deleted = bulk_delete_customers(ctx.store_mut(), &[victim], now).expect("delete");
        assert_eq!(deleted, 1);
        let after = load(&mut ctx, &CustomerFilters::default()).expect("load after");
        assert_eq!(after.total, before.total - 1);
        assert!(after.rows.iter().all(|row| row.id != victim));
    }
}
