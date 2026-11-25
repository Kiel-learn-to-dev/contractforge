# Đợt 5 — Đóng gói, nghiệm thu, xóa bản Python

Blueprint: `docs/07_KIEM_THU.md` §4, `docs/08_LO_TRINH.md` (Đợt 6–7).

### T5.01 — Nhúng assets và icon · S
- [ ] Xong
- **Làm:** `include_bytes!` các `sample_*.docx` (08a, 5 báo giá, 3 yêu cầu báo giá, hợp đồng mẫu), `dia_chinh.json`, icon
  `cf_icon.ico` (qua `winres`-tương đương: dùng `slint-build` + file `.rc`; nếu cần thêm thư viện, ghi lý do vào `01_KIEN_TRUC.md` §2 trước).
  Ưu tiên file trong `<exe>/assets/default_templates/` nếu có (bản riêng của tổ chức), không thì bản nhúng.
- **File:** `crates/cf-app/build.rs`, `crates/cf-app/src/assets.rs`
- **Xong khi:** chép riêng file `.exe` sang thư mục trống, chạy được báo giá tự nhập và biểu 08a.
- **Phụ thuộc:** hết Đợt 4

### T5.02 — Build release một file · XS
- [ ] Xong
- **Làm:** `[profile.release]` `lto = "fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`, `opt-level = 3`;
  ẩn cửa sổ console (`#![windows_subsystem = "windows"]`); script `tools/build_release.ps1` ra `dist/ContractForge.exe`.
- **File:** `Cargo.toml`, `crates/cf-app/src/main.rs`, `tools/build_release.ps1`
- **Xong khi:** `pwsh tools/build_release.ps1` ra một file exe; ghi kích thước file.
- **Phụ thuộc:** T5.01

### T5.03 — Đo hiệu năng · XS
- [ ] Xong
- **Làm:** trên bản sao DB thật: RAM khi mở danh sách hợp đồng, thời gian khởi động, thời gian mở dashboard, thời gian lọc khi gõ;
  ghi vào `docs/NGHIEM_THU.md`. So với mục tiêu G1, G3, G6.
- **File:** `docs/NGHIEM_THU.md`
- **Xong khi:** bảng số đo đủ; mục nào trượt → mở task sửa.
- **Phụ thuộc:** T5.02

### T5.04 — So văn bản 10 hợp đồng thật với bản Python · S
- [ ] Xong
- **Làm:** `tools/compare_outputs.py`: trên bản sao DB (đường dẫn đã viết lại như T3.17), sinh 10 hợp đồng bằng Python và Rust
  (`cf-render` hoặc binary nhỏ gọi `generate_docx`), so `docx_texts` từng đoạn; in khác biệt; chỉ được khác ở KB-03/KB-05.
- **File:** `tools/compare_outputs.py`
- **Xong khi:** script in `0 khác biệt ngoài KB`.
- **Phụ thuộc:** T5.02

### T5.05 — Hướng dẫn sử dụng · S
- [ ] Xong
- **Làm:** `HUONG_DAN.md` của bản Rust: cài (chép exe), thư mục dữ liệu, **không mở cùng lúc với bản Python**, sao lưu,
  cách sửa mẫu Word sang `{{?KHOA}}`, các thay đổi so với bản cũ (tóm tắt `06_KHAC_BIET_CO_CHU_Y.md` bằng lời người dùng).
- **File:** `contract_rust/HUONG_DAN.md`
- **Xong khi:** chủ dự án đọc và đồng ý.
- **Phụ thuộc:** T5.02

### T5.06 — Kiểm công khai · XS
- [ ] Xong
- **Làm:** chạy `python scripts/check_public_repo.py` trên toàn repo (gồm `contract_rust/`); sửa mọi vi phạm.
- **Xong khi:** script sạch.
- **Phụ thuộc:** T5.05

### T5.07 — Nghiệm thu và chuyển đổi (ĐD5) · M
- [ ] Xong
- **Làm:** tick đủ `docs/07_KIEM_THU.md` §4; sao lưu DB thật (sqlite backup API, vào `backups/`); chủ dự án chuyển sang bản Rust.
  Sau 2 tuần dùng ổn: chủ dự án quyết định xóa bản Python → mở Đợt 7 (migration Rust 1–4 ở `02_DU_LIEU.md` §5.3) thành task mới.
- **File:** `docs/NGHIEM_THU.md`
- **Xong khi:** **ĐD5** — chủ dự án ký nghiệm thu.
- **Phụ thuộc:** T5.03, T5.04, T5.06
