use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

use slint::{
    ComponentHandle, GraphicsAPI, ModelRc, RenderingState, VecModel, language::StandardListViewItem,
};

slint::include_modules!();

fn as_model(rows: &[Vec<String>]) -> ModelRc<ModelRc<StandardListViewItem>> {
    let outer = rows
        .iter()
        .map(|row| {
            let cells = row
                .iter()
                .map(|text| StandardListViewItem::from(text.as_str()))
                .collect::<Vec<_>>();
            ModelRc::new(VecModel::from(cells))
        })
        .collect::<Vec<_>>();
    ModelRc::new(VecModel::from(outer))
}

fn main() -> Result<(), slint::PlatformError> {
    let started = Instant::now();
    let ui = MainWindow::new()?;
    let rows = Rc::new(RefCell::new(
        (0..1_000)
            .map(|index| {
                vec![
                    format!("Trạm y tế xã Đồng Tâm {index:04}"),
                    format!("CF-{index:04}"),
                    format!("{}.000.000", 12 + index % 30),
                    format!("{:02}/03/2026", index % 28 + 1),
                    if index % 3 == 0 {
                        "Hoạt động".to_owned()
                    } else {
                        "Đang xử lý".to_owned()
                    },
                ]
            })
            .collect::<Vec<_>>(),
    ));
    ui.set_rows(as_model(&rows.borrow()));

    let ascending_rows = Rc::clone(&rows);
    let ascending_ui = ui.as_weak();
    ui.on_sort_ascending(move |column| {
        if let Ok(column) = usize::try_from(column) {
            ascending_rows
                .borrow_mut()
                .sort_by(|left, right| left.get(column).cmp(&right.get(column)));
            if let Some(ui) = ascending_ui.upgrade() {
                ui.set_rows(as_model(&ascending_rows.borrow()));
            }
        }
    });
    let descending_rows = Rc::clone(&rows);
    let descending_ui = ui.as_weak();
    ui.on_sort_descending(move |column| {
        if let Ok(column) = usize::try_from(column) {
            descending_rows
                .borrow_mut()
                .sort_by(|left, right| right.get(column).cmp(&left.get(column)));
            if let Some(ui) = descending_ui.upgrade() {
                ui.set_rows(as_model(&descending_rows.borrow()));
            }
        }
    });

    // Đo tới khung hình đầu tiên được vẽ (cửa sổ đã hiện), chỉ in một lần.
    let backend = std::env::var("SLINT_BACKEND").unwrap_or_else(|_| "(mặc định)".to_owned());
    // Dự phòng: renderer skia của Slint 1.19.1 không gọi rendering notifier trong bản thử này,
    // nên luôn in thêm mốc "vòng lặp sự kiện bắt đầu" (gần với lúc cửa sổ được tạo).
    let loop_backend = backend.clone();
    let _ = slint::invoke_from_event_loop(move || {
        println!(
            "SLINT_BACKEND={loop_backend} · mốc dự phòng: vòng lặp sự kiện bắt đầu sau {:?}",
            started.elapsed()
        );
    });
    let reported = Cell::new(false);
    let notifier = ui.window().set_rendering_notifier(move |state, api| {
        if !matches!(state, RenderingState::AfterRendering) || reported.replace(true) {
            return;
        }
        let api_name = match api {
            GraphicsAPI::NativeOpenGL { .. } => "OpenGL",
            _ => "khác",
        };
        println!(
            "SLINT_BACKEND={backend} · GraphicsAPI={api_name} · tới khung hình đầu tiên: {:?}",
            started.elapsed()
        );
    });
    if let Err(error) = notifier {
        eprintln!("Không đặt được bộ đo khung hình: {error}");
    }
    ui.run()
}
