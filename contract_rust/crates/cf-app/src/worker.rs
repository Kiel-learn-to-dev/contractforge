//! Luồng worker sở hữu trạng thái (`Ctx` trong ứng dụng thật). UI gửi việc qua kênh; kết quả quay về
//! luồng UI bằng `slint::invoke_from_event_loop` ở phía gọi (xem `main.rs`).
//!
//! Generic theo `S` để test không cần cơ sở dữ liệu. Ứng dụng dùng `Worker<Ctx>`.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    thread,
};

use crate::ui_helpers::post_to_ui;

#[allow(dead_code)] // dùng từ việc dài đầu tiên (T4.H hàng loạt)
/// Tiến độ một việc dài, gửi về UI qua callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub label: String,
}

/// Ngữ cảnh một việc đang chạy trên worker: cờ hủy và nơi báo tiến độ.
pub struct Task {
    cancel: Arc<AtomicBool>,
    progress: Box<dyn Fn(Progress) + Send>,
}

impl Task {
    /// Việc đã được yêu cầu hủy chưa. Việc dài nên kiểm tra định kỳ rồi dừng sớm.
    #[allow(dead_code)] // dùng từ việc dài đầu tiên
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    #[allow(dead_code)] // dùng từ việc dài đầu tiên
    pub fn report(&self, done: usize, total: usize, label: impl Into<String>) {
        (self.progress)(Progress {
            done,
            total,
            label: label.into(),
        });
    }
}

type Job<S> = Box<dyn FnOnce(&mut S) + Send>;

/// Tay cầm gửi việc tới luồng worker. Có thể sao chép để gửi từ nhiều nơi.
pub struct Worker<S> {
    sender: Sender<Job<S>>,
}

impl<S> Clone for Worker<S> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<S: Send + 'static> Worker<S> {
    /// Khởi động luồng worker và chuyển quyền sở hữu `state` cho luồng đó.
    /// Luồng thoát khi mọi `Worker` đã bị bỏ.
    pub fn spawn(mut state: S) -> Self {
        let (sender, receiver) = mpsc::channel::<Job<S>>();
        thread::spawn(move || {
            for job in receiver {
                job(&mut state);
            }
        });
        Self { sender }
    }

    /// Chạy `job` trên worker rồi đưa `on_done` cùng kết quả về luồng UI.
    /// Trả false nếu luồng worker đã dừng.
    pub fn run<T, F, D>(&self, job: F, on_done: D) -> bool
    where
        T: Send + 'static,
        F: FnOnce(&mut S) -> T + Send + 'static,
        D: FnOnce(T) + Send + 'static,
    {
        self.run_task(
            Arc::new(AtomicBool::new(false)),
            |_| {},
            move |state, _| job(state),
            on_done,
        )
    }

    /// Như `run`, nhưng việc nhận được cờ hủy và gửi được tiến độ.
    /// `cancel` do phía gọi giữ để có thể bật hủy từ UI.
    pub fn run_task<T, P, F, D>(
        &self,
        cancel: Arc<AtomicBool>,
        progress: P,
        job: F,
        on_done: D,
    ) -> bool
    where
        T: Send + 'static,
        P: Fn(Progress) + Send + 'static,
        F: FnOnce(&mut S, &Task) -> T + Send + 'static,
        D: FnOnce(T) + Send + 'static,
    {
        let packed: Job<S> = Box::new(move |state| {
            let progress = Arc::new(std::sync::Mutex::new(progress));
            let task = Task {
                cancel,
                progress: Box::new(move |value| {
                    let callback = Arc::clone(&progress);
                    post_to_ui(move || {
                        if let Ok(callback) = callback.lock() {
                            callback(value);
                        }
                    });
                }),
            };
            let result = job(state, &task);
            post_to_ui(move || on_done(result));
        });
        self.sender.send(packed).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::Worker;
    use std::sync::{Arc, atomic::AtomicBool, mpsc};

    #[test]
    fn job_runs_on_worker_state_and_result_comes_back() {
        let worker = Worker::spawn(10_i64);
        let (tx, rx) = mpsc::channel();
        assert!(worker.run(
            |state| {
                *state += 5;
                *state
            },
            move |value| {
                let _ = tx.send(value);
            }
        ));
        assert_eq!(rx.recv().ok(), Some(15));
    }

    #[test]
    fn jobs_run_in_submission_order() {
        let worker = Worker::spawn(Vec::<i32>::new());
        let (tx, rx) = mpsc::channel();
        for value in 0..3 {
            let tx = tx.clone();
            worker.run(
                move |state| state.push(value),
                move |()| {
                    let _ = tx.send(());
                },
            );
        }
        for _ in 0..3 {
            let _ = rx.recv();
        }
        let (tx, rx) = mpsc::channel();
        worker.run(
            |state| state.clone(),
            move |values| {
                let _ = tx.send(values);
            },
        );
        assert_eq!(rx.recv().ok(), Some(vec![0, 1, 2]));
    }

    #[test]
    fn cancel_flag_is_visible_to_the_job() {
        let worker = Worker::spawn(());
        let cancel = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::channel();
        worker.run_task(
            Arc::clone(&cancel),
            |_| {},
            |(), task| task.is_cancelled(),
            move |flag| {
                let _ = tx.send(flag);
            },
        );
        assert_eq!(rx.recv().ok(), Some(true));
    }
}
