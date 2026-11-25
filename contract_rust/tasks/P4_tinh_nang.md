# Đợt 4 — Lát cắt tính năng (service → màn hình)

Blueprint: `docs/03_NGHIEP_VU.md` (luật), `docs/05_GIAO_DIEN.md` (màn hình), `docs/01_KIEN_TRUC.md` §6–8.

Quy ước chung của đợt này:
- **Service** ở `crates/cf-services/src/<lát>/…`. Hàm nhận `&mut Ctx` (hoặc `&Ctx` nếu chỉ đọc), trả
  `Result<T, ServiceError>`. Không đụng Slint. Test ở `crates/cf-services/tests/<lát>_*.rs` dùng
  `cf_testkit::seeded_db` + `tempfile` + `FixedClock`. **Câu thông báo cho người dùng chép nguyên văn từ Python.**
- **Màn hình** ở `crates/cf-app/ui/screens/<tên>.slint` + `crates/cf-app/src/screens/<tên>.rs`. Không tính nghiệp vụ trong
  `.slint`. "Xong khi" của task UI = build được + danh sách kiểm tay ghi trong task (chạy trên DB tạo bởi `seeded_db`:
  `cargo run -p cf-app -- --data-root <thư mục tạm>`).
- Đường dẫn Python tính từ gốc repo (`ContractForge/app/...`).

---

## A. Khung ứng dụng và cf-services

### T4.A01 — `cf-services`: Ctx và ServiceError · S
- [ ] Xong
- **Làm:** `ServiceError` theo `01_KIEN_TRUC.md` §3.3 (+ `From<CoreError>`, `From<DbError>`, `From<DocxError>`, `From<io::Error>`);
  `pub struct Ctx { store: Store, paths: DataPaths, clock: Box<dyn Clock>, templates: TemplateCache }`;
  `pub fn event(contract_id: ChangeRef, kind, description, actor, now_utc) -> Change` (một chỗ duy nhất tạo sự kiện).
- **File:** `crates/cf-services/src/lib.rs`, `src/error.rs`, `src/ctx.rs`, `src/events.rs`
- **Xong khi:** `cargo test -p cf-services` (test: `user_message()` của `Invalid("x")` = `"x"`).
- **Phụ thuộc:** T3.16, T2.18

### T4.A02 — `cf-app`: khởi động · S
- [ ] Xong
- **Làm:** `main.rs`: đọc tham số `--data-root` (ghi đè, cho test) → `resolve_data_root` → `ensure_dirs` → `lock::acquire`
  (Locked → hộp thoại lỗi `01_KIEN_TRUC.md` §5, thoát) → `open` → `check_schema` (lỗi → hộp thoại, thoát) →
  `Store::load` → `Ctx` → cửa sổ. Log `tracing` ra `logs/cf-rust.log`. Đo và log thời gian khởi động.
  Renderer theo kết quả ĐD0.
- **File:** `crates/cf-app/src/main.rs`, `crates/cf-app/build.rs`, `crates/cf-app/ui/app.slint`
- **Xong khi:** chạy mở cửa sổ trống; mở cửa sổ thứ hai → báo đang mở.
- **Phụ thuộc:** T4.A01, T0.09

### T4.A03 — Luồng worker và lệnh · S
- [ ] Xong
- **Làm:** `worker.rs`: thread sở hữu `Ctx`; `type Job = Box<dyn FnOnce(&mut Ctx) + Send>`; `fn run<T>(job, on_done: impl FnOnce(T) + Send)`
  trả kết quả về UI bằng `slint::invoke_from_event_loop`; `Progress { done, total, label }` qua callback; cờ hủy `Arc<AtomicBool>`.
- **File:** `crates/cf-app/src/worker.rs`
- **Xong khi:** test thủ công: nút thử đếm khách hàng hiện đúng số mà UI không đứng.
- **Phụ thuộc:** T4.A02

### T4.A04 — Khung màn hình và điều hướng · S
- [ ] Xong
- **Làm:** thanh bên 9 mục (`05_GIAO_DIEN.md` §2), `enum Screen` (Rust) + `current-screen` (Slint), ngăn xếp Quay lại (20),
  phiên bản ở chân thanh bên. Mỗi màn hình tạm là placeholder có tiêu đề.
- **File:** `crates/cf-app/ui/app.slint`, `ui/sidebar.slint`, `src/nav.rs`
- **Xong khi:** bấm từng mục đổi màn hình; Quay lại hoạt động.
- **Phụ thuộc:** T4.A02

### T4.A05 — Theme và thành phần dùng chung · S
- [ ] Xong
- **Làm:** `ui/theme.slint` (màu 7 biến thể trạng thái, khoảng cách, cỡ chữ); `StatusBadge` (nhận `label`, `variant`);
  `MoneyText`, `Card`, `PageHeader`, `EmptyState`. View model Rust luôn đưa chuỗi đã định dạng (`fmt_currency`, `dd/mm/yyyy`).
- **File:** `crates/cf-app/ui/theme.slint`, `ui/components/*.slint`
- **Xong khi:** màn hình thử hiển thị đủ 10 nhãn trạng thái đúng màu.
- **Phụ thuộc:** T4.A04

### T4.A06 — Toast và hộp xác nhận · S
- [ ] Xong
- **Làm:** `Toast` (success/warning/danger; tự ẩn 5 s trừ danger); `ConfirmDialog` (tiêu đề, nội dung, nút Đồng ý/Hủy);
  API Rust `ui::toast(kind, msg)`, `ui::confirm(title, msg, on_ok)`.
- **File:** `crates/cf-app/ui/components/toast.slint`, `ui/components/confirm.slint`, `src/ui_helpers.rs`
- **Xong khi:** gọi thử hiện đúng; danger không tự ẩn.
- **Phụ thuộc:** T4.A05

### T4.A07 — Hộp thoại file và mở file · XS
- [ ] Xong
- **Làm:** `files::save_as(suggested_name, filter) -> Option<PathBuf>`, `files::pick_file(filters)`, `files::pick_files`,
  `files::open_path(p)`, `files::open_folder(p)`; ghi bytes ra đường dẫn chọn bằng `std::fs::write` (người dùng đã chọn, cho phép ghi đè).
- **File:** `crates/cf-app/src/files.rs`
- **Xong khi:** nút thử lưu một file .txt và mở thư mục chứa nó.
- **Phụ thuộc:** T4.A02

---

## B. Khách hàng

### T4.B01 — Service: danh sách khách hàng · S
- [ ] Xong
- **Làm:** `CustomerFilters {q, ward, customer_type, active_only, page, per_page, sort_by, sort_dir}`;
  `list_customers(&Store, f) -> (Vec<(Customer, contract_count)>, total)`; bỏ khách `is_deleted`; từ khóa khớp
  `search_key` (KB-11); `filter_options()` = danh sách xã/phường và loại khác rỗng, sắp xếp.
- **Nguồn:** `ContractForge/app/services/customer_service.py::list_customers, get_filter_options`
- **File:** `src/customers/list.rs`, `tests/customers_list.rs`
- **Xong khi:** `cargo test -p cf-services --test customers_list` (lọc, phân trang, sắp xếp, tìm không dấu, bỏ khách đã xóa).
- **Phụ thuộc:** T4.A01

### T4.B02 — Service: tạo và sửa khách hàng · S
- [ ] Xong
- **Làm:** `generate_next_code`, `create_customer(data)`, `update_customer(id, data)`; `_clean` (chỉ nhận các trường hợp lệ,
  trim, chuỗi rỗng → NULL, ngày đọc `parse_date_input`).
- **Nguồn:** `customer_service.py::generate_next_code, create_customer, update_customer, _clean, _CUSTOMER_FIELDS`
- **File:** `src/customers/edit.rs`, `tests/customers_edit.rs`
- **Xong khi:** test: mã tự sinh `KH-003` khi có `KH-001`, `KH-002`, `KH-TEST`; trùng mã báo đúng câu; sửa không xóa mã.
- **Phụ thuộc:** T4.B01

### T4.B03 — Service: tạm ngưng, xóa mềm, khôi phục · XS
- [ ] Xong
- **Nguồn:** `customer_service.py::toggle_active, delete_customer, bulk_delete_customers, restore_customer, list_deleted_customers`
- **File:** `src/customers/state.rs`, `tests/customers_state.rs`
- **Xong khi:** test: xóa → biến khỏi danh sách, có trong thùng rác (mới nhất trước); khôi phục → trở lại.
- **Phụ thuộc:** T4.B01

### T4.B04 — Service: đơn vị con và cơ sở tính phí · S
- [ ] Xong
- **Làm:** `save_sub_units(customer_id, Vec<UnitInput{id?, name, unit_type, address, notes}>)` (thêm/sửa/xóa theo id);
  `save_billing(customer_id, bill_main: bool, included_unit_ids: &[i64])`.
- **Nguồn:** `customer_service.py::save_sub_units`, `ContractForge/app/routers/customers.py::save_billing, _extract_sub_units`
- **File:** `src/customers/units.rs`, `tests/customers_units.rs`
- **Xong khi:** test: `total_units`/`unit_names_list` đổi đúng sau khi lưu.
- **Phụ thuộc:** T4.B02

### T4.B05 — Service: giấy tờ khách hàng · S
- [ ] Xong
- **Làm:** `upload_doc(customer_id, doc_type, label, bytes, original_name)` theo `03_NGHIEP_VU.md` §5 (magic bytes,
  5 MB, tên file, ghi mới → DB → xóa cũ); `delete_doc(customer_id, doc_id)` (kiểm doc thuộc khách); `doc_path(customer_id, doc_id)`.
- **Nguồn:** `ContractForge/app/routers/customers.py::upload_doc, delete_doc, _check_doc_upload`
- **File:** `src/customers/docs.rs`, `tests/customers_docs.rs`
- **Xong khi:** test: upload CCCD 2 lần → 1 bản ghi, file cũ bị xóa; `other` thiếu nhãn → `"Vui lòng nhập tên giấy tờ"`; file .exe → từ chối.
- **Phụ thuộc:** T4.B02

### T4.B06 — Màn hình danh sách khách hàng (S03) · S
- [ ] Xong
- **Làm:** bộ lọc, bảng (mã, tên, xã/phường, loại, số cơ sở, số HĐ, trạng thái), phân trang, chọn nhiều → xóa (xác nhận),
  nút Thêm / Nhập Excel / Xuất báo cáo / Thùng rác. Bấm dòng → chi tiết.
- **File:** `crates/cf-app/ui/screens/customers_list.slint`, `src/screens/customers_list.rs`
- **Xong khi:** kiểm tay: lọc "song xanh" ra khách "Sông Xanh"; xóa 2 khách cùng lúc.
- **Phụ thuộc:** T4.B01, T4.B03, T4.A06

### T4.B07 — Màn hình form khách hàng (S04) · M
- [ ] Xong
- **Làm:** mọi trường; chọn tỉnh → xã từ `ContractForge/static/dia_chinh.json` (nhúng `include_str!`, parse 1 lần — đọc cấu trúc file
  trước khi viết); bảng đơn vị con thêm/xóa dòng; lỗi hiện đầu form, giữ dữ liệu đã nhập.
- **Nguồn:** `ContractForge/templates/customers/form.html`
- **File:** `ui/screens/customer_form.slint`, `src/screens/customer_form.rs`, `src/dia_chinh.rs`
- **Xong khi:** kiểm tay: tạo khách có 2 đơn vị con, sửa lại, mã tự sinh.
- **Phụ thuộc:** T4.B02, T4.B04

### T4.B08 — Màn hình chi tiết khách hàng (S05) · M
- [ ] Xong
- **Làm:** khối hồ sơ; cơ sở tính phí (tick + Lưu); giấy tờ (upload qua `pick_file`, mở bằng app mặc định, xóa có xác nhận);
  danh sách hợp đồng (bấm → chi tiết HĐ); nút Sửa, Tạm ngưng/Kích hoạt, Tạo hợp đồng (mở S14 chọn sẵn khách).
- **Nguồn:** `ContractForge/templates/customers/detail.html`
- **File:** `ui/screens/customer_detail.slint`, `src/screens/customer_detail.rs`
- **Xong khi:** kiểm tay đủ các nút.
- **Phụ thuộc:** T4.B04, T4.B05, T4.A07

### T4.B09 — Màn hình thùng rác · XS
- [ ] Xong
- **File:** `ui/screens/customers_trash.slint`, `src/screens/customers_trash.rs`
- **Xong khi:** khôi phục được.
- **Phụ thuộc:** T4.B03

### T4.B10 — Service: nhập Excel phẳng · S
- [ ] Xong
- **Làm:** calamine đọc sheet đầu; dò tiêu đề trong 5 dòng đầu theo `EXCEL_COLUMN_MAP`; có mã → cập nhật, không mã → tạo;
  `ImportResult {created, updated, errors: Vec<(row, msg)>}`. Một transaction cho cả file.
- **Nguồn:** `customer_service.py::import_from_excel, EXCEL_COLUMN_MAP`
- **File:** `src/customers/import_flat.rs`, `tests/customers_import.rs`
- **Xong khi:** test với file .xlsx dựng bằng rust_xlsxwriter trong test (3 dòng: tạo, cập nhật, lỗi).
- **Phụ thuộc:** T4.B02

### T4.B11 — Service: nhập Excel theo nhóm · S
- [ ] Xong
- **Nguồn:** `customer_service.py::import_grouped_from_excel, _sync_sub_units_preserve, _make_short_name`
- **File:** `src/customers/import_grouped.rs`, `tests/customers_import.rs`
- **Xong khi:** test: file có 2 trạm + 3 điểm trạm → đúng quan hệ; nhập lại file bỏ 1 điểm → điểm đó `is_active=false`.
- **Phụ thuộc:** T4.B10

### T4.B12 — Service: file mẫu nhập và xuất danh sách · XS
- [ ] Xong
- **Nguồn:** `ContractForge/app/routers/customers.py` route `/template.xlsx` (dòng ~297)
- **File:** `src/customers/export_list.rs`
- **Xong khi:** test: đọc lại file sinh ra bằng calamine, tiêu đề đúng thứ tự.
- **Phụ thuộc:** T4.B10

### T4.B13 — Service: phân hạng khách hàng (KB-09) · S
- [ ] Xong
- **Làm:** hàm thuần `classify(customer, contracts, catalog_labels, today) -> (tier, action_text, days, Option<contract_id>)`
  giữ nguyên thứ tự điều kiện của `_classify`, nhưng tập trạng thái theo **KB-09**.
- **Nguồn:** `ContractForge/app/routers/customers.py::export_report::_classify`
- **File:** `src/customers/tier.rs`, `tests/customers_tier.rs`
- **Xong khi:** test: mỗi tier 1–5 (và 2a/2b/2c, 5a/5b) một ca.
- **Phụ thuộc:** T4.B01, T1.23

### T4.B14 — Service: báo cáo Excel 4 sheet · M
- [ ] Xong
- **Làm:** chép cột, màu tier, định dạng `#,##0`/`DD/MM/YYYY` của `export_report`; nhãn trạng thái từ `STATUS_LABELS` (KB-09).
- **Nguồn:** `customers.py::export_report` (dòng ~357–885)
- **File:** `src/customers/report.rs`, `tests/customers_report.rs`
- **Xong khi:** test: 4 sheet đúng tên; số dòng sheet 1 = số khách chưa xóa.
- **Phụ thuộc:** T4.B13

### T4.B15 — Service: xuất thông tin (.md) và gói hồ sơ ZIP · S
- [ ] Xong
- **Nguồn:** `customers.py::_build_info_md, export_info, download_bundle` (KB-14)
- **File:** `src/customers/info.rs`, `tests/customers_info.rs`
- **Xong khi:** test: markdown có các mục; ZIP gồm `.md` + giấy tờ.
- **Phụ thuộc:** T4.B05

### T4.B16 — Màn hình nhập Excel và nút xuất (S07) · S
- [ ] Xong
- **Làm:** chọn file, chọn kiểu phẳng/nhóm (+ tên sheet), bảng kết quả từng dòng; gắn nút Xuất báo cáo / Tải file mẫu / Xuất thông tin / Tải gói hồ sơ vào S03, S05 (qua `save_as`).
- **File:** `ui/screens/customers_import.slint`, `src/screens/customers_import.rs`
- **Xong khi:** kiểm tay nhập file mẫu vừa tải.
- **Phụ thuộc:** T4.B11, T4.B12, T4.B14, T4.B15

---

## C. Sản phẩm

### T4.C01 — Service: danh sách và đếm đang phục vụ · XS
- [ ] Xong
- **Nguồn:** `ContractForge/app/services/product_service.py::list_products, in_service_counts`
- **File:** `src/products/list.rs`, `tests/products.rs`
- **Xong khi:** test thứ tự sắp xếp; đếm chỉ `IN_SERVICE`.
- **Phụ thuộc:** T4.A01

### T4.C02 — Service: `clean_form` · S
- [ ] Xong
- **Làm:** gom mọi lỗi (`ProductError(Vec<String>)`, `user_message` nối bằng xuống dòng); mọi luật của Python.
- **Nguồn:** `product_service.py::clean_form, _text, _optional_int, _parse_bool, parse_feature_text`
- **File:** `src/products/clean.rs`, `tests/products.rs`
- **Xong khi:** test: form trống → đủ 3 lỗi đầu; gói addon thiếu nhóm; thuế "8.5" hợp lệ; thuế "abc" → đúng câu.
- **Phụ thuộc:** T4.C01

### T4.C03 — Service: tạo, sửa, ngừng bán · XS
- [ ] Xong
- **Nguồn:** `product_service.py::create_product, update_product, toggle_active, feature_text, form_values`
- **File:** `src/products/edit.rs`, `tests/products.rs`
- **Xong khi:** test roundtrip `feature_text` ↔ `parse_feature_text`.
- **Phụ thuộc:** T4.C02

### T4.C04 — Service: xuất danh mục và file mẫu · S
- [ ] Xong
- **Nguồn:** `product_service.py::make_catalog_xlsx, _export_row, _excel_number, IMPORT_COLUMNS, _TEMPLATE_EXAMPLES`
- **File:** `src/products/catalog_export.rs`, `tests/products_catalog.rs`
- **Xong khi:** test: xuất rồi đọc lại bằng calamine đúng cột/giá trị.
- **Phụ thuộc:** T4.C03

### T4.C05 — Service: nhập danh mục (xem trước + áp dụng) · S
- [ ] Xong
- **Nguồn:** `product_service.py::parse_catalog_xlsx, apply_catalog_rows, _cell_text, _cell_price`
- **File:** `src/products/catalog_import.rs`, `tests/products_catalog.rs`
- **Xong khi:** test: xuất → nhập lại = toàn "cập nhật", không đổi gì; một dòng lỗi → áp dụng bị từ chối, DB không đổi.
- **Phụ thuộc:** T4.C04

### T4.C06 — Màn hình danh sách sản phẩm (S08) · S
- [ ] Xong
- **File:** `ui/screens/products_list.slint`, `src/screens/products_list.rs`
- **Xong khi:** kiểm tay: nhóm hiển thị đúng; ngừng bán/bán lại.
- **Phụ thuộc:** T4.C03

### T4.C07 — Màn hình form sản phẩm (S09) · S
- [ ] Xong
- **Làm:** mọi trường; dropdown mẫu mặc định (chỉ mẫu vai trò hợp đồng đúng nhóm); hiện **tất cả** lỗi.
- **File:** `ui/screens/product_form.slint`, `src/screens/product_form.rs`
- **Xong khi:** kiểm tay tạo gói chính + gói bổ sung cùng nhóm.
- **Phụ thuộc:** T4.C06

### T4.C08 — Màn hình nhập danh mục (S10) · S
- [ ] Xong
- **File:** `ui/screens/products_import.slint`, `src/screens/products_import.rs`
- **Xong khi:** kiểm tay: xem trước → áp dụng; nút tải file mẫu/xuất.
- **Phụ thuộc:** T4.C05

---

## D. Mẫu

### T4.D01 — Service: mapping mặc định · XS
- [ ] Xong
- **Làm:** `default_mapping(placeholders) = CANONICAL_MAP ∪ ROW_PLACEHOLDERS`, thiếu → `""`.
- **Nguồn:** `ContractForge/app/services/template_analyzer.py::build_default_mapping`
- **File:** `src/templates/mapping.rs`, `tests/templates.rs`
- **Xong khi:** test: khớp `template_analysis.json → results[*].mapping`.
- **Phụ thuộc:** T2.15, T1.38

### T4.D02 — Service: tạo mẫu · S
- [ ] Xong
- **Làm:** kiểm (tên, loại, .docx, ≤10 MB, mã trùng) → `Template::compile` + analysis (lỗi → không lưu gì) → mã `TPL-NNN` →
  thứ tự: (1) `apply` lô 1 = insert bản ghi, lấy id; (2) ghi file `uploads/templates/{id}_{uuid8}_{tên_an_toàn}`
  (`tên_an_toàn` = tên gốc thay `[^\w\-_\.]` bằng `_`); (3) `apply` lô 2 = update đường dẫn, tên file, placeholder, mapping.
  Bước 2 lỗi → `apply` Delete bản ghi. Bước 3 lỗi → xóa file vừa ghi rồi Delete bản ghi.
- **Nguồn:** `ContractForge/app/services/template_service.py::create_template, generate_next_code, _safe_filename`
- **File:** `src/templates/create.rs`, `tests/templates.rs`
- **Xong khi:** test: tạo từ `sample_service_contract.docx` → có file, placeholder đúng; file hỏng → không có bản ghi, không có file.
- **Phụ thuộc:** T4.D01

### T4.D03 — Service: sửa thông tin, mapping, phân tích lại, bật/tắt · S
- [ ] Xong
- **Nguồn:** `template_service.py::update_metadata, update_mapping, reanalyze, toggle_active`
- **File:** `src/templates/edit.rs`, `tests/templates.rs`
- **Xong khi:** test: mapping khóa lạ bị bỏ; phân tích lại giữ mapping cũ.
- **Phụ thuộc:** T4.D02

### T4.D04 — Service: tải lên nhiều file · S
- [ ] Xong
- **Làm:** mỗi file: loại = `template_kind(placeholders, heading)`; `heading` = dòng bắt đầu "BIÊN BẢN" trong 10 đoạn chữ khác rỗng
  đầu tiên của thân + 2 dòng kế (dùng `cf_docx::inspect`). Tên = tên file bỏ đuôi, `_` → khoảng trắng.
- **Nguồn:** `template_service.py::_document_heading, _template_kind`, `ContractForge/app/routers/templates.py::bulk_upload`
- **File:** `src/templates/bulk.rs`, `tests/templates.rs`
- **Xong khi:** test: 2 file mẫu → 2 bản ghi, loại đúng.
- **Phụ thuộc:** T4.D02

### T4.D05 — Service: nhập mẫu cục bộ khi khởi động (KB-12) · S
- [ ] Xong
- **Nguồn:** `template_service.py::seed_from_project_files, _import_bundled_template_if_missing, _bundled_template_code, _existing_template_with_bytes`
- **File:** `src/templates/bundled.rs`, `tests/templates.rs`
- **Xong khi:** test: chạy 2 lần → chỉ 1 bản ghi; file `sample_*` bị bỏ qua.
- **Phụ thuộc:** T4.D04

### T4.D06 — Service: dọn file mẫu thừa (thủ công) · XS
- [ ] Xong
- **Nguồn:** `template_service.py::cleanup_orphan_template_files`
- **File:** `src/templates/cleanup.rs`, `tests/templates.rs`
- **Xong khi:** test: file không ai tham chiếu bị xóa, file đang dùng còn.
- **Phụ thuộc:** T4.D02

### T4.D07 — Màn hình danh sách mẫu (S11) · S
- [ ] Xong
- **Làm:** lọc theo loại/nhóm, bật/tắt, tải lên nhiều file (`pick_files`), Dọn file thừa (xác nhận), tải file gốc (`save_as`).
- **File:** `ui/screens/templates_list.slint`, `src/screens/templates_list.rs`
- **Xong khi:** kiểm tay.
- **Phụ thuộc:** T4.D03, T4.D04, T4.D06

### T4.D08 — Màn hình form + chi tiết mẫu (S12) · M
- [ ] Xong
- **Làm:** form tạo/sửa (tên, loại — gợi ý `CONTRACT_TYPES` nhưng cho gõ tự do, nhóm, mô tả, file); chi tiết: kết quả phân tích,
  bảng placeholder → dropdown `ALL_CANONICAL_FIELDS` + "(tự nhập)", Lưu mapping, Phân tích lại; nếu `uses_legacy_party_a` → khối
  hướng dẫn chuyển sang `{{?KHOA}}` (`04_DOCX_ENGINE.md` §6).
- **File:** `ui/screens/template_form.slint`, `ui/screens/template_detail.slint`, `src/screens/template_*.rs`
- **Xong khi:** kiểm tay đổi mapping rồi phân tích lại.
- **Phụ thuộc:** T4.D07

---

## E. Hợp đồng

### T4.E01 — Service: chuyển trạng thái duy nhất + hàng loạt (KB-07) · S
- [ ] Xong
- **Làm:** `change_status_changes(store, contract_id, target, actor, note) -> Result<Vec<Change>>` (không apply — để ghép vào lô khác);
  `change_status(ctx, …)` (apply); `bulk_change_status(ctx, ids, target)` (tất cả hoặc không, câu lỗi y Python).
- **Nguồn:** `ContractForge/app/services/contract_service.py::_apply_status_change, update_status, bulk_update_status`
- **File:** `src/contracts/status.rs`, `tests/contracts_status.rs`
- **Xong khi:** test: chuyển hợp lệ ghi 1 sự kiện; thiếu hóa đơn → câu lỗi; bulk có 1 lỗi → không đổi cái nào.
- **Phụ thuộc:** T4.A01, T1.22

### T4.E02 — Service: quét hết hạn (KB-08) · XS
- [ ] Xong
- **Làm:** `expire_due(ctx) -> usize` theo `03_NGHIEP_VU.md` §2; gọi lúc khởi động (`cf-app`) và hẹn giờ qua 00:00
  (`slint::Timer` kiểm mỗi phút xem ngày đã đổi chưa → gửi job).
- **Nguồn:** `ContractForge/app/services/dashboard_service.py::auto_update_contract_statuses`
- **File:** `src/contracts/expiry.rs`, `tests/contracts_status.rs`, `crates/cf-app/src/main.rs`
- **Xong khi:** test với `FixedClock`: Signed hết hạn hôm qua → Expired + sự kiện `expired`.
- **Phụ thuộc:** T4.E01

### T4.E03 — Service: điều kiện "chưa gia hạn" và danh sách sắp hết hạn · XS
- [ ] Xong
- **Làm:** `not_renewed(store, id)`; `expiring(store, today, days) -> Vec<ExpiringRow>` (`(today, today+days]`,
  `ACTIVE_LIFECYCLE`, chưa gia hạn, không phải `addendum_addon`, sắp `end_date`, urgency).
- **Nguồn:** `dashboard_service.py::_not_renewed_filter, _not_addon_appendix_filter, _get_expiring_list`
- **File:** `src/contracts/expiring.rs`, `tests/contracts_expiring.rs`
- **Xong khi:** test: HĐ có phụ lục gia hạn Signed → không còn trong danh sách; phụ lục gia hạn Draft → vẫn còn.
- **Phụ thuộc:** T4.A01

### T4.E04 — Service: danh sách hợp đồng · M
- [ ] Xong
- **Làm:** `ContractFilters` (§7.1) → `list_contracts(store, f, today) -> ListResult {rows, total, sum_total, years, cnt_unsent, cnt_expiring}`;
  mỗi dòng có `flag_unsent`, `flag_expiring` (KB-15); tìm không dấu (KB-11); sắp 6 cột; `per_page` ∈ {10,25,50,100} (khác → 25).
- **Nguồn:** `contract_service.py::_build_contract_query, list_contracts, sum_contracts`, `ContractForge/app/routers/contracts.py::contract_list`
- **File:** `src/contracts/list.rs`, `tests/contracts_list.rs`
- **Xong khi:** test từng bộ lọc cảnh báo; năm lấy theo `sign_date`; tổng tiền đúng.
- **Phụ thuộc:** T4.E03

### T4.E05 — Service: xuất Excel hợp đồng · S
- [ ] Xong
- **Nguồn:** `ContractForge/app/routers/contracts.py::export_contracts`
- **File:** `src/contracts/export.rs`, `tests/contracts_list.rs`
- **Xong khi:** test: 2 sheet, tiêu đề đúng, tiền cắt + chấm ngăn nghìn; tên file gợi ý `hop-dong-YYYYMMDD.xlsx`.
- **Phụ thuộc:** T4.E04

### T4.E06 — Service: tùy chọn gói cho form · S
- [ ] Xong
- **Làm:** `product_options(store, product_id, customer_id, unit_count)` = main brief + addons + templates (`contract`, `attachments` với
  `preselect`, `default_id`).
- **Nguồn:** `contract_service.py::product_options, _product_brief, _template_lists_for, _default_quantity`
- **File:** `src/contracts/options.rs`, `tests/contracts_form.rs`
- **Xong khi:** test: gói không phải main → rỗng; mẫu mặc định không thuộc danh sách → mẫu đầu.
- **Phụ thuộc:** T4.A01, T1.34, T1.41

### T4.E07 — Service: xem trước tiền · XS
- [ ] Xong
- **Nguồn:** `contract_service.py::price_preview`
- **File:** `src/contracts/preview.rs`, `tests/contracts_form.rs`
- **Xong khi:** test: kết quả = `pricing.json → sum_lines` cho cùng dữ liệu; tháng 0 → `"Phải có số tháng để tính tiền."`.
- **Phụ thuộc:** T4.E06

### T4.E08 — Service: gợi ý số và danh sách cơ sở · XS
- [ ] Xong
- **Làm:** `suggest_number(store, customer_id, product_id, year)` (KB-02, khuôn của sản phẩm); `customer_facilities(store, customer_id)`
  (`key`, `name`, `type`, `checked` như route `/customer-units`).
- **Nguồn:** `ContractForge/app/routers/contracts.py::suggest_contract_number, get_customer_units`, `contract_service.py::resolve_number_format`
- **File:** `src/contracts/suggest.rs`, `tests/contracts_form.rs`
- **Xong khi:** test: khách có `01/…/2026` → gợi ý `02/…`; cơ sở tạm ngưng không xuất hiện.
- **Phụ thuộc:** T4.A01, T1.25

### T4.E09 — Service: tạo bản nháp · M
- [ ] Xong
- **Làm:** `create_draft(ctx, ContractForm, opts{require_main, template_role, template_group})` theo §7.2, một lô Change:
  insert hợp đồng + dòng + sự kiện `created`. Dùng `legacy_amounts`, `build_lines`, `apply_lines` (viết `apply_lines` ở đây).
- **Nguồn:** `contract_service.py::create_draft, apply_lines, add_line_rows, _payment_mode, _parse_selected_units`
- **File:** `src/contracts/create.rs`, `tests/contracts_create.rs`
- **Xong khi:** test: legacy (không dòng) ra đúng số `legacy_amounts`; items ra đúng tổng dòng; trùng số → câu lỗi; mẫu sai nhóm → câu lỗi.
- **Phụ thuộc:** T4.E06, T1.31

### T4.E10 — Service: dữ liệu điền mẫu của một hợp đồng · S
- [ ] Xong
- **Làm:** `render_data_for(store, contract_id, template_id) -> RenderData`: ngữ cảnh (§11) hoặc 08a nếu mẫu có trường `form08a.*`;
  `apply_template_mapping`; `lists` từ dòng + `feature_list` của gói; `legacy_party_a_rows = !analysis.optional_keys.any()`.
- **Nguồn:** `ContractForge/app/services/docx_renderer.py::build_render_context, build_render_lists`, `attachment_service.py::render_attachment`
- **File:** `src/contracts/render.rs`, `tests/contracts_render.rs`
- **Xong khi:** test: ngữ cảnh khớp `render_context.json → cases[0].context` khi dữ liệu Store giống `fake_contract/fake_customer`.
- **Phụ thuộc:** T4.E09, T1.36–T1.39

### T4.E11 — Service: văn bản kèm theo · S
- [ ] Xong
- **Làm:** `resolve_attachment_templates(store, ids, contract)` (bỏ trùng giữ thứ tự, các câu lỗi); `render_attachments(ctx, contract_id, &[tpl]) -> Vec<(name, bytes)>`;
  `build_bundle_zip(items)` (trùng tên → `_2`).
- **Nguồn:** `ContractForge/app/services/attachment_service.py`
- **File:** `src/contracts/attachments.rs`, `tests/contracts_render.rs`
- **Xong khi:** test: mẫu khác nhóm → câu lỗi; ZIP 2 file trùng tên → `a.docx`, `a_2.docx`.
- **Phụ thuộc:** T4.E10

### T4.E12 — Service: sinh file hợp đồng · M
- [ ] Xong
- **Làm:** `generate_docx(ctx, contract_id, attachment_ids) -> GenerateResult {file_path, file_name, bundle_path?, attachments_count}`
  đúng **7 bước** `03_NGHIEP_VU.md` §7.3; tên file `make_output_filename(…, now_local)`; ghi bằng `write_new_file`;
  Draft → Generated qua `change_status_changes` ghép cùng lô.
- **Nguồn:** `contract_service.py::generate_docx, _write_bundle, _clear_bundles, bundle_path_for`, `batch_generator.py::_auto_pricing_description`
- **File:** `src/contracts/generate.rs`, `tests/contracts_generate.rs`
- **Xong khi:** test: Draft → Generated + 1 file + sự kiện; sinh lại → file cũ bị xóa, trạng thái giữ; có kèm theo → ZIP; mẫu mất file →
  `FileMissing` và không đổi gì; `inspect::docx_texts` của file ra chứa số HĐ.
- **Phụ thuộc:** T4.E11, T2.17

### T4.E13 — Service: upload chứng từ ×3 · S
- [ ] Xong
- **Làm:** một hàm chung theo bảng `03_NGHIEP_VU.md` §7.4 (`EvidenceKind::{Scan, Invoice, PaymentSlip}`), tự chuyển trạng thái qua E01 (KB-07),
  trả câu thông báo y Python (kèm hậu tố "— trạng thái tự động cập nhật → ...").
- **Nguồn:** `ContractForge/app/routers/contracts.py::upload_scan, upload_invoice, upload_payment_slip, _is_valid_slip_file`
- **File:** `src/contracts/evidence.rs`, `tests/contracts_evidence.rs`
- **Xong khi:** test: Generated + scan → Signed; Signed + hóa đơn → Invoiced; Invoiced + UNC ảnh PNG → PaidActive; 6 MB → câu lỗi.
- **Phụ thuộc:** T4.E01

### T4.E14 — Service: xóa chứng từ ×3 · XS
- [ ] Xong
- **Nguồn:** `contracts.py::delete_scan, delete_invoice, delete_payment_slip`
- **File:** `src/contracts/evidence.rs`, `tests/contracts_evidence.rs`
- **Xong khi:** test: file bị xóa, 3 cột NULL, sự kiện đúng loại, trạng thái không đổi.
- **Phụ thuộc:** T4.E13

### T4.E15 — Service: xóa hợp đồng · S
- [ ] Xong
- **Làm:** kiểm phụ lục (câu y Python); một lô: xóa sự kiện → xóa hợp đồng (dòng tự CASCADE); sau commit xóa file riêng + thư mục bundle
  (best-effort). Xóa nhiều: kiểm hết trước.
- **Nguồn:** `contract_service.py::delete_contract, bulk_delete, _delete_owned_files, _refuse_if_has_addenda`
- **File:** `src/contracts/delete.rs`, `tests/contracts_delete.rs`
- **Xong khi:** test: còn phụ lục → từ chối; xóa → file biến mất, giấy tờ khách còn nguyên.
- **Phụ thuộc:** T4.E12

### T4.E16 — Màn hình danh sách hợp đồng: bảng + bộ lọc (S13) · M
- [ ] Xong
- **Làm:** thanh lọc (từ khóa, trạng thái `SELECTABLE`, khách, sản phẩm, viên năm, khoảng ngày + chọn cột ngày), huy hiệu
  "Chưa gửi > 10 ngày" / "Sắp hết hạn 60 ngày" (bấm → lọc), bảng có cờ, sắp xếp khi bấm tiêu đề, phân trang, tổng tiền kết quả lọc.
- **File:** `ui/screens/contracts_list.slint`, `src/screens/contracts_list.rs`
- **Xong khi:** kiểm tay mọi bộ lọc trên `seeded_db`.
- **Phụ thuộc:** T4.E04

### T4.E17 — Màn hình danh sách hợp đồng: chọn nhiều + xuất · S
- [ ] Xong
- **Làm:** tick nhiều → Đổi trạng thái (dropdown, xác nhận) / Xóa (xác nhận); Xuất Excel (`save_as`).
- **File:** `ui/screens/contracts_list.slint`, `src/screens/contracts_list.rs`
- **Xong khi:** kiểm tay; lỗi bulk hiện toast danger đủ câu.
- **Phụ thuộc:** T4.E16, T4.E05, T4.E15

### T4.E18 — Form sinh hợp đồng (S14) phần 1: khách hàng · M
- [ ] Xong
- **Làm:** hàng 1–3 của bảng `05_GIAO_DIEN.md` §4 (ô lọc không dấu, chọn khách → gợi ý số, địa điểm ký, danh sách cơ sở tick, số cơ sở chỉ đọc),
  băng cảnh báo hồ sơ Bên B. Cờ "sửa tay" cho số HĐ, địa điểm.
- **File:** `ui/screens/contract_form.slint`, `src/screens/contract_form.rs`
- **Xong khi:** kiểm tay theo bảng.
- **Phụ thuộc:** T4.E08, T4.A06

### T4.E19 — Form sinh hợp đồng phần 2: gói và tiền · M
- [ ] Xong
- **Làm:** hàng 4–7 của bảng §4: chọn gói → `product_options`; bảng dòng (gói chính + gói bổ sung tick); sửa ô → `price_preview` (debounce 300 ms bằng `slint::Timer`);
  hiển thị tổng, bằng chữ, mô tả; lỗi → khóa nút Lưu; ô bằng chữ/mô tả có cờ sửa tay.
- **Nguồn:** `ContractForge/static/js/contract_items.js`
- **File:** `ui/screens/contract_form.slint`, `ui/components/items_table.slint`, `src/screens/contract_form.rs`
- **Xong khi:** kiểm tay: đổi SL → tổng đổi sau ~0,3 s; số khớp `price_preview`.
- **Phụ thuộc:** T4.E18, T4.E07

### T4.E20 — Form sinh hợp đồng phần 3: ngày, mẫu, lưu · M
- [ ] Xong
- **Làm:** hàng 8–13: ngày ký → hiệu lực; hiệu lực/tháng → kết thúc (`compute_end_date`, KB-10), thanh lý, nghiệm thu (+2 ngày);
  hai ô tháng đồng bộ; cảnh báo ngày; ô mẫu (khóa khi chưa chọn gói) + văn bản kèm theo tick sẵn; Bên B riêng của hợp đồng;
  Lưu → `create_draft` → `generate_docx` (job nền) → mở S15 + toast (câu y Python).
- **File:** `ui/screens/contract_form.slint`, `src/screens/contract_form.rs`
- **Xong khi:** kiểm tay tạo + sinh được hợp đồng items và legacy; lỗi giữ nguyên dữ liệu đã nhập.
- **Phụ thuộc:** T4.E19, T4.E09, T4.E12

### T4.E21 — Chi tiết hợp đồng (S15) phần 1 · M
- [ ] Xong
- **Làm:** khối thông tin, bảng dòng, hợp đồng gốc/phụ lục (liên kết), timeline sự kiện (mới nhất trên cùng), đổi trạng thái
  (chỉ bước hợp lệ của `valid_targets`, ô ghi chú), nhãn hết hạn (`expiry_bucket`).
- **File:** `ui/screens/contract_detail.slint`, `src/screens/contract_detail.rs`
- **Xong khi:** kiểm tay đổi trạng thái; bước không hợp lệ không hiện.
- **Phụ thuộc:** T4.E01, T4.E12

### T4.E22 — Chi tiết hợp đồng phần 2: chứng từ, file · M
- [ ] Xong
- **Làm:** 3 khối chứng từ (upload qua `pick_file`, ngày hóa đơn/thanh toán, xem = mở app mặc định, xóa có xác nhận);
  Sinh lại file (tick văn bản kèm theo); Mở file .docx / Mở thư mục / Mở ZIP; Xóa hợp đồng.
- **File:** `ui/screens/contract_detail.slint`, `src/screens/contract_detail.rs`
- **Xong khi:** kiểm tay đủ luồng Generated → Signed → Invoiced → PaidActive bằng upload.
- **Phụ thuộc:** T4.E21, T4.E13, T4.E14, T4.E15

#### Điểm dừng ĐD4-E
- [ ] Chủ dự án tạo + sinh 3 hợp đồng trên bản sao dữ liệu thật; so file với bản Python (chỉ khác KB-03/KB-05)

---

## F. Phụ lục và gia hạn

### T4.F01 — Service: phụ lục bổ sung · S
- [ ] Xong
- **Nguồn:** `ContractForge/app/services/addendum_service.py::_parent_or_raise, can_add_addendum, addendum_defaults, create_addendum, _appendix_template_id, _used_seqs, _addon_candidates`
- **File:** `src/addenda/addon.rs`, `tests/addenda.rs`
- **Xong khi:** test: số phụ lục `PL01/…` rồi `PL02/…`; ngày bắt đầu trước gốc → câu lỗi; gói main → câu lỗi; sự kiện ở cả hai hợp đồng.
- **Phụ thuộc:** T4.E09

### T4.F02 — Service: gợi ý gia hạn · S
- [ ] Xong
- **Nguồn:** `addendum_service.py::_current_product, renewal_lines, renewal_comparison, renewal_defaults`
- **File:** `src/addenda/renewal_defaults.rs`, `tests/addenda.rs`
- **Xong khi:** test: chuỗi `replaced_by_id` 2 bước tìm ra gói đang bán; vòng lặp → None.
- **Phụ thuộc:** T4.F01

### T4.F03 — Service: phụ lục gia hạn · XS
- [ ] Xong
- **Nguồn:** `addendum_service.py::create_renewal_appendix, _renewal_items, _fill_quantities`
- **File:** `src/addenda/renewal.rs`, `tests/addenda.rs`
- **Xong khi:** test: bắt đầu ≤ ngày hết hạn gốc → câu lỗi; tạo được, `doc_kind=addendum_renewal`.
- **Phụ thuộc:** T4.F02

### T4.F04 — Service: gia hạn bằng hợp đồng mới · XS
- [ ] Xong
- **Nguồn:** `addendum_service.py::create_renewal_contract`
- **File:** `src/addenda/renewal.rs`, `tests/addenda.rs`
- **Xong khi:** test: `renewed_from_id` đúng; HĐ gốc ra khỏi danh sách sắp hết hạn khi HĐ mới ở trạng thái `RENEWAL_COUNTS`.
- **Phụ thuộc:** T4.F03, T4.E03

### T4.F05 — Màn hình phụ lục bổ sung (S16) · S
- [ ] Xong
- **Nguồn:** `ContractForge/templates/contracts/addendum_form.html`, `contracts.py::_addendum_context, _clip_to_parent`
- **File:** `ui/screens/addendum_form.slint`, `src/screens/addendum_form.rs`
- **Xong khi:** kiểm tay; nút "Tạo phụ lục" ở S15 chỉ hiện khi `can_add_addendum`.
- **Phụ thuộc:** T4.F01, T4.E21

### T4.F06 — Màn hình gia hạn (S17) · S
- [ ] Xong
- **Nguồn:** `ContractForge/templates/contracts/renewal_form.html`, `contracts.py::_renewal_context`
- **File:** `ui/screens/renewal_form.slint`, `src/screens/renewal_form.rs`
- **Xong khi:** kiểm tay cả hai chế độ.
- **Phụ thuộc:** T4.F04, T4.E21

---

## G. Văn bản kèm theo lẻ và biểu 08a

### T4.G01 — Service: biểu 08a · XS
- [ ] Xong
- **Làm:** mẫu: `assets/default_templates/template_08A.docx` nếu có, không thì bản nhúng `sample_form_08a.docx`; tên `08A_{mã}_{số}.docx`.
- **Nguồn:** `ContractForge/app/services/form08a_service.py::render_form_08a`, `ContractForge/app/paths.py::resolve_form_08a_template`
- **File:** `src/contracts/form08a.rs`, `tests/contracts_render.rs`
- **Xong khi:** test: file ra chứa `BUDGET_UNIT_NAME` viết hoa.
- **Phụ thuộc:** T4.E10

### T4.G02 — Nút văn bản kèm theo lẻ và 08a trên S15 · XS
- [ ] Xong
- **Làm:** danh sách mẫu kèm theo của nhóm → "Tải" từng cái (`render_attachments` 1 mẫu → `save_as`); nút "Biểu 08a" → `save_as`.
- **File:** `ui/screens/contract_detail.slint`, `src/screens/contract_detail.rs`
- **Xong khi:** kiểm tay; không đổi trạng thái hợp đồng.
- **Phụ thuộc:** T4.G01, T4.E22

---

## H. Sinh hàng loạt

### T4.H01 — Service: file Excel mẫu cho lô · XS
- [ ] Xong
- **Nguồn:** `ContractForge/app/services/batch_generator.py::make_batch_template_xlsx` (dữ liệu ví dụ thay bằng hư cấu trung tính như trong hàm)
- **File:** `src/batch/excel.rs`, `tests/batch.rs`
- **Xong khi:** test đọc lại tiêu đề đúng.
- **Phụ thuộc:** T4.A01

### T4.H02 — Service: đọc Excel ghi đè theo khách · S
- [ ] Xong
- **Nguồn:** `batch_generator.py::import_excel_mapping, BATCH_EXCEL_COLUMNS`
- **File:** `src/batch/excel.rs`, `tests/batch.rs`
- **Xong khi:** test: khách không tìm thấy → lỗi đúng số dòng; dòng trống bị bỏ.
- **Phụ thuộc:** T4.H01

### T4.H03 — Service: dựng một hợp đồng của lô · S
- [ ] Xong
- **Làm:** `batch_row(ctx, template, customer, params, year) -> Result<GeneratedRow>`: số HĐ, dòng gói (`main_code`/`addon_codes` hoặc `items_json`),
  legacy, kiểm nhóm mẫu, một lô Change (hợp đồng Generated + dòng + sự kiện "[Batch {job}] Sinh file {tên}" actor `batch`), file độc quyền (KB-17).
- **Nguồn:** `batch_generator.py::run_batch (thân vòng lặp), _lines_for_row, _build_contract_from_params, _lists_for_lines`
- **File:** `src/batch/row.rs`, `tests/batch.rs`
- **Xong khi:** test: dòng có `main_code` không tồn tại → lỗi riêng dòng đó; file trùng tên → `_2`.
- **Phụ thuộc:** T4.E12, T4.H02

### T4.H04 — Service: chạy lô nền (KB-13) · S
- [ ] Xong
- **Làm:** `run_batch(ctx, job, progress: impl Fn(done,total), cancel: &AtomicBool) -> BatchResult`; biên dịch mẫu 1 lần (cache);
  lỗi một dòng không dừng lô; hủy → dừng sau dòng hiện tại; ZIP `batch_{job}_{YYYYMMDD_HHMMSS}.zip` các file thành công.
- **File:** `src/batch/run.rs`, `tests/batch.rs`
- **Xong khi:** test: 3 khách (1 lỗi) → 2 thành công + ZIP 2 file; hủy sau dòng 1 → 1 thành công.
- **Phụ thuộc:** T4.H03

### T4.H05 — Màn hình lô: form + xem trước (S18) · M
- [ ] Xong
- **Nguồn:** `ContractForge/templates/batch/form.html`, `preview.html`, `ContractForge/app/routers/batch.py` (`batch_form`, `batch_preview`)
- **File:** `ui/screens/batch_form.slint`, `src/screens/batch_form.rs`
- **Xong khi:** kiểm tay: chọn mẫu + nhiều khách + tham số → bảng xem trước số HĐ dự kiến.
- **Phụ thuộc:** T4.H04

### T4.H06 — Màn hình lô: chạy + kết quả · S
- [ ] Xong
- **Làm:** thanh tiến độ, nút Hủy, bảng kết quả (thành công/lỗi), Lưu ZIP (`save_as`), mở thư mục batch.
- **File:** `ui/screens/batch_result.slint`, `src/screens/batch_result.rs`
- **Xong khi:** kiểm tay lô 10 khách, UI không đứng; hủy được.
- **Phụ thuộc:** T4.H05

### T4.H07 — Màn hình lô từ Excel · S
- [ ] Xong
- **Nguồn:** `ContractForge/templates/batch/import.html`, `batch.py` (`/import`, `/run-from-excel`)
- **File:** `ui/screens/batch_import.slint`, `src/screens/batch_import.rs`
- **Xong khi:** kiểm tay với file mẫu tải từ T4.H01.
- **Phụ thuộc:** T4.H06

---

## I. Báo giá

### T4.I01 — Service: báo giá theo danh mục — chọn gói/mẫu + ngữ cảnh legacy · S
- [ ] Xong
- **Nguồn:** `ContractForge/app/routers/quotation.py::resolve_quotation_selection, product_price_with_vat, _build_context`
- **File:** `src/quotations/catalog.rs`, `tests/quotations.rs`
- **Xong khi:** test: mẫu loại "Hợp đồng" → câu lỗi; ngữ cảnh legacy đúng `TOTAL_AMOUNT`, `PRICING_DESCRIPTION`.
- **Phụ thuộc:** T4.D02

### T4.I02 — Service: báo giá theo danh mục — nhiều dòng + sinh file · S
- [ ] Xong
- **Nguồn:** `quotation.py::_items_context_and_lists, _build_context_from_lines, quotation_generate`
- **File:** `src/quotations/catalog.rs`, `tests/quotations.rs`
- **Xong khi:** test: tên file `BaoGia_{mã}_{tên}_{YYYYMMDD}.docx`; số tháng 121 → câu lỗi; gói dòng 1 khác gói chọn → câu lỗi.
- **Phụ thuộc:** T4.I01

### T4.I03 — Service: báo giá tự nhập (KB-19) · S
- [ ] Xong
- **Nguồn:** `ContractForge/app/services/manual_quotation.py::LAYOUTS, parse_rows, build_lines, build_document`, `ContractForge/app/routers/manual_quotation.py`
- **File:** `src/quotations/manual.rs`, `tests/quotations.rs`
- **Xong khi:** test: ngữ cảnh + dòng khớp `docx_render.json → manual_quotation`; tên file theo KB-19; 101 hạng mục → câu lỗi.
- **Phụ thuộc:** T4.A01, T1.11

### T4.I04 — Service: hồ sơ người phát hành · XS
- [ ] Xong
- **Làm:** `fingerprint` = SHA-256 hex của chuỗi JSON **giống hệt Python** `json.dumps(fields, ensure_ascii=False, sort_keys=True)`:
  khóa sắp xếp, phân cách `", "` và `": "`, giữ nguyên tiếng Việt, escape `"` `\` và ký tự điều khiển như Python. Tự dựng chuỗi, **không** dùng `serde_json::to_string`.
- **Nguồn:** `ContractForge/app/services/quotation_issuer_service.py`
- **File:** `src/quotations/issuers.rs`, `tests/quotations.rs`
- **Xong khi:** test: với `{"name":"Công ty Mẫu","address":"Số 1","tax_code":"","representative":"","title":"","phone":"","email":""}` hash bằng giá trị
  tính bởi Python (chạy một lần `python -c` rồi dán vào test); lưu 2 lần cùng dữ liệu → 1 bản ghi.
- **Phụ thuộc:** T4.A01

### T4.I05 — Service: yêu cầu báo giá · S
- [ ] Xong
- **Nguồn:** `ContractForge/app/services/quote_request.py` (toàn bộ), `ContractForge/app/routers/quote_request.py`
- **File:** `src/quotations/request.rs`, `tests/quotations.rs`
- **Xong khi:** test: ngữ cảnh **không có** khóa nào chứa `PRICE`, `AMOUNT`, `TOTAL`, `VAT`; người nhận trống → chuỗi chấm; tên file đúng.
- **Phụ thuộc:** T4.D02

### T4.I06 — Service: cài mẫu yêu cầu báo giá · XS
- [ ] Xong
- **Nguồn:** `quote_request.py::SAMPLE_REQUEST_TEMPLATES, install_sample_templates`
- **File:** `src/quotations/request.rs`, `tests/quotations.rs`
- **Xong khi:** test: chạy 2 lần không tạo trùng.
- **Phụ thuộc:** T4.I05

### T4.I07 — Màn hình báo giá: tab theo danh mục · S
- [ ] Xong
- **Nguồn:** `ContractForge/templates/quotation/form.html`
- **File:** `ui/screens/quotation.slint`, `src/screens/quotation_catalog.rs`
- **Xong khi:** kiểm tay; ô mẫu **chỉ** liệt kê mẫu "Báo giá" đúng nhóm (không tự nhảy sang mẫu hợp đồng).
- **Phụ thuộc:** T4.I02

### T4.I08 — Màn hình báo giá: tab tự nhập · S
- [ ] Xong
- **Nguồn:** `ContractForge/templates/quotation/manual_form.html`
- **File:** `ui/screens/quotation.slint`, `src/screens/quotation_manual.rs`
- **Xong khi:** kiểm tay: 5 bố cục; lưu/chọn hồ sơ người phát hành; tải mẫu trống của bố cục.
- **Phụ thuộc:** T4.I03, T4.I04

### T4.I09 — Màn hình báo giá: tab yêu cầu báo giá · S
- [ ] Xong
- **Nguồn:** `ContractForge/templates/quotation/request_form.html`
- **File:** `ui/screens/quotation.slint`, `src/screens/quotation_request.rs`
- **Xong khi:** kiểm tay; nút cài mẫu mẫu.
- **Phụ thuộc:** T4.I05, T4.I06

---

## J. Hồ sơ đấu nối

### T4.J01 — Service: lập kế hoạch hồ sơ · M
- [ ] Xong
- **Nguồn:** `ContractForge/app/services/dossier_service.py::Slot, SlotResult, DossierPlan, dossier_dir_for, contracts_for_dossier, _product_positions, build_plan, _fill`
- **File:** `src/dossier/plan.rs`, `tests/dossier.rs` (đối chiếu thêm `tests/unit/test_dossier.py` của bản Python)
- **Xong khi:** test: khách 2 hợp đồng 2 sản phẩm → số ô và tên đích đúng; thiếu giấy tờ → ô "thiếu".
- **Phụ thuộc:** T4.B05, T4.E13

### T4.J02 — Service: tạo hồ sơ · S
- [ ] Xong
- **Nguồn:** `dossier_service.py::materialize, _clear_managed_files, _link_or_copy, build_checklist_text, is_stale, newest_source_mtime, open_in_file_manager`
- **File:** `src/dossier/build.rs`, `tests/dossier.rs`
- **Xong khi:** test: tạo 2 lần, lần 2 không để lại file thừa; file lạ do người dùng thêm vào thư mục không bị xóa.
- **Phụ thuộc:** T4.J01

### T4.J03 — Màn hình hồ sơ đấu nối (S06) · S
- [ ] Xong
- **Nguồn:** `ContractForge/templates/customers/dossier.html`
- **File:** `ui/screens/dossier.slint`, `src/screens/dossier.rs`
- **Xong khi:** kiểm tay tạo + mở thư mục; cảnh báo hồ sơ cũ.
- **Phụ thuộc:** T4.J02, T4.B08

---

## K. Dashboard và tìm kiếm

### T4.K01 — Service: số tổng hợp · S
- [ ] Xong
- **Làm:** một lượt duyệt Store → `Summary` (mọi khóa của `summary` Python) + `status_counts`.
- **Nguồn:** `ContractForge/app/services/dashboard_service.py::get_dashboard_data` (phần 1, 4, 6)
- **File:** `src/dashboard/summary.rs`, `tests/dashboard.rs`
- **Xong khi:** test trên `seeded_db` khớp số đếm tay; `in_force` = số `ACTIVE_LIFECYCLE`.
- **Phụ thuộc:** T4.E03

### T4.K02 — Service: theo sản phẩm, hoạt động gần đây · XS
- [ ] Xong
- **Nguồn:** `dashboard_service.py::_contracts_by_product`, `get_dashboard_data` (phần 5)
- **File:** `src/dashboard/breakdown.rs`, `tests/dashboard.rs`
- **Xong khi:** test: nhóm "Không phân loại"; tổng giá trị các nhóm = `total_value_active`.
- **Phụ thuộc:** T4.K01

### T4.K03 — Service: thống kê tháng, top khách, dự báo · S
- [ ] Xong
- **Nguồn:** `dashboard_service.py::get_monthly_stats, get_top_customers, get_monthly_expiry_forecast`
- **File:** `src/dashboard/trends.rs`, `tests/dashboard.rs`
- **Xong khi:** test với `FixedClock` tháng 1 (so với tháng 12 năm trước); dự báo đủ 12 phần tử.
- **Phụ thuộc:** T4.K01

### T4.K04 — Service: việc cần làm · XS
- [ ] Xong
- **Nguồn:** `dashboard_service.py::get_action_items`
- **File:** `src/dashboard/actions.rs`, `tests/dashboard.rs`
- **Xong khi:** test từng mục 1–5 (câu y Python); liên kết `link` đổi thành `ScreenTarget` (màn hình + bộ lọc).
- **Phụ thuộc:** T4.K01

### T4.K05 — Service: trang sắp hết hạn + xuất · XS
- [ ] Xong
- **Nguồn:** `ContractForge/app/routers/dashboard.py::expiring, export_expiring` (dòng ~109–175)
- **File:** `src/dashboard/expiring_page.rs`, `tests/dashboard.rs`
- **Xong khi:** test: `days` ngoài {7,30,60,90,365} → 30 (đọc route để xác nhận); Excel đúng cột.
- **Phụ thuộc:** T4.E03

### T4.K06 — Service: tìm kiếm toàn cục (KB-11) · XS
- [ ] Xong
- **Nguồn:** `ContractForge/app/routers/dashboard.py::api_search`
- **File:** `src/dashboard/search.rs`, `tests/dashboard.rs`
- **Xong khi:** test: "song xanh" ra khách + hợp đồng; giới hạn số kết quả như Python.
- **Phụ thuộc:** T3.12

### T4.K07 — Màn hình dashboard: thẻ và danh sách (S01) · M
- [ ] Xong
- **Nguồn:** `ContractForge/templates/dashboard/index.html`
- **File:** `ui/screens/dashboard.slint`, `src/screens/dashboard.rs`
- **Xong khi:** kiểm tay; mở dashboard < 100 ms (log); bấm thẻ/việc cần làm → màn hình đã lọc đúng.
- **Phụ thuộc:** T4.K01–T4.K04

### T4.K08 — Màn hình dashboard: biểu đồ · S
- [ ] Xong
- **Làm:** `05_GIAO_DIEN.md` §5 (tái dùng component từ bản thử T0.07).
- **File:** `ui/components/bar_chart.slint`, `ui/components/pie_chart.slint`, `ui/screens/dashboard.slint`
- **Xong khi:** kiểm tay với dữ liệu 0 và nhiều tháng.
- **Phụ thuộc:** T4.K07

### T4.K09 — Màn hình sắp hết hạn (S02) và ô tìm kiếm toàn cục · S
- [ ] Xong
- **File:** `ui/screens/expiring.slint`, `src/screens/expiring.rs`, `ui/components/global_search.slint`
- **Xong khi:** kiểm tay; Enter mở mục đầu.
- **Phụ thuộc:** T4.K05, T4.K06

---

## L. Cài đặt

### T4.L01 — Service: cài đặt · XS
- [ ] Xong
- **Nguồn:** `ContractForge/app/services/settings_service.py`, `ContractForge/app/routers/settings.py`
- **File:** `src/settings.rs`, `tests/settings.rs`
- **Xong khi:** test: khuôn phụ lục thiếu `{parent}` → câu lỗi, không lưu; `is_organization_profile_complete`.
- **Phụ thuộc:** T4.A01

### T4.L02 — Màn hình cài đặt (S20) · S
- [ ] Xong
- **Nguồn:** `ContractForge/templates/settings/index.html`
- **File:** `ui/screens/settings.slint`, `src/screens/settings.rs`
- **Xong khi:** kiểm tay lưu Bên B; hiện thư mục dữ liệu + nút mở.
- **Phụ thuộc:** T4.L01

---

## Điểm kiểm Đợt 4 (ĐD4)
- [ ] `cargo test --workspace` xanh
- [ ] Mọi tính năng của bản web có trong bản Rust (đối chiếu danh sách route ở `ContractForge/app/routers/*.py`)
- [ ] Chủ dự án dùng thử 1 tuần trên bản sao dữ liệu, ghi lỗi vào `docs/LOI_DUNG_THU.md`
