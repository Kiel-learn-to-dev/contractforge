# ContractForge

> Ứng dụng desktop quản lý hợp đồng — một người dùng, chạy offline, viết bằng Rust + Slint.

ContractForge quản lý toàn bộ vòng đời hợp đồng: soạn, sinh văn bản Word từ mẫu của bạn,
theo dõi ký kết, hóa đơn, thanh toán đến hết hạn — **không cần internet và không gửi dữ liệu
đi đâu**. Mọi thứ nằm trong một tệp SQLite trên máy bạn.

## Tính năng

- Danh bạ khách hàng, đơn vị trực thuộc, giấy tờ đính kèm, nhập/xuất Excel.
- Mẫu Word: đánh dấu điểm điền bằng `{{TEN_TRUONG}}` (hoặc `{{?TEN_TRUONG}}` để ẩn dòng khi trống).
- Sinh hàng loạt cho nhiều khách hàng, đóng gói `.zip`.
- Vòng đời hợp đồng có kiểm soát, ghi lịch sử mọi lần đổi trạng thái.
- Bảng điều khiển, nhắc hạn, báo giá và phiếu yêu cầu báo giá từ cùng dữ liệu.
- Hồ sơ đấu nối: gom giấy tờ một khách hàng vào một thư mục (hardlink, không tốn thêm dung lượng).

## Vòng đời hợp đồng

```
Draft ──► Generated ──► Sent ──► Signed ──► Invoiced ──► PaidActive ──► Expired
                                    └──────────┴─────────────┴────────► Terminated
```

"Sắp hết hạn" không phải trạng thái — nó được tính từ `end_date` mỗi lần hiển thị.

## Xây dựng và chạy

Mã nguồn nằm trong [`contract_rust/`](contract_rust/) (Cargo workspace). Cần Rust toolchain
(xem `contract_rust/rust-toolchain.toml`).

```powershell
cd contract_rust
powershell -File tools/check.ps1          # fmt + clippy + test
powershell -File tools/build_release.ps1  # tạo dist/ContractForge.exe
```

Hướng dẫn sử dụng: [`contract_rust/HUONG_DAN.md`](contract_rust/HUONG_DAN.md). Tài liệu thiết kế:
[`contract_rust/docs/`](contract_rust/docs/).

## Nhánh

- `rust-rewrite` — nhánh phát triển chính (bản Rust).
- `pricing-model`, `main` — bản Python cũ, **chỉ để tham khảo**, không phát triển thêm.

## Giấy phép

Xem [LICENSE](LICENSE).
