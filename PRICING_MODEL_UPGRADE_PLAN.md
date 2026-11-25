# Kế hoạch nâng cấp: danh mục gói cước, hợp đồng nhiều dòng, biểu mẫu mới

Tài liệu này mô tả thay đổi kỹ thuật để ContractForge hỗ trợ một chính sách giá
theo gói (gói chính + gói bổ sung), nhiều đơn vị tính, và biểu mẫu hợp đồng có
bảng giá trị nhiều dòng.

Nguyên tắc giữ nguyên từ CONTRIBUTING.md: **giá, mã gói, tên gói và nội dung
biểu mẫu của một tổ chức là dữ liệu, không nằm trong mã nguồn.** Mọi ví dụ trong
repo dùng sản phẩm hư cấu. Danh mục thật và mẫu Word thật nằm trong `data/`.

---

## 1. Hiện trạng và khoảng cách

| Hạng mục | Hiện tại | Cần có |
|---|---|---|
| Danh mục sản phẩm | 1 dòng = 1 sản phẩm, 1 giá, 1 VAT. Trang `/products` chỉ xem, không thêm/sửa được (`app/routers/products.py` có đúng 1 route GET) | Vài chục gói, chia nhóm, có gói chính và gói bổ sung, thêm/sửa/ngừng bán trên giao diện, nhập từ Excel |
| Cấu trúc giá hợp đồng | 1 hợp đồng = 1 sản phẩm × số cơ sở × số tháng, 1 mức VAT (các cột `unit_price_*`, `vat_rate`, `quantity` trên `contracts`) | 1 hợp đồng = nhiều dòng, mỗi dòng có đơn vị tính, số lượng, đơn giá, thuế suất riêng |
| Đơn vị tính | Ngầm định "cơ sở/tháng" | Theo cơ sở/tháng, theo số hồ sơ/tháng (có mức sàn và bước làm tròn), theo dung lượng/năm |
| Công thức tiền | Chép ở ít nhất 5 nơi: `contract_service.create_draft`, `batch_generator._build_contract_from_params`, `batch_generator._auto_pricing_description`, `quotation._build_context`, JS `recalcPrice()` trong `templates/contracts/form.html` | Một module duy nhất |
| Cách làm tròn | Làm tròn đơn giá có VAT rồi nhân số lượng | Biểu mẫu mới tính theo dòng: thành tiền trước thuế → tiền thuế → thành tiền sau thuế |
| Sinh file Word | Thay `{{PLACEHOLDER}}` + chèn dòng tùy chọn bảng Bên A (`docx_renderer.py`) | Thêm lặp dòng bảng (bảng giá trị, bảng chức năng) |
| Loại văn bản | Hợp đồng, báo giá, biên bản nghiệm thu 08a | Thêm phụ lục bổ sung gói/tài khoản, phụ lục gia hạn, biên bản bàn giao nghiệm thu |
| Điều khoản thanh toán | Không lưu | Trả trước / trả sau, chu kỳ thanh toán |

Rủi ro lớn nhất: **hợp đồng cũ không được đổi một đồng nào.** Hai cách làm tròn
cho kết quả lệch nhau vài đồng trên cùng dữ liệu, nên tuyệt đối không tính lại
hợp đồng cũ bằng công thức mới.

---

## 2. Quyết định thiết kế

1. **Một nguồn sự thật cho tiền:** `app/services/pricing.py`. Mọi nơi khác gọi
   vào đây, kể cả giao diện (qua một endpoint xem trước), giống cách
   `lifecycle.py` và `numbering.py` đang làm với vòng đời và số hợp đồng.
2. **Dòng hợp đồng là ảnh chụp.** Mỗi dòng lưu tên gói, đơn vị tính, đơn giá,
   thuế suất tại thời điểm lập. Sửa danh mục về sau không làm đổi hợp đồng đã có.
3. **Hai chế độ tính, khóa theo hợp đồng.** Cột `contracts.pricing_model`:
   `legacy` (giữ nguyên số đã lưu, không tính lại) và `items` (tính theo dòng).
4. **Ngừng bán thay vì xóa.** Gói cũ chuyển `is_active = False`; hợp đồng cũ vẫn
   trỏ tới nó và vẫn sinh lại file được.
5. **Phụ lục là một bản ghi hợp đồng có cha.** Dùng lại toàn bộ vòng đời, chứng
   từ, đánh số; không tạo bảng song song.
6. **Biến thể điều khoản = mẫu Word riêng.** Trả trước và trả sau là hai mẫu,
   không thêm cú pháp điều kiện vào renderer ở đợt này.

---

## 3. Thay đổi dữ liệu

Tất cả đi qua `app/models/seed.py::run_migrations` theo kiểu hiện có
(`PRAGMA table_info` → `ALTER TABLE ADD COLUMN`, `CREATE TABLE IF NOT EXISTS`),
chạy lại nhiều lần vẫn an toàn.

### 3.1 `products` — thêm cột

| Cột | Kiểu | Ý nghĩa |
|---|---|---|
| `group_code` | String(50) | Nhóm sản phẩm, dùng để gom trên giao diện và ghép gói bổ sung với gói chính |
| `kind` | String(20) | `main` \| `addon` \| `storage` |
| `unit_label` | String(50) | Đơn vị tính in ra hợp đồng |
| `price_period_months` | Integer, mặc định 1 | Đơn giá áp cho bao nhiêu tháng (1 = giá tháng, 12 = giá năm) |
| `quantity_source` | String(20) | `units` (số cơ sở được chọn) \| `records` (số hồ sơ của khách) \| `manual` |
| `min_quantity` | Integer, null | Số lượng tối thiểu |
| `quantity_step` | Integer, null | Làm tròn lên theo bội số này |
| `account_limit` | Integer, null | Số tài khoản kèm theo gói (thông tin, in ra phụ lục) |
| `feature_list` | Text (JSON) | Danh sách chức năng in vào phụ lục phạm vi: `[{"no","name","description"}]` |
| `replaced_by_id` | FK products, null | Gói thay thế, dùng khi gợi ý gia hạn |
| `sort_order` | Integer | Thứ tự hiển thị |

`default_price`, `default_vat_rate`, `default_duration_months`,
`default_template_id`, `contract_number_format`, `is_active` giữ nguyên.
Các dòng sản phẩm đang có nhận `kind='main'`, `price_period_months=1`,
`quantity_source='units'`.

### 3.2 Bảng mới `contract_items`

```
id, contract_id (FK, cascade), product_id (FK, null), sort_order,
description, unit_label,
quantity          NUMERIC(18,2)   -- số cơ sở / số hồ sơ / số GB
period_count      NUMERIC(10,2)   -- số kỳ tính giá (tháng ÷ price_period_months)
unit_price_pre_vat NUMERIC(18,2)
vat_rate          NUMERIC(5,2)
amount_pre_vat    NUMERIC(18,0)
vat_amount        NUMERIC(18,0)
amount_total      NUMERIC(18,0)
note
```

Model: `app/models/contract_item.py`, quan hệ `Contract.items`
(`order_by=sort_order`, `cascade="all, delete-orphan"`).

### 3.3 `contracts` — thêm cột

| Cột | Ý nghĩa |
|---|---|
| `pricing_model` | `legacy` \| `items` |
| `total_pre_vat`, `total_vat` | Tổng trước thuế, tổng thuế (tổng sau thuế vẫn là `total_amount`) |
| `payment_mode` | `prepaid` \| `postpaid` |
| `payment_cycle_months` | Chu kỳ thanh toán |
| `parent_contract_id` | Hợp đồng gốc của phụ lục |
| `doc_kind` | `contract` \| `addendum_addon` \| `addendum_renewal` |

Các cột `unit_price_pre_vat`, `vat_rate`, `unit_price_vat`, `unit_count`,
`quantity` **giữ lại** và phản chiếu dòng gói chính, để báo cáo Excel, dashboard
và biên bản 08a tiếp tục chạy mà không phải sửa cùng lúc.

### 3.4 `customers` — thêm cột

`record_count` (Integer, null): số hồ sơ / dân số phục vụ, làm số lượng mặc định
cho gói tính theo hồ sơ.

### 3.5 Chuyển dữ liệu cũ

- Mỗi hợp đồng hiện có nhận `pricing_model='legacy'`, `doc_kind='contract'` và
  đúng **một** dòng `contract_items` chép nguyên số đã lưu
  (`amount_total = total_amount`, không tính lại).
- Hợp đồng có `unit_price_pre_vat` rỗng: suy từ `unit_price_vat` và `vat_rate`,
  ghi chú lại, không đổi `total_amount`.
- Trước khi migrate, tự sao lưu DB vào `data/backups/` bằng sqlite backup API.

---

## 4. Module tính giá: `app/services/pricing.py`

```python
@dataclass(frozen=True)
class LineInput:  product, quantity, months, unit_price_override, vat_override
@dataclass(frozen=True)
class Line:       description, unit_label, quantity, period_count,
                  unit_price_pre_vat, vat_rate,
                  amount_pre_vat, vat_amount, amount_total
@dataclass(frozen=True)
class Totals:     pre_vat, vat, total, amount_text

def normalize_quantity(product, raw) -> Decimal   # sàn + làm tròn lên theo bước
def default_quantity(product, customer, selected_units) -> Decimal
def price_line(inp: LineInput) -> Line
def price_contract(lines: list[LineInput]) -> tuple[list[Line], Totals]
def addon_months(main_end, addon_start) -> int    # thời gian còn lại, làm tròn lên theo tháng
def describe(lines, totals) -> str                # thay _auto_pricing_description
```

Quy tắc, theo đúng cột của bảng giá trị trong biểu mẫu:

```
period_count   = months / price_period_months
amount_pre_vat = round_half_up(unit_price_pre_vat × quantity × period_count)
vat_amount     = round_half_up(amount_pre_vat × vat_rate / 100)
amount_total   = amount_pre_vat + vat_amount
```

Tổng hợp đồng = tổng các dòng. Không bao giờ nhân từ đơn giá có VAT đã làm tròn.

Hợp đồng `legacy` không đi qua `price_contract`; `pricing.py` có
`legacy_lines(contract)` chỉ đọc lại số đã lưu để hiển thị.

Tính lẻ tháng theo ngày: **chưa làm ở đợt này.** Cho phép ghi đè
`amount_pre_vat` của một dòng kèm ghi chú bắt buộc.

---

## 5. Sinh file Word

### 5.1 Lặp dòng bảng — `app/services/docx_renderer.py`

Thêm `_expand_repeat_rows(doc, ctx_lists)` chạy trước bước thay placeholder,
dùng lại `_copy_row_format` và `_set_cell_text` đang có:

- Một dòng bảng chứa placeholder bắt đầu bằng `ITEM_` là dòng mẫu của bảng giá
  trị: nhân bản theo số dòng hợp đồng.
- Một dòng chứa placeholder bắt đầu bằng `FEATURE_` là dòng mẫu của bảng chức
  năng: nhân bản theo `feature_list` gộp từ các gói trong hợp đồng.
- Danh sách rỗng thì xóa dòng mẫu.

Placeholder mới:

| Nhóm | Placeholder |
|---|---|
| Dòng giá trị | `ITEM_NO`, `ITEM_NAME`, `ITEM_UNIT`, `ITEM_QTY`, `ITEM_PRICE`, `ITEM_AMOUNT_PRE_VAT`, `ITEM_VAT_RATE`, `ITEM_VAT_AMOUNT`, `ITEM_AMOUNT_TOTAL` |
| Tổng | `TOTAL_PRE_VAT`, `TOTAL_VAT` (đã có `TOTAL_AMOUNT`, `AMOUNT_TEXT`) |
| Dòng chức năng | `FEATURE_NO`, `FEATURE_NAME`, `FEATURE_DESC` |
| Thanh toán | `PAYMENT_CYCLE_MONTHS` |
| Bên A | `PARTY_A_PHONE`, `PARTY_A_ESTABLISHMENT_DECISION`, `PARTY_A_ESTABLISHMENT_DATE`, `PARTY_A_CONTACT_NAME`, `PARTY_A_EMAIL` (dữ liệu đã có sẵn trên `customers`) |
| Bên B | `PARTY_B_PHONE`, `PARTY_B_CONTACT_NAME`, `PARTY_B_CONTACT_EMAIL` (thêm khóa vào `settings_service`) |
| Phụ lục | `PARENT_CONTRACT_NUMBER`, `PARENT_SIGN_DATE`, `ADDENDUM_NO` |

`build_render_context` trả thêm hai danh sách (`items`, `features`) bên cạnh dict
chuỗi; `contract.render_data` lưu cả hai để tái tạo được file.

### 5.2 `app/services/template_analyzer.py`

- Bổ sung các placeholder trên vào `CANONICAL_MAP`.
- Nhận diện dòng lặp, báo trong kết quả phân tích ("mẫu có bảng giá trị",
  "mẫu có bảng chức năng"), cảnh báo khi placeholder `ITEM_*` nằm ngoài bảng.
- `template_service._template_kind` phân biệt thêm: phụ lục bổ sung, phụ lục gia
  hạn, biên bản bàn giao.

### 5.3 Mẫu Word

- Mẫu thật: chuyển từ biểu mẫu dạng dấu chấm sang `{{PLACEHOLDER}}`, tách thành
  các file riêng (hợp đồng trả trước, hợp đồng trả sau, phụ lục bổ sung, phụ lục
  gia hạn, biên bản bàn giao), tải lên qua `/templates`. **Không đưa vào repo.**
- Mẫu hư cấu cho repo: thêm vào `scripts/make_sample_templates.py` một mẫu
  `sample_service_contract_items.docx` có bảng giá trị và bảng chức năng, để
  tính năng chạy được trên bản clone mới và có thứ cho test dùng.

---

## 6. Giao diện và route

### 6.1 Danh mục — `app/routers/products.py`, `templates/products/`

- `GET/POST /products/new`, `GET/POST /products/{id}/edit`,
  `POST /products/{id}/toggle-active`.
- Trang danh sách gom theo `group_code`, tách gói chính / gói bổ sung, lọc
  "đang bán / đã ngừng", hiện số hợp đồng còn hiệu lực của từng gói.
- `GET /products/template.xlsx`, `POST /products/import`: nhập cả danh mục từ
  Excel (xem trước rồi mới ghi; mã trùng thì cập nhật). File danh mục thật để
  trong `data/`.
- Trình sửa `feature_list` dạng bảng, có nhập từ Excel.

### 6.2 Lập hợp đồng — `app/routers/contracts.py`, `templates/contracts/form.html`

- Thay khối giá đơn bằng bảng dòng: chọn gói chính → hệ thống gợi ý các gói bổ
  sung cùng `group_code` → thêm/bớt dòng.
- Số lượng mặc định lấy từ `pricing.default_quantity` (số cơ sở đã tích chọn,
  hoặc `customers.record_count`).
- `POST /contracts/price-preview` trả về các dòng và tổng đã tính. JS chỉ hiển
  thị; xóa công thức trong `recalcPrice()`.
- Thêm trường hình thức và chu kỳ thanh toán; mẫu Word gợi ý theo lựa chọn này.
- `contract_service.create_draft` nhận `items`, gọi `pricing.price_contract`,
  ghi `contract_items`, đặt `pricing_model='items'`.

### 6.3 Chi tiết hợp đồng — `templates/contracts/detail.html`

Bảng dòng, ba tổng, danh sách phụ lục con, nút "Tạo phụ lục bổ sung" và
"Gia hạn".

### 6.4 Phụ lục

- `GET/POST /contracts/{id}/addendum/new?kind=addon|renewal`.
- Bổ sung gói: chỉ chọn được gói `addon`; số tháng = `pricing.addon_months`, khóa
  không cho vượt ngày hết hạn của hợp đồng gốc.
- Gia hạn: điền sẵn từ hợp đồng gốc; gói đã ngừng bán thì đổi sang
  `replaced_by_id` và báo rõ chênh lệch giá cho người dùng trước khi lưu.
- Số phụ lục: thêm token `{parent}` và `{addendum_seq}` vào
  `app/services/numbering.py`; khuôn mặc định cấu hình trong Cài đặt.

### 6.5 Sinh hàng loạt, báo giá, báo cáo

- `batch_generator`: tham số gói chính + danh sách gói bổ sung áp chung; cột mã
  gói trong file Excel; bỏ công thức riêng, gọi `pricing`.
- `quotation`: chọn nhiều gói, dùng chung renderer lặp dòng và `pricing`.
- `contracts.export_contracts`: thêm cột tổng trước thuế, thuế, và sheet chi tiết
  dòng.
- `dashboard_service`: kiểm tra phụ lục được cộng đúng một lần vào doanh số; hợp
  đồng gốc đã có phụ lục gia hạn không bị đếm là "sắp hết hạn chưa xử lý".
- Thêm bộ lọc và màn hình "hợp đồng theo gói đã ngừng bán sắp hết hạn" để chuẩn
  bị gia hạn.

---

## 7. Các đợt triển khai

Mỗi đợt tự đứng được, có test, chạy được trên dữ liệu thật trước khi sang đợt sau.

| Đợt | Nội dung | Kết quả dùng được ngay |
|---|---|---|
| **0** | Chốt câu hỏi nghiệp vụ (mục 9). Lập file Excel danh mục. Chuyển mẫu Word sang placeholder. | Không có code |
| **1** | `pricing.py` + test. Cột mới của `products`. CRUD + nhập Excel danh mục. Chuyển 4 nơi tính tiền phía server sang gọi `pricing` ở chế độ một dòng. | Nhập được danh mục mới, ngừng bán gói cũ, lập hợp đồng một gói theo giá và thuế suất mới bằng form hiện tại |
| **2** | `contract_items`, cột mới của `contracts`, chuyển dữ liệu cũ. Form và trang chi tiết nhiều dòng. Endpoint xem trước giá. | Hợp đồng gói chính + gói bổ sung, tổng tính đúng |
| **3** | Renderer lặp dòng, analyzer, placeholder mới, mẫu hư cấu, tải mẫu thật. | Sinh hợp đồng theo biểu mẫu mới kèm phụ lục giá trị và phạm vi |
| **4** | Phụ lục bổ sung, phụ lục gia hạn, biên bản bàn giao, đánh số phụ lục. | Xử lý mua thêm giữa kỳ và gia hạn sang gói mới |
| **5** | Sinh hàng loạt, báo giá nhiều dòng, báo cáo Excel, dashboard, màn hình chuẩn bị gia hạn. | Làm việc theo lô cho toàn bộ khách |
| **6** | `HUONG_DAN.md`, `CHANGELOG.md`, tăng `app/version.py` + `pyproject.toml`, build lại exe, chạy `tests/smoke/windows_release_checklist.md`. | Bản phát hành |

Thứ tự phụ thuộc: 1 → 2 → 3 → 4; đợt 5 cần 2 và 3. Đợt 0 chạy song song với 1.

---

## 8. Kiểm thử và kiểm chứng

Test mới:

- `tests/unit/test_pricing.py`: làm tròn theo dòng, mức sàn và bước làm tròn số
  lượng, giá theo kỳ nhiều tháng, nhiều thuế suất trong một hợp đồng, số tháng
  của gói bổ sung, ghi đè thành tiền.
- `tests/integration/test_contract_items_migration.py`: DB kiểu cũ → sau migrate
  mỗi hợp đồng có đúng một dòng, `sum(amount_total) == total_amount`, chạy
  migrate lần hai không đổi gì.
- `tests/unit/test_docx_repeat_rows.py`: 0, 1, nhiều dòng; giữ định dạng dòng;
  placeholder bị tách run.
- `tests/integration/test_multi_item_contract.py`: lập → sinh file → đọc lại
  bảng trong docx, khớp tổng.
- `tests/integration/test_addendum.py`: giới hạn số tháng, đánh số, vòng đời.
- `tests/integration/test_product_catalog.py`: CRUD, nhập Excel, ngừng bán không
  làm hỏng việc sinh lại hợp đồng cũ.
- Mở rộng `test_generic_quotation.py`, `test_neutral_seed.py` (seed chỉ có gói
  hư cấu), `test_dashboard_numbers.py`.

Kiểm chứng trên dữ liệu thật, sau đợt 2 và trước mỗi lần phát hành:

1. Sao DB thật bằng sqlite backup API (không cần tắt app).
2. Chạy migrate trên bản sao.
3. So từng hợp đồng: `total_amount`, `amount_text`, số hợp đồng, trạng thái
   trước và sau phải giống hệt.
4. Sinh lại vài hợp đồng cũ từ bản sao, so nội dung với file đã phát hành.
5. Đối chiếu hash DB gốc trước và sau: không đổi.
6. `python scripts/check_public_repo.py` trước mỗi commit.

---

## 9. Câu hỏi nghiệp vụ phải chốt trước đợt 2

1. Khi bán kèm hai sản phẩm cho cùng một cơ sở: dùng mã gói bổ sung của sản phẩm
   thứ nhất, hay mã gói chính của sản phẩm thứ hai? (Quyết định một hợp đồng hay
   hai hợp đồng, và khuôn số hợp đồng.)
2. Gói cao nhất đã gồm bản cơ bản của sản phẩm kèm theo: có bán thêm gói kèm
   nữa không?
3. Gia hạn: lập phụ lục gia hạn trên hợp đồng cũ, hay ký hợp đồng mới theo biểu
   mẫu mới?
4. Khuôn số hợp đồng và số phụ lục theo biểu mẫu mới.
5. Phí bán hàng theo gói có cần theo dõi trong app không? (Kế hoạch này chưa gồm.)

---

## 10. Ngoài phạm vi

- Tính lẻ tháng theo số ngày thực tế (chỉ hỗ trợ ghi đè thủ công có ghi chú).
- Phân kỳ doanh thu.
- Theo dõi phí bán hàng.
- Cú pháp điều kiện trong mẫu Word.
