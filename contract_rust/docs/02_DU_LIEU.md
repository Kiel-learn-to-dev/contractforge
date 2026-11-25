# 02 — Dữ liệu

## 1. Nguyên tắc

- **Một file DB dùng chung** với bản Python: `contract_manager.db` (SQLite), cho tới khi xóa bản Python.
- Bản Rust nạp toàn bộ DB vào `Store` trong RAM khi mở (xem `01_KIEN_TRUC.md` §6); DB là nơi lưu bền.
- Bản Rust **không đổi schema** trong giai đoạn chuyển đổi: không thêm/xóa/đổi cột, không thêm bảng.
- Mọi giá trị Rust ghi xuống phải **đọc lại được bằng bản Python** (SQLAlchemy) và ngược lại.

## 2. Schema

DDL nguyên văn của một DB cài mới nằm ở [`fixtures/schema_baseline.sql`](../fixtures/schema_baseline.sql)
(do chính bản Python sinh ra). Danh sách cột bắt buộc của từng bảng ở
[`fixtures/schema.json`](../fixtures/schema.json) → khóa `required_columns`.

DB đang dùng thật được tạo dần qua nhiều phiên bản nên **thứ tự cột khác** bản cài mới (các cột
thêm bằng `ALTER TABLE` nằm cuối). Vì vậy code Rust **luôn đọc cột theo tên**, không theo vị trí
(`row.get::<_, T>("ten_cot")`), và `INSERT` luôn liệt kê tên cột.

### 2.1 Bảng và ý nghĩa

| Bảng | Nội dung | Quan hệ |
|---|---|---|
| `customers` | Khách hàng (Bên A). Xóa mềm bằng `is_deleted` | 1–n `customer_units`, `customer_documents`, `contracts` |
| `customer_units` | Đơn vị con / điểm trạm. `include_in_billing` = có tính vào số cơ sở | `customer_id` → customers (ON DELETE CASCADE) |
| `customer_documents` | Giấy tờ: `cccd_front`, `cccd_back`, `establishment` (tối đa 1 mỗi loại), `other` (nhiều) | `customer_id` → customers |
| `products` | Danh mục gói: `kind` ∈ main/addon/storage; `group_code` gom gói chính với gói bổ sung; `contract_number_format` là khuôn số | `default_template_id` → contract_templates; `replaced_by_id` → products |
| `contract_templates` | Mẫu .docx: `contract_type` (nhãn tự do quyết định vai trò), `product_group` (NULL = dùng chung), `detected_placeholders` JSON, `field_mapping` JSON | |
| `contracts` | Hợp đồng **và** phụ lục (cùng bảng). `doc_kind` ∈ contract/addendum_addon/addendum_renewal | `customer_id`, `product_id`, `template_id`, `parent_contract_id` (phụ lục → gốc), `renewed_from_id` (hợp đồng gia hạn → hợp đồng cũ) |
| `contract_items` | Dòng bảng giá trị (ảnh chụp tại lúc lập) | `contract_id` (ON DELETE CASCADE), `product_id` |
| `contract_events` | Lịch sử (timeline) | `contract_id` — **không** ON DELETE CASCADE: xóa hợp đồng phải xóa sự kiện trước |
| `app_settings` | Key–value cấu hình (Bên B, khuôn số phụ lục…) | |
| `quotation_issuers` | Hồ sơ đơn vị phát hành báo giá tự nhập, khóa duy nhất `fingerprint` | |

### 2.2 Giá trị liệt kê (enum) lưu dạng chuỗi

| Cột | Giá trị hợp lệ |
|---|---|
| `contracts.status` | `Draft`, `Generated`, `Sent`, `Signed`, `Invoiced`, `PaidActive`, `Expired`, `Terminated`; **chỉ đọc** (dữ liệu cũ): `Active`, `ExpiringSoon` |
| `contracts.pricing_model` | `legacy` (giữ số đã lưu, không tính lại) \| `items` (tính theo dòng) |
| `contracts.doc_kind` | `contract` \| `addendum_addon` \| `addendum_renewal` |
| `contracts.payment_mode` | `prepaid` \| `postpaid` \| NULL |
| `products.kind` | `main` \| `addon` \| `storage` |
| `products.quantity_source` | `units` \| `records` \| `manual` |
| `contract_templates.mode` | `placeholder` (thực tế chỉ dùng giá trị này) \| `legacy_anchor` |
| `customer_documents.doc_type` | `cccd_front` \| `cccd_back` \| `establishment` \| `other` |
| `customer_documents.mime_hint` | `pdf` \| `image` |
| `customers.representative_gender` | `Nam` \| `Nữ` \| NULL |
| `contract_events.event_type` | `created`, `generated`, `status_changed`, `expired`, `scan_uploaded`, `scan_deleted`, `invoice_uploaded`, `invoice_deleted`, `payment_slip_uploaded`, `payment_slip_deleted`, `addendum`, `renewal` (và dữ liệu cũ có thể có `expiring_soon_alert`, `note`…: hiển thị nguyên văn) |

Trong Rust: mỗi enum là `enum` có `as_str()` và `FromStr`. Giá trị lạ đọc từ DB **không làm sập
app**: `ContractStatus` có biến thể `Unknown(String)` để hiển thị nguyên chuỗi.

## 3. Cách lưu từng kiểu (codec) — BẮT BUỘC đúng

| Kiểu khai báo | Đọc vào Rust | Ghi xuống từ Rust |
|---|---|---|
| `INTEGER` | `i64` | `i64` |
| `BOOLEAN` | `0/1` → `bool` (chấp nhận cả `'0'/'1'` text) | `0` hoặc `1` |
| `NUMERIC(p,s)` | Có thể là INTEGER, REAL hoặc TEXT trong file. INTEGER → `Decimal::from(i)`; REAL → `Decimal::from_str(&format!("{}", f))` (giống Python `Decimal(str(float))`); TEXT → `Decimal::from_str` | Ghi `Decimal` dưới dạng **TEXT chuẩn hóa** (`d.normalize().to_string()`). Cột có affinity NUMERIC nên SQLite tự đổi thành INTEGER/REAL — y như dữ liệu Python ghi |
| `DATE` | TEXT `YYYY-MM-DD` → `NaiveDate` | TEXT `YYYY-MM-DD` |
| `DATETIME` | TEXT `YYYY-MM-DD HH:MM:SS[.ffffff]` → `NaiveDateTime` (UTC) | TEXT `%Y-%m-%d %H:%M:%S%.6f` (UTC) |
| `VARCHAR/TEXT` | `String` (NULL → `None`) | Chuỗi rỗng sau khi trim → ghi **NULL** ở các trường tùy chọn (Python làm `(x or "").strip() or None`) |

Hàm codec đặt ở `cf-db/src/codec.rs`, có test riêng cho từng dòng của bảng trên.

## 4. Cột JSON

| Cột | Dạng | Ghi chú khi đọc |
|---|---|---|
| `contracts.render_data` | object `{PLACEHOLDER: "giá trị", ..., "_lists": {"items":[...], "features":[...]}}` | Ảnh chụp dữ liệu lúc sinh file. Chỉ ghi, hiếm khi đọc |
| `contracts.selected_unit_names` | array chuỗi tên cơ sở được tích | JSON hỏng → `[]` |
| `contract_templates.detected_placeholders` | array chuỗi, đã sắp xếp, không trùng | NULL/rỗng → `[]` |
| `contract_templates.field_mapping` | object `{placeholder: canonical_field}` | NULL/rỗng → `{}` |
| `products.feature_list` | array `[{"no"?, "name", "description"?}]` | Hỏng hoặc không phải array → `[]` |
| `contract_events.meta_json` | tự do | Không dùng |

Ghi JSON: `serde_json::to_string` (không escape ký tự tiếng Việt — giống `ensure_ascii=False`).
Khác biệt khoảng trắng so với Python (`", "` / `": "`) không ảnh hưởng vì không ai so chuỗi JSON.

## 5. Chính sách migration

### 5.1 Khi mở một DB có sẵn

1. Với mỗi bảng trong `fixtures/schema.json::required_columns`, chạy `PRAGMA table_info(bang)`.
2. Thiếu bảng hoặc thiếu cột → **không tự sửa**. Hiện thông báo:
   "Cơ sở dữ liệu thuộc phiên bản cũ. Hãy mở bản ContractForge (Python) 0.10 trở lên một lần để
   nâng cấp, rồi mở lại." và thoát.
   *Lý do:* bản Python có ~40 bước migration lịch sử (`seed.run_migrations`) kèm backfill dữ liệu.
   Viết lại chúng bằng Rust tốn công mà chỉ phục vụ DB rất cũ.
3. Cột **thừa** (do bản Python tương lai thêm) → bỏ qua, không lỗi.

### 5.2 Khi chưa có DB (cài mới)

1. Chạy nguyên văn `fixtures/schema_baseline.sql` (nhúng bằng `include_str!`) trong một transaction.
2. Seed hai sản phẩm hư cấu theo `fixtures/schema.json::seed_products`
   (chỉ khi bảng `products` rỗng).
3. Không seed thông tin Bên B (để trống có chủ ý).
4. Nhập các mẫu `.docx` người dùng tự đặt trong `assets/default_templates/` (bỏ qua file
   `sample_*`) — xem `03_NGHIEP_VU.md` §6.6.

### 5.3 Migration riêng của Rust (sau này)

Dùng `PRAGMA user_version`. Hiện tại bằng 0 và **phải giữ 0** cho tới khi bản Rust qua nghiệm
thu và chủ dự án xóa hẳn bản Python (đã chốt). Khi đó mới đánh số migration Rust từ 1, dự kiến:

1. Đổi mọi cột đường dẫn file sang tương đối so với data root.
2. Thêm cột `contract_templates.content_sha256` (bỏ việc băm lại file mỗi lần khởi động).
3. Thêm chỉ mục còn thiếu (`contract_templates.product_group`, `contracts.parent_contract_id`…).
4. Bỏ hẳn hai trạng thái legacy `Active`, `ExpiringSoon` sau khi kiểm không còn dòng nào.

### 5.4 Các backfill dữ liệu của bản Python — Rust KHÔNG làm

Chỉ để biết vì sao dữ liệu cũ trông như vậy:

- Hợp đồng `legacy` có đúng **một** `contract_items` chép từ số đã lưu
  (`note = "Chép từ số đã lưu của hợp đồng"`), `amount_total = total_amount`, trước thuế suy ngược
  `round_half_up(total / (1 + vat/100))`.
- `products.contract_number_format` được suy ngược từ số hợp đồng đã có (`numbering.infer_format`).
- Trạng thái `Active` → `PaidActive`; `ExpiringSoon` → `PaidActive`/`Invoiced`/`Signed` tùy chứng từ.

## 6. Thiết lập (`app_settings`) đang dùng

| Khóa | Nghĩa | Mặc định |
|---|---|---|
| `party_b_name`, `party_b_address`, `party_b_tax_code`, `party_b_title` | Thông tin tổ chức Bên B | `""` |
| `party_b_representative`, `party_b_authorization_number`, `party_b_authorization_date` | Người đại diện ký và giấy ủy quyền | `""` |
| `party_b_bank_account`, `party_b_bank_name`, `party_b_beneficiary` | Ngân hàng Bên B | `""` |
| `party_b_phone`, `party_b_contact_name`, `party_b_contact_email` | Đầu mối liên hệ Bên B | `""` |
| `addendum_number_format` | Khuôn số phụ lục, phải có `{seq}` và `{parent}` | `""` → dùng `PL{seq}/{parent}` |

`get_setting(key, default)`: không có dòng **hoặc** giá trị rỗng → trả `default`.
"Hồ sơ tổ chức đầy đủ" = `party_b_name` và `party_b_representative` đều khác rỗng sau trim.
