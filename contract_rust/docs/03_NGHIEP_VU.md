# 03 — Nghiệp vụ

Quy ước:
- **Nguồn:** file/hàm Python là đặc tả. Câu chữ lỗi lấy nguyên văn từ đó (hoặc từ fixture).
- **[KB-xx]**: chỗ bản Rust cố ý khác bản Python — chi tiết ở `06_KHAC_BIET_CO_CHU_Y.md`.
- **Fixture:** file trong `fixtures/` mà code phải qua.
- Vị trí code Rust: `cf-core::<module>` cho luật thuần; `cf-services::<module>` cho ca sử dụng.

---

## 1. Tiện ích nền (`cf-core`)

### 1.1 Số tiền bằng chữ — `cf-core::words`
Nguồn: `app/utils/amount_to_words_vi.py`. Fixture: `amount_to_words.json`.
- Nhóm tỷ / triệu / nghìn / đơn vị, nối bằng `", "`, thêm `" đồng"`, viết hoa chữ đầu. `0` → `"Không đồng"`.
- Quy tắc đọc: `mốt` (chục ≥ 2, đơn vị 1), `lăm` (chục ≥ 2, đơn vị 5), `mười lăm` (15),
  `không mươi X` (nhóm có trăm hoặc nhóm không đầu mà chục = 0), `mười một`…
- Đầu vào thập phân → **cắt** phần lẻ (`int(Decimal)`); âm → lỗi `"Không hỗ trợ số âm"`;
  không phải số → lỗi `"Không hợp lệ: '...'"`.
- Bản `chan`: thêm `" chẵn"`.
- **[KB-01]** Phần "tỷ" ≥ 1000 đọc đệ quy ("Một nghìn tỷ đồng") thay vì sập.

### 1.2 Ngày tháng — `cf-core::dates`
Nguồn: `app/utils/date_helpers.py`. Fixture: `dates.json`.
- `add_months(d, n)`: cộng tháng, kẹp ngày cuối tháng (31/01 + 1 = 28/02 hoặc 29/02).
- `compute_end_date(start, n) = add_months(start, n) − 1 ngày` (ngày cuối cùng của kỳ).
- `months_between(start, end) = (end+1 ngày).year*12+month − (start.year*12+month)`.
- `parse_date_input`: thử lần lượt `%Y-%m-%d`, `%d/%m/%Y`, `%d-%m-%Y`, `%d/%m/%y`, `%d-%m-%y`;
  rỗng → `None`; không khớp → lỗi `"Không nhận dạng được định dạng ngày: '...'"`.
  Chấp nhận ngày/tháng 1 chữ số ("5/1/2026").
- `format_date_vn` → `dd/mm/yyyy`; `split_date_parts` → ngày/tháng 2 chữ số, năm 4 chữ số.

### 1.3 Văn bản, tên file — `cf-core::text`, `cf-core::names`
Nguồn: `contract_service.make_contract_slug`, `utils/file_naming.py`. Fixture: `names.json`.
- `make_contract_slug(name)`: "trạm y tế"/"trung tâm y tế"/"phòng y tế" → `TYT` (không phân biệt
  hoa thường, theo ranh giới từ); bỏ các từ `xã, phường, thị trấn, thị xã, thành phố, quận, huyện,
  tỉnh`; `Đ/đ → D/d`; bỏ dấu; chỉ giữ `[A-Za-z0-9]`; viết hoa.
- `sanitize_for_filename(s, max_len)`: thay `ĐđƠơƯư → DdOoUu`, NFD bỏ dấu, ký tự không phải
  `\w` hoặc `-` → `_`, gộp `_` liên tiếp, bỏ `_` hai đầu, **cắt `max_len` rồi mới viết hoa**.
- `make_output_filename(number, customer_code, contract_type, ts)` =
  `{type≤10}_{cust≤15}_{num≤30}_{YYYYMMDD_HHMMSS}.docx`; giá trị rỗng thay bằng `HD`, `KH`, `UNKNOWN`.
- Bỏ dấu dùng chung: `strip_diacritics()`; khóa tìm kiếm: `search_key(s) = strip_diacritics(s).to_lowercase()`.

### 1.4 Định dạng số và đọc số từ form — `cf-core::fmt`, `cf-core::parse`
Nguồn: `docx_renderer._fmt_*`, `contract_service._safe_*`, `pricing.fmt_money`,
`manual_quotation._number`. Fixture: `formatting.json`.
- `fmt_currency(v)`: **cắt** về số nguyên rồi chấm ngăn nghìn (`24.000.000`); `None` → `""`.
- `fmt_number(v)`: số nguyên → không có phần lẻ; có lẻ → in nguyên `Decimal`.
- `fmt_vat(v)`: `10` → `"10%"`, `8.5` → `"8.5%"`.
- `fmt_money(v)`: **làm tròn half-up** rồi chấm ngăn nghìn (khác `fmt_currency`!).
- `safe_decimal(s)`: bỏ mọi `.` rồi đổi `,` thành `.` (`"82.5"` → `825`, `"1.234,56"` → `1234.56`); hỏng → `None`.
- `safe_int(s)`: bỏ `,` và `.` rồi parse; hỏng → `None`.
- `manual_number(s, money)`: có `,` → kiểu Việt; không có `,` mà `money` và khớp `\d{1,3}(\.\d{3})+` → bỏ chấm; còn lại đọc kiểu máy.

---

## 2. Vòng đời hợp đồng — `cf-core::lifecycle`

Nguồn: `app/services/lifecycle.py`. Fixture: `lifecycle.json` (ma trận đầy đủ từ × đến × chứng từ).

- Trạng thái: `Draft → Generated → {Sent, Signed}`, `Sent → Signed`, `Signed → {Invoiced, Terminated}`,
  `Invoiced → {PaidActive, Terminated}`, `PaidActive → {Expired, Terminated}`,
  `Expired → {Terminated}`, `Terminated → ∅`. Legacy `Active`, `ExpiringSoon` →
  `{Invoiced, PaidActive, Expired, Terminated}`. Không bước nào đi **tới** `Active`/`ExpiringSoon`.
- Chứng từ bắt buộc: tới `Invoiced` cần `invoice_pdf_path`; tới `PaidActive` cần `payment_slip_path`.
- Tập trạng thái (dùng ở mọi nơi, không định nghĩa lại):
  - `ACTIVE_LIFECYCLE` = Signed, Invoiced, PaidActive, Active, ExpiringSoon
  - `RENEWAL_COUNTS` = như trên
  - `IN_SERVICE` = Invoiced, PaidActive, Active, ExpiringSoon
  - `UNPAID` = Signed, Active, ExpiringSoon
- Nhãn hiển thị + màu: `STATUS_LABELS` (fixture). **Một bảng duy nhất** cho toàn app **[KB-09]**.
- Hết hạn là thông tin dẫn xuất: `days_to_expiry`, `expiry_bucket` (≤0 expired, ≤7 critical,
  ≤14 warning, ≤30 info, còn lại None), `is_expiring_soon` (0 < ngày ≤ 30).
- **Hàm chuyển trạng thái duy nhất** (`cf-services::contracts::change_status`) **[KB-07]**:
  `(contract_id, đích, actor, ghi chú)` → kiểm luật → đổi trạng thái → ghi sự kiện
  `status_changed` với mô tả `"Chuyển trạng thái {cũ} → {mới}"` (+ `": {ghi chú}"`). Mọi nơi khác
  (upload chứng từ, quét hết hạn, sinh file lần đầu, bulk) gọi hàm này.
- Đổi hàng loạt: kiểm **tất cả** trước; một cái lỗi → không đổi cái nào; thông báo
  `"Không cập nhật hợp đồng nào. N hợp đồng không hợp lệ — "` + 5 lỗi đầu + `" (và K lỗi khác)"`.
- Quét hết hạn: hợp đồng thuộc `ACTIVE_LIFECYCLE` có `end_date ≤ hôm nay` → `Expired`, sự kiện
  `expired` mô tả `"Tự động cập nhật: hết hạn ngày {YYYY-MM-DD}"`, actor `system`.
  Chạy lúc khởi động và mỗi lần qua nửa đêm **[KB-08]** (Python: mỗi lần mở dashboard).
  Bước chuyển tự động này **không** đi qua `validate_transition` (Signed → Expired không có trong
  bảng) — giữ đúng như Python, viết thành hàm riêng `expire_due()` trong cùng module.

---

## 3. Đánh số — `cf-core::numbering`

Nguồn: `app/services/numbering.py`, `contract_service.get_next_contract_seq`. Fixture: `numbering.json`.

- Khuôn hợp đồng: chuỗi có `{seq}` (bắt buộc), `{slug}`, `{year}`; chỗ điền khác → lỗi
  `InvalidNumberFormat` (câu chữ trong fixture). Mặc định `{seq}/{slug}/{year}`.
- Khuôn của sản phẩm: `products.contract_number_format` nếu khác rỗng, không thì mặc định.
- `{seq}` = số thứ tự đệm 0 hai chữ số; **lấp khoảng trống** (số dương nhỏ nhất chưa dùng).
- Bộ đếm riêng theo **(khách hàng, khuôn, năm)**.
- **[KB-02]** Dịch khuôn thành regex khớp nguyên chuỗi:
  `{seq}` → `(?P<seq>\d+)`, `{slug}` → `(?P<slug>.+?)`, `{year}` → năm cụ thể, phần còn lại
  `regex::escape`. Số đã dùng = các hợp đồng của khách hàng đó khớp regex. Thay cho `LIKE` +
  "lấy phần trước dấu `/` đầu tiên".
- Gợi ý số: `build_number(next_seq, make_contract_slug(legal_name), khuôn, năm nay)`.
- `infer_format` chỉ cần cho công cụ, không dùng khi chạy app (bỏ qua ở Đợt đầu).
- Khuôn phụ lục: setting `addendum_number_format` (mặc định `PL{seq}/{parent}`), phải có cả
  `{seq}` và `{parent}`. Số thứ tự phụ lục đếm theo các phụ lục của **cùng hợp đồng gốc**, đọc
  ngược bằng regex của khuôn, lấp khoảng trống.

---

## 4. Tính tiền — `cf-core::pricing`

Nguồn: `app/services/pricing.py`. Fixture: `pricing.json`.

- Dòng: `số kỳ = tháng / kỳ giá`; `trước thuế = round_half_up(đơn giá × SL × số kỳ)`;
  `thuế = round_half_up(trước thuế × thuế suất / 100)`; `sau thuế = trước thuế + thuế`.
- Ghi đè thành tiền: phải có ghi chú, không thì lỗi `"Ghi đè thành tiền phải kèm ghi chú lý do."`.
- Kiểm: SL > 0; đơn giá ≥ 0; 0 ≤ thuế suất ≤ 100; tháng và kỳ giá > 0.
- `normalize_quantity(raw, min, step)`: áp sàn `min`, rồi làm tròn **lên** bội số `step`.
- Tổng = tổng các dòng; `amount_text` = chữ của tổng (rỗng nếu tổng 0). Không có dòng → lỗi.
- `unit_price_with_vat(p, v) = round_half_up(p × (1 + v/100))` — chỉ để hiển thị.
- `describe(lines, totals)` = `"{tên}: {fmt_money(sau thuế)} VNĐ; ..." + ". Tổng cộng: {..} VNĐ"`.
- `addon_months(start, main_end)`: số tháng nhỏ nhất `m ≥ 1` sao cho `add_months(start, m) > main_end`;
  `start > main_end` → lỗi; quá 600 → lỗi.
- **Hợp đồng `legacy` không đi qua module này** (giữ số đã lưu). Công thức legacy cho nhánh không
  có dòng: `đơn giá có VAT = đơn giá × (1 + VAT/100)` (**không làm tròn**), `SL = số cơ sở × số tháng`,
  `tổng = đơn giá có VAT × SL`. Viết **một** hàm `legacy_amounts()` dùng chung cho form và batch
  (Python có hai bản chép).

### 4.1 Dòng hợp đồng từ form (`items_json`)
Nguồn: `contract_service._build_lines`.
- Mỗi phần tử: `{product_id, quantity?, unit_price_pre_vat?, vat_rate?, description?, note?, amount_pre_vat_override?}`.
- Dòng 1 phải là gói `main` (trừ phụ lục bổ sung); các dòng sau **không** được là `main` và phải
  cùng `group_code` với gói chính. Gói phải tồn tại và đang bán.
- Thiếu đơn giá/thuế suất → lấy mặc định của gói. SL qua `normalize_quantity` với min/step của gói.
- Mọi lỗi có tiền tố `"Dòng {n}: "`.
- **Số lượng mặc định** theo `quantity_source`: `units` → số cơ sở đã tích (≥1);
  `records` → `customer.record_count` qua `normalize_quantity` (thiếu → rỗng = bắt nhập);
  `manual` → `1`.
- `apply_lines`: hợp đồng thành `items`; `product_id` = gói dòng 1; `unit_count = int(SL dòng 1)`;
  `quantity = SL × số kỳ` dòng 1; tổng/trước thuế/thuế lấy từ tổng dòng; chỉ điền `amount_text`
  và `pricing_description` nếu đang rỗng.

---

## 5. Khách hàng — `cf-services::customers`

Nguồn: `app/services/customer_service.py`, `app/routers/customers.py`, `app/models/customer*.py`.

- **Mã**: trống → `KH-NNN` với NNN = (lớn nhất trong các mã đúng dạng `^KH-(\d+)$`) + 1, đệm 3 chữ số.
  Trùng mã → `"Mã khách hàng '{code}' đã tồn tại."`.
- **Trạng thái 3 mức**: đang hoạt động; tạm ngưng (`is_active=false`, ẩn khỏi form tạo hợp đồng);
  đã xóa (`is_deleted=true`, `deleted_at`, ẩn khỏi mọi nơi, khôi phục được từ thùng rác).
- **Địa chỉ đầy đủ** = nối `address, ward, province` khác rỗng bằng `", "` (bỏ `district`).
- **Cơ sở tính phí**: trạm chính (nếu `bill_main_station`) + đơn vị con `is_active && include_in_billing`.
  `total_units` = số đó; `unit_names_list` = tên ngắn (hoặc tên pháp lý) của trạm chính + tên các đơn vị con.
- **Đơn vị con**: lưu đồng bộ danh sách từ form (thêm/sửa/xóa theo `id`); loại trong `UNIT_TYPES`.
- **Giấy tờ**: PDF/JPG/PNG theo magic bytes, ≤ 5 MB. Loại cố định (`cccd_front`, `cccd_back`,
  `establishment`) → tối đa 1, upload mới thay file cũ (ghi file mới trước, commit, rồi mới xóa
  file cũ). Loại `other` → cần nhãn, không giới hạn số lượng. Tên file đích:
  `{id}_{doc_type}{ext}` hoặc `{id}_other_{epoch_ms}{ext}` trong `customer_docs/{id}/`.
- **Nhập Excel (phẳng)**: dò dòng tiêu đề trong 5 dòng đầu theo `EXCEL_COLUMN_MAP` (không phân
  biệt hoa thường); có mã → cập nhật, không mã → tạo mới. Báo lỗi theo số dòng.
- **Nhập Excel (nhóm)**: dòng "Trạm y tế" → khách hàng; dòng "Điểm trạm" → đơn vị con của trạm
  ngay trên; đồng bộ bảo toàn (tên trùng → giữ, có trong DB mà không có trong file → ẩn, không xóa).
  Đặc tả chi tiết: `import_grouped_from_excel`, `_sync_sub_units_preserve`, `_make_short_name`.
- **Xuất**: danh sách (`/customers/template.xlsx` là file mẫu), báo cáo 4 sheet
  (`export_report` — Danh sách/tier, Hồ sơ đầy đủ, Lịch sử hợp đồng, Tóm tắt), thông tin một
  khách hàng dạng Markdown (`_build_info_md`), gói ZIP hồ sơ khách hàng (`download_bundle`).
  - **[KB-09]** Báo cáo dùng tập trạng thái của `lifecycle` và `STATUS_LABELS`:
    "đang hiệu lực" = `ACTIVE_LIFECYCLE`; "đã ký" = `ACTIVE_LIFECYCLE ∪ {Expired, Terminated}`;
    "nháp" = `{Draft, Generated, Sent}`; "không còn hiệu lực" = `{Expired, Terminated}`.
  - Thuật toán phân hạng (tier 1–5) giữ nguyên thứ tự điều kiện của `_classify`.
- **Tìm kiếm/lọc danh sách**: từ khóa khớp **không dấu** trên mã, tên pháp lý, tên ngắn **[KB-11]**;
  lọc xã/phường, loại; mặc định chỉ hiện đang hoạt động; sắp xếp mặc định theo xã/phường.

---

## 6. Sản phẩm và mẫu

### 6.1 Sản phẩm — `cf-services::products`
Nguồn: `app/services/product_service.py`.
- `clean_form`: gom **tất cả** lỗi rồi báo một lần (câu chữ trong hàm). Mã viết hoa, duy nhất.
  Gói không phải `main` bắt buộc có `group_code`. Thuế suất đọc kiểu máy (`"8.5"` = 8,5%); đơn giá
  từ form đọc bằng `safe_decimal`, từ Excel đọc kiểu máy. Mẫu mặc định của gói chính phải đúng
  vai trò hợp đồng và đúng nhóm.
- Danh sách: theo nhóm (không nhóm xuống cuối), gói chính trước, `sort_order`, tên.
- Ngừng bán/bán lại: không xóa, không đụng hợp đồng đã có. Hiển thị số hợp đồng đang phục vụ (`IN_SERVICE`).
- `feature_list`: ô văn bản mỗi dòng `"Tên | Mô tả"`.
- Nhập/xuất danh mục Excel: đọc và kiểm từng dòng (không ghi), xem trước, rồi ghi **tất cả trong
  một transaction** (lỗi một dòng → không ghi gì). Cột `IMPORT_COLUMNS`; giữ nguyên
  `default_template_id`, `replaced_by_id`, `feature_list` khi nhập.

### 6.2 Vai trò của mẫu — `cf-core::template_rules`
Nguồn: `template_service.py`. Fixture: `template_rules.json`.
- Vai trò suy từ `contract_type`: `addon` ← {"Phụ lục bổ sung", "Phụ lục"}; `renewal` ←
  {"Phụ lục gia hạn", "Phụ lục"}; `attachment` ← {"Nghiệm thu", "Bàn giao", "Thanh lý", "Khác"};
  `quotation` ← {"Báo giá"}; `quote_request` ← {"Yêu cầu báo giá"}; `contract` ← mọi nhãn còn lại.
- Nhóm: `product_group` NULL = dùng chung; khác NULL thì chỉ dùng cho đúng nhóm.
- `templates_for(group, role)`: đang bật, đúng vai trò, đúng nhóm hoặc dùng chung; mẫu riêng của
  nhóm đứng trước, rồi theo tên (không phân biệt hoa thường).
- `default_template_for`: mẫu **riêng** của nhóm nếu có đúng một.
- Văn bản kèm theo được tick sẵn: mẫu riêng của nhóm và loại "Bàn giao"/"Nghiệm thu".
- Nhóm của một văn bản = nhóm gói chính của **hợp đồng gốc** (phụ lục lấy theo cha).

### 6.3 Thư viện mẫu — `cf-services::templates`
- Tạo: tên bắt buộc; chỉ `.docx`, ≤ 10 MB; **phân tích trước khi lưu** (hỏng → không lưu gì);
  mã trống → `TPL-NNN`; file lưu `uploads/templates/{id}_{uuid8}_{tên_an_toàn}`.
- Phân tích (`cf-docx::analyze`, fixture `template_analysis.json`): placeholder đã sắp xếp, không
  trùng; mapping mặc định theo `CANONICAL_MAP`/`ROW_PLACEHOLDERS`; cảnh báo split-run; nhóm dòng lặp;
  cảnh báo placeholder dòng lặp nằm ngoài bảng. Thêm: liệt kê dòng tùy chọn `{{?KHOA}}` **[KB-06]**.
- Sửa mapping: chỉ giữ khóa có trong `detected_placeholders`; placeholder thiếu → `""`.
- Phân tích lại: giữ mapping cũ cho placeholder còn tồn tại.
- Bật/tắt; tải file gốc; tải lên nhiều file (bulk) với loại suy bằng `_template_kind`.
- **Nhập mẫu cục bộ** khi khởi động: file `.docx` trong `assets/default_templates/` (trừ `sample_*`),
  mã `BUNDLED-{SHA256[:12]}`; đã có (theo mã hoặc trùng nội dung) → bỏ qua. **[KB-12]** Băm file
  mẫu đã có một lần mỗi phiên (cache theo đường dẫn + kích thước + thời điểm sửa); **không** tự
  dọn file mồ côi lúc khởi động — có nút "Dọn file mẫu thừa" trong trang Mẫu.

---

## 7. Hợp đồng — `cf-services::contracts`

Nguồn: `contract_service.py`, `routers/contracts.py`.

### 7.1 Danh sách
- Lọc: từ khóa (số HĐ, ghi chú, tên khách hàng — **không dấu** [KB-11]), trạng thái, khách hàng,
  sản phẩm, năm ký, khoảng ngày theo `sign_date` hoặc `end_date`, và bộ lọc cảnh báo:
  - `unsent_10d`: Sent và `updated_at` < hôm nay − 10 ngày;
  - `expiring_60`: `ACTIVE_LIFECYCLE`, 0 < còn ≤ 60 ngày, **chưa gia hạn**, không phải phụ lục bổ sung;
  - `generated`: có `output_file_path` khác rỗng;
  - `all_active`: `IN_SERVICE`.
- "Chưa gia hạn": không có phụ lục gia hạn (`parent=id`, `doc_kind=addendum_renewal`) và không có
  hợp đồng mới (`renewed_from_id=id`) nào ở trạng thái `RENEWAL_COUNTS`. Một hàm dùng chung.
- Sắp xếp: created_at (mặc định, giảm dần), sign_date, end_date, total_amount, contract_number, status.
- Phân trang 10/25/50/100; tổng tiền của kết quả lọc; danh sách năm có hợp đồng; huy hiệu đếm
  `unsent_10d`, `expiring_60`. Cờ dòng: "chưa gửi > 10 ngày", "sắp hết hạn ≤ 60 ngày".
- Xuất Excel 2 sheet (cột y như `export_contracts`): tiền **cắt** về số nguyên, ngày `dd/mm/yyyy`.

### 7.2 Tạo bản nháp (`create_draft`)
- Bắt buộc: số HĐ (không trùng), khách hàng. Mẫu (nếu chọn) phải tồn tại và đang bật.
- Ngày đọc bằng `parse_date_input` (lỗi → None). Có tháng + bắt đầu, thiếu kết thúc →
  `compute_end_date`; có bắt đầu + kết thúc, thiếu tháng → `months_between`.
- Không có `items_json` → nhánh legacy (`legacy_amounts`) và `amount_text` tự sinh nếu trống.
- Có `items_json` → cần số tháng; tính dòng (§4.1), `apply_lines`, `payment_mode`, `payment_cycle_months`.
- Kiểm mẫu theo vai trò và **nhóm của gói chính cuối cùng** (gói dòng 1, không phải ô chọn).
- Lưu tên cơ sở đã tích (`selected_unit_names`). Ghi sự kiện `created` "Tạo bản nháp hợp đồng".
- Sau khi tạo, nếu có mẫu → sinh file ngay (lỗi sinh file không xóa bản nháp; báo cảnh báo).

### 7.3 Sinh file (`generate_docx`) — thứ tự bắt buộc
1. Kiểm: có hợp đồng, có mẫu, mẫu đúng vai trò/nhóm (**không** kiểm đang bật — HĐ cũ phải sinh lại
   được), file mẫu tồn tại; kiểm các văn bản kèm theo được tick (đang bật, đúng vai trò/nhóm, có file).
2. Legacy và `pricing_description` trống → tự sinh mô tả
   `"Số tiền = Đơn giá có VAT (X VNĐ) × N cơ sở (tên; tên) × M tháng = T VNĐ"` (danh sách tên chỉ
   hiện khi > 1 cơ sở; nguồn `_auto_pricing_description`).
3. Dựng ngữ cảnh (§11) + dòng lặp; **render hợp đồng và mọi văn bản kèm theo trong bộ nhớ**.
4. Ghi file hợp đồng vào `outputs/contracts/` bằng **tạo mới độc quyền**; trùng tên → `_2`, `_3`…
5. Có văn bản kèm theo → ghi ZIP `outputs/bundles/{id}/{tên_docx}_kem_theo.zip` (gồm hợp đồng +
   kèm theo; trùng tên trong ZIP → `_2`…). Lỗi → xóa file bước 4.
6. Một transaction: lưu `render_data` (ngữ cảnh + `_lists`), đường dẫn/tên file; Draft → Generated
   qua hàm chuyển trạng thái; ghi sự kiện `generated` "Sinh file {tên}". Lỗi → xóa file bước 4–5.
7. Sau commit: xóa file .docx cũ (nếu khác), xóa các ZIP cũ (chỉ giữ ZIP vừa tạo, không có → xóa thư mục).

### 7.4 Chứng từ (upload)
| Loại | Định dạng | Tên file | Tự chuyển trạng thái | Sự kiện |
|---|---|---|---|---|
| Scan hợp đồng ký | PDF (`%PDF`), ≤ 5 MB | `signed_scans/scan_{id}_{số_an_toàn}.pdf` | Generated/Sent → Signed | `scan_uploaded` |
| Hóa đơn | PDF, ≤ 5 MB, kèm `invoice_date` tùy chọn | `invoice_docs/invoice_{id}_{số_an_toàn}.pdf` | Signed/Active → Invoiced | `invoice_uploaded` |
| Ủy nhiệm chi | PDF/JPG/PNG, ≤ 5 MB, kèm `payment_date` | `payment_slips/payment_{id}_{số_an_toàn}{ext}` | Invoiced → PaidActive | `payment_slip_uploaded` |

- `số_an_toàn` = số HĐ thay mọi ký tự không phải `[\w-]` bằng `_`.
- Ghi file mới trước → cập nhật DB → xóa file cũ (nếu khác đường dẫn).
- Tự chuyển trạng thái đi qua hàm chuyển trạng thái **[KB-07]**, actor `system`, ghi chú
  `"tự động sau khi upload ..."`. Câu báo kết quả giữ như Python.
- Xóa chứng từ: xóa file, xóa 3 cột, sự kiện `*_deleted`. **Không** đổi trạng thái.
- Xem chứng từ: mở bằng ứng dụng mặc định.

### 7.5 Xóa
- Không xóa hợp đồng còn phụ lục: `"Không xóa được hợp đồng {số}: còn {n} phụ lục. Hãy xóa các phụ lục trước."`.
- Xóa kèm file riêng của hợp đồng (docx, scan, hóa đơn, ủy nhiệm chi, thư mục bundle).
  **Không** xóa giấy tờ khách hàng. Xóa nhiều: kiểm hết trước khi xóa cái nào.

---

## 8. Phụ lục và gia hạn — `cf-services::addenda`

Nguồn: `app/services/addendum_service.py`.

- Hợp đồng gốc hợp lệ: `doc_kind = contract`, trạng thái `IN_SERVICE`, có ngày bắt đầu và kết thúc.
- **Phụ lục bổ sung**: ngày bắt đầu ≥ ngày bắt đầu gốc; số tháng = `addon_months(start, end gốc)`;
  ngày kết thúc = ngày kết thúc gốc; mọi dòng là gói không phải `main`, cùng nhóm gốc; SL trống →
  mặc định theo nguồn số lượng; số = khuôn phụ lục; mẫu = chọn tay (đúng vai trò `addon`/nhóm) hoặc
  mẫu riêng duy nhất của nhóm. Ghi sự kiện `addendum` ở cả phụ lục và hợp đồng gốc.
- **Gia hạn** (người dùng chọn một trong hai):
  - Phụ lục gia hạn: bắt đầu **sau** ngày kết thúc gốc; số tháng ≥ 1; dòng 1 là gói chính.
  - Hợp đồng mới: số HĐ nhập tay; `renewed_from_id = gốc`; mẫu = chọn hoặc mẫu mặc định của gói dòng 1.
  - Gợi ý: bắt đầu = kết thúc gốc + 1 ngày; số tháng = thời hạn mặc định của gói chính → số tháng gốc → 12;
    dòng gợi ý theo từng dòng gốc, đi theo `replaced_by_id` tối đa 5 bước tới gói đang bán.
  - So sánh giá theo tháng: cũ = `round(total/months)`; mới = tổng giá 1 tháng × SL 1 của các gói gợi ý.
- Ngày ký mặc định = hôm nay; địa điểm ký = của hợp đồng gốc.

---

## 9. Văn bản kèm theo và biểu 08a

- **Kèm theo** (`attachment_service.py`): mẫu vai trò `attachment` đúng nhóm; dùng ngữ cảnh hợp
  đồng, hoặc ngữ cảnh 08a nếu mẫu có trường `form08a.*`. Tải lẻ từng văn bản không đổi trạng thái.
- **Biểu 08a** (`form08a_service.py`): mẫu `template_08A.docx` nếu có, không thì bản nhúng
  `sample_form_08a.docx`; ngữ cảnh = ngữ cảnh hợp đồng + `BUDGET_UNIT_NAME` (tên pháp lý viết
  hoa), `BUDGET_UNIT_CODE`, `WORK_CONTENT` (tên sản phẩm bỏ phần `(...)` cuối, rỗng → "Phần mềm"),
  `PARTY_A_TITLE_UPPER`, `PARTY_B_TITLE_UPPER`. Tên file `08A_{mã_KH}_{số}.docx`. Không lưu trạng thái.

---

## 10. Sinh hàng loạt — `cf-services::batch`

Nguồn: `app/services/batch_generator.py`, `routers/batch.py`.
- Đầu vào: 1 mẫu + danh sách khách hàng + tham số chung + ghi đè theo từng khách (từ Excel).
- Mỗi khách: số HĐ = cột Excel hoặc tự gợi ý (§3); trùng → lỗi dòng đó; dòng gói từ `main_code` +
  `addon_codes` (tách `;` hoặc `,`) hoặc `items_json` chung; không có → legacy; mẫu phải đúng nhóm
  gói chính của dòng đó. Thành công → hợp đồng `Generated`, `is_batch`, `batch_job_id`, sự kiện
  `generated` "[Batch {job}] Sinh file {tên}", actor `batch`. Lỗi một khách không dừng cả lô.
- Kết quả: bảng từng dòng + ZIP các file thành công `batch_{job}_{YYYYMMDD_HHMMSS}.zip`.
- **[KB-13]** Chạy nền, có tiến độ và nút hủy; **biên dịch mẫu một lần** cho cả lô; ghi file bằng
  tạo mới độc quyền (Python ghi đè nếu trùng tên trong cùng giây) **[KB-17]**.
- Excel nhập lô: cột theo `BATCH_EXCEL_COLUMNS`; tìm khách theo mã hoặc tên; file mẫu `template.xlsx`.

---

## 11. Ngữ cảnh điền mẫu — `cf-core::render_context`

Nguồn: `docx_renderer.build_render_context`, `apply_template_mapping`, `lists_from_lines`.
Fixture: `render_context.json` (đầy đủ từng khóa cho 4 tình huống).

- Hàm thuần nhận struct (hợp đồng, khách hàng, cha, sản phẩm, cài đặt Bên B), trả
  `BTreeMap<String,String>`. Danh sách khóa đầy đủ: xem fixture; khóa chính:
  `PARTY_A_*`, `PARTY_B_*`, `CONTRACT_NUMBER`, `ADDENDUM_SEQ`, `SIGN_*`, `START_DATE`, `END_DATE`,
  `MONTH_COUNT`, `UNIT_COUNT`, `QUANTITY`, `UNIT_PRICE_*`, `VAT_RATE`, `TOTAL_*`, `AMOUNT_TEXT(_CHAN)`,
  `PRICING_DESCRIPTION`, `ACCEPTANCE_*`, `LIQUIDATION_*`, `PRODUCT_NAME`, `PARENT_*`, `PAYMENT_CYCLE_MONTHS`.
- Điểm dễ sai: `PARTY_A_REPRESENTATIVE_FULL` = `"(Ông) Tên"`/`"(Bà) Tên"` theo giới tính;
  `SIGN_LOCATION` = địa điểm ký hoặc tên pháp lý; Bên B: giá trị riêng của hợp đồng ưu tiên hơn cài đặt;
  `UNIT_COUNT` rỗng → `"1"`; `AMOUNT_TEXT` = nhập tay → chữ của tổng → rỗng.
- `apply_template_mapping`: với mỗi `placeholder → canonical` trong mapping của mẫu, nếu canonical
  có giá trị (qua `CANONICAL_MAP`) thì thêm khóa placeholder tùy biến.
- Dòng lặp `items` (ITEM_NO, NAME, UNIT, QTY = SL × số kỳ, PRICE, AMOUNT_PRE_VAT, VAT_RATE,
  VAT_AMOUNT, AMOUNT_TOTAL) và `features` (gộp theo tên, bỏ trùng, bỏ tên rỗng, đánh số lại).

---

## 12. Báo giá — `cf-services::quotations`

Ba luồng, **không lưu DB** (trừ hồ sơ người phát hành), kết quả là file để lưu:

| Luồng | Nguồn | Giá lấy từ | Tên file |
|---|---|---|---|
| Báo giá theo danh mục | `routers/quotation.py` | Danh mục sản phẩm; có dòng → `pricing`; không dòng → legacy (`product_price_with_vat` làm tròn) | `BaoGia_{mã_SP}_{tên_ngắn≤30, khoảng trắng→_}_{YYYYMMDD}.docx` |
| Báo giá tự nhập | `services/manual_quotation.py` | Người dùng gõ (1–100 hạng mục); không đọc danh mục, không đọc cài đặt | `BaoGia_{layout}_{ngày}.docx` **[KB-19]** |
| Yêu cầu báo giá | `services/quote_request.py` | **Không có giá** — dữ liệu không có khóa tiền nào | `YeuCauBaoGia_{slug}_{YYYYMMDD}.docx` |

- Báo giá theo danh mục: gói chính đang bán, đơn giá > 0; mẫu phải loại "Báo giá" và đúng nhóm;
  1 ≤ số tháng ≤ 120.
- Báo giá tự nhập: 5 bố cục `sample_quote_{compact,formal,landscape,features,terms}.docx` (nhúng);
  `QUOTE_DATE_DAY/MONTH` **không** đệm 0 (giữ như Python); `ITEM_PRICE` giữ phần lẻ;
  `omit_empty_tables={"features"}`. Hồ sơ người phát hành lưu vào `quotation_issuers` theo
  `fingerprint` (băm các trường), không lưu giá.
- Yêu cầu báo giá: gói chính + gói bổ sung cùng nhóm; số lượng mặc định theo nguồn số lượng;
  `REQUEST_ADDRESSEE` trống → chuỗi chấm `……………………………………………`; `omit_empty_tables={"features"}`;
  nút cài 3 mẫu mẫu (`install_sample_templates`, không ghi đè mẫu đã có).

---

## 13. Hồ sơ đấu nối — `cf-services::dossier`

Nguồn: `app/services/dossier_service.py` (đặc tả đầy đủ trong docstring của module).
- Gom giấy tờ khách hàng + scan/hóa đơn/ủy nhiệm chi của **mọi** hợp đồng của khách vào
  `dau-noi/{mã}_{tên}/`, đặt tên theo số thứ tự ô (3 ô cố định, khối hợp đồng bắt đầu từ ô 4,
  2 ô mỗi sản phẩm theo vị trí sản phẩm trong danh mục, giấy tờ khác từ ô 90).
- Tính **kế hoạch** trước (đủ/thiếu) rồi mới ghi; ưu tiên hardlink, không được thì copy; ghi
  `_checklist.txt`; đánh dấu thư mục bằng `.cf_dossier`; chỉ xóa file do chính nó tạo.
- Phát hiện hồ sơ cũ (nguồn mới hơn thư mục). Mở thư mục chỉ khi nằm trong `DOSSIER_DIR`.

---

## 14. Dashboard — `cf-services::dashboard`

Nguồn: `app/services/dashboard_service.py`, `routers/dashboard.py`.
Tính **một lượt trên Store** (không còn 15 truy vấn riêng). Các con số:
- Đếm theo trạng thái; `in_force` = `ACTIVE_LIFECYCLE`; `in_service` = `IN_SERVICE`; tổng giá trị
  đang hiệu lực, đã thanh toán (PaidActive), đã xuất hóa đơn (Invoiced), chưa thu (`UNPAID`);
  `generated` = có file; khách hàng đang hoạt động; mẫu đang bật.
- Sắp hết hạn ≤ 30 và 31–60 ngày (chưa gia hạn, không phải phụ lục bổ sung), sắp theo ngày; mức
  khẩn = `expiry_bucket` hoặc `info`.
- Theo sản phẩm (`ACTIVE_LIFECYCLE`), kể cả nhóm "Không phân loại".
- 8 sự kiện gần nhất (trừ `expiring_soon_alert`, `expired`).
- Tháng này vs tháng trước: HĐ mới (theo `created_at` UTC), HĐ ký (theo `sign_date` ≤ hôm nay, trạng
  thái đã ký), HĐ hết hạn trong tháng, giá trị HĐ mới.
- Top 5 khách hàng theo số HĐ đang hiệu lực. Dự báo hết hạn 12 tháng (đủ 12 cột kể cả 0).
- Việc cần làm: ≤ 7 ngày hết hạn (high); nháp chưa có file (medium); Generated > 14 ngày (medium,
  theo `updated_at` UTC); Signed > 7 ngày theo `sign_date` (medium); không có mẫu nào đang bật (low).
- Trang "Sắp hết hạn" chọn 7/30/60/90/365 ngày + xuất Excel; tìm kiếm toàn cục (khách hàng + hợp đồng, không dấu).

---

## 15. Cài đặt — `cf-services::settings`
- Hồ sơ tổ chức Bên B (13 khóa, `02_DU_LIEU.md` §6). Thiếu tên hoặc người đại diện → nhắc trên
  trang tạo hợp đồng.
- Khuôn số phụ lục: kiểm bằng `validate_addendum_format` trước khi lưu.
- Thư mục dữ liệu đang dùng (chỉ hiển thị) + nút mở thư mục.
