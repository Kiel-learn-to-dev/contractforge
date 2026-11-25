# ContractForge Rust — Kế hoạch task

Rã từ blueprint `docs/00`–`08`. Mỗi task đủ nhỏ để **một phiên AI** làm xong: một file hoặc một
hàm, có nguồn Python làm chuẩn, có test, có lệnh "xong khi".

## Cách làm một task (bắt buộc, đọc trước task đầu tiên)

1. Đọc `docs/00_TONG_QUAN.md` (một lần) và mục blueprint được trỏ trong task.
2. Mở **file Python nguồn** ghi trong task, đọc đúng hàm đó. Python là đặc tả về **kết quả**,
   không phải về cách viết code. Đường dẫn Python tính từ gốc repo, ví dụ `ContractForge/app/services/pricing.py`.
3. Nếu task có mã `KB-xx`: đọc dòng đó trong `docs/06_KHAC_BIET_CO_CHU_Y.md` và **làm theo KB**, không theo Python.
4. Viết test trước (fixture hoặc ví dụ ghi trong task), rồi viết code cho test xanh.
5. Chạy lệnh "Xong khi". Rồi chạy kiểm chung (DoD bên dưới).
6. Không sửa file ngoài danh sách "File" của task, trừ `lib.rs`/`mod.rs` để khai báo module.
7. Không thêm thư viện ngoài bảng ở `docs/01_KIEN_TRUC.md` §2.
8. Kẹt quá 2 lần cùng một lỗi → dừng, ghi lại lỗi vào cuối task trong file đợt, báo người giao việc.

## Definition of Done (mọi task)

- [ ] `cargo fmt --all -- --check` sạch
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` sạch
- [ ] `cargo test --workspace` xanh (không chỉ test của task)
- [ ] Không có `unwrap()`/`expect()` trong code không phải test (trừ regex hằng trong `LazyLock`)
- [ ] Hàm port có doc comment dòng đầu `/// Port của <đường dẫn Python>::<hàm>`
- [ ] Không có dữ liệu tổ chức thật (tên đơn vị, khuôn số thật, đường dẫn máy) trong code/test
- [ ] Tick ô của task trong file đợt

Lệnh gộp: `pwsh contract_rust/tools/check.ps1` (tạo ở T0.04).

## Quy ước mã task

`T<đợt>.<số>` — ví dụ `T1.12`. Cỡ: **XS** (1 hàm) · **S** (1–2 file) · **M** (3–5 file).
Không có task L/XL. Thứ tự trong file đợt = thứ tự nên làm.

## Các đợt

| Đợt | File | Số task | Làm song song được với |
|---|---|---|---|
| 0 — Khung + bản thử UI | [`tasks/P0_khung.md`](tasks/P0_khung.md) | 9 | — |
| 1 — `cf-core` nghiệp vụ thuần | [`tasks/P1_core.md`](tasks/P1_core.md) | 41 | Đợt 2 |
| 2 — `cf-docx` engine | [`tasks/P2_docx.md`](tasks/P2_docx.md) | 19 | Đợt 1 |
| 3 — `cf-db` Store | [`tasks/P3_db.md`](tasks/P3_db.md) | 17 | sau T1.21, T1.35 |
| 4 — Lát cắt tính năng (service + màn hình) | [`tasks/P4_tinh_nang.md`](tasks/P4_tinh_nang.md) | 99 | các lát cắt độc lập nhau sau khi xong khung app |
| 5 — Đóng gói + nghiệm thu | [`tasks/P5_dong_goi.md`](tasks/P5_dong_goi.md) | 7 | — |

Ghi chú: `docs/08_LO_TRINH.md` tách "services" và "app" thành Đợt 4 và 5. Kế hoạch task gộp hai
phần đó thành **lát cắt dọc** (mỗi tính năng: service → màn hình) để mỗi lát cắt dùng thử được
ngay. Đợt đóng gói vì vậy là P5 (tương ứng Đợt 6 của lộ trình).

## Đồ thị phụ thuộc

```
T0.01–T0.04 khung workspace
   ├── P1 cf-core ──────────────┐
   ├── P2 cf-docx ──────────────┤
   │                            ├── P3 cf-db (cần T1.21 enum trạng thái, T1.35 view struct)
   │                            │      └── P4.A khung app + cf-services
   │                            │             ├── P4.B Khách hàng
   │                            │             ├── P4.C Sản phẩm
   │                            │             ├── P4.D Mẫu
   │                            │             └── P4.E Hợp đồng (cần B, C, D)
   │                            │                    ├── P4.F Phụ lục/gia hạn
   │                            │                    ├── P4.G Kèm theo + 08a
   │                            │                    ├── P4.H Hàng loạt
   │                            │                    ├── P4.J Hồ sơ đấu nối
   │                            │                    └── P4.K Dashboard + tìm kiếm
   │                            │             ├── P4.I Báo giá (cần C, D)
   │                            │             └── P4.L Cài đặt
   └── T0.05–T0.09 bản thử UI (ĐD0) — chặn P4.A
                                                    └── P5 đóng gói, nghiệm thu, xóa Python
```

## Điểm dừng (chủ dự án kiểm)

| Mã | Sau | Chủ dự án làm gì |
|---|---|---|
| ĐD0 | T0.09 | Tự gõ tiếng Việt bằng Unikey/EVKey vào bản thử; xem RAM. Đồng ý renderer |
| ĐD2 | T2.19 | Mở 5 file .docx do engine sinh ra bằng Word: không lỗi, định dạng đúng |
| ĐD3 | T3.17 | Xem kết quả kiểm tương thích trên bản sao DB thật |
| ĐD4-E | hết P4.E | Dùng thử tạo + sinh hợp đồng trên bản sao dữ liệu |
| ĐD4 | hết P4 | Dùng thử 1 tuần trên bản sao dữ liệu |
| ĐD5 | T5.07 | Nghiệm thu, chuyển dữ liệu thật, xóa bản Python |

## Rủi ro

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| Slint gõ tiếng Việt (IME Unikey/EVKey) lỗi | Cao | Bản thử Đợt 0 trước mọi màn hình; thử cả 2 renderer; dừng ở ĐD0 nếu hỏng |
| Engine docx làm hỏng file Word | Cao | Fixture + test "mọi entry không đổi giữ nguyên bytes" + ĐD2 mở bằng Word |
| Store trong RAM lệch DB khi bản Python cùng mở | Trung bình | Khóa ghi + `PRAGMA data_version` (T3.14) + hướng dẫn không mở song song |
| Sinh file trên bản sao DB xóa nhầm file thật | Cao | T3.17 bắt buộc viết lại đường dẫn trước; không bao giờ chạy ca sử dụng ghi file trên DB thật trong test |
| AI yếu "sửa" hành vi lạ của Python | Trung bình | Fixture vàng + danh sách KB; quy tắc TODO(KB) |
| Python đổi nghiệp vụ trong lúc port | Trung bình | Blueprint ghi commit gốc; đổi thì chạy lại `gen_fixtures.py`, diff fixture |
