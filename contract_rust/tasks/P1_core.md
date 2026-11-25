# Đợt 1 — `cf-core`: nghiệp vụ thuần

Blueprint: `docs/03_NGHIEP_VU.md` §1–4, §6.2, §11; `docs/01_KIEN_TRUC.md` §3.
Mọi file trong `crates/cf-core/`. **Không I/O**: không đọc file, không DB, không lấy giờ hệ thống
(hàm cần "hôm nay" nhận tham số `today: NaiveDate`).
Test fixture đặt ở `crates/cf-core/tests/fixture_<module>.rs`, dùng `cf-testkit` (dev-dependency).

Lệnh chạy test một nhóm: `cargo test -p cf-core --test fixture_<module>`.

---

## 1A. Nền

### T1.01 — Khung module và lỗi · S
- [ ] Xong
- **Làm:** khai báo các module rỗng trong `lib.rs`: `text, money, fmt, parse, words, dates, names,
  lifecycle, numbering, pricing, template_rules, render_context, customer`. Tạo `error.rs`:
  `pub enum CoreError { Invalid(String) }` (thiserror, `Display` = chuỗi bên trong, không thêm tiền tố).
- **File:** `src/lib.rs`, `src/error.rs`, `src/<module>.rs` (rỗng)
- **Xong khi:** `cargo build -p cf-core`.
- **Phụ thuộc:** T0.03

### T1.02 — `text::strip_diacritics`, `text::search_key` · XS
- [ ] Xong
- **Làm:** `strip_diacritics(s)`: `Đ→D, đ→d` rồi NFD, bỏ ký tự combining (dùng
  `unicode_normalization::char::is_combining_mark`). `search_key(s) = strip_diacritics(s).to_lowercase()`.
- **Nguồn:** `ContractForge/app/utils/file_naming.py::sanitize_for_filename` (phần bỏ dấu), JS `normalize` trong `ContractForge/templates/contracts/form.html`
- **File:** `src/text.rs`
- **Test (unit):** `"Đồng Tâm"`→`"Dong Tam"`; `search_key("Trạm Y Tế")`=`"tram y te"`; `"Ứ ơ"`→`"U o"`.
- **Xong khi:** `cargo test -p cf-core text`.
- **Phụ thuộc:** T1.01

### T1.03 — `money`: làm tròn và đọc Decimal · XS
- [ ] Xong
- **Làm:** `round_half_up(d) -> Decimal` (MidpointAwayFromZero, 0 chữ số);
  `trunc_int(d) -> i128`; `to_decimal(raw: &str, field: &str) -> Result<Decimal, CoreError>`:
  rỗng/space → `"Thiếu {field}."`; không phải số → `"{field} không phải là số: '{raw}'"` (dấu nháy đơn như Python `repr`).
  Đọc kiểu máy (`"82.5"` = 82,5); chấp nhận dạng mũ `1E+3`.
- **Nguồn:** `ContractForge/app/services/pricing.py::to_decimal, round_money`
- **File:** `src/money.rs`, `tests/fixture_pricing.rs`
- **Test:** `pricing.json → round_money`.
- **Xong khi:** `cargo test -p cf-core --test fixture_pricing round_money`.
- **Phụ thuộc:** T1.01

### T1.04 — `fmt::fmt_currency` · XS
- [ ] Xong
- **Làm:** `fmt_currency(Option<Decimal>) -> String`: None → `""`; **cắt** về số nguyên; chấm ngăn nghìn; số âm giữ dấu `-`.
- **Nguồn:** `ContractForge/app/services/docx_renderer.py::_fmt_currency`
- **File:** `src/fmt.rs`, `tests/fixture_formatting.rs`
- **Test:** `formatting.json → fmt_currency` (bỏ qua ca đầu vào không phải số như `"abc"` — Rust nhận `Decimal`).
- **Xong khi:** `cargo test -p cf-core --test fixture_formatting fmt_currency`.
- **Phụ thuộc:** T1.03

### T1.05 — `fmt::fmt_number` · XS
- [ ] Xong
- **Làm:** None → `""`; giá trị nguyên → không phần lẻ; có lẻ → in Decimal **giữ nguyên số chữ số** như đã lưu (`12.3400` → `"12.3400"`, không bỏ 0 thừa).
- **Nguồn:** `docx_renderer.py::_fmt_number`
- **File:** `src/fmt.rs`, `tests/fixture_formatting.rs`
- **Test:** `formatting.json → fmt_number`.
- **Xong khi:** `cargo test -p cf-core --test fixture_formatting fmt_number`.
- **Phụ thuộc:** T1.04

### T1.06 — `fmt::fmt_vat` · XS
- [ ] Xong
- **Làm:** như `fmt_number` nhưng thêm `%`.
- **Nguồn:** `docx_renderer.py::_fmt_vat`
- **File:** `src/fmt.rs`, `tests/fixture_formatting.rs`
- **Test:** `formatting.json → fmt_vat`.
- **Xong khi:** `cargo test -p cf-core --test fixture_formatting fmt_vat`.
- **Phụ thuộc:** T1.05

### T1.07 — `fmt::fmt_money` · XS
- [ ] Xong
- **Làm:** **làm tròn half-up** rồi chấm ngăn nghìn (khác `fmt_currency`).
- **Nguồn:** `pricing.py::fmt_money`
- **File:** `src/fmt.rs`, `tests/fixture_formatting.rs`
- **Test:** `formatting.json → fmt_money`.
- **Xong khi:** `cargo test -p cf-core --test fixture_formatting fmt_money`.
- **Phụ thuộc:** T1.04

### T1.08 — `parse::safe_decimal` · XS
- [ ] Xong
- **Làm:** `safe_decimal(&str) -> Option<Decimal>`: trim; rỗng → None; **bỏ mọi `.`** rồi `,`→`.`; parse; lỗi → None.
  Lưu ý `"12 000"` → xem fixture (Python `Decimal("12 000")` lỗi → None).
- **Nguồn:** `ContractForge/app/services/contract_service.py::_safe_decimal`
- **File:** `src/parse.rs`, `tests/fixture_formatting.rs`
- **Test:** `formatting.json → safe_decimal`.
- **Xong khi:** `cargo test -p cf-core --test fixture_formatting safe_decimal`.
- **Phụ thuộc:** T1.03

### T1.09 — `parse::safe_int` · XS
- [ ] Xong
- **Làm:** trim; rỗng → None; bỏ `,` và `.`; parse `i64`; lỗi → None. `"007"` → 7, `"-5"` → -5.
- **Nguồn:** `contract_service.py::_safe_int`
- **File:** `src/parse.rs`, `tests/fixture_formatting.rs`
- **Test:** `formatting.json → safe_int`.
- **Xong khi:** `cargo test -p cf-core --test fixture_formatting safe_int`.
- **Phụ thuộc:** T1.08

### T1.10 — `parse::dec_str` · XS
- [ ] Xong
- **Làm:** `dec_str(Option<Decimal>) -> String`: None → `""`; bỏ số 0 thừa, không dạng mũ (`1E+3` → `"1000"`).
- **Nguồn:** `contract_service.py::_dec_str`
- **File:** `src/parse.rs`, `tests/fixture_formatting.rs`
- **Test:** `formatting.json → dec_str`.
- **Xong khi:** `cargo test -p cf-core --test fixture_formatting dec_str`.
- **Phụ thuộc:** T1.08

### T1.11 — `parse::manual_number`, `fmt::unit_price_text` · XS
- [ ] Xong
- **Làm:** `manual_number(raw, label, money) -> Result<Decimal, CoreError>` theo `03_NGHIEP_VU.md` §1.4;
  `unit_price_text(Decimal) -> String` (giữ phần lẻ, chấm ngăn nghìn, phẩy thập phân, bỏ 0 thừa).
- **Nguồn:** `ContractForge/app/services/manual_quotation.py::_number, _unit_price_text`
- **File:** `src/parse.rs`, `src/fmt.rs`, `tests/fixture_formatting.rs`
- **Test:** `formatting.json → manual_number, unit_price_text`.
- **Xong khi:** `cargo test -p cf-core --test fixture_formatting manual_number unit_price_text`.
- **Phụ thuộc:** T1.10

## 1B. Chữ, ngày, tên

### T1.12 — `words::amount_to_words` (+ KB-01) · S
- [ ] Xong
- **Làm:** `amount_to_words(n: u64) -> String`, `amount_to_words_chan(n: u64)`, và
  `amount_to_words_dec(d: Decimal) -> Result<String, CoreError>` (cắt phần lẻ; âm → `"Không hỗ trợ số âm"`).
  Phần tỷ ≥ 1000 → gọi đệ quy (không suffix) rồi thêm `" tỷ"` (**KB-01**).
- **Nguồn:** `ContractForge/app/utils/amount_to_words_vi.py`
- **File:** `src/words.rs`, `tests/fixture_words.rs`
- **Test:** `amount_to_words.json → cases` (ca `err` của chuỗi `"abc"`/`""` chỉ áp cho hàm nhận chuỗi — bỏ qua);
  ca lỗi IndexError → dùng `intentional_differences.json → KB-01_amount_to_words`.
- **Xong khi:** `cargo test -p cf-core --test fixture_words`.
- **Phụ thuộc:** T1.03

### T1.13 — `dates::add_months` · XS
- [ ] Xong
- **Làm:** `add_months(d: NaiveDate, months: i32) -> NaiveDate`, kẹp ngày cuối tháng.
- **Nguồn:** `ContractForge/app/utils/date_helpers.py::add_months`
- **File:** `src/dates.rs`, `tests/fixture_dates.rs`
- **Test:** `dates.json → add_months`.
- **Xong khi:** `cargo test -p cf-core --test fixture_dates add_months`.
- **Phụ thuộc:** T1.01

### T1.14 — `dates::compute_end_date`, `months_between` · XS
- [ ] Xong
- **Nguồn:** `date_helpers.py::compute_end_date, months_between`
- **File:** `src/dates.rs`, `tests/fixture_dates.rs`
- **Test:** `dates.json → compute_end_date, months_between`.
- **Xong khi:** `cargo test -p cf-core --test fixture_dates compute_end_date months_between`.
- **Phụ thuộc:** T1.13

### T1.15 — `dates::parse_date_input` · XS
- [ ] Xong
- **Làm:** `-> Result<Option<NaiveDate>, CoreError>`; 5 định dạng theo thứ tự; năm 2 chữ số theo quy tắc
  Python `%y` (00–68 → 20xx, 69–99 → 19xx); câu lỗi y fixture.
- **Nguồn:** `date_helpers.py::parse_date_input`
- **File:** `src/dates.rs`, `tests/fixture_dates.rs`
- **Test:** `dates.json → parse_date_input`.
- **Xong khi:** `cargo test -p cf-core --test fixture_dates parse_date_input`.
- **Phụ thuộc:** T1.13

### T1.16 — `dates::format_date_vn`, `split_date_parts` · XS
- [ ] Xong
- **Nguồn:** `date_helpers.py::format_date_vn, split_date_parts`
- **File:** `src/dates.rs`, `tests/fixture_dates.rs`
- **Test:** `dates.json → format`.
- **Xong khi:** `cargo test -p cf-core --test fixture_dates format`.
- **Phụ thuộc:** T1.13

### T1.17 — `names::make_contract_slug` · XS
- [ ] Xong
- **Làm:** theo `03_NGHIEP_VU.md` §1.3. Regex không phân biệt hoa thường, `\b` Unicode. Thay thế lần lượt như Python.
- **Nguồn:** `ContractForge/app/services/contract_service.py::make_contract_slug`
- **File:** `src/names.rs`, `tests/fixture_names.rs`
- **Test:** `names.json → slug`.
- **Xong khi:** `cargo test -p cf-core --test fixture_names slug`.
- **Phụ thuộc:** T1.02

### T1.18 — `names::sanitize_for_filename` · XS
- [ ] Xong
- **Làm:** thay `ĐđƠơƯư`, NFD bỏ dấu, `[^\w\-]`→`_`, gộp `_`, bỏ `_` hai đầu, **cắt theo số ký tự** rồi viết hoa.
- **Nguồn:** `ContractForge/app/utils/file_naming.py::sanitize_for_filename`
- **File:** `src/names.rs`, `tests/fixture_names.rs`
- **Test:** `names.json → sanitize`.
- **Xong khi:** `cargo test -p cf-core --test fixture_names sanitize`.
- **Phụ thuộc:** T1.02

### T1.19 — `names::make_output_filename` · XS
- [ ] Xong
- **Làm:** nhận `ts: NaiveDateTime` (không tự lấy giờ).
- **Nguồn:** `file_naming.py::make_output_filename`
- **File:** `src/names.rs`, `tests/fixture_names.rs`
- **Test:** `names.json → output_filename`.
- **Xong khi:** `cargo test -p cf-core --test fixture_names output_filename`.
- **Phụ thuộc:** T1.18

### T1.20 — `names::safe_number_part`, `names::windows_safe_filename` (KB-19) · XS
- [ ] Xong
- **Làm:** `safe_number_part(s)` = thay mọi ký tự không khớp `[\w-]` bằng `_` (dùng cho tên file scan/hóa đơn/ủy nhiệm chi).
  `windows_safe_filename(s)` = thay `\ / : * ? " < > |` bằng `-`.
- **Nguồn:** `ContractForge/app/routers/contracts.py` (`re.sub(r"[^\w\-]", "_", ...)`), **KB-19**
- **File:** `src/names.rs`
- **Test (unit):** `safe_number_part("01/ABC/2026")`=`"01_ABC_2026"`; `intentional_differences.json → KB-19`.
- **Xong khi:** `cargo test -p cf-core names`.
- **Phụ thuộc:** T1.18

## 1C. Vòng đời

### T1.21 — `lifecycle::ContractStatus` · S
- [ ] Xong
- **Làm:** enum 10 giá trị + `Unknown(String)`; `as_str`, `FromStr` (chuỗi lạ → `Unknown`), `parse_status`
  (chuỗi lạ → lỗi `"Trạng thái không hợp lệ: '...'"`); `label()` và `color()` từ `STATUS_LABELS`;
  hằng `ACTIVE_LIFECYCLE`, `RENEWAL_COUNTS`, `IN_SERVICE`, `UNPAID`, `DEPRECATED` (`&[ContractStatus]`);
  `SELECTABLE` = các trạng thái không deprecated, không legacy (cho dropdown lọc).
- **Nguồn:** `ContractForge/app/services/lifecycle.py`, `ContractForge/app/models/contract.py`
- **File:** `src/lifecycle.rs`, `tests/fixture_lifecycle.rs`
- **Test:** `lifecycle.json → sets, status_labels`.
- **Xong khi:** `cargo test -p cf-core --test fixture_lifecycle sets labels`.
- **Phụ thuộc:** T1.01

### T1.22 — `lifecycle::validate_transition` · S
- [ ] Xong
- **Làm:** `pub struct Evidence { has_invoice: bool, has_payment_slip: bool }`;
  `valid_targets(from) -> &[ContractStatus]`; `validate_transition(from, to: &str, ev) -> Result<ContractStatus, CoreError>`
  câu lỗi y Python (danh sách cho phép sắp theo tên, rỗng → `'không có'`).
  **Lưu ý:** Python in danh sách dạng `['Invoiced', 'Terminated']` — xem fixture để in đúng.
- **Nguồn:** `lifecycle.py::VALID_TRANSITIONS, REQUIRED_EVIDENCE, validate_transition`
- **File:** `src/lifecycle.rs`, `tests/fixture_lifecycle.rs`
- **Test:** `lifecycle.json → valid_transitions, transition_matrix` (toàn bộ ma trận).
- **Xong khi:** `cargo test -p cf-core --test fixture_lifecycle transition`.
- **Phụ thuộc:** T1.21

### T1.23 — `lifecycle`: hết hạn · XS
- [ ] Xong
- **Làm:** `days_to_expiry(end, today)`, `expiry_bucket(end, today) -> Option<Bucket>`
  (`Expired|Critical|Warning|Info`), `is_expiring_soon`. Hằng ngưỡng 7/14/30/60.
- **Nguồn:** `lifecycle.py::days_to_expiry, expiry_bucket, is_expiring_soon`
- **File:** `src/lifecycle.rs`, `tests/fixture_lifecycle.rs`
- **Test:** `lifecycle.json → expiry, thresholds`.
- **Xong khi:** `cargo test -p cf-core --test fixture_lifecycle expiry`.
- **Phụ thuộc:** T1.21

## 1D. Đánh số

### T1.24 — `numbering::validate_format`, `build_number` · XS
- [ ] Xong
- **Làm:** câu lỗi y fixture (`{sorted(unknown)}` in dạng `['foo']`). `build_number(seq, slug, fmt: Option<&str>, year)`.
- **Nguồn:** `ContractForge/app/services/numbering.py::validate_format, build_number`
- **File:** `src/numbering.rs`, `tests/fixture_numbering.rs`
- **Test:** `numbering.json → validate_format, build_and_like` (chỉ phần `result`; bỏ `like` — Rust không dùng LIKE).
- **Xong khi:** `cargo test -p cf-core --test fixture_numbering validate build`.
- **Phụ thuộc:** T1.01

### T1.25 — `numbering::next_number` (KB-02) · S
- [ ] Xong
- **Làm:** `format_regex(fmt, year) -> Regex` (mục KB-02); `next_seq(used: &BTreeSet<u32>) -> String` (lấp khoảng trống, đệm 2);
  `next_number(fmt, slug, year, existing_numbers_of_customer: &[&str]) -> String`.
  `seq_of` của Python **không** port.
- **Nguồn:** `numbering.py::next_seq`, `contract_service.py::get_next_contract_seq`, **KB-02**
- **File:** `src/numbering.rs`, `tests/fixture_numbering.rs`
- **Test:** `numbering.json → next_seq`; `intentional_differences.json → KB-02_numbering_next_seq`.
- **Xong khi:** `cargo test -p cf-core --test fixture_numbering next`.
- **Phụ thuộc:** T1.24

### T1.26 — `numbering`: khuôn phụ lục · XS
- [ ] Xong
- **Làm:** `validate_addendum_format`, `build_addendum_number`, `parse_addendum_seq`, `next_addendum_seq`. Mặc định `PL{seq}/{parent}`.
- **Nguồn:** `numbering.py` (phần addendum)
- **File:** `src/numbering.rs`, `tests/fixture_numbering.rs`
- **Test:** `numbering.json → addendum`.
- **Xong khi:** `cargo test -p cf-core --test fixture_numbering addendum`.
- **Phụ thuộc:** T1.24

## 1E. Tính tiền

### T1.27 — `pricing::normalize_quantity`, `period_count` · XS
- [ ] Xong
- **Làm:** `PricingError` = `CoreError::Invalid` với câu y Python.
- **Nguồn:** `ContractForge/app/services/pricing.py::normalize_quantity, period_count`
- **File:** `src/pricing.rs`, `tests/fixture_pricing.rs`
- **Test:** `pricing.json → normalize_quantity, period_count`.
- **Xong khi:** `cargo test -p cf-core --test fixture_pricing normalize period`.
- **Phụ thuộc:** T1.03

### T1.28 — `pricing::price_line` · S
- [ ] Xong
- **Làm:** `pub struct Line {description, unit_label, quantity, period_count, unit_price_pre_vat, vat_rate,
  amount_pre_vat, vat_amount, amount_total, product_id: Option<i64>, note}`;
  `pub struct LineInput {...}` (các trường chuỗi/Decimal như Python); `price_line(LineInput) -> Result<Line, CoreError>`.
  Thứ tự kiểm lỗi y Python.
- **Nguồn:** `pricing.py::price_line`
- **File:** `src/pricing.rs`, `tests/fixture_pricing.rs`
- **Test:** `pricing.json → price_line`.
- **Xong khi:** `cargo test -p cf-core --test fixture_pricing price_line`.
- **Phụ thuộc:** T1.27

### T1.29 — `pricing::sum_lines`, `unit_price_with_vat`, `describe` · XS
- [ ] Xong
- **Nguồn:** `pricing.py::sum_lines, unit_price_with_vat, describe`
- **File:** `src/pricing.rs`, `tests/fixture_pricing.rs`
- **Test:** `pricing.json → sum_lines, unit_price_with_vat, empty_sum`.
- **Xong khi:** `cargo test -p cf-core --test fixture_pricing sum unit_price`.
- **Phụ thuộc:** T1.28, T1.12, T1.07

### T1.30 — `pricing::addon_months` · XS
- [ ] Xong
- **Nguồn:** `pricing.py::addon_months`
- **File:** `src/pricing.rs`, `tests/fixture_pricing.rs`
- **Test:** `pricing.json → addon_months`.
- **Xong khi:** `cargo test -p cf-core --test fixture_pricing addon_months`.
- **Phụ thuộc:** T1.13

### T1.31 — `pricing::legacy_amounts` · XS
- [ ] Xong
- **Làm:** gộp công thức legacy của `create_draft` và `_build_contract_from_params` thành một hàm:
  vào `(unit_count, month_count, unit_price_pre_vat, vat_rate, unit_price_vat, quantity, total_amount)` (Option),
  ra các giá trị đã điền. **Không làm tròn.** `amount_text` = `amount_to_words_dec(total)` nếu có tổng.
- **Nguồn:** `contract_service.py::create_draft` (khối "Giá trị"), `batch_generator.py::_build_contract_from_params`
- **File:** `src/pricing.rs`
- **Test (unit):** giá 440909, VAT 10, 2 cơ sở, 12 tháng → giá có VAT `484999.9`, SL `24`, tổng `11639997.6`,
  chữ `"Mười một triệu, sáu trăm ba mươi chín nghìn, chín trăm chín mươi bảy đồng"`;
  có sẵn tổng → giữ nguyên; thiếu giá có VAT mà có giá trước VAT → tổng = giá trước VAT × SL? **Không**: Python
  dùng giá có VAT đã tự tính; chỉ khi giá có VAT rỗng/0 mới dùng giá trước VAT — viết đúng thứ tự `if/elif` của Python.
- **Xong khi:** `cargo test -p cf-core pricing::legacy`.
- **Phụ thuộc:** T1.29

## 1F. Mẫu và ngữ cảnh

### T1.32 — `template_rules`: vai trò · XS
- [ ] Xong
- **Làm:** `enum Role {Contract, Addon, Renewal, Attachment, Quotation, QuoteRequest}`; hằng nhãn loại;
  `CONTRACT_TYPES`; `has_role(contract_type, role)`; `normalize_group(Option<&str>) -> Option<String>`;
  `role_of_doc_kind`.
- **Nguồn:** `ContractForge/app/services/template_service.py` (đầu file tới `role_of_doc_kind`)
- **File:** `src/template_rules.rs`, `tests/fixture_template_rules.rs`
- **Test:** `template_rules.json → CONTRACT_TYPES, has_role, role_of_doc_kind`.
- **Xong khi:** `cargo test -p cf-core --test fixture_template_rules has_role`.
- **Phụ thuộc:** T1.01

### T1.33 — `template_rules::check_template_for`, `preselected_attachment`, `template_kind` · XS
- [ ] Xong
- **Làm:** `pub struct TemplateInfo { id, name, contract_type, product_group: Option<String>, is_active }`.
- **Nguồn:** `template_service.py::check_template_for, preselected_attachment, _template_kind`
- **File:** `src/template_rules.rs`, `tests/fixture_template_rules.rs`
- **Test:** `template_rules.json → check_template_for, preselected_attachment, template_kind`.
- **Xong khi:** `cargo test -p cf-core --test fixture_template_rules`.
- **Phụ thuộc:** T1.32

### T1.34 — `template_rules::templates_for`, `default_template_for` · XS
- [ ] Xong
- **Làm:** nhận `&[TemplateInfo]`, trả `Vec<&TemplateInfo>` lọc + sắp (mẫu riêng trước, rồi tên `to_lowercase`).
- **Nguồn:** `template_service.py::templates_for, default_template_for`
- **File:** `src/template_rules.rs`
- **Test (unit):** 4 mẫu (2 nhóm G1, 1 dùng chung, 1 nhóm G2, 1 tắt) → thứ tự và lọc đúng; default chỉ khi đúng 1 mẫu riêng.
- **Xong khi:** `cargo test -p cf-core template_rules`.
- **Phụ thuộc:** T1.33

### T1.35 — `render_context`: struct đầu vào · S
- [ ] Xong
- **Làm:** struct thuần (không phụ thuộc cf-db): `CustomerView` (mọi trường dùng trong ngữ cảnh + `units: Vec<UnitView>`),
  `ContractView`, `ParentView {contract_number, sign_date}`, `ProductView {name, code}`, `PartyB` (13 trường),
  `TemplateMapping(BTreeMap<String,String>)`. `ContractStatus` dùng của `lifecycle`.
  Thêm `customer::full_address`, `customer::total_units`, `customer::unit_names_list` trên `CustomerView`.
- **Nguồn:** `ContractForge/app/models/customer.py` (helpers), `docx_renderer.py::build_render_context` (trường được dùng)
- **File:** `src/render_context.rs`, `src/customer.rs`
- **Test (unit):** khách `fake_customer` trong `tools/gen_fixtures.py` → `full_address`=`"Ấp 1, xã Sông Xanh, Tỉnh Mẫu"`,
  `total_units`=2, `unit_names_list`=`["TYT Sông Xanh","Điểm trạm Ấp 2"]`.
- **Xong khi:** `cargo test -p cf-core customer`.
- **Phụ thuộc:** T1.21

### T1.36 — `render_context::build_render_context` · S
- [ ] Xong
- **Làm:** `build_render_context(c: &ContractView, cust: &CustomerView, parent: Option<&ParentView>,
  product: Option<&ProductView>, party_b: &PartyB, addendum_format: Option<&str>) -> BTreeMap<String,String>`.
  Mọi khóa y fixture.
- **Nguồn:** `docx_renderer.py::build_render_context, _addendum_seq_text`
- **File:** `src/render_context.rs`, `tests/fixture_render_context.rs`
- **Test:** `render_context.json → cases[*].context` (khớp từng khóa, đủ số khóa).
- **Xong khi:** `cargo test -p cf-core --test fixture_render_context context`.
- **Phụ thuộc:** T1.35, T1.04–T1.07, T1.12, T1.16, T1.26

### T1.37 — `render_context::build_08a_context`, `work_content` · XS
- [ ] Xong
- **Nguồn:** `ContractForge/app/services/form08a_service.py::build_08a_context, _work_content`
- **File:** `src/render_context.rs`, `tests/fixture_render_context.rs`
- **Test:** `render_context.json → cases[*].context_08a, work_content`.
- **Xong khi:** `cargo test -p cf-core --test fixture_render_context 08a work_content`.
- **Phụ thuộc:** T1.36

### T1.38 — `render_context`: `CANONICAL_MAP` và `apply_template_mapping` · XS
- [ ] Xong
- **Làm:** hằng `CANONICAL_MAP: &[(&str,&str)]`, `ROW_PLACEHOLDERS`, `ALL_CANONICAL_FIELDS` (sắp xếp, không trùng) —
  **chép từ fixture** `render_context.json → canonical_map, row_placeholders`. `apply_template_mapping(ctx, mapping)`.
- **Nguồn:** `ContractForge/app/services/template_analyzer.py::CANONICAL_MAP`, `docx_renderer.py::apply_template_mapping`
- **File:** `src/render_context.rs`, `tests/fixture_render_context.rs`
- **Test:** `render_context.json → canonical_map, row_placeholders, template_mapping`.
- **Xong khi:** `cargo test -p cf-core --test fixture_render_context mapping canonical`.
- **Phụ thuộc:** T1.36

### T1.39 — `render_context::lists_from_lines` · XS
- [ ] Xong
- **Làm:** vào `&[Line]` + `&[Vec<Feature>]` (`Feature {name, description: Option<String>}`), ra
  `BTreeMap<String, Vec<BTreeMap<String,String>>>` với khóa `items`, `features`.
- **Nguồn:** `docx_renderer.py::lists_from_lines`
- **File:** `src/render_context.rs`, `tests/fixture_render_context.rs`
- **Test:** `render_context.json → lists_from_lines`.
- **Xong khi:** `cargo test -p cf-core --test fixture_render_context lists`.
- **Phụ thuộc:** T1.28, T1.36

### T1.40 — `pricing::auto_pricing_description` (legacy) · XS
- [ ] Xong
- **Làm:** `"Số tiền = Đơn giá có VAT ({X} VNĐ) × {N} cơ sở{ (tên; tên) nếu >1} × {M} tháng = {T} VNĐ"`;
  thiếu giá có VAT hoặc tháng → `""`. Tiền **cắt** (`int`) rồi chấm ngăn nghìn.
- **Nguồn:** `ContractForge/app/services/batch_generator.py::_auto_pricing_description`
- **File:** `src/pricing.rs`
- **Test (unit):** giá 485000, 2 cơ sở `["A","B"]`, 12 tháng →
  `"Số tiền = Đơn giá có VAT (485.000 VNĐ) × 2 cơ sở (A; B) × 12 tháng = 11.640.000 VNĐ"`; 1 cơ sở → không có ngoặc.
- **Xong khi:** `cargo test -p cf-core pricing::auto`.
- **Phụ thuộc:** T1.04

### T1.41 — `pricing::build_lines` (dòng từ form) · S
- [ ] Xong
- **Làm:** `pub struct ProductRule { id, name, kind, group_code, is_active, unit_label, default_price,
  default_vat_rate, price_period_months, min_quantity, quantity_step }`;
  `build_lines(items: &[ItemInput], products: impl Fn(i64)->Option<ProductRule>, month_count, require_main) -> Result<Vec<Line>, CoreError>`;
  `default_quantity(product, record_count: Option<i64>, unit_count) -> String`.
  Mọi luật và câu lỗi `"Dòng {n}: ..."` của Python.
- **Nguồn:** `contract_service.py::_build_lines, _item_decimal, _default_quantity, _parse_items_json`
- **File:** `src/pricing.rs`
- **Test (unit):** dòng 1 không phải main → `"Dòng 1 phải là gói chính."`; dòng 2 main →
  `"Dòng 2: chỉ dòng đầu tiên được là gói chính."`; khác nhóm; gói ngừng bán; thiếu giá lấy mặc định; SL áp min/step.
- **Xong khi:** `cargo test -p cf-core pricing::build_lines`.
- **Phụ thuộc:** T1.28

---

## Điểm kiểm Đợt 1
- [ ] `cargo test -p cf-core` xanh, mọi fixture của cf-core được dùng ít nhất một lần
- [ ] `cargo clippy -p cf-core -- -D warnings` sạch
- [ ] `cargo tree -p cf-core` không có rusqlite/zip/slint
