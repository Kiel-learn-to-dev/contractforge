//! Thông báo (toast), hộp xác nhận và cầu nối từ luồng worker về luồng UI.
//! Mọi hàm ở đây chỉ được gọi trên luồng UI, trừ `post_to_ui`.

use std::{cell::RefCell, rc::Rc, time::Duration};

use slint::{ComponentHandle, TimerMode, Weak};

use crate::MainWindow;

/// Thời gian toast tự ẩn (05_GIAO_DIEN §2). Lỗi (danger) không tự ẩn.
pub const TOAST_HIDE_AFTER: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Warning,
    Danger,
}

impl ToastKind {
    fn as_str(self) -> &'static str {
        match self {
            ToastKind::Success => "success",
            ToastKind::Warning => "warning",
            ToastKind::Danger => "danger",
        }
    }
}

type PendingAction = Box<dyn FnOnce()>;

/// Giữ trạng thái toast và hộp xác nhận. Tạo một lần khi khởi động.
pub struct Feedback {
    ui: Weak<MainWindow>,
    toast_timer: slint::Timer,
    pending: Rc<RefCell<Option<PendingAction>>>,
}

impl Feedback {
    /// Gắn callback đóng toast và hai nút của hộp xác nhận.
    pub fn install(ui: &MainWindow) -> Rc<Self> {
        let feedback = Rc::new(Self {
            ui: ui.as_weak(),
            toast_timer: slint::Timer::default(),
            pending: Rc::new(RefCell::new(None)),
        });

        let weak = ui.as_weak();
        ui.on_toast_closed(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_toast_shown(false);
            }
        });

        let ok_feedback = Rc::clone(&feedback);
        ui.on_confirm_ok(move || {
            let action = ok_feedback.pending.borrow_mut().take();
            ok_feedback.close_confirm();
            if let Some(action) = action {
                action();
            }
        });

        let cancel_feedback = Rc::clone(&feedback);
        ui.on_confirm_cancel(move || {
            cancel_feedback.pending.borrow_mut().take();
            cancel_feedback.close_confirm();
        });

        FEEDBACK.with(|cell| *cell.borrow_mut() = Some(Rc::clone(&feedback)));
        feedback
    }

    /// Hiện toast. `Success` và `Warning` tự ẩn sau `TOAST_HIDE_AFTER`; `Danger` ở lại tới khi người dùng đóng.
    pub fn toast(&self, kind: ToastKind, message: &str) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        ui.set_toast_kind(kind.as_str().into());
        ui.set_toast_message(message.into());
        ui.set_toast_shown(true);

        if kind == ToastKind::Danger {
            self.toast_timer.stop();
            return;
        }
        let weak = self.ui.clone();
        self.toast_timer
            .start(TimerMode::SingleShot, TOAST_HIDE_AFTER, move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_toast_shown(false);
                }
            });
    }

    /// Mở hộp xác nhận. Chỉ khi người dùng bấm "Đồng ý" thì `on_ok` mới chạy.
    pub fn confirm(&self, title: &str, message: &str, on_ok: impl FnOnce() + 'static) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        *self.pending.borrow_mut() = Some(Box::new(on_ok));
        ui.set_confirm_title(title.into());
        ui.set_confirm_message(message.into());
        ui.set_confirm_shown(true);
    }

    fn close_confirm(&self) {
        if let Some(ui) = self.ui.upgrade() {
            ui.set_confirm_shown(false);
        }
    }
}

thread_local! {
    // Bản Feedback của luồng UI, để việc từ worker hiện toast mà không phải mang Rc qua luồng khác.
    static FEEDBACK: RefCell<Option<Rc<Feedback>>> = const { RefCell::new(None) };
}

/// Đưa một việc về luồng UI. Dùng trong `on_done` của worker.
pub fn post_to_ui(job: impl FnOnce() + Send + 'static) {
    #[cfg(test)]
    {
        job();
    }
    #[cfg(not(test))]
    {
        // Lỗi chỉ xảy ra khi vòng lặp sự kiện đã dừng, lúc đó không còn UI để cập nhật.
        let _ = slint::invoke_from_event_loop(job);
    }
}

/// Hiện toast từ bất kỳ luồng nào: việc hiện thật được đẩy về luồng UI.
pub fn toast_from_worker(kind: ToastKind, message: String) {
    post_to_ui(move || {
        FEEDBACK.with(|cell| {
            if let Some(feedback) = cell.borrow().as_ref() {
                feedback.toast(kind, &message);
            }
        });
    });
}
