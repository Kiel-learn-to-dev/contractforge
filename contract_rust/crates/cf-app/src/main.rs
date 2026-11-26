#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! ContractForge (bản Rust): khởi động, khóa ghi, nạp dữ liệu, rồi mở cửa sổ.
//! Luồng khởi động theo 01_KIEN_TRUC §7; khóa theo §5.

mod customers_view;
mod dashboard_view;
mod dia_chinh;
mod files;
mod nav;
mod screens;
mod ui_helpers;
mod view;
mod worker;

use std::{
    cell::RefCell,
    env,
    fs::OpenOptions,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use cf_db::{
    DbError,
    clock::SystemClock,
    lock::WriterLock,
    paths::{DataPaths, resolve_data_root},
    schema::check_schema,
    store::Store,
};
use cf_services::Ctx;
use chrono::NaiveDate;
use nav::{Nav, Screen};
use slint::{ComponentHandle, ModelRc, SharedString, TimerMode, VecModel};
use ui_helpers::Feedback;
use worker::Worker;

slint::include_modules!();

/// Câu hiện khi đã có một cửa sổ ContractForge đang giữ khóa ghi (01_KIEN_TRUC §5).
const LOCKED_MESSAGE: &str = "ContractForge đang mở ở một cửa sổ khác. Hãy đóng cửa sổ đó trước.";

/// Tên file log trong thư mục `logs/` của data root.
const LOG_FILE: &str = "cf-rust.log";

// Ctx phải được chuyển sang luồng worker, nên phải là Send. Lỗi biên dịch ở đây nếu không đúng.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<Ctx>();
};

fn main() -> Result<()> {
    let started = Instant::now();
    let data_root = data_root_from_args().map_err(|error| startup_failure(format!("{error:#}")))?;
    let paths = DataPaths::new(data_root);
    paths
        .ensure_dirs()
        .map_err(|error| startup_failure(format!("Không tạo được thư mục dữ liệu: {error}")))?;
    init_logging(&paths)?;
    tracing::info!("thư mục dữ liệu: {}", paths.root.display());

    // Khóa giữ tới khi hàm kết thúc (sau ui.run()). Đừng đổi thành `let _ = ...`: khi đó khóa nhả ngay.
    let _lock = match WriterLock::acquire(&paths.root) {
        Ok(lock) => lock,
        Err(DbError::Locked) => return Err(startup_failure(LOCKED_MESSAGE.to_owned())),
        Err(error) => {
            return Err(startup_failure(format!("Không lấy được khóa ghi: {error}")));
        }
    };

    let conn = cf_db::open::open(&paths.db)
        .map_err(|error| startup_failure(format!("Không mở được cơ sở dữ liệu: {error}")))?;
    check_schema(&conn)
        .map_err(|error| startup_failure(format!("Cơ sở dữ liệu không đúng schema: {error}")))?;
    let store = Store::from_connection(conn)
        .map_err(|error| startup_failure(format!("Không nạp được dữ liệu: {error}")))?;
    let settings_data_root =
        std::fs::canonicalize(&paths.root).unwrap_or_else(|_| paths.root.clone());
    let mut ctx = Ctx::new(store, paths, Box::new(SystemClock));
    seed_bundled_templates(&mut ctx);

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()
        .map_err(|error| anyhow!("Không chọn được renderer femtovg: {error}"))?;

    let ui = MainWindow::new().context("không tạo được cửa sổ chính")?;
    ui.set_version(env!("CARGO_PKG_VERSION").into());
    let titles: Vec<SharedString> = Screen::ALL
        .iter()
        .map(|screen| SharedString::from(screen.title()))
        .collect();
    ui.set_screen_titles(ModelRc::new(VecModel::from(titles)));

    let nav = Rc::new(RefCell::new(Nav::default()));
    let feedback = Feedback::install(&ui);
    sync_screen(&ui, &nav.borrow());

    let worker = Worker::spawn(ctx);
    let _expiry_timer = wire_expiry(&ui, &worker);
    wire_navigation(&ui, &nav, &worker);
    wire_refresh(&ui, &worker);
    wire_dashboard(&ui, &nav, &worker);
    customers_view::install(&ui, &worker, &feedback);
    screens::customer_form::install(&ui, &worker);
    screens::customer_detail::install(&ui, &worker, &feedback);
    screens::customers_trash::install(&ui, &worker);
    screens::customers_import::install(&ui, &worker);
    screens::dossier::install(&ui, &worker);
    screens::products::install(&ui, &worker);
    screens::product_form::install(&ui, &worker);
    screens::products_import::install(&ui, &worker);
    screens::templates_list::install(&ui, &worker, &feedback);
    screens::template_editor::install(&ui, &worker, &feedback);
    screens::settings::install(&ui, &worker, settings_data_root);
    screens::contracts_list::install(&ui, &worker, &feedback);
    screens::contract_form::install(&ui, &worker, &feedback);
    screens::contract_detail::install(&ui, &worker, &feedback);
    screens::addendum_form::install(&ui, &worker);
    screens::renewal_form::install(&ui, &worker);
    screens::batch::install(&ui, &worker, &feedback);
    screens::quotation::install(&ui, &worker, &feedback);
    screens::expiring::install(&ui, &worker, &feedback);
    screens::global_search::install(&ui, &worker);

    tracing::info!("khởi động xong trong {} ms", started.elapsed().as_millis());
    ui.run().context("vòng lặp giao diện bị lỗi")?;
    Ok(())
}

thread_local! {
    static DASHBOARD_ACTION_TARGETS: RefCell<Vec<cf_services::dashboard::actions::ScreenTarget>> =
        const { RefCell::new(Vec::new()) };
}

/// Nạp số liệu dashboard trên worker rồi cập nhật view model ở luồng Slint.
fn wire_dashboard(ui: &MainWindow, nav: &Rc<RefCell<Nav>>, worker: &Worker<Ctx>) {
    reload_dashboard(ui.as_weak(), worker);

    let weak = ui.as_weak();
    let nav_open = Rc::clone(nav);
    ui.on_dashboard_open(move |route| {
        let (Some(screen), Some(ui)) = (Screen::from_index(route), weak.upgrade()) else {
            return;
        };
        let mut nav = nav_open.borrow_mut();
        nav.navigate(screen);
        sync_screen(&ui, &nav);
    });

    let hub = ui.global::<DashboardHub>();
    {
        let weak = ui.as_weak();
        let w = worker.clone();
        hub.on_refresh(move || {
            reload_dashboard(weak.clone(), &w);
        });
    }
    {
        let weak = ui.as_weak();
        let nav_quick = Rc::clone(nav);
        hub.on_quick_nav(move |idx| {
            let (Some(screen), Some(ui)) = (Screen::from_index(idx), weak.upgrade()) else {
                return;
            };
            let mut nav = nav_quick.borrow_mut();
            nav.navigate(screen);
            sync_screen(&ui, &nav);
        });
    }
    {
        let weak = ui.as_weak();
        let nav_status = Rc::clone(nav);
        hub.on_open_value_status(move |status| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            screens::contracts_list::open_with_status(status.as_str());
            let mut nav = nav_status.borrow_mut();
            nav.navigate(Screen::Contracts);
            sync_screen(&ui, &nav);
        });
    }
    hub.on_open_expiring(|days| {
        screens::expiring::open(days);
    });
    {
        let weak = ui.as_weak();
        hub.on_back_to_overview(move || {
            if let Some(ui) = weak.upgrade() {
                ui.global::<DashboardHub>().set_submode("overview".into());
            }
        });
    }
    {
        let weak = ui.as_weak();
        let nav_contract = Rc::clone(nav);
        hub.on_open_contract(move |id| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut nav = nav_contract.borrow_mut();
            nav.navigate(Screen::Contracts);
            sync_screen(&ui, &nav);
            ui.invoke_contracts_open(id);
        });
    }
    {
        let weak = ui.as_weak();
        let nav_cust = Rc::clone(nav);
        hub.on_open_customer(move |id| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut nav = nav_cust.borrow_mut();
            nav.navigate(Screen::Customers);
            sync_screen(&ui, &nav);
            ui.global::<CustomerHub>().invoke_open_detail(id);
        });
    }
    {
        let weak = ui.as_weak();
        let nav_metric = Rc::clone(nav);
        hub.on_open_metric(move |idx| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            match idx {
                0 => {
                    screens::contracts_list::open_with_alert("all_active");
                    let mut nav = nav_metric.borrow_mut();
                    nav.navigate(Screen::Contracts);
                    sync_screen(&ui, &nav);
                }
                1 => {
                    screens::contracts_list::open_with_status("Signed");
                    let mut nav = nav_metric.borrow_mut();
                    nav.navigate(Screen::Contracts);
                    sync_screen(&ui, &nav);
                }
                2 => {
                    screens::expiring::open(30);
                }
                3 => {
                    screens::contracts_list::open_with_status("Expired");
                    let mut nav = nav_metric.borrow_mut();
                    nav.navigate(Screen::Contracts);
                    sync_screen(&ui, &nav);
                }
                4 => {
                    let mut nav = nav_metric.borrow_mut();
                    nav.navigate(Screen::Customers);
                    sync_screen(&ui, &nav);
                }
                5 => {
                    screens::contracts_list::open_with_alert("generated");
                    let mut nav = nav_metric.borrow_mut();
                    nav.navigate(Screen::Contracts);
                    sync_screen(&ui, &nav);
                }
                _ => {
                    screens::contracts_list::open_with_status("");
                    let mut nav = nav_metric.borrow_mut();
                    nav.navigate(Screen::Contracts);
                    sync_screen(&ui, &nav);
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let nav_action = Rc::clone(nav);
        hub.on_open_action(move |idx| {
            let target = usize::try_from(idx)
                .ok()
                .and_then(|i| DASHBOARD_ACTION_TARGETS.with(|t| t.borrow().get(i).cloned()));
            let (Some(target), Some(ui)) = (target, weak.upgrade()) else {
                return;
            };
            use cf_services::dashboard::actions::ScreenTarget;
            match target {
                ScreenTarget::Expiring { days } => {
                    screens::expiring::open(i32::from(days));
                }
                ScreenTarget::ContractsByStatus { status } => {
                    screens::contracts_list::open_with_status(&status);
                    let mut nav = nav_action.borrow_mut();
                    nav.navigate(Screen::Contracts);
                    sync_screen(&ui, &nav);
                }
                ScreenTarget::NewTemplate => {
                    let mut nav = nav_action.borrow_mut();
                    nav.navigate(Screen::Templates);
                    sync_screen(&ui, &nav);
                }
            }
        });
    }
}

fn reload_dashboard(weak: slint::Weak<MainWindow>, worker: &Worker<Ctx>) {
    worker.run(
        |ctx| dashboard_view::load(ctx),
        move |data| set_dashboard(weak, data),
    );
}

fn set_dashboard(weak: slint::Weak<MainWindow>, data: dashboard_view::DashboardView) {
    let Some(ui) = weak.upgrade() else {
        return;
    };
    let metrics = data
        .metrics
        .into_iter()
        .map(|metric| DashboardMetric {
            label: metric.label.into(),
            value: metric.value.into(),
            route: metric.route,
            accent: slint::Color::from_rgb_u8(metric.rgb[0], metric.rgb[1], metric.rgb[2]),
        })
        .collect::<Vec<_>>();
    let mut targets = Vec::with_capacity(data.actions.len());
    let actions = data
        .actions
        .into_iter()
        .map(|action| {
            targets.push(action.target);
            DashboardAction {
                message: action.message.into(),
                level: action.level.into(),
                route: action.route,
            }
        })
        .collect::<Vec<_>>();
    DASHBOARD_ACTION_TARGETS.with(|t| *t.borrow_mut() = targets);
    ui.set_dashboard_metrics(ModelRc::new(VecModel::from(metrics)));
    ui.set_dashboard_actions(ModelRc::new(VecModel::from(actions)));

    let bars = data
        .forecast_bars
        .into_iter()
        .enumerate()
        .map(|(i, b)| ForecastBarItem {
            label: b.label.into(),
            count: b.count as i32,
            ratio: b.ratio,
            month_index: i as i32,
        })
        .collect::<Vec<_>>();
    let dist = data
        .status_dist
        .into_iter()
        .map(|s| StatusDistItem {
            label: s.label.into(),
            count: s.count as i32,
            ratio: s.ratio,
            accent: slint::Color::from_rgb_u8(s.rgb[0], s.rgb[1], s.rgb[2]),
            status_code: s.status_code.into(),
        })
        .collect::<Vec<_>>();
    let pies = data
        .pie_items
        .into_iter()
        .map(|p| ProductPieItem {
            name: p.name.into(),
            count: p.count as i32,
            value_text: p.value_text.into(),
            fill: slint::Color::from_rgb_u8(p.rgb[0], p.rgb[1], p.rgb[2]),
            commands: p.commands.into(),
        })
        .collect::<Vec<_>>();
    let map_exp = |rows: Vec<dashboard_view::ExpiringSummary>| {
        rows.into_iter()
            .map(|r| ExpiringSummaryRow {
                contract_id: r.contract_id as i32,
                contract_number: r.contract_number.into(),
                customer_name: r.customer_name.into(),
                end_date: r.end_date.into(),
                days_left: r.days_left as i32,
                urgency: r.urgency.into(),
            })
            .collect::<Vec<_>>()
    };
    let tops = data
        .top_customers
        .into_iter()
        .map(|c| TopCustomerRow {
            id: c.id as i32,
            name: c.name.into(),
            count: c.count as i32,
        })
        .collect::<Vec<_>>();
    let acts = data
        .recent_activities
        .into_iter()
        .map(|a| RecentActivityRow {
            contract_id: a.contract_id as i32,
            title: a.title.into(),
            subtitle: a.subtitle.into(),
            when_text: a.when_text.into(),
        })
        .collect::<Vec<_>>();

    let hub = ui.global::<DashboardHub>();
    hub.set_today_label(data.today_label.into());
    let bars = if bars.iter().any(|bar| bar.count > 0) {
        bars
    } else {
        Vec::new()
    };
    hub.set_forecast_bars(ModelRc::new(VecModel::from(bars)));
    hub.set_status_dist(ModelRc::new(VecModel::from(dist)));
    hub.set_pie_items(ModelRc::new(VecModel::from(pies)));
    hub.set_expiring_30(ModelRc::new(VecModel::from(map_exp(data.expiring_30))));
    hub.set_expiring_31_60(ModelRc::new(VecModel::from(map_exp(data.expiring_31_60))));
    hub.set_value_total_text(data.value_total_text.into());
    hub.set_value_in_force_sub(data.value_in_force_sub.into());
    hub.set_value_paid_text(data.value_paid_text.into());
    hub.set_value_invoiced_text(data.value_invoiced_text.into());
    hub.set_value_unpaid_text(data.value_unpaid_text.into());
    hub.set_monthly_title(data.monthly_title.into());
    hub.set_monthly_new_text(data.monthly_new_text.into());
    hub.set_monthly_signed_text(data.monthly_signed_text.into());
    hub.set_monthly_expired_text(data.monthly_expired_text.into());
    hub.set_monthly_value_text(data.monthly_value_text.into());
    hub.set_top_customers(ModelRc::new(VecModel::from(tops)));
    hub.set_recent_activities(ModelRc::new(VecModel::from(acts)));
}

/// Runs expiry once at startup, then again when the local calendar day changes.
fn wire_expiry(ui: &MainWindow, worker: &Worker<Ctx>) -> slint::Timer {
    let last_success = Arc::new(Mutex::new(None::<NaiveDate>));
    let in_flight = Arc::new(AtomicBool::new(false));
    schedule_expiry(
        worker.clone(),
        ui.as_weak(),
        Arc::clone(&last_success),
        Arc::clone(&in_flight),
    );

    let timer = slint::Timer::default();
    let worker = worker.clone();
    let weak = ui.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_secs(60), move || {
        let today = chrono::Local::now().date_naive();
        if last_success.lock().is_ok_and(|date| *date == Some(today)) {
            return;
        }
        schedule_expiry(
            worker.clone(),
            weak.clone(),
            Arc::clone(&last_success),
            Arc::clone(&in_flight),
        );
    });
    timer
}

fn schedule_expiry(
    worker: Worker<Ctx>,
    ui: slint::Weak<MainWindow>,
    last_success: Arc<Mutex<Option<NaiveDate>>>,
    in_flight: Arc<AtomicBool>,
) {
    if in_flight
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let pending = Arc::clone(&in_flight);
    let completion_date = Arc::clone(&last_success);
    let reload_worker = worker.clone();
    let accepted = worker.run(
        |ctx| {
            let today = ctx.clock().today();
            (today, cf_services::contracts::expiry::expire_due(ctx))
        },
        move |(today, result)| {
            pending.store(false, Ordering::Relaxed);
            match result {
                Ok(changed) => {
                    if let Ok(mut last_success) = completion_date.lock() {
                        *last_success = Some(today);
                    }
                    if changed > 0 {
                        reload_dashboard(ui.clone(), &reload_worker);
                        crate::screens::contracts_list::reload();
                    }
                }
                Err(error) => {
                    tracing::warn!("không quét được hợp đồng hết hạn: {}", error.user_message())
                }
            }
        },
    );
    if !accepted {
        in_flight.store(false, Ordering::Relaxed);
    }
}

/// Chọn thư mục dữ liệu theo ưu tiên tham số dòng lệnh, biến môi trường, INI cạnh exe, mặc định.
fn data_root_from_args() -> Result<PathBuf> {
    let mut args = env::args().skip(1);
    if let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--data-root=") {
            return Ok(PathBuf::from(value));
        }
        if arg != "--data-root" {
            return Err(anyhow!("tham số không hợp lệ: {arg}"));
        }
        let value = args
            .next()
            .ok_or_else(|| anyhow!("thiếu đường dẫn sau --data-root"))?;
        return Ok(PathBuf::from(value));
    }

    let env_root = env::var("CONTRACTFORGE_DATA_ROOT").ok();
    if let Some(path) = env_root.as_deref().filter(|path| !path.trim().is_empty()) {
        return Ok(PathBuf::from(path));
    }

    if let Some(exe_dir) = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        && let Some(path) = data_root_from_ini(&exe_dir)?
    {
        return Ok(path);
    }

    let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let exe = env::current_exe().ok();
    let exe_dir = exe.as_deref().and_then(Path::parent);
    Ok(resolve_data_root(
        env_root.as_deref(),
        local_app_data.as_deref(),
        exe_dir,
    ))
}

/// Đọc `ContractForge.ini` cạnh exe. Đường dẫn tương đối tính từ thư mục chứa exe.
fn data_root_from_ini(exe_dir: &Path) -> Result<Option<PathBuf>> {
    let config_path = exe_dir.join("ContractForge.ini");
    let content = match std::fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("không đọc được cấu hình {}", config_path.display()));
        }
    };
    let configured_root = parse_data_root_ini(&content)
        .with_context(|| format!("cấu hình {} không hợp lệ", config_path.display()))?;
    let root = PathBuf::from(configured_root);
    let root = if root.is_absolute() {
        root
    } else {
        exe_dir.join(root)
    };
    if !root.join("contract_manager.db").is_file() {
        return Err(anyhow!(
            "ContractForge.ini trỏ tới thư mục dữ liệu không có contract_manager.db: {}",
            root.display()
        ));
    }
    let root = std::path::absolute(&root)
        .with_context(|| format!("không mở được thư mục dữ liệu {}", root.display()))?;
    Ok(Some(normalize_absolute_path(&root)))
}

fn normalize_absolute_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn parse_data_root_ini(content: &str) -> Result<String> {
    let mut configured_root = None;
    for (index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim().trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| anyhow!("dòng {} cần có dạng data_root=đường_dẫn", index + 1))?;
        if key.trim() != "data_root" {
            return Err(anyhow!(
                "khóa không được hỗ trợ ở dòng {}: {}",
                index + 1,
                key.trim()
            ));
        }
        if configured_root.is_some() {
            return Err(anyhow!("data_root bị khai báo nhiều lần"));
        }
        let value = value.trim();
        if value.is_empty() {
            return Err(anyhow!("data_root không được để trống"));
        }
        configured_root = Some(value.to_owned());
    }
    configured_root.ok_or_else(|| anyhow!("thiếu khóa data_root"))
}

/// Ghi log ra `logs/cf-rust.log`, không màu.
fn init_logging(paths: &DataPaths) -> Result<()> {
    let log_path = paths.logs.join(LOG_FILE);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("không mở được file log {}", log_path.display()))?;
    tracing_subscriber::fmt()
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .init();
    Ok(())
}

/// Hiện hộp thoại lỗi trước khi cửa sổ chính tồn tại, rồi trả về lỗi để thoát.
fn startup_failure(message: String) -> anyhow::Error {
    tracing::error!("{message}");
    rfd::MessageDialog::new()
        .set_title("ContractForge")
        .set_description(message.as_str())
        .set_level(rfd::MessageLevel::Error)
        .show();
    anyhow!(message)
}

/// Đưa tiêu đề và trạng thái Quay lại của `nav` lên cửa sổ.
fn sync_screen(ui: &MainWindow, nav: &Nav) {
    let screen = nav.current();
    ui.set_current_screen(screen.index());
    ui.set_screen_title(screen.title().into());
    ui.set_can_go_back(nav.can_go_back());
}

/// Nút "Làm mới": nạp lại toàn bộ dữ liệu từ file DB rồi vẽ lại các màn hình đang dùng.
fn wire_refresh(ui: &MainWindow, worker: &Worker<Ctx>) {
    let weak = ui.as_weak();
    let worker = worker.clone();
    ui.on_refresh_db(move || {
        let weak = weak.clone();
        let dash_worker = worker.clone();
        worker.run(
            |ctx| ctx.store_mut().reload().map_err(|error| error.to_string()),
            move |result| {
                ui_helpers::post_to_ui(move || match result {
                    Ok(()) => {
                        reload_dashboard(weak, &dash_worker);
                        customers_view::request_load();
                        screens::customer_detail::reload();
                        screens::products::reload();
                        screens::templates_list::reload();
                        screens::contracts_list::reload();
                        screens::contract_detail::reload();
                        screens::batch::reload();
                        screens::quotation::reload();
                        ui_helpers::toast_from_worker(
                            ui_helpers::ToastKind::Success,
                            "Đã làm mới dữ liệu".to_owned(),
                        );
                    }
                    Err(error) => ui_helpers::toast_from_worker(
                        ui_helpers::ToastKind::Danger,
                        format!("Không làm mới được dữ liệu: {error}"),
                    ),
                });
            },
        );
    });
}

fn wire_navigation(ui: &MainWindow, nav: &Rc<RefCell<Nav>>, worker: &Worker<Ctx>) {
    let weak = ui.as_weak();
    let shared = Rc::clone(nav);
    let dash_worker = worker.clone();
    ui.on_navigate(move |index| {
        let (Some(screen), Some(ui)) = (Screen::from_index(index), weak.upgrade()) else {
            return;
        };
        if screen == Screen::Dashboard {
            ui.global::<DashboardHub>().set_submode("overview".into());
            reload_dashboard(ui.as_weak(), &dash_worker);
        } else if screen == Screen::Batch {
            screens::batch::reload();
        } else if screen == Screen::Quotations {
            screens::quotation::reload();
        }
        let mut nav = shared.borrow_mut();
        nav.navigate(screen);
        sync_screen(&ui, &nav);
    });

    let weak = ui.as_weak();
    let shared = Rc::clone(nav);
    ui.on_back(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut nav = shared.borrow_mut();
        nav.back();
        sync_screen(&ui, &nav);
    });
}

/// Nạp sẵn các mẫu hợp đồng/báo giá đi kèm bộ cài chỉ khi DB chưa có mẫu nào.
fn seed_bundled_templates(ctx: &mut Ctx) {
    if !ctx.store().templates.is_empty() {
        return;
    }

    let mut candidates = Vec::new();
    if let Ok(exe) = env::current_exe()
        && let Some(directory) = exe.parent()
    {
        candidates.push(directory.join("assets").join("default_templates"));
    }
    if cfg!(debug_assertions) {
        candidates.push(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("..")
                .join("ContractForge")
                .join("assets")
                .join("default_templates"),
        );
    }

    if let Some(dir) = candidates.into_iter().find(|p| p.is_dir()) {
        match cf_services::templates::bundled::seed_from_project_files(ctx, &dir) {
            Ok(0) => {}
            Ok(n) => tracing::info!("đã nạp sẵn {n} mẫu từ {}", dir.display()),
            Err(err) => tracing::warn!("không nạp được mẫu sẵn: {}", err.user_message()),
        }
    }
    match cf_services::templates::bundled::seed_embedded_sample_contract(ctx) {
        Ok(true) => tracing::info!("đã nạp mẫu hợp đồng dịch vụ đi kèm"),
        Ok(false) => {}
        Err(err) => tracing::warn!("không nạp được mẫu hợp đồng mẫu: {}", err.user_message()),
    }
}

#[cfg(test)]
mod data_root_ini_tests {
    use super::{data_root_from_ini, parse_data_root_ini};
    use std::fs;

    #[test]
    fn parses_relative_data_root_and_comments() {
        assert_eq!(
            parse_data_root_ini("# dữ liệu\n; máy dùng thử\ndata_root = ..\\..\\data\n")
                .expect("valid ini"),
            r"..\..\data"
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_unknown_keys() {
        assert!(parse_data_root_ini("# empty").is_err());
        assert!(parse_data_root_ini("data_root=a\ndata_root=b").is_err());
        assert!(parse_data_root_ini("other=a").is_err());
        assert!(parse_data_root_ini("data_root=").is_err());
    }

    #[test]
    fn resolves_relative_root_from_executable_directory_and_requires_database() {
        let temp = tempfile::tempdir().expect("temp directory");
        let exe_dir = temp.path().join("install").join("dist");
        let data_dir = temp.path().join("data");
        fs::create_dir_all(&exe_dir).expect("exe directory");
        fs::create_dir_all(&data_dir).expect("data directory");
        fs::write(data_dir.join("contract_manager.db"), []).expect("db marker");
        fs::write(
            exe_dir.join("ContractForge.ini"),
            "data_root=..\\..\\data\n",
        )
        .expect("configuration");

        assert_eq!(
            data_root_from_ini(&exe_dir).expect("ini").as_deref(),
            Some(data_dir.as_path())
        );

        fs::remove_file(data_dir.join("contract_manager.db")).expect("remove marker");
        assert!(data_root_from_ini(&exe_dir).is_err());
    }
}
