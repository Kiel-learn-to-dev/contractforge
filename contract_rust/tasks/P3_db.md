# Đợt 3 — `cf-db`: SQLite và Store trong RAM

Blueprint: `docs/02_DU_LIEU.md` (toàn bộ), `docs/01_KIEN_TRUC.md` §4–6.
Mọi file trong `crates/cf-db/`. Test dùng DB tạm (`tempfile`) tạo từ `fixtures/schema_baseline.sql`.
**Không bao giờ** mở `data/contract_manager.db` thật trong test.

---

### T3.01 — Lỗi và codec Decimal · S
- [ ] Xong
- **Làm:** `pub enum DbError { Sqlite(rusqlite::Error), Schema(String), Codec(String), Locked }`.
  `codec::read_decimal(ValueRef) -> Result<Option<Decimal>>` (INTEGER/REAL/TEXT/NULL theo `02_DU_LIEU.md` §3);
  `codec::write_decimal(Option<Decimal>) -> rusqlite::types::Value` (TEXT chuẩn hóa).
- **File:** `src/lib.rs`, `src/error.rs`, `src/codec.rs`
- **Test (unit):** bảng tạm `t(x NUMERIC(18,2))`: ghi `82.5` → đọc lại `82.5` và `typeof(x)`=`real`; ghi `24000000.00` → `typeof`=`integer`;
  đọc REAL `0.1` → `Decimal("0.1")` (không phải `0.1000000000000000055…`).
- **Xong khi:** `cargo test -p cf-db codec::decimal`.
- **Phụ thuộc:** T0.03

### T3.02 — Codec ngày, giờ, bool · XS
- [ ] Xong
- **Làm:** `read_date`/`write_date` (`YYYY-MM-DD`); `read_datetime` (có hoặc không `.ffffff`)/`write_datetime`
  (`%Y-%m-%d %H:%M:%S%.6f`); `read_bool` (0/1, `'0'/'1'`). `opt_text`: chuỗi trim rỗng → NULL.
- **File:** `src/codec.rs`
- **Test (unit):** `"2026-03-24 04:40:37.560192"` và `"2026-03-24 04:40:37"` đọc được; ghi ra đúng 6 chữ số micro giây.
- **Xong khi:** `cargo test -p cf-db codec`.
- **Phụ thuộc:** T3.01

### T3.03 — Đường dẫn dữ liệu · S
- [ ] Xong
- **Làm:** `paths::resolve_data_root(env, localappdata, exe_dir) -> PathBuf` theo 4 bước `01_KIEN_TRUC.md` §4
  (tham số truyền vào để test được); `struct DataPaths` với mọi đường dẫn con; `ensure_dirs()`;
  `resolve_stored(&str) -> PathBuf` (KB-16).
- **Nguồn:** `ContractForge/app/paths.py::_resolve_data_root, _refresh_derived_paths`
- **File:** `src/paths.rs`
- **Test (unit, tempfile):** 4 trường hợp chọn root; `resolve_stored("uploads/x.pdf")` nối vào root; tuyệt đối giữ nguyên.
- **Xong khi:** `cargo test -p cf-db paths`.
- **Phụ thuộc:** T3.01

### T3.04 — Khóa ghi một tiến trình · XS
- [ ] Xong
- **Làm:** `lock::acquire(root) -> Result<WriterLock, DbError>` (fs4 `try_lock_exclusive` trên `.cf_writer.lock`);
  đang bị khóa → `DbError::Locked`. Khóa nhả khi `drop`.
- **File:** `src/lock.rs`
- **Test (unit):** lấy 2 lần trong cùng test → lần 2 `Locked`; drop lần 1 → lấy lại được.
- **Xong khi:** `cargo test -p cf-db lock`.
- **Phụ thuộc:** T3.01

### T3.05 — Mở DB và tạo DB mới · S
- [ ] Xong
- **Làm:** `open(path) -> Result<Connection>`: `foreign_keys=ON`, `busy_timeout=5000`, không đổi journal_mode.
  File chưa có → chạy `include_str!("../../../fixtures/schema_baseline.sql")` trong transaction + seed sản phẩm
  theo `schema.json → seed_products` (chỉ khi bảng rỗng; `created_at`/`updated_at` = giờ UTC hiện tại).
- **Nguồn:** `ContractForge/app/models/seed.py::seed_products`
- **File:** `src/open.rs`
- **Test (unit):** DB mới có đủ bảng; có 2 sản phẩm; mở lần 2 không seed thêm.
- **Xong khi:** `cargo test -p cf-db open`.
- **Phụ thuộc:** T3.02

### T3.06 — Kiểm schema DB có sẵn · XS
- [ ] Xong
- **Làm:** `check_schema(conn) -> Result<(), DbError>` theo `schema.json → required_columns`; thiếu → `Schema` với câu
  `02_DU_LIEU.md` §5.1 + danh sách bảng.cột thiếu. Cột thừa → bỏ qua.
- **File:** `src/schema.rs`
- **Test (unit):** DB mới → OK; xóa một cột (tạo bảng thiếu cột) → lỗi nêu đúng tên cột; thêm cột lạ → OK.
- **Xong khi:** `cargo test -p cf-db schema`.
- **Phụ thuộc:** T3.05

### T3.07 — Struct khách hàng · S
- [ ] Xong
- **Làm:** `Customer`, `CustomerUnit`, `CustomerDocument` (mọi cột ở `required_columns`), `from_row(&Row)` đọc **theo tên cột**,
  `to_values()` cho insert/update. `Customer::to_view() -> cf_core::render_context::CustomerView`.
- **File:** `src/model/customer.rs`, `src/model/mod.rs`
- **Test (unit):** insert bằng SQL thô → `from_row` đúng mọi trường; roundtrip `to_values` → `from_row` bằng nhau.
- **Xong khi:** `cargo test -p cf-db model::customer`.
- **Phụ thuộc:** T3.05, T1.35

### T3.08 — Struct sản phẩm và mẫu · S
- [ ] Xong
- **Làm:** `Product` (`feature_list` JSON hỏng → `[]`), `ContractTemplate` (`detected_placeholders`, `field_mapping` JSON).
  `Product::to_rule() -> cf_core::pricing::ProductRule`, `ContractTemplate::to_info() -> TemplateInfo`.
- **File:** `src/model/product.rs`, `src/model/template.rs`
- **Test (unit):** JSON hỏng/NULL → rỗng; roundtrip.
- **Xong khi:** `cargo test -p cf-db model::product model::template`.
- **Phụ thuộc:** T3.07, T1.33, T1.41

### T3.09 — Struct hợp đồng, dòng, sự kiện · S
- [ ] Xong
- **Làm:** `Contract` (status là `ContractStatus`, `render_data` giữ `serde_json::Value`, `selected_unit_names` JSON),
  `ContractItem`, `ContractEvent`. `Contract::to_view()`, `Contract::evidence()`.
- **File:** `src/model/contract.rs`
- **Test (unit):** status lạ `"Bogus"` → `Unknown("Bogus")`; roundtrip đủ cột.
- **Xong khi:** `cargo test -p cf-db model::contract`.
- **Phụ thuộc:** T3.07, T1.21

### T3.10 — Cài đặt và người phát hành báo giá · XS
- [ ] Xong
- **Làm:** `AppSetting` (key, value); `QuotationIssuer`. `party_b(settings) -> PartyB` (13 khóa, rỗng → mặc định `""`);
  `get_setting(key, default)` theo luật `02_DU_LIEU.md` §6.
- **Nguồn:** `ContractForge/app/services/settings_service.py`
- **File:** `src/model/setting.rs`
- **Test (unit):** khóa có nhưng rỗng → default.
- **Xong khi:** `cargo test -p cf-db model::setting`.
- **Phụ thuộc:** T3.07

### T3.11 — `Store::load` · S
- [ ] Xong
- **Làm:** struct `Store` theo `01_KIEN_TRUC.md` §6; nạp mọi bảng vào `BTreeMap`; ghi nhận `PRAGMA data_version`.
- **File:** `src/store/mod.rs`
- **Test (unit):** DB tạm có 2 khách, 3 hợp đồng, 4 dòng → đếm đúng.
- **Xong khi:** `cargo test -p cf-db store::load`.
- **Phụ thuộc:** T3.08, T3.09, T3.10

### T3.12 — Chỉ mục · S
- [ ] Xong
- **Làm:** `Indexes` (danh sách §6) dựng từ Store; `search_key` dùng `cf_core::text::search_key` trên
  khách hàng: `code + legal_name + short_name`; hợp đồng: `contract_number + notes + tên khách`.
  Các hàm tra cứu: `contract_by_number`, `contracts_of_customer`, `children_of`, `renewals_of`, `items_of`
  (đã sắp `sort_order`), `units_of` (sắp `sort_order, id`), `docs_of` (sắp `doc_type, id`), `events_of` (sắp `occurred_at`).
- **File:** `src/store/index.rs`
- **Test (unit):** mỗi hàm tra cứu một ca; tìm `"dong tam"` ra khách "Đồng Tâm".
- **Xong khi:** `cargo test -p cf-db store::index`.
- **Phụ thuộc:** T3.11, T1.02

### T3.13 — `Store::apply` (ghi transaction rồi mới cập nhật RAM) · M
- [ ] Xong
- **Làm:** `enum Change { Insert{table, values}, Update{table, id, values}, Delete{table, id}, SetSetting{key, value} }`
  (values = danh sách (cột, Value)); `apply(Vec<Change>) -> Result<Vec<i64 /*id mới*/>>`: một transaction;
  thành công → **nạp lại toàn bộ Store** (vài ms ở quy mô này; tránh mọi lỗi lệch do CASCADE), dựng lại chỉ mục;
  thất bại → RAM không đổi. `Insert` có thể tham chiếu id vừa tạo trong cùng lô bằng `Value::Ref(idx)`
  (cần cho tạo hợp đồng + dòng + sự kiện).
  **Lưu ý:** `contract_events.contract_id` **không** có ON DELETE CASCADE (Python xóa qua ORM) → nơi xóa hợp đồng
  phải tự thêm `Delete` các sự kiện trước. `contract_items`, `customer_units` thì có CASCADE.
- **File:** `src/store/apply.rs`
- **Test (unit):** insert hợp đồng + 2 dòng + 1 sự kiện trong một lô → có trong RAM với id đúng;
  lô có lệnh vi phạm UNIQUE → không dòng nào được ghi, RAM giữ nguyên; xóa hợp đồng (kèm xóa sự kiện) → dòng cũng biến mất khỏi RAM;
  xóa hợp đồng còn sự kiện mà không xóa sự kiện → lỗi khóa ngoại, RAM giữ nguyên.
- **Xong khi:** `cargo test -p cf-db store::apply`.
- **Phụ thuộc:** T3.12

### T3.14 — Phát hiện tiến trình khác ghi DB · XS
- [ ] Xong
- **Làm:** trước mỗi `apply`: đọc `PRAGMA data_version`; khác → `reload()` toàn bộ, trả cờ `reloaded=true` để UI cảnh báo.
- **File:** `src/store/mod.rs`
- **Test (unit):** mở connection thứ hai, insert một khách → `apply` kế tiếp báo `reloaded` và RAM có khách đó.
- **Xong khi:** `cargo test -p cf-db store::external_change`.
- **Phụ thuộc:** T3.13

### T3.15 — Đồng hồ (Clock) · XS
- [ ] Xong
- **Làm:** `trait Clock { fn today(&self) -> NaiveDate; fn now_utc(&self) -> NaiveDateTime; fn now_local(&self) -> NaiveDateTime; }`
  + `SystemClock` + `FixedClock` (test). Đặt ở cf-db để cf-services dùng chung.
- **File:** `src/clock.rs`
- **Xong khi:** `cargo test -p cf-db clock`.
- **Phụ thuộc:** T3.01

### T3.16 — Dữ liệu mẫu cho test ca sử dụng · S
- [ ] Xong
- **Làm:** trong `cf-testkit`: `pub fn seeded_db(dir) -> PathBuf` tạo DB từ baseline + dữ liệu hư cấu:
  2 nhóm sản phẩm (G1: gói chính `G1-MAIN` 440909đ VAT 10, gói bổ sung `G1-ADD` 100000đ; G2: `G2-MAIN`),
  3 khách (một có 2 đơn vị con, một tạm ngưng, một đã xóa), 4 mẫu (hợp đồng G1, kèm theo G1 "Nghiệm thu",
  phụ lục bổ sung dùng chung, báo giá G1) trỏ tới bản sao `sample_service_contract.docx` trong `dir`,
  5 hợp đồng ở các trạng thái khác nhau. Không dùng tên thật.
- **File:** `crates/cf-testkit/src/seed.rs`
- **Xong khi:** `Store::load` trên DB đó thành công (test trong cf-db).
- **Phụ thuộc:** T3.13

### T3.17 — Kiểm tương thích hai chiều với Python · M
- [ ] Xong
- **Làm:** `tools/compat_check.py` (chạy bằng `.venv\Scripts\python.exe -I`):
  1. sao DB thật bằng sqlite backup API vào thư mục tạm; **viết lại mọi cột đường dẫn** (`*_path`, `file_path`)
     sang thư mục tạm (copy file tương ứng nếu có); 2. chạy binary `cf-db-compat` (tạo trong task này ở
     `crates/cf-db/src/bin/cf-db-compat.rs`: nạp Store, insert 1 khách + 1 hợp đồng + 1 dòng + 1 sự kiện, update 1 hợp đồng);
  3. mở bằng SQLAlchemy của app (`CONTRACTFORGE_DATA_ROOT` = thư mục tạm): đọc lại mọi bản ghi, so giá trị;
  4. Python ghi 1 bản ghi → chạy lại `cf-db-compat --verify` đọc được; 5. so SHA-256 DB thật trước/sau: giống hệt.
- **File:** `tools/compat_check.py`, `crates/cf-db/src/bin/cf-db-compat.rs`
- **Xong khi:** script in `COMPAT OK`; **ĐD3**: chủ dự án xem kết quả.
- **Phụ thuộc:** T3.16

---

## Điểm kiểm Đợt 3
- [ ] `cargo test -p cf-db` xanh
- [ ] `compat_check.py` in `COMPAT OK`, hash DB thật không đổi
- [ ] Không có `SELECT` nào ngoài `store/mod.rs` (`rg "SELECT" crates/cf-db/src` chỉ ra file đó và `schema.rs`)
