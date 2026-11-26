# 01 — Kiến trúc

## 1. Workspace

```
contract_rust/
├── Cargo.toml                 # [workspace], resolver = "3", edition 2024 cho mọi crate
├── rust-toolchain.toml        # channel = "1.95.0"  (Slint 1.19.1 yêu cầu Rust 1.95)
├── crates/
│   ├── cf-core/               # Nghiệp vụ thuần. KHÔNG I/O, KHÔNG rusqlite, KHÔNG file.
│   ├── cf-docx/               # Engine docx: đọc mẫu, điền, phân tích placeholder.
│   ├── cf-db/                 # SQLite + Store trong RAM: mở DB, kiểm schema, nạp, ghi-xuống.
│   ├── cf-services/           # Ca sử dụng: gộp core + db + docx + file + Excel.
│   ├── cf-app/                # Binary Slint. Chỉ UI và nối sự kiện → cf-services.
│   └── cf-testkit/            # Chỉ dùng làm dev-dependency: đọc fixture, dựng docx/DB mẫu cho test.
├── fixtures/                  # Fixture vàng (sinh bởi tools/gen_fixtures.py) — chỉ đọc
├── tools/gen_fixtures.py
└── docs/
```

**Luật phụ thuộc (một chiều, không vòng):**

```
cf-app ──► cf-services ──► cf-db ──► cf-core
                   └──► cf-docx
```

- `cf-core` không phụ thuộc crate nội bộ nào.
- `cf-docx` không biết gì về hợp đồng hoặc `cf-core`: nhận `HashMap<String,String>` + danh sách dòng lặp, trả bytes.
- `cf-app` không gọi `cf-db` trực tiếp; mọi thứ đi qua `cf-services`.

## 2. Thư viện (đã tra phiên bản bằng `cargo search` ngày 25/11/2026)

Khai báo ở `[workspace.dependencies]`, crate con dùng `x = { workspace = true }`.

| Crate | Phiên bản | Dùng ở | Ghi chú |
|---|---|---|---|
| `rust_decimal` | `1.43` | core, db, services | feature `serde-with-str` |
| `chrono` | `0.4.45` | core, db, services | feature mặc định tắt bớt: `["clock","std","serde"]` |
| `regex` | `1.13` | core, docx | |
| `unicode-normalization` | `0.1.26` | core | NFD để bỏ dấu |
| `serde` / `serde_json` | `1.0.229` / `1.0.154` | mọi crate | `derive` |
| `thiserror` | `2.0` | mọi crate thư viện | |
| `zip` | `9.8` | docx, services | tắt mặc định, bật `deflate` |
| `quick-xml` | `0.42` | docx | |
| `rusqlite` | `0.43` | db, services | feature `bundled` (không cần SQLite cài sẵn) |
| `calamine` | `0.38` | services | đọc .xlsx |
| `rust_xlsxwriter` | `0.104` | services | ghi .xlsx |
| `sha2` | `0.11` | services | băm mẫu (mã `BUNDLED-…`) |
| `uuid` | `1.29` | services | feature `v4` (mã batch job, tên file mẫu) |
| `fs4` | `1.1` | db | khóa file `.cf_writer.lock` qua API `cf-db::lock` |
| `slint` / `slint-build` | `1.19.1` | app | |
| `rfd` | `0.18` | app | hộp thoại Mở/Lưu file native |
| `open` | `5.4` | app | mở file/thư mục bằng ứng dụng mặc định |
| `tracing` / `tracing-subscriber` | `0.1.44` / `0.3.23` | app, services | log ra `logs/cf-rust.log` |
| `anyhow` | `1.0` | **chỉ** cf-app `main.rs` | |
| `tempfile` | `3.27` | dev-dependency | thư mục tạm cho test |
| `embed-resource` | `3.0.12` | app build script | biên dịch resource `.rc` để nhúng icon Windows vào exe |

Không thêm thư viện ngoài danh sách khi chưa ghi lý do vào file này.

## 3. Quy ước bắt buộc

### 3.1 Số tiền và số thập phân

- Mọi số tiền/số lượng/thuế suất: `rust_decimal::Decimal`. **Cấm `f64`** trong nghiệp vụ.
- "Làm tròn tới đồng, 0,5 lên" (Python `ROUND_HALF_UP`) =
  `d.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)`.
- "Làm tròn lên theo bội số" (Python `ROUND_CEILING`) = `RoundingStrategy::ToPositiveInfinity`.
- **Cắt, không làm tròn** ở các chỗ sau (Python dùng `int(...)`), xem fixture `formatting.json`:
  `fmt_currency`, `amount_to_words`, cột tiền trong Excel xuất ra. Dùng `d.trunc()`.
- Đọc số người gõ trên form hợp đồng (`safe_decimal`): **bỏ hết dấu chấm rồi đổi phẩy thành chấm**.
  Vì vậy `"82.5"` → `825`. Đây là hành vi đúng của bản Python, đừng sửa (xem fixture).
- Đọc số "máy" (Excel, JSON dòng hợp đồng): `Decimal::from_str` trực tiếp, `"82.5"` → `82.5`.

### 3.2 Ngày giờ

- Ngày nghiệp vụ: `chrono::NaiveDate`. "Hôm nay" = `Local::now().date_naive()` (Python `date.today()`).
- Thời điểm lưu DB (`created_at`, `updated_at`, `occurred_at`, `*_uploaded_at`, `deleted_at`):
  **giờ UTC không múi giờ** (Python `datetime.utcnow()`), định dạng ghi
  `%Y-%m-%d %H:%M:%S%.6f` (luôn 6 chữ số micro giây). Đọc chấp nhận cả có và không có phần micro giây.
- Tên file sinh ra dùng **giờ địa phương** (`Local::now()`), định dạng `%Y%m%d_%H%M%S`.
- Hằng tháng/kỳ: dùng hàm `add_months` của `cf-core::dates` (kẹp cuối tháng), không dùng
  `chrono::Months` trực tiếp ở nơi khác.

### 3.3 Lỗi

- Mỗi crate thư viện có `pub enum Error` dùng `thiserror`.
- Lỗi người dùng nhìn thấy: biến thể mang `String` là **câu tiếng Việt y hệt bản Python**.
  Ví dụ `cf_core::pricing::PricingError::Invalid(String)` với chuỗi `"Số lượng phải lớn hơn 0."`.
- `cf-services` trả `Result<T, ServiceError>`:
  ```rust
  pub enum ServiceError {
      NotFound(String),     // Python LookupError / HTTP 404
      Invalid(String),      // Python ValueError — hiện nguyên câu cho người dùng
      FileMissing(String),  // Python FileNotFoundError
      Io(std::io::Error),
      Db(cf_db::Error),
  }
  impl ServiceError { pub fn user_message(&self) -> String }
  ```
- Không `unwrap()`/`expect()` trong code thư viện, trừ hằng regex dựng trong `LazyLock`.

### 3.4 Văn bản tiếng Việt

- Bỏ dấu: thay tay `Đ→D, đ→d` (và nơi Python có thay `Ơ/ơ/Ư/ư`) **trước**, rồi NFD, rồi bỏ ký
  tự thuộc nhóm "Mn" (combining marks). Hàm chung: `cf_core::text::strip_diacritics`.
- So khớp tìm kiếm không phân biệt dấu (ô lọc khách hàng): `strip_diacritics(s).to_lowercase()`.
- Python `str.upper()`/`lower()` trên tiếng Việt = Rust `to_uppercase()`/`to_lowercase()`.
- Regex `\w` của Python là Unicode → dùng `regex` crate mặc định (cũng Unicode).

### 3.5 Đặt tên trong code

- Tên hàm Rust = tên hàm Python bỏ gạch dưới đầu: `_fmt_currency` → `fmt_currency`.
- Mỗi hàm port có doc comment dòng đầu: `/// Port của app/services/pricing.py::price_line`.

## 4. Thư mục dữ liệu (giống hệt `app/paths.py`)

Thứ tự chọn **data root**:

1. Biến môi trường `CONTRACTFORGE_DATA_ROOT` nếu có.
2. `%LOCALAPPDATA%\ContractForge` nếu trong đó có `contract_manager.db`.
3. `<thư mục chứa file .exe>\data` nếu trong đó có `contract_manager.db` (cài đặt kiểu cũ).
4. Ngược lại: `%LOCALAPPDATA%\ContractForge` (cài mới).

Các đường dẫn con (tạo nếu thiếu khi khởi động):

| Hằng | Đường dẫn |
|---|---|
| `DB_PATH` | `<root>/contract_manager.db` |
| `UPLOAD_TEMPLATES_DIR` | `<root>/uploads/templates` |
| `SIGNED_SCANS_DIR` | `<root>/uploads/signed_scans` |
| `CUSTOMER_DOCS_DIR` | `<root>/uploads/customer_docs` (con: `<customer_id>/`) |
| `INVOICE_DOCS_DIR` | `<root>/uploads/invoice_docs` |
| `PAYMENT_SLIPS_DIR` | `<root>/uploads/payment_slips` |
| `OUTPUT_CONTRACTS_DIR` | `<root>/outputs/contracts` |
| `OUTPUT_BUNDLES_DIR` | `<root>/outputs/bundles/<contract_id>/` |
| `OUTPUT_BATCH_DIR` | `<root>/outputs/batch` |
| `DOSSIER_DIR` | `<root>/dau-noi` |
| `LOGS_DIR` | `<root>/logs` |
| `BACKUPS_DIR` | `<root>/backups` |

**Assets** (mẫu đi kèm ứng dụng): `<thư mục exe>\assets\default_templates\`. Mẫu biểu 08a:
`template_08A.docx` nếu có, không thì `sample_form_08a.docx`. Bản Rust **nhúng sẵn**
`sample_form_08a.docx` và 5 file `sample_quote_*.docx`, 3 file `sample_request_*.docx` bằng
`include_bytes!` để luôn chạy được kể cả khi thiếu thư mục assets.

**Đường dẫn file trong DB:** bản Python lưu tuyệt đối. Bản Rust:
- **Đọc** qua một hàm duy nhất `paths::resolve_stored(&str) -> PathBuf`: tuyệt đối → dùng luôn;
  tương đối → nối vào data root.
- **Ghi** tuyệt đối (dạng Windows, dấu `\`) trong lúc còn dùng chung DB với Python. Khi xóa hẳn
  Python: đổi sang ghi tương đối so với data root + migration Rust số 1 đổi các dòng cũ.

## 5. Khóa ghi một tiến trình (QĐ-3)

- Khởi động: mở/tạo `<root>/.cf_writer.lock`, gọi `try_lock_exclusive()` (fs4).
- Thất bại → hộp thoại: "ContractForge đang mở ở một cửa sổ khác. Hãy đóng cửa sổ đó trước." rồi thoát.
- Khóa giữ tới khi thoát. Bản Python không biết khóa này → tài liệu hướng dẫn ghi rõ
  **không mở bản Python và bản Rust cùng lúc** (Store trong RAM sẽ không thấy thay đổi của tiến
  trình khác). Phòng hờ: trước mỗi lần ghi, Store đọc `PRAGMA data_version`; nếu khác lần nạp
  trước → nạp lại toàn bộ rồi mới ghi, và hiện cảnh báo.
- Kết nối SQLite: `PRAGMA foreign_keys=ON`, `busy_timeout = 5000`. **Không** đổi `journal_mode`
  (giữ mặc định DELETE để bản Python dùng chung không bất ngờ).

## 6. Store — bản sao dữ liệu trong RAM (QĐ-8)

```rust
pub struct Store {
    conn: rusqlite::Connection,          // chỉ Store chạm vào
    pub customers: BTreeMap<i64, Customer>,
    pub units: BTreeMap<i64, CustomerUnit>,
    pub documents: BTreeMap<i64, CustomerDocument>,
    pub products: BTreeMap<i64, Product>,
    pub templates: BTreeMap<i64, ContractTemplate>,
    pub contracts: BTreeMap<i64, Contract>,
    pub items: BTreeMap<i64, ContractItem>,
    pub events: BTreeMap<i64, ContractEvent>,
    pub settings: BTreeMap<String, String>,
    pub issuers: BTreeMap<i64, QuotationIssuer>,
    idx: Indexes,                        // dựng lại sau mỗi lần ghi (xem dưới)
}
```

- **Nạp:** `Store::load(path)` đọc toàn bộ các bảng (vài nghìn dòng, vài ms).
- **Chỉ mục** (`Indexes`), dựng lại toàn bộ sau mỗi lần ghi (rẻ ở quy mô này, không lo lệch):
  `contract_by_number: HashMap<String, i64>`, `contracts_by_customer`, `children_by_parent`,
  `renewals_by_old`, `items_by_contract` (đã sắp theo `sort_order`), `units_by_customer`,
  `docs_by_customer`, `events_by_contract`, `customer_by_code`, `product_by_code`, và
  `search_key: HashMap<i64, String>` = chuỗi đã bỏ dấu + chữ thường của mỗi khách hàng/hợp đồng
  để tìm kiếm không dấu.
- **Ghi:** mọi thay đổi đi qua `Store::apply(Vec<Change>)`:
  1. mở transaction SQLite → thực thi các `Change` (Insert/Update/Delete có tên cột) → commit;
  2. **chỉ sau khi commit thành công** mới nạp lại toàn bộ `BTreeMap` từ DB (vài ms) và dựng lại chỉ mục;
  3. commit lỗi → RAM giữ nguyên, trả lỗi. RAM không bao giờ đi trước DB.
- `id` mới lấy từ `last_insert_rowid()` của SQLite, không tự sinh trong RAM.
- Truy vấn = hàm thuần trên `&Store` (lọc iterator). Không có SQL `SELECT` nào ngoài `Store::load`.

## 7. Luồng chạy của cf-app

```
main()
 ├─ resolve data root → tạo thư mục con
 ├─ lấy khóa ghi (.cf_writer.lock)
 ├─ cf_db::open(DB_PATH)
 │    ├─ DB chưa có → tạo schema (02_DU_LIEU §2) + seed trung tính
 │    └─ DB có → kiểm schema (02_DU_LIEU §5); thiếu cột → báo lỗi, thoát
 ├─ Store::load
 ├─ cf_services::startup(): quét hết hạn, nhập mẫu cục bộ
 ├─ hẹn giờ: mỗi lần qua 00:00 giờ địa phương → quét hết hạn lại
 └─ chạy cửa sổ Slint
```

**Mô hình luồng:**

- Một **luồng worker** sở hữu `Store` (và `Connection`). UI gửi lệnh qua
  `std::sync::mpsc::Sender<Command>`; mỗi lệnh mang closure trả kết quả về UI bằng
  `slint::invoke_from_event_loop`.
- Việc nặng (sinh hàng loạt, xuất Excel, dựng hồ sơ đấu nối): worker chạy từng bước, gửi
  tiến độ `Progress { done, total }` về UI, kiểm cờ hủy `Arc<AtomicBool>` giữa các bước.
- Luồng UI **không bao giờ** chạm vào `Store` hay file.

## 8. Thay thế tính năng web → native

| Bản Python (web) | Bản Rust (native) |
|---|---|
| FastAPI router, URL | Hàm `cf-services` gọi thẳng; điều hướng màn hình trong Slint |
| Flash message sau redirect | Thanh thông báo (toast) trong cửa sổ, màu theo loại success/warning/danger |
| `FileResponse` tải .docx/.zip/.xlsx | Hộp thoại **Lưu** (rfd), tên gợi ý = tên file Python sẽ trả; với file đã nằm trong `outputs/` thì nút **Mở file** và **Mở thư mục** (crate `open`) |
| Xem PDF scan/hóa đơn inline trong trình duyệt | Mở bằng ứng dụng mặc định của Windows (`open::that`) |
| Upload file qua form | Hộp thoại **Mở** (rfd) có bộ lọc đuôi file |
| JSON endpoint (`/contracts/product-options`, `/price-preview`, `/customer-units`, `/suggest-number`, `/api/search`, `/api/stats`) | Gọi hàm service tương ứng trực tiếp từ handler UI |
| Chart.js trên dashboard | Vẽ bằng `Rectangle`/`Path` của Slint (cột, tròn) |
| `static/dia_chinh.json` (chọn tỉnh/xã) | Nhúng `include_str!`, parse một lần khi khởi động |
| Flatpickr chọn ngày | Ô nhập `dd/mm/yyyy` + `DatePickerPopup` của Slint std-widgets |
| Middleware chặn origin lạ, bind 127.0.0.1 | Không áp dụng: không có socket |
| `content_disposition` RFC 5987 | Không cần: tên file đi thẳng vào hộp thoại Lưu |
