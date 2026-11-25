# Đợt 0 — Khung workspace và bản thử UI

Blueprint: `docs/01_KIEN_TRUC.md` §1–2, `docs/05_GIAO_DIEN.md` §6.

### T0.01 — Tạo workspace rỗng · S
- [ ] Xong
- **Làm:** `contract_rust/Cargo.toml` (`[workspace]`, `resolver = "3"`, `members = ["crates/*"]`,
  `[workspace.package] edition = "2024"`, `version = "0.1.0"`); `rust-toolchain.toml` (`channel = "1.94.1"`);
  6 crate rỗng: `cf-core`, `cf-docx`, `cf-db`, `cf-services` (lib), `cf-app` (bin, `main` in "ContractForge"),
  `cf-testkit` (lib, chỉ dùng làm dev-dependency). `.gitignore` có `target/`.
- **File:** `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `crates/*/Cargo.toml`, `crates/*/src/lib.rs|main.rs`
- **Xong khi:** `cargo build --workspace` (chạy trong `contract_rust/`) thành công.
- **Phụ thuộc:** —

### T0.02 — Khai báo thư viện dùng chung · XS
- [ ] Xong
- **Làm:** `[workspace.dependencies]` đúng bảng `docs/01_KIEN_TRUC.md` §2 (phiên bản, feature).
  Mỗi crate chỉ kéo thư viện nó cần theo cột "Dùng ở". Luật phụ thuộc giữa crate nội bộ theo §1.
- **File:** `Cargo.toml`, `crates/*/Cargo.toml`
- **Xong khi:** `cargo build --workspace` thành công; `cargo tree -p cf-core` không có `rusqlite`, `zip`, `slint`.
- **Phụ thuộc:** T0.01

### T0.03 — `cf-testkit`: đọc fixture · S
- [ ] Xong
- **Làm:** `pub fn fixture(name: &str) -> serde_json::Value` đọc `contract_rust/fixtures/{name}.json`
  (đường dẫn dựng từ `env!("CARGO_MANIFEST_DIR")`); `pub fn fixture_path(rel) -> PathBuf`;
  `pub fn dec(v: &Value) -> Decimal` (nhận chuỗi hoặc số); `pub enum PyResult { Ok(Value), Err{class, msg} }`
  + `pub fn py(v: &Value) -> PyResult` đọc dạng `{"ok":..}`/`{"err":..,"msg":..}`;
  `pub fn kb(code: &str) -> Value` đọc khóa trong `intentional_differences.json`.
- **File:** `crates/cf-testkit/src/lib.rs`, `crates/cf-testkit/Cargo.toml`
- **Test:** test trong crate: đọc `amount_to_words` có ≥ 40 ca; `py()` phân biệt đúng ok/err.
- **Xong khi:** `cargo test -p cf-testkit` xanh.
- **Phụ thuộc:** T0.02

### T0.04 — Script kiểm chung · XS
- [ ] Xong
- **Làm:** `tools/check.ps1` chạy lần lượt `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` trong `contract_rust/`; dừng ở lệnh lỗi đầu tiên, mã thoát ≠ 0.
- **File:** `tools/check.ps1`
- **Xong khi:** `pwsh contract_rust/tools/check.ps1` chạy hết, thoát 0.
- **Phụ thuộc:** T0.03

### T0.05 — Bản thử UI: cửa sổ + ô nhập · S
- [ ] Xong
- **Làm:** crate riêng `spikes/ui-spike` (**ngoài** `crates/`, thêm vào `members`). Cửa sổ 1000×700,
  font "Segoe UI", style `fluent`. 3 `LineEdit` + 1 `TextEdit` nhiều dòng, nhãn tiếng Việt.
  Chọn renderer qua biến môi trường `SLINT_BACKEND` (`winit-femtovg` / `winit-skia`).
- **File:** `spikes/ui-spike/Cargo.toml`, `spikes/ui-spike/build.rs`, `spikes/ui-spike/ui/main.slint`, `spikes/ui-spike/src/main.rs`
- **Xong khi:** `cargo run -p ui-spike` mở cửa sổ, gõ được chữ.
- **Phụ thuộc:** T0.02

### T0.06 — Bản thử UI: bảng 1.000 dòng · XS
- [ ] Xong
- **Làm:** thêm `StandardTableView` 5 cột, 1.000 dòng sinh trong Rust (tên hư cấu có dấu, số tiền
  dạng `24.000.000`, ngày `dd/mm/yyyy`), sắp xếp khi bấm tiêu đề cột.
- **File:** `spikes/ui-spike/ui/main.slint`, `spikes/ui-spike/src/main.rs`
- **Xong khi:** cuộn bảng mượt, bấm tiêu đề sắp xếp được.
- **Phụ thuộc:** T0.05

### T0.07 — Bản thử UI: biểu đồ cột và tròn · XS
- [ ] Xong
- **Làm:** 12 cột (`Rectangle`, chiều cao theo tỷ lệ, nhãn `T1/26`…) và 1 biểu đồ tròn 3 phần (`Path` cung tròn).
- **File:** `spikes/ui-spike/ui/chart.slint`, `spikes/ui-spike/ui/main.slint`
- **Xong khi:** hiển thị đúng với dữ liệu giả.
- **Phụ thuộc:** T0.05

### T0.08 — Bản thử UI: đo đạc · XS
- [ ] Xong
- **Làm:** in ra console thời gian từ `main()` tới khi cửa sổ hiện (dùng `std::time::Instant` + callback
  `Window::show`); hướng dẫn đo RAM bằng Task Manager. Build `--release`.
- **File:** `spikes/ui-spike/src/main.rs`
- **Xong khi:** `cargo run -p ui-spike --release` in thời gian khởi động.
- **Phụ thuộc:** T0.06, T0.07

### T0.09 — Ghi kết quả bản thử (người + AI) · XS
- [ ] Xong
- **Làm:** chạy đủ 4 kiểm tra của `docs/05_GIAO_DIEN.md` §6 với **cả hai** renderer; ghi vào
  `docs/DOT0_KET_QUA.md`: bảng (renderer × kiểm tra × đạt/không × ghi chú), RAM, thời gian khởi động,
  phiên bản Slint, renderer đề xuất. Phần gõ Unikey/EVKey **do người thật gõ**.
- **File:** `docs/DOT0_KET_QUA.md`
- **Xong khi:** file có đủ bảng; **ĐD0**: chủ dự án đồng ý.
- **Phụ thuộc:** T0.08
