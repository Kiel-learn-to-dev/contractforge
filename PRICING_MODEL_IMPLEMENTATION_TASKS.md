# Kế hoạch thực hiện: danh mục gói cước, hợp đồng nhiều dòng, biểu mẫu mới

Tài liệu này là bản chia việc của [PRICING_MODEL_UPGRADE_PLAN.md](PRICING_MODEL_UPGRADE_PLAN.md).
Bản kia trả lời "làm gì và vì sao". Bản này trả lời "sửa file nào, viết hàm gì,
kiểm tra thế nào", theo thứ tự làm được từ trên xuống dưới.

Đối tượng đọc: người chưa quen codebase. Làm đúng thứ tự task, không gộp task,
không bỏ bước kiểm tra.

---

## 0. Đọc trước khi bắt đầu

### 0.1 Chạy dự án

```bash
python -m pytest -q
```

```bash
cd ContractForge && python -m uvicorn main:app --host 127.0.0.1 --port 8000
```

```bash
python scripts/check_public_repo.py
```

- Mã nguồn nằm trong `ContractForge/` (có `main.py`, `app/`, `templates/`).
  Test nằm trong `tests/` ở gốc repo. Mọi đường dẫn `app/...` bên dưới là
  `ContractForge/app/...`.
- Test luôn chạy trên DB tạm (`tests/conftest.py`). Không bao giờ trỏ test vào
  `data/contract_manager.db`.
- `pyproject.toml` đặt `filterwarnings = ["error"]`: **một warning cũng làm test
  đỏ**. Không dùng API đã deprecated, luôn đóng file và session.

### 0.2 Năm luật không được phá

1. **Không viết tên, mã, giá của sản phẩm thật vào mã nguồn, test, tài liệu.**
   Ví dụ trong code dùng `SERVICE-BASIC`, "Gói Dịch vụ Cơ bản", giá tròn. Dữ liệu
   thật chỉ nằm trong `data/` (đã bị `.gitignore`).
2. **Không tính lại tiền của hợp đồng đã có.** Hợp đồng cũ chỉ được đọc số đã lưu.
3. **Mọi phép tính tiền của hợp đồng theo dòng nằm trong `app/services/pricing.py`.**
   Không viết công thức ở router, template hay JavaScript.
4. **Luật vòng đời chỉ ở `app/services/lifecycle.py`, khuôn số chỉ ở
   `app/services/numbering.py`.** Cần tập trạng thái thì import, không tự liệt kê.
5. **Server chỉ bind `127.0.0.1`.**

### 0.3 Định nghĩa "xong" cho mọi task

- [ ] `python -m pytest -q` xanh toàn bộ, không chỉ test mới.
- [ ] `python scripts/check_public_repo.py` in `OK`.
- [ ] Mở app bằng DB có sẵn dữ liệu cũ: khởi động không lỗi, trang `/contracts`
      và một trang chi tiết hợp đồng cũ hiện bình thường.
- [ ] Một commit cho một task. Thông điệp tiếng Anh, một câu mệnh lệnh nói về
      hành vi, ví dụ `Let the catalog hold add-on packages`.
- [ ] Không sửa file ngoài danh sách "File" của task. Nếu buộc phải sửa, ghi lý
      do vào thông điệp commit.

### 0.4 Từ vựng

| Từ | Nghĩa trong tài liệu này |
|---|---|
| Gói chính (`main`) | Sản phẩm bán độc lập, mỗi hợp đồng có đúng một |
| Gói bổ sung (`addon`) | Mua kèm hoặc mua thêm sau, phụ thuộc gói chính cùng nhóm |
| Gói lưu trữ (`storage`) | Gói bổ sung tính theo dung lượng và theo kỳ nhiều tháng |
| Nhóm (`group_code`) | Chuỗi gom gói chính với các gói bổ sung của nó |
| Dòng hợp đồng | Một bản ghi `contract_items`: một gói × số lượng × số kỳ |
| Kỳ tính giá | Số tháng mà đơn giá áp dụng (`price_period_months`) |
| `legacy` / `items` | Hai chế độ tiền của hợp đồng: giữ số cũ / tính theo dòng |
| Phụ lục | Bản ghi `contracts` có `parent_contract_id` |

### 0.5 Bẫy đã biết trong codebase

- `contract_service._safe_decimal("82.5")` trả về `825`: hàm này đọc số kiểu Việt
  Nam, dấu chấm là phân cách nghìn. Chỉ dùng nó cho ô nhập do người gõ. Dữ liệu
  máy sinh (JSON, `data-*`) phải parse bằng `pricing.to_decimal`.
- `dict(await request.form())` chỉ giữ **một** giá trị cho mỗi tên trường. Dữ
  liệu dạng danh sách phải gửi trong một trường ẩn chứa JSON, giống trường
  `selected_units` đang có.
- `templates/contracts/form.html` hiện có hai ô cùng `name="month_count"` (dòng
  ~123 và ~178). Task 2.5 xóa ô thứ hai.
- Đối tượng ORM đọc trong `try/finally: db.close()` rồi đưa sang template phải
  được nạp hết quan hệ trước khi đóng session (xem `events = list(c.events)`
  trong `contracts.contract_detail`).
- `app/security.py::LocalOriginMiddleware` chặn POST từ nguồn lạ. Test gọi POST
  phải dùng fixture `client` kiểu `tests/integration/test_local_security.py`.

---

## 1. Sơ đồ phụ thuộc

```
T1.1 pricing.py ─────────────────────────┐
T1.2 cột products ── T1.3 form sản phẩm ── T1.4 ngừng bán ── T1.5 nhập Excel
        │                                 │
        └── T2.1 contract_items + migrate ─┤
                    │                      │
                    T2.2 create_draft theo dòng ── T2.3 API xem trước giá
                    │                                      │
                    T2.4 trang chi tiết            T2.5 form nhiều dòng
                    │
        T3.1 lặp dòng bảng ── T3.2 context + danh sách ── T3.4 mẫu hư cấu
                    │              T3.3 analyzer
                    │
        T4.1 số phụ lục ── T4.2 phụ lục bổ sung ── T4.3 gia hạn ── T4.4 báo cáo hết hạn
                                                   T4.5 văn bản kèm theo
                    │
        T5.1 batch ── T5.2 báo giá ── T5.3 xuất Excel ── T5.4 dọn công thức cũ
                    │
        T6.1 tài liệu + phiên bản + build
```

Làm song song được: T1.1 với T1.2–T1.5; T2.4 với T2.5; T3.3 với T3.4; T5.1,
T5.2, T5.3 với nhau. Không làm song song: mọi task sửa `app/models/seed.py`.

---

## Đợt 0 — Chuẩn bị (không có code)

### Task 0.1: Chốt câu hỏi nghiệp vụ

Lấy câu trả lời bằng văn bản cho 5 câu ở mục 9 của bản kế hoạch tổng. Câu 1 và 3
chặn Đợt 2 và Đợt 4. Ghi câu trả lời vào `data/` (không vào repo).

### Task 0.2: Lập file Excel danh mục

Tạo `data/catalog.xlsx` theo đúng cột của Task 1.5. Mỗi gói một dòng. File này là
đầu vào để nhập danh mục thật; không commit.

### Task 0.3: Chuyển biểu mẫu Word sang placeholder

Từ biểu mẫu gốc, tách thành các file riêng và thay chỗ trống bằng
`{{PLACEHOLDER}}` theo bảng ở Task 3.2. Trong bảng giá trị chỉ để **một** dòng
dữ liệu chứa `{{ITEM_NO}}`, `{{ITEM_NAME}}`…; trong bảng chức năng một dòng chứa
`{{FEATURE_NO}}`, `{{FEATURE_NAME}}`, `{{FEATURE_DESC}}`. Lưu vào `data/`, tải
lên qua `/templates` sau khi xong Đợt 3.

---

## Đợt 1 — Module tính giá và danh mục

Kết quả: nhập được danh mục mới, ngừng bán gói cũ, lập hợp đồng một gói bằng form
hiện tại với giá và thuế suất mới.

### Task 1.1: Module `pricing.py`

**Mô tả:** Viết các hàm thuần (không đụng DB) tính tiền theo dòng. Chưa nơi nào
gọi module này; task chỉ gồm module và test.

**File:**
- Tạo `app/services/pricing.py`
- Tạo `tests/unit/test_pricing.py`

**Các bước:**

1. Tạo file với nội dung khung sau. Giữ nguyên tên hàm và chữ ký.

```python
"""pricing.py — nguồn duy nhất cho phép tính tiền của hợp đồng theo dòng.

Quy tắc, theo đúng các cột của bảng giá trị trong hợp đồng:

    số kỳ            = số tháng / số tháng của một kỳ giá
    thành tiền trước thuế = làm tròn(đơn giá × số lượng × số kỳ)
    tiền thuế        = làm tròn(thành tiền trước thuế × thuế suất / 100)
    thành tiền sau thuế   = thành tiền trước thuế + tiền thuế

Tổng hợp đồng là tổng các dòng. Không nhân từ đơn giá có thuế đã làm tròn: cách
đó lệch vài đồng so với bảng in ra, và là cách hợp đồng `legacy` đã dùng. Hợp
đồng `legacy` vì thế KHÔNG đi qua module này.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date
from decimal import Decimal, InvalidOperation, ROUND_CEILING, ROUND_HALF_UP
from typing import Optional, Sequence

from app.utils.amount_to_words_vi import amount_to_words
from app.utils.date_helpers import compute_end_date

_ONE = Decimal("1")
_HUNDRED = Decimal("100")


class PricingError(ValueError):
    """Dữ liệu đầu vào không tính được tiền."""


def to_decimal(value, field: str = "giá trị") -> Decimal:
    """Đọc số ở dạng máy ("82.5", 82.5, Decimal). KHÔNG đọc số kiểu "1.000.000"."""
    if isinstance(value, Decimal):
        return value
    if value is None or str(value).strip() == "":
        raise PricingError(f"Thiếu {field}.")
    try:
        return Decimal(str(value).strip())
    except InvalidOperation:
        raise PricingError(f"{field} không phải là số: {value!r}")


def round_money(value) -> Decimal:
    """Làm tròn tới đồng, 0,5 làm tròn lên."""
    return to_decimal(value).quantize(_ONE, rounding=ROUND_HALF_UP)


def normalize_quantity(raw, min_quantity=None, step=None) -> Decimal:
    """Áp mức sàn rồi làm tròn LÊN theo bội số của `step`."""
    qty = to_decimal(raw, "số lượng")
    if qty <= 0:
        raise PricingError("Số lượng phải lớn hơn 0.")
    if min_quantity:
        qty = max(qty, Decimal(min_quantity))
    if step:
        step_d = Decimal(step)
        qty = (qty / step_d).to_integral_value(rounding=ROUND_CEILING) * step_d
    return qty


def period_count(months, price_period_months=1) -> Decimal:
    months_d = to_decimal(months, "số tháng")
    period_d = to_decimal(price_period_months or 1, "kỳ giá")
    if months_d <= 0 or period_d <= 0:
        raise PricingError("Số tháng và kỳ giá phải lớn hơn 0.")
    return months_d / period_d


@dataclass(frozen=True)
class Line:
    description: str
    unit_label: str
    quantity: Decimal
    period_count: Decimal
    unit_price_pre_vat: Decimal
    vat_rate: Decimal
    amount_pre_vat: Decimal
    vat_amount: Decimal
    amount_total: Decimal
    product_id: Optional[int] = None
    note: str = ""


@dataclass(frozen=True)
class Totals:
    pre_vat: Decimal
    vat: Decimal
    total: Decimal
    amount_text: str


def price_line(*, description: str, unit_label: str = "", quantity, months,
               unit_price_pre_vat, vat_rate=0, price_period_months=1,
               amount_pre_vat_override=None, product_id: Optional[int] = None,
               note: str = "") -> Line:
    qty = to_decimal(quantity, "số lượng")
    price = to_decimal(unit_price_pre_vat, "đơn giá")
    vat = to_decimal(vat_rate if vat_rate not in (None, "") else 0, "thuế suất")
    if qty <= 0:
        raise PricingError("Số lượng phải lớn hơn 0.")
    if price < 0:
        raise PricingError("Đơn giá không được âm.")
    if not (0 <= vat <= 100):
        raise PricingError("Thuế suất phải trong khoảng 0–100.")
    periods = period_count(months, price_period_months)

    if amount_pre_vat_override not in (None, ""):
        if not note.strip():
            raise PricingError("Ghi đè thành tiền phải kèm ghi chú lý do.")
        pre = round_money(amount_pre_vat_override)
    else:
        pre = round_money(price * qty * periods)
    vat_amount = round_money(pre * vat / _HUNDRED)
    return Line(description=description, unit_label=unit_label, quantity=qty,
                period_count=periods, unit_price_pre_vat=price, vat_rate=vat,
                amount_pre_vat=pre, vat_amount=vat_amount,
                amount_total=pre + vat_amount, product_id=product_id, note=note)


def sum_lines(lines: Sequence[Line]) -> Totals:
    if not lines:
        raise PricingError("Hợp đồng phải có ít nhất một dòng.")
    pre = sum((l.amount_pre_vat for l in lines), Decimal("0"))
    vat = sum((l.vat_amount for l in lines), Decimal("0"))
    total = pre + vat
    return Totals(pre_vat=pre, vat=vat, total=total,
                  amount_text=amount_to_words(total) if total else "")


def addon_months(start: date, main_end: date) -> int:
    """Số tháng còn lại của gói chính tính từ `start`, làm tròn LÊN theo tháng."""
    if start > main_end:
        raise PricingError("Ngày bắt đầu gói bổ sung sau ngày hết hạn gói chính.")
    months = 1
    while compute_end_date(start, months) <= main_end:
        months += 1
        if months > 600:
            raise PricingError("Khoảng thời gian không hợp lệ.")
    return months


def fmt_money(value) -> str:
    """1234567 → '1.234.567'."""
    return f"{int(round_money(value)):,}".replace(",", ".")


def describe(lines: Sequence[Line], totals: Totals) -> str:
    """Mô tả cách tính, điền vào {{PRICING_DESCRIPTION}}."""
    parts = [f"{l.description}: {fmt_money(l.amount_total)} VNĐ" for l in lines]
    return "; ".join(parts) + f". Tổng cộng: {fmt_money(totals.total)} VNĐ"
```

2. Viết test. Mỗi dòng bảng dưới là một test riêng, tên hàm mô tả hành vi.

| Đầu vào | Kết quả phải có |
|---|---|
| `price_line(quantity=3, months=12, unit_price_pre_vat=500000, vat_rate=10)` | trước thuế 18.000.000; thuế 1.800.000; sau thuế 19.800.000 |
| `quantity=2000, months=12, unit_price_pre_vat=75, vat_rate=10` | 1.800.000; 180.000; 1.980.000 |
| `quantity=5, months=12, unit_price_pre_vat=16320, vat_rate=10, price_period_months=12` | `period_count == 1`; 81.600; 8.160; 89.760 |
| `quantity=3, months=12, unit_price_pre_vat=440909, vat_rate=8` | 15.872.724; 1.269.818; 17.142.542 |
| Cùng dữ liệu trên tính kiểu cũ: `round(440909*1.08) * 36` | 17.142.552 — test khẳng định hai kết quả **khác nhau** và ghi chú đây là lý do không tính lại hợp đồng cũ |
| `normalize_quantity(1200, 2000, 500)` | 2000 |
| `normalize_quantity(2300, 2000, 500)` | 2500 |
| `normalize_quantity(2500, 2000, 500)` | 2500 |
| `normalize_quantity(0)` | `PricingError` |
| `price_line(..., amount_pre_vat_override=100000, note="")` | `PricingError` |
| `price_line(..., amount_pre_vat_override=100000, note="lẻ tháng")`, thuế 10 | 100.000; 10.000; 110.000 |
| `sum_lines` hai dòng thuế suất 10 và 0 | ba tổng bằng tổng từng cột; `amount_text` không rỗng |
| `sum_lines([])` | `PricingError` |
| `addon_months(date(2026,10,1), date(2027,3,30))` | 6 |
| `addon_months(date(2026,10,15), date(2026,10,20))` | 1 |
| `addon_months(date(2027,4,1), date(2027,3,30))` | `PricingError` |
| `to_decimal("82.5")` | `Decimal("82.5")` (không phải 825) |

**Tiêu chí chấp nhận:**
- [ ] Mọi dòng trong bảng có test và test xanh.
- [ ] `pricing.py` không import gì từ `app.models`, `app.database`, `fastapi`.
- [ ] Không file nào khác thay đổi.

**Kiểm tra:** `python -m pytest tests/unit/test_pricing.py -q`

**Phụ thuộc:** Không. **Cỡ:** S.

---

### Task 1.2: Cột mới của `products`

**Mô tả:** Mở rộng danh mục để mô tả gói chính, gói bổ sung, đơn vị tính, kỳ giá
và quy tắc số lượng. Chưa có giao diện.

**File:**
- Sửa `app/models/product.py`
- Sửa `app/models/seed.py` (hàm `run_migrations`)
- Tạo `tests/integration/test_product_schema.py`

**Các bước:**

1. Trong `app/models/product.py`, thêm ngay trên `class Product`:

```python
PRODUCT_KINDS = ("main", "addon", "storage")
QUANTITY_SOURCES = ("units", "records", "manual")
```

2. Thêm cột vào `Product`, đặt sau `contract_number_format`:

```python
    group_code = Column(String(50), nullable=True, index=True,
                        comment="Nhóm: gom gói chính với gói bổ sung của nó")
    kind = Column(String(20), nullable=False, default="main",
                  comment="main | addon | storage")
    unit_label = Column(String(50), nullable=True,
                        comment="Đơn vị tính in ra hợp đồng")
    price_period_months = Column(Integer, nullable=False, default=1,
                                 comment="Đơn giá áp cho bao nhiêu tháng")
    quantity_source = Column(String(20), nullable=False, default="units",
                             comment="units | records | manual")
    min_quantity = Column(Integer, nullable=True)
    quantity_step = Column(Integer, nullable=True)
    account_limit = Column(Integer, nullable=True)
    _feature_list = Column("feature_list", Text, nullable=True,
                           comment="JSON: [{no, name, description}]")
    replaced_by_id = Column(Integer, ForeignKey("products.id"), nullable=True)
    sort_order = Column(Integer, nullable=False, default=0)
```

   Thêm `Text` vào dòng import của sqlalchemy. Thêm property `feature_list`
   (getter trả `list`, setter nhận `list`) theo đúng kiểu `selected_unit_names`
   trong `app/models/contract.py`.

3. Trong `seed.run_migrations`, ngay sau khối `contract_number_format` (trong
   cùng `with db.bind.connect() as conn`), thêm theo kiểu vòng lặp đang có:

```python
        cols_p = [row[1] for row in conn.execute(text("PRAGMA table_info(products)")).fetchall()]
        _product_migrations = [
            ("group_code",          "ALTER TABLE products ADD COLUMN group_code VARCHAR(50)"),
            ("kind",                "ALTER TABLE products ADD COLUMN kind VARCHAR(20) NOT NULL DEFAULT 'main'"),
            ("unit_label",          "ALTER TABLE products ADD COLUMN unit_label VARCHAR(50)"),
            ("price_period_months", "ALTER TABLE products ADD COLUMN price_period_months INTEGER NOT NULL DEFAULT 1"),
            ("quantity_source",     "ALTER TABLE products ADD COLUMN quantity_source VARCHAR(20) NOT NULL DEFAULT 'units'"),
            ("min_quantity",        "ALTER TABLE products ADD COLUMN min_quantity INTEGER"),
            ("quantity_step",       "ALTER TABLE products ADD COLUMN quantity_step INTEGER"),
            ("account_limit",       "ALTER TABLE products ADD COLUMN account_limit INTEGER"),
            ("feature_list",        "ALTER TABLE products ADD COLUMN feature_list TEXT"),
            ("replaced_by_id",      "ALTER TABLE products ADD COLUMN replaced_by_id INTEGER REFERENCES products(id)"),
            ("sort_order",          "ALTER TABLE products ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0"),
        ]
        for col_name, sql in _product_migrations:
            if col_name not in cols_p:
                conn.execute(text(sql))
                conn.commit()
```

4. Test (dùng `db_engine`, `db_session`):
   - Tạo bảng `products` kiểu cũ bằng SQL thô (chỉ các cột trước task này), chèn
     một dòng, gọi `run_migrations(db_session)`, rồi đọc lại bằng ORM: dòng cũ có
     `kind == "main"`, `price_period_months == 1`, `quantity_source == "units"`.
     Để làm được: `DROP TABLE products` trên engine tạm rồi `CREATE TABLE` kiểu cũ.
   - Gọi `run_migrations` lần hai: không lỗi, dữ liệu không đổi.
   - `feature_list` gán list rồi đọc lại ra đúng list; mặc định là `[]`.

**Tiêu chí chấp nhận:**
- [ ] DB cũ khởi động được, sản phẩm cũ nhận giá trị mặc định đúng.
- [ ] Migration chạy hai lần không lỗi.
- [ ] `tests/integration/test_neutral_seed.py` vẫn xanh.

**Kiểm tra:** `python -m pytest tests/integration/test_product_schema.py tests/integration/test_neutral_seed.py -q`

**Phụ thuộc:** Không. **Cỡ:** S.

---

### Task 1.3: Thêm và sửa sản phẩm trên giao diện

**Mô tả:** Trang `/products` hiện chỉ xem. Thêm form tạo và sửa.

**File:**
- Tạo `app/services/product_service.py`
- Sửa `app/routers/products.py`
- Tạo `templates/products/form.html`
- Sửa `templates/products/list.html`
- Tạo `tests/integration/test_product_catalog.py`

**Các bước:**

1. `product_service.py` — toàn bộ logic, router chỉ gọi:

```python
class ProductError(ValueError): ...

def list_products(db, include_inactive=True) -> list[Product]
    # sắp theo group_code (None cuối), kind (main trước), sort_order, name
def get_product(db, product_id) -> Optional[Product]
def clean_form(db, data: dict, product_id: Optional[int] = None) -> dict
def create_product(db, data: dict) -> Product
def update_product(db, product_id: int, data: dict) -> Product
```

   `clean_form` nhận dict từ form, trả dict đã ép kiểu, ném `ProductError` với
   thông báo tiếng Việt khi:
   - `code` rỗng, hoặc trùng với sản phẩm khác (so sánh sau khi `.strip().upper()`);
   - `name` rỗng;
   - `kind` không thuộc `PRODUCT_KINDS`; `quantity_source` không thuộc `QUANTITY_SOURCES`;
   - `default_price` âm; `default_vat_rate` ngoài 0–100;
   - `price_period_months` < 1; `quantity_step` < 1; `min_quantity` < 0;
   - `contract_number_format` không rỗng mà `numbering.validate_format` ném lỗi
     (bắt `InvalidNumberFormat`, đổi thành `ProductError`);
   - `replaced_by_id` bằng chính `product_id`;
   - `kind != "main"` mà `group_code` rỗng (gói bổ sung phải thuộc một nhóm).

   Ô giá do người gõ: parse bằng `contract_service._safe_decimal`. Ô thuế suất
   và các ô số nguyên: parse bằng `Decimal(str(v))` / `int`, vì người dùng gõ
   "8.5" nghĩa là 8,5%.

2. `app/routers/products.py` — thêm route. Khai báo `/new` **trước** route có
   `{product_id}`:

```
GET  /products/new                → form trống
POST /products/new                → create_product, lỗi thì render lại form kèm errors, status 422
GET  /products/{product_id}/edit  → form điền sẵn, 404 nếu không có
POST /products/{product_id}/edit  → update_product
```

   Mẫu xử lý lỗi và flash: chép cách `contracts.new_submit` và hàm `_flash` /
   `_fv` trong `app/routers/contracts.py`. Luôn `db.close()` trong `finally`.

3. `templates/products/form.html` — `{% extends "base.html" %}`, dùng lớp CSS
   `fc fc-sm`, `fs fs-sm`, `lbl`, `card`, `card-h`, `card-b` như
   `templates/contracts/form.html`. Các ô, theo thứ tự:
   mã, tên, loại (`product_type`), nhóm, kiểu (select 3 lựa chọn), đơn vị tính,
   đơn giá chưa thuế, thuế suất %, kỳ giá (tháng), nguồn số lượng (select), số
   lượng tối thiểu, bước làm tròn, số tài khoản kèm theo, thời hạn mặc định
   (tháng), mẫu hợp đồng mặc định (select từ `ContractTemplate` đang bật), khuôn
   số hợp đồng, gói thay thế (select sản phẩm khác), thứ tự hiển thị.
   Dưới mỗi ô khó hiểu có một dòng chú thích ngắn.

4. `templates/products/list.html`:
   - Đổi nút tiêu đề thành "Thêm sản phẩm" trỏ `/products/new`.
   - Thêm cột "Nhóm", "Kiểu", "ĐVT", và cột cuối có nút "Sửa".
   - Xóa câu mô tả "Trang này dùng để kiểm tra nhanh dữ liệu seed…".
   - Sắp theo `list_products`; chèn một dòng tiêu đề nhóm mỗi khi `group_code` đổi.

5. Test (dùng fixture `client` — chép từ `tests/integration/test_local_security.py`
   sang `tests/conftest.py` thành fixture dùng chung trong task này):
   - POST tạo hợp lệ → 303/302, DB có dòng mới với đủ trường.
   - Mã trùng → 422, trang chứa thông báo, DB không thêm dòng.
   - Gói `addon` không có nhóm → 422.
   - Khuôn số thiếu `{seq}` → 422.
   - POST sửa đổi giá → DB đổi; một hợp đồng đã tạo trước đó với sản phẩm này
     (dùng `make_contract(unit_price_pre_vat=...)`) **không** đổi giá.

**Tiêu chí chấp nhận:**
- [ ] Tạo và sửa được sản phẩm với mọi trường của Task 1.2.
- [ ] Mọi lỗi nhập hiện thông báo tiếng Việt và giữ lại dữ liệu đã gõ.
- [ ] Sửa giá danh mục không đổi hợp đồng đã có.

**Kiểm tra:** test trên xanh; mở `/products`, tạo một gói chính và một gói bổ
sung cùng nhóm, thấy chúng gom dưới một tiêu đề nhóm.

**Phụ thuộc:** 1.2. **Cỡ:** M.

---

### Task 1.4: Ngừng bán và bật lại sản phẩm

**Mô tả:** Cho phép ẩn một gói khỏi form lập hợp đồng mà không ảnh hưởng hợp đồng
đã có.

**File:** `app/services/product_service.py`, `app/routers/products.py`,
`templates/products/list.html`, `tests/integration/test_product_catalog.py`

**Các bước:**

1. Thêm vào service:

```python
def count_in_service(db, product_id: int) -> int:
    """Số hợp đồng đang trong thời gian phục vụ của sản phẩm này."""
    # lọc Contract.status.in_(lifecycle.IN_SERVICE_STATUSES) — import, không tự liệt kê
def toggle_active(db, product_id: int) -> Product
```

2. Route `POST /products/{product_id}/toggle-active` → flash "Đã ngừng bán …" /
   "Đã bán lại …".
3. Trang danh sách: nút "Ngừng bán" / "Bán lại" trong một `<form method="post">`;
   thêm cột "HĐ đang phục vụ"; thêm bộ lọc `?show=active|inactive|all` (mặc định
   `active`), dòng đã ngừng hiển thị mờ.
4. Test:
   - Ngừng bán → sản phẩm không còn trong `products` mà `contracts.new_form` đưa
     ra (gọi `GET /contracts/new`, kiểm tra mã không xuất hiện trong HTML).
   - Hợp đồng đã có của sản phẩm đó: `GET /contracts/{id}` trả 200 và hiện tên
     sản phẩm; `GET /contracts/suggest-number?customer_id=…&product_id=…` vẫn
     trả số theo khuôn của sản phẩm.
   - `count_in_service` đếm đúng với hai hợp đồng `Signed` và một `Draft`.

**Tiêu chí chấp nhận:**
- [ ] Gói ngừng bán biến mất khỏi form lập hợp đồng và form báo giá.
- [ ] Hợp đồng cũ của gói đó xem được, sinh lại file được.

**Phụ thuộc:** 1.3. **Cỡ:** S.

---

### Task 1.5: Nhập và xuất danh mục bằng Excel

**Mô tả:** Nhập vài chục gói một lần, có bước xem trước.

**File:** `app/services/product_service.py`, `app/routers/products.py`,
tạo `templates/products/import.html`, sửa `templates/products/list.html`,
`tests/integration/test_product_catalog.py`

**Các bước:**

1. Cột của file, đúng thứ tự, dòng 1 là tiêu đề:

   `code, name, product_type, group_code, kind, unit_label, default_price,
   default_vat_rate, price_period_months, quantity_source, min_quantity,
   quantity_step, account_limit, default_duration_months,
   contract_number_format, is_active, sort_order`

2. Service:

```python
IMPORT_COLUMNS = [...]                                  # danh sách trên
def make_catalog_xlsx(db, with_data: bool) -> bytes     # file mẫu hoặc xuất danh mục hiện có
def parse_catalog_xlsx(db, file_bytes: bytes) -> list[dict]
    # mỗi phần tử: {"row": 2, "action": "create"|"update"|"error",
    #               "data": {...đã qua clean_form...}, "error": "..."}
def apply_catalog_rows(db, rows: list[dict]) -> tuple[int, int]   # (tạo, cập nhật)
```

   - Đọc bằng `openpyxl.load_workbook(io.BytesIO(b), read_only=True, data_only=True)`;
     **đóng workbook** (`wb.close()`) trước khi return, nếu không test sẽ đỏ vì
     `ResourceWarning`.
   - Số trong Excel là số máy: ép bằng `Decimal(str(v))`, không dùng `_safe_decimal`.
   - Dòng có `code` trùng sản phẩm đã có → `update`; chưa có → `create`.
   - `apply_catalog_rows` ghi trong **một** transaction: có dòng `error` thì
     không ghi gì và ném `ProductError`.
   - Nhập không bao giờ đổi `default_template_id`, `feature_list`, `replaced_by_id`.

3. Route:

```
GET  /products/template.xlsx   → file mẫu (chỉ tiêu đề + 2 dòng ví dụ hư cấu)
GET  /products/export.xlsx     → danh mục hiện có
GET  /products/import          → form chọn file
POST /products/import          → xem trước: bảng từng dòng, hành động, lỗi
POST /products/import/apply    → ghi; dữ liệu xem trước gửi lại trong trường ẩn JSON
```

   Mẫu hai bước: xem `import_form` / `import_submit` trong `app/routers/batch.py`
   và `templates/customers/import.html`. Đặt các route này **trước** route
   `/{product_id}/…`.

4. Test: file 3 dòng (1 tạo, 1 cập nhật, 1 lỗi) → xem trước báo đúng 3 hành động,
   `apply` từ chối; bỏ dòng lỗi → `apply` tạo 1, cập nhật 1; xuất rồi nhập lại
   chính file đó → 0 tạo, mọi dòng là cập nhật, dữ liệu không đổi.

**Tiêu chí chấp nhận:**
- [ ] Nhập được file theo mẫu; lỗi báo theo số dòng Excel.
- [ ] Không ghi một phần khi có lỗi.
- [ ] Xuất → nhập lại không làm đổi dữ liệu.

**Phụ thuộc:** 1.3. **Cỡ:** M.

---

### Điểm dừng 1

- [ ] Toàn bộ test xanh; scanner sạch.
- [ ] Trên **bản sao** DB thật: nhập `data/catalog.xlsx`, gán mẫu và khuôn số cho
      gói chính, ngừng bán các gói cũ, lập thử một hợp đồng một gói bằng form
      hiện tại. Thuế suất và tổng tiền đúng.
- [ ] Người phụ trách nghiệp vụ duyệt danh mục trước khi làm trên DB thật.

---

## Đợt 2 — Hợp đồng nhiều dòng

Kết quả: một hợp đồng chứa gói chính và các gói bổ sung, tổng tính theo dòng,
hợp đồng cũ không đổi một đồng.

### Task 2.1: Bảng `contract_items`, cột mới của `contracts`, chuyển dữ liệu

**Mô tả:** Thêm cấu trúc lưu dòng hợp đồng và điền một dòng cho mỗi hợp đồng cũ
bằng cách chép số đã lưu.

**File:**
- Tạo `app/models/contract_item.py`
- Sửa `app/models/__init__.py`, `app/models/contract.py`, `app/models/customer.py`
- Sửa `app/models/seed.py`
- Tạo `tests/integration/test_contract_items_migration.py`

**Các bước:**

1. `contract_item.py`:

```python
class ContractItem(Base):
    __tablename__ = "contract_items"

    id = Column(Integer, primary_key=True, index=True)
    contract_id = Column(Integer, ForeignKey("contracts.id", ondelete="CASCADE"),
                         nullable=False, index=True)
    product_id = Column(Integer, ForeignKey("products.id"), nullable=True)
    sort_order = Column(Integer, nullable=False, default=0)
    description = Column(String(300), nullable=False)
    unit_label = Column(String(50), nullable=True)
    quantity = Column(Numeric(18, 2), nullable=False)
    period_count = Column(Numeric(10, 4), nullable=False)
    unit_price_pre_vat = Column(Numeric(18, 2), nullable=True)
    vat_rate = Column(Numeric(5, 2), nullable=False, default=0)
    amount_pre_vat = Column(Numeric(18, 0), nullable=False)
    vat_amount = Column(Numeric(18, 0), nullable=False)
    amount_total = Column(Numeric(18, 0), nullable=False)
    note = Column(Text, nullable=True)

    contract = relationship("Contract", back_populates="items")
    product = relationship("Product")
```

2. `app/models/__init__.py`: import và thêm `ContractItem` vào `__all__`. Thiếu
   bước này thì `Base.metadata.create_all` không tạo bảng.

3. `Contract` — thêm cột và quan hệ:

```python
    pricing_model = Column(String(10), nullable=False, default="legacy",
                           comment="legacy = giữ số đã lưu | items = tính theo dòng")
    total_pre_vat = Column(Numeric(18, 0), nullable=True)
    total_vat = Column(Numeric(18, 0), nullable=True)
    payment_mode = Column(String(10), nullable=True, comment="prepaid | postpaid")
    payment_cycle_months = Column(Integer, nullable=True)
    parent_contract_id = Column(Integer, ForeignKey("contracts.id"), nullable=True, index=True)
    doc_kind = Column(String(20), nullable=False, default="contract",
                      comment="contract | addendum_addon | addendum_renewal")

    items = relationship("ContractItem", back_populates="contract",
                         order_by="ContractItem.sort_order",
                         cascade="all, delete-orphan")
    parent = relationship("Contract", remote_side=[id], backref="addenda")
```

   Mặc định `pricing_model` là `"legacy"` có chủ ý: mọi đường tạo hợp đồng chưa
   được chuyển (sinh hàng loạt) vẫn đúng nhãn. Chỉ `create_draft` theo dòng mới
   đặt `"items"`.

4. `Customer`: thêm `record_count = Column(Integer, nullable=True, comment="Số hồ sơ / dân số phục vụ")`.

5. `seed.run_migrations` — thêm `ALTER TABLE` cho 7 cột của `contracts`
   (`pricing_model VARCHAR(10) NOT NULL DEFAULT 'legacy'`,
   `doc_kind VARCHAR(20) NOT NULL DEFAULT 'contract'`, các cột còn lại nullable)
   và `record_count` của `customers`, theo đúng vòng lặp `_ct_migrations`.

6. Thêm hàm `_backfill_contract_items(db)` trong `seed.py`, gọi ở cuối
   `run_migrations` sau `_backfill_contract_number_formats(db)`:

```python
def _backfill_contract_items(db: Session) -> None:
    """Mỗi hợp đồng chưa có dòng nhận đúng một dòng chép từ số đã lưu.

    KHÔNG tính lại tổng tiền. amount_total của dòng = total_amount của hợp đồng.
    Phần trước thuế / thuế được suy ngược để hiển thị, và luôn cộng lại đúng tổng.
    """
    from decimal import Decimal, ROUND_HALF_UP
    from app.models.contract import Contract
    from app.models.contract_item import ContractItem

    pending = (
        db.query(Contract)
        .outerjoin(ContractItem, ContractItem.contract_id == Contract.id)
        .filter(ContractItem.id.is_(None))
        .all()
    )
    if not pending:
        return
    _backup_database_once("before-contract-items")   # bước 7

    for c in pending:
        total = Decimal(str(c.total_amount or 0))
        vat_rate = Decimal(str(c.vat_rate or 0))
        pre = (total / (1 + vat_rate / 100)).quantize(Decimal("1"), rounding=ROUND_HALF_UP)
        months = Decimal(str(c.month_count or 1))
        units = Decimal(str(c.unit_count or 1))
        db.add(ContractItem(
            contract_id=c.id, product_id=c.product_id, sort_order=0,
            description=(c.product.name if c.product else "Dịch vụ"),
            unit_label=(c.product.unit_label if c.product else None),
            quantity=units, period_count=months,
            unit_price_pre_vat=c.unit_price_pre_vat, vat_rate=vat_rate,
            amount_pre_vat=pre, vat_amount=total - pre, amount_total=total,
            note="Chép từ số đã lưu của hợp đồng",
        ))
        c.total_pre_vat = pre
        c.total_vat = total - pre
    db.commit()
    safe_print(f"  [migration] Đã tạo dòng cho {len(pending)} hợp đồng cũ.")
```

   Không được gán `c.total_amount`, `c.amount_text`, `c.unit_price_*` trong hàm này.

7. `_backup_database_once(tag)`: sao DB hiện tại sang thư mục backup bằng
   `sqlite3.Connection.backup`. Lấy đường dẫn từ `app.paths.DB_PATH` và
   `app.paths.BACKUPS_DIR`; đọc chúng **bên trong hàm** (`import app.paths as
   paths` rồi `paths.BACKUPS_DIR`), không import tên ở đầu file, vì test đổi
   data root lúc chạy. Tên file: `contract_manager-<tag>-<YYYYmmdd-HHMMSS>.db`.
   Đóng cả hai kết nối. Bỏ qua khi DB là file tạm rỗng (không có hợp đồng thì
   hàm ở bước 6 đã return trước).

8. Test:
   - Tạo 3 hợp đồng bằng `make_contract` với `total_amount`, `vat_rate`,
     `unit_count`, `month_count` khác nhau (một cái thuế 8, tổng lẻ; một cái
     `unit_price_pre_vat=None`; một cái `total_amount=None`). Chạy
     `run_migrations`.
   - Mỗi hợp đồng có đúng 1 dòng; `amount_pre_vat + vat_amount == amount_total`;
     `amount_total == total_amount` (hoặc 0 khi tổng rỗng).
   - `total_amount`, `amount_text`, `unit_price_vat` của hợp đồng trước và sau
     giống hệt.
   - Chạy lần hai: số dòng không tăng.
   - Có file backup trong thư mục backup của data root tạm.
   - Xóa hợp đồng (`contract_service.delete_contract`) xóa luôn dòng của nó.

**Tiêu chí chấp nhận:**
- [ ] DB cũ khởi động, mỗi hợp đồng có đúng một dòng, tổng không đổi.
- [ ] Chạy lại không sinh dòng trùng.
- [ ] Có bản sao lưu trước khi ghi.

**Kiểm tra:** test trên; rồi làm "Kiểm chứng trên dữ liệu thật" ở mục 9.

**Phụ thuộc:** 1.2. **Cỡ:** M. **Rủi ro cao nhất của cả kế hoạch — làm sớm.**

---

### Task 2.2: `create_draft` nhận danh sách dòng

**Mô tả:** Khi form gửi `items_json`, hợp đồng được tính theo dòng. Không có
`items_json` thì chạy đúng như cũ.

**File:** `app/services/contract_service.py`, tạo
`tests/integration/test_multi_item_contract.py`

**Các bước:**

1. Định dạng `items_json` (chuỗi JSON, số ở dạng máy):

```json
[
  {"product_id": 3, "quantity": "2", "unit_price_pre_vat": "500000",
   "vat_rate": "10", "description": "", "amount_pre_vat_override": "", "note": ""}
]
```

   Dòng đầu tiên là gói chính. `description` rỗng thì lấy tên sản phẩm.

2. Thêm vào `contract_service.py`:

```python
def _parse_items_json(raw) -> list[dict]:
    """Trả [] khi rỗng. Ném ValueError khi không phải JSON list of dict."""

def _build_lines(db, raw_items: list[dict], month_count: int) -> list[pricing.Line]:
    """Đổi dữ liệu form thành các dòng đã tính tiền."""
```

   Quy tắc của `_build_lines`, theo từng phần tử:
   - Nạp `Product` theo `product_id`; không có → `ValueError("Gói ... không tồn tại.")`.
   - Phần tử đầu phải có `kind == "main"`; các phần tử sau phải khác `"main"` và
     cùng `group_code` với gói chính. Sai → `ValueError`.
   - `quantity = pricing.normalize_quantity(item["quantity"], product.min_quantity, product.quantity_step)`.
   - Đơn giá: `item["unit_price_pre_vat"]` nếu có, không thì `product.default_price`.
     Thuế suất tương tự với `default_vat_rate`.
   - Gọi `pricing.price_line(..., months=month_count, price_period_months=product.price_period_months, product_id=product.id)`.
   - Bắt `pricing.PricingError` và ném lại `ValueError` có số thứ tự dòng.

3. Trong `create_draft`, ngay sau khối tính `month_count`:

```python
    raw_items = _parse_items_json(form_data.get("items_json"))
    if raw_items:
        if not month_count:
            raise ValueError("Phải có số tháng để tính tiền.")
        lines = _build_lines(db, raw_items, month_count)
        totals = pricing.sum_lines(lines)
        main = lines[0]
```

   Rồi khi tạo `Contract`:
   - `product_id = main.product_id`
   - `unit_price_pre_vat = main.unit_price_pre_vat`, `vat_rate = main.vat_rate`
   - `unit_price_vat = pricing.round_money(main.unit_price_pre_vat * (1 + main.vat_rate / 100))`
   - `quantity = main.quantity * main.period_count`
   - `total_pre_vat`, `total_vat`, `total_amount` từ `totals`
   - `amount_text = totals.amount_text` (trừ khi form gửi `amount_text`)
   - `pricing_description`: giá trị form nếu có, không thì `pricing.describe(lines, totals)`
   - `pricing_model = "items"`
   - `payment_mode`, `payment_cycle_months` từ form (chấp nhận `prepaid`/`postpaid`, khác → `None`)

   Sau `db.flush()`, thêm `ContractItem` cho từng `Line` với `sort_order` tăng dần.

   Nhánh `else` (không có `raw_items`) giữ **nguyên văn** code hiện tại.

4. Trong `generate_docx`: khối tự sinh `pricing_description` bằng
   `_auto_pricing_description` chỉ chạy khi `contract.pricing_model == "legacy"`.

5. Test:
   - Gói chính 2 cơ sở × 12 tháng + một gói bổ sung → 2 dòng; ba tổng khớp
     `pricing.sum_lines` tính độc lập trong test; `pricing_model == "items"`.
   - Gói tính theo hồ sơ có sàn 2000, bước 500, gửi 1200 → dòng lưu 2000.
   - Gói kỳ 12 tháng, hợp đồng 12 tháng → `period_count == 1`.
   - Dòng đầu là gói bổ sung → `ValueError`.
   - Gói bổ sung khác nhóm → `ValueError`.
   - Không gửi `items_json` → kết quả giống hệt trước task: viết test này
     **trước khi sửa code**, chạy xanh, rồi mới sửa (test đặc tả hành vi cũ).
   - `items_json` có đơn giá `"82.5"` → lưu 82.5.

**Tiêu chí chấp nhận:**
- [ ] Có `items_json` → hợp đồng `items`, số dòng và tổng đúng.
- [ ] Không có `items_json` → hành vi không đổi.
- [ ] Mọi lỗi dữ liệu thành `ValueError` có nội dung đọc được.

**Phụ thuộc:** 1.1, 2.1. **Cỡ:** M.

---

### Task 2.3: API xem trước giá và danh sách gói bổ sung

**Mô tả:** Hai endpoint JSON để form hiển thị tiền mà không tự tính.

**File:** `app/routers/contracts.py`, `app/services/contract_service.py`,
`tests/integration/test_multi_item_contract.py`

**Các bước:**

1. Đặt hai route trong khối "CONTRACT NUMBER SUGGESTION API", trước mọi route
   `/{contract_id}`:

```
GET  /contracts/product-options?product_id=&customer_id=&unit_count=
POST /contracts/price-preview        (body JSON)
```

2. `product-options` trả:

```json
{"main": {"id":3,"code":"…","name":"…","unit_label":"…","price":"500000","vat":"10",
          "price_period_months":1,"quantity_source":"units","default_quantity":"2",
          "template_id":5},
 "addons": [ {…cùng cấu trúc…} ]}
```

   `default_quantity`: `units` → `unit_count` gửi lên (mặc định 1); `records` →
   `customer.record_count` đã qua `normalize_quantity` (không có thì chuỗi rỗng);
   `manual` → `"1"`. `addons` là sản phẩm đang bán, khác `main`, cùng
   `group_code`, sắp theo `sort_order`. Viết logic trong
   `contract_service.product_options(db, product_id, customer_id, unit_count)`.

3. `price-preview` nhận `{"month_count": 12, "items": [...]}` (cùng cấu trúc
   `items_json`), gọi `_build_lines` + `pricing.sum_lines`, trả:

```json
{"lines":[{"description":"…","unit_label":"…","quantity":"2","period_count":"12",
           "unit_price_pre_vat":"500000","vat_rate":"10","amount_pre_vat":"12000000",
           "vat_amount":"1200000","amount_total":"13200000"}],
 "totals":{"pre_vat":"12000000","vat":"1200000","total":"13200000","amount_text":"…"},
 "description":"…"}
```

   Lỗi → HTTP 422 với `{"error": "..."}`. Mọi số trả về là **chuỗi** (`str(Decimal)`),
   không dùng `float`.

4. Test bằng `client`: hai endpoint với dữ liệu hợp lệ; `price-preview` với
   `items: []` → 422; kết quả `price-preview` bằng đúng số mà `create_draft` lưu
   cho cùng đầu vào.

**Tiêu chí chấp nhận:**
- [ ] Xem trước và lưu thật cho cùng đầu vào ra cùng số.
- [ ] Không có `float` trong đường đi của tiền.

**Phụ thuộc:** 2.2. **Cỡ:** S.

---

### Task 2.4: Trang chi tiết hợp đồng hiện các dòng

**File:** `app/routers/contracts.py` (`contract_detail`),
`templates/contracts/detail.html`

**Các bước:**

1. Trong `contract_detail`, trước `db.close()`: `items = list(c.items)`; truyền
   `items` vào template.
2. Trong `detail.html`, quanh dòng ~127–132 (khối "Số đơn vị", "Đơn giá…"):
   - Giữ khối cũ khi `c.pricing_model == 'legacy'`.
   - Khi `'items'`: thay bằng bảng cột *TT, Dịch vụ, ĐVT, Số lượng, Số kỳ, Đơn
     giá, Trước thuế, Thuế suất, Tiền thuế, Sau thuế*, và ba dòng tổng. Định
     dạng tiền bằng đúng biểu thức `'{:,.0f}'.format(x).replace(',', '.')` đang dùng.
   - Hiện hình thức và chu kỳ thanh toán nếu có.
3. Test: `GET /contracts/{id}` của hợp đồng `items` chứa tên cả hai gói và tổng
   đã định dạng; của hợp đồng `legacy` vẫn chứa "Đơn giá chưa VAT".

**Phụ thuộc:** 2.2. **Cỡ:** S.

---

### Task 2.5: Form lập hợp đồng nhiều dòng

**Mô tả:** Thay khối giá đơn bằng bảng dòng. Đây là task giao diện lớn nhất.

**File:** `templates/contracts/form.html`, `app/routers/contracts.py`
(`new_form`, `new_submit`)

**Các bước:**

1. `new_form` / `new_submit`: chỉ đưa sản phẩm `kind == "main"` đang bán vào
   dropdown "Sản phẩm / Dịch vụ". Trong `<option>`, bỏ `data-price`, `data-vat`
   (giá giờ lấy từ `product-options`); giữ `data-code`, `data-template`,
   `data-shortname`.

2. Trong card giá (ô `unit_price_pre_vat` tới `pricing_description`, dòng ~150–230):
   - Xóa các ô: `unit_price_pre_vat`, `vat_rate`, `unit_price_vat`, ô
     `month_count` thứ hai, `quantity`, `total_amount`.
   - Giữ: `unit_count` (chuyển thành chỉ đọc, hiển thị số cơ sở đã tích),
     checklist cơ sở, `selected_units`, `amount_text`, `pricing_description`.
   - Thêm:

```html
<table class="table table-sm" id="tbl-items">
  <thead><tr>
    <th>Dịch vụ</th><th>ĐVT</th><th>Số lượng</th><th>Đơn giá</th><th>Thuế %</th>
    <th class="text-end">Trước thuế</th><th class="text-end">Thuế</th>
    <th class="text-end">Sau thuế</th><th></th>
  </tr></thead>
  <tbody></tbody>
  <tfoot><!-- ba dòng tổng: id="sum-pre", "sum-vat", "sum-total" --></tfoot>
</table>
<div id="addon-picker"><!-- danh sách gói bổ sung dạng checkbox --></div>
<div id="price-error" class="text-danger small"></div>
<input type="hidden" name="items_json" id="inp-items-json">
<select name="payment_mode">…prepaid / postpaid…</select>
<input type="number" name="payment_cycle_months" min="1">
```

3. JavaScript — thay `priceOfOption` và `recalcPrice` bằng bốn hàm. Không hàm nào
   chứa phép nhân tiền:

```
loadProductOptions()   gọi GET /contracts/product-options khi đổi sản phẩm, đổi
                       khách hàng, hoặc đổi số cơ sở đã tích. Dựng dòng gói chính
                       và danh sách checkbox gói bổ sung. Giữ lại số lượng/đơn giá
                       người dùng đã sửa tay (đánh dấu data-manual="1" như ô
                       amount_text đang làm).
collectItems()         đọc bảng → mảng theo định dạng items_json. Ô đơn giá do
                       người gõ kiểu "500.000": đổi về số máy bằng parseN rồi
                       String(). Ô thuế: parseFloat.
refreshPreview()       POST /contracts/price-preview với {month_count, items}.
                       Điền ba cột tiền của từng dòng, ba tổng, amount_text và
                       pricing_description (nếu không data-manual). Lỗi 422 →
                       hiện vào #price-error và vô hiệu nút lưu. Debounce 250 ms.
beforeSubmit()         gán JSON.stringify(collectItems()) vào #inp-items-json.
```

   - Dựng HTML bằng `document.createElement` + `textContent`, hoặc qua hàm
     `escapeHtml` đã có trong file. **Không** nối tên sản phẩm vào `innerHTML`
     (lỗi XSS kiểu này đã từng phải vá).
   - Dữ liệu server đưa vào `<script>` dùng `| tojson`.
   - Gắn `refreshPreview` vào sự kiện `input` của mọi ô trong bảng, ô số tháng,
     và sau khi checklist cơ sở đổi.
   - Dòng có `quantity_source == "units"`: số lượng tự theo số cơ sở đã tích,
     trừ khi người dùng đã sửa tay.

4. Khi form bị trả lại do lỗi (`prefill`): dựng lại bảng từ `prefill.items_json`.

5. Test:
   - `GET /contracts/new` không còn chứa `name="unit_price_pre_vat"`, có
     `name="items_json"`; dropdown không chứa gói bổ sung.
   - `POST /contracts/new` với `items_json` hợp lệ → chuyển hướng tới trang chi
     tiết, hợp đồng có đúng số dòng.
   - `POST` với `items_json` lỗi → 422, HTML trả về còn chứa `items_json` đã gửi.

6. Kiểm tra tay trên trình duyệt (ghi kết quả vào mô tả commit):
   chọn khách có 3 cơ sở → chọn gói chính → số lượng 3; tích thêm 1 gói bổ sung →
   2 dòng, tổng cập nhật; bỏ tích 1 cơ sở → số lượng 2; sửa tay đơn giá → tổng đổi,
   đổi cơ sở không ghi đè đơn giá; lưu → trang chi tiết khớp số trên form.

**Tiêu chí chấp nhận:**
- [ ] Form không còn công thức tiền nào trong JavaScript (tìm `* (1 +` và
      `/ 100` trong file: không còn trong phần giá).
- [ ] Số trên form bằng số lưu trong DB với mọi kịch bản ở bước 6.
- [ ] Lỗi nhập giữ nguyên các dòng đã chọn.

**Phụ thuộc:** 2.3. **Cỡ:** L — nếu quá một buổi, tách: (a) HTML + `loadProductOptions`,
(b) `refreshPreview` + submit + prefill.

---

### Điểm dừng 2

- [ ] Toàn bộ test xanh; scanner sạch.
- [ ] Làm đủ "Kiểm chứng trên dữ liệu thật" (mục 9) và lưu kết quả đối chiếu.
- [ ] Lập 3 hợp đồng mẫu trên bản sao: một gói; gói chính + 2 gói bổ sung; gói
      tính theo hồ sơ. So tay với bảng tính Excel.
- [ ] Người phụ trách nghiệp vụ duyệt trước khi sang Đợt 3.

---

## Đợt 3 — Sinh file Word theo biểu mẫu mới

### Task 3.1: Lặp dòng bảng trong renderer

**Mô tả:** Một dòng bảng chứa placeholder `ITEM_*` hoặc `FEATURE_*` được nhân bản
theo danh sách dữ liệu.

**File:** `app/services/docx_renderer.py`, tạo `tests/unit/test_docx_repeat_rows.py`

**Các bước:**

1. Thêm dưới `_PH_RE`:

```python
#: Tiền tố placeholder của dòng lặp → tên danh sách dữ liệu.
REPEAT_GROUPS = {"ITEM_": "items", "FEATURE_": "features"}
```

2. Thêm hàm (đặt trước `render_docx`):

```python
def _row_placeholders(tr) -> set[str]:
    text = "".join(t.text or "" for t in tr.iter(qn("w:t")))
    return set(_PH_RE.findall(text))


def _repeat_group_of(tr):
    names = _row_placeholders(tr)
    for prefix, key in REPEAT_GROUPS.items():
        if any(n.startswith(prefix) for n in names):
            return key
    return None


def _expand_repeat_rows(doc, lists: dict) -> None:
    """Nhân bản dòng mẫu theo từng phần tử; danh sách rỗng thì xóa dòng mẫu."""
    from docx.table import _Row

    for table in doc.tables:
        for tr in list(table._tbl.findall(qn("w:tr"))):
            key = _repeat_group_of(tr)
            if key is None:
                continue
            anchor = tr
            for row_ctx in lists.get(key) or []:
                new_tr = copy.deepcopy(tr)
                anchor.addnext(new_tr)
                anchor = new_tr
                ctx = {k: ("" if v is None else str(v)) for k, v in row_ctx.items()}
                for cell in _Row(new_tr, table).cells:
                    for para in cell.paragraphs:
                        _render_paragraph(para, ctx)
            tr.getparent().remove(tr)
```

   `_render_paragraph` chỉ thay khóa có trong `ctx` và để nguyên khóa khác, nên
   placeholder chung trong dòng (ví dụ `{{VAT_RATE}}`) vẫn được lượt thay toàn
   văn bản phía sau xử lý.

3. `render_docx` nhận thêm tham số `lists: Optional[dict] = None`. Ngay sau khi
   dựng `ctx` và **trước** `_insert_party_a_optional_rows`:

```python
    if lists is not None:
        _expand_repeat_rows(doc, lists)
```

   `lists is None` (báo giá, biểu 08a hiện tại) → không đụng vào bảng nào.

4. Test — dựng tài liệu trong bộ nhớ bằng `python-docx`:
   - Bảng 3 dòng (tiêu đề / dòng mẫu `{{ITEM_NO}} {{ITEM_NAME}} {{ITEM_AMOUNT_TOTAL}}` / dòng tổng `{{TOTAL_AMOUNT}}`),
     2 phần tử → 4 dòng, đúng thứ tự, dòng tổng vẫn ở cuối và được thay.
   - 0 phần tử → còn 2 dòng.
   - 1 phần tử → 3 dòng.
   - Dòng mẫu có chữ đậm → dòng sinh ra vẫn đậm (kiểm `run.bold`).
   - Placeholder bị tách thành 3 run trong dòng mẫu → vẫn thay đúng.
   - Hai bảng, một `ITEM_`, một `FEATURE_` → mỗi bảng nhận đúng danh sách của mình.
   - `lists=None` với tài liệu có dòng `ITEM_` → tài liệu không đổi số dòng.
   - Tài liệu không có dòng lặp + `lists={"items": [...]}` → kết quả byte-tương
     đương về nội dung văn bản với lần gọi không có `lists`.

**Tiêu chí chấp nhận:**
- [ ] Đủ 8 test, xanh.
- [ ] Mọi test hiện có của renderer, báo giá và 08a vẫn xanh.

**Phụ thuộc:** Không (làm song song với Đợt 2 được). **Cỡ:** M.

---

### Task 3.2: Context và danh sách cho hợp đồng theo dòng

**File:** `app/services/docx_renderer.py`, `app/services/contract_service.py`
(`generate_docx`), `app/services/settings_service.py`,
`templates/settings/index.html`, `app/routers/settings.py`,
tạo `tests/integration/test_items_rendering.py`

**Các bước:**

1. Thêm hàm công khai trong `docx_renderer.py`:

```python
def build_render_lists(contract) -> dict[str, list[dict[str, str]]]:
    """Dữ liệu cho các dòng lặp. Mọi giá trị đã là chuỗi hiển thị."""
```

   `items` — mỗi dòng hợp đồng, theo `sort_order`:

| Khóa | Giá trị |
|---|---|
| `ITEM_NO` | 1, 2, 3… |
| `ITEM_NAME` | `description` |
| `ITEM_UNIT` | `unit_label` |
| `ITEM_QTY` | `quantity × period_count`, qua `_fmt_number` |
| `ITEM_PRICE` | `unit_price_pre_vat` qua `_fmt_currency` |
| `ITEM_AMOUNT_PRE_VAT`, `ITEM_VAT_AMOUNT`, `ITEM_AMOUNT_TOTAL` | qua `_fmt_currency` |
| `ITEM_VAT_RATE` | qua `_fmt_vat` |

   `features` — gộp `product.feature_list` của các dòng theo thứ tự, bỏ trùng
   theo `name`, đánh lại `FEATURE_NO` từ 1; khóa `FEATURE_NAME`, `FEATURE_DESC`.

   Câu hỏi cần chốt ở Task 0.1 trước khi viết `ITEM_QTY`: cột "Số lượng" của
   biểu mẫu là số cơ sở, hay số cơ sở × số tháng. Mặc định theo bảng trên; nếu
   khác, thêm `ITEM_PERIODS` và sửa bảng trong mẫu, không đổi cách tính tiền.

2. `build_render_context` — thêm, không đổi kiểu trả về:

| Placeholder | Nguồn |
|---|---|
| `TOTAL_PRE_VAT`, `TOTAL_VAT` | `contract.total_pre_vat`, `contract.total_vat` |
| `PAYMENT_CYCLE_MONTHS` | `contract.payment_cycle_months` |
| `PARTY_A_PHONE`, `PARTY_A_EMAIL`, `PARTY_A_CONTACT_NAME` | `customer.phone`, `.email`, `.contact_name` |
| `PARTY_A_ESTABLISHMENT_DECISION` | `customer.establishment_decision` |
| `PARTY_A_ESTABLISHMENT_DATE` | `format_date_vn(customer.establishment_date)` |
| `PARTY_B_PHONE`, `PARTY_B_CONTACT_NAME`, `PARTY_B_CONTACT_EMAIL` | AppSetting (bước 3) |
| `PRODUCT_NAME` | tên gói chính |
| `PARENT_CONTRACT_NUMBER`, `PARENT_SIGN_DATE` | hợp đồng cha, rỗng nếu không có |

3. `settings_service.py`: thêm `party_b_phone`, `party_b_contact_name`,
   `party_b_contact_email` vào `PARTY_B_DEFAULTS` (giá trị rỗng) và vào dict trả
   về của `get_party_b_settings` (khóa `phone`, `contact_name`, `contact_email`).
   Cập nhật nhánh `else` dựng `_pbs` trong `build_render_context`. Thêm ba ô vào
   `templates/settings/index.html` và vào danh sách khóa được lưu trong
   `app/routers/settings.py`.

4. `generate_docx`:

```python
    ctx = build_render_context(contract, customer, template=template, db=db)
    lists = build_render_lists(contract)
    contract.render_data = {**ctx, "_lists": lists}
    ...
    rendered_bytes = render_docx(template.file_path, ctx, lists=lists)
```

   Trước khi gọi `build_render_lists`, đảm bảo `contract.items` và
   `item.product` đã nạp (session còn mở tại đây).

5. Test: tạo hợp đồng 2 dòng qua `create_draft`; dựng mẫu docx trong test có
   bảng `ITEM_*`, bảng `FEATURE_*`, và `{{TOTAL_PRE_VAT}}`; lưu vào thư mục mẫu
   của data root tạm; `generate_docx`; mở file kết quả bằng `python-docx` và
   kiểm: số dòng bảng, tên gói, ba tổng đã định dạng, không còn chuỗi `{{` nào.
   Thêm một hợp đồng `legacy` dùng mẫu cũ không có dòng lặp → sinh file bình thường.

**Tiêu chí chấp nhận:**
- [ ] File sinh ra không còn placeholder chưa thay.
- [ ] Tổng trong file bằng tổng trong DB.
- [ ] Hợp đồng `legacy` sinh lại file được bằng mẫu cũ.

**Phụ thuộc:** 2.2, 3.1. **Cỡ:** M.

---

### Task 3.3: Analyzer nhận biết placeholder mới và dòng lặp

**File:** `app/services/template_analyzer.py`, `app/services/template_service.py`,
`templates/contract_templates/detail.html`, tạo `tests/unit/test_template_analyzer_rows.py`

**Các bước:**

1. `CANONICAL_MAP`: thêm mọi placeholder vô hướng của Task 3.2, theo kiểu
   `"TOTAL_PRE_VAT": "contract.total_pre_vat"`.
2. Thêm hằng `ROW_PLACEHOLDERS: dict[str, str]` ánh xạ từng `ITEM_*` / `FEATURE_*`
   sang `"items.<tên>"` / `"features.<tên>"`. `build_default_mapping` tra
   `CANONICAL_MAP` trước, rồi `ROW_PLACEHOLDERS`.
3. `AnalysisResult`: thêm `repeat_groups: list[str]` (ví dụ `["items", "features"]`)
   và `row_warnings: list[str]`.
4. Trong `analyze`: placeholder có tiền tố lặp tìm thấy ở `doc.paragraphs`,
   header hoặc footer → thêm cảnh báo "`{{ITEM_NAME}}` phải nằm trong một dòng
   bảng". Tìm thấy trong bảng → ghi nhóm vào `repeat_groups`.
5. `template_service.CONTRACT_TYPES`: thêm `"Bàn giao"`. `_template_kind`: thứ tự
   kiểm tra giữ nguyên, thêm nhánh — có `PARENT_CONTRACT_NUMBER` → `"Phụ lục"`.
6. Trang chi tiết mẫu: hiện "Có bảng giá trị", "Có bảng chức năng", và
   `row_warnings`.
7. Test: mẫu có dòng lặp đúng chỗ → `repeat_groups` đúng, không cảnh báo;
   `{{ITEM_NAME}}` ngoài bảng → có cảnh báo; mọi placeholder mới có ánh xạ không rỗng.

**Phụ thuộc:** 3.1. **Cỡ:** S.

---

### Task 3.4: Mẫu hư cấu đi kèm repo

**File:** `scripts/make_sample_templates.py`,
`ContractForge/assets/default_templates/sample_service_contract.docx` (file sinh ra),
`tests/integration/test_sample_assets.py`

**Các bước:**

1. Đổi tên `build()` hiện tại thành `build_form_08a()`; thêm
   `build_service_contract()` dựng một hợp đồng hư cấu ngắn: tiêu đề, khối hai
   bên bằng placeholder `PARTY_A_*` / `PARTY_B_*`, bảng giá trị 9 cột đúng thứ
   tự (TT, Dịch vụ, ĐVT, Số lượng, Đơn giá, Thành tiền trước thuế, Thuế suất,
   Tiền thuế GTGT, Thành tiền sau thuế) với một dòng `ITEM_*` và một dòng tổng,
   câu "Bằng chữ: {{AMOUNT_TEXT}}", bảng chức năng với một dòng `FEATURE_*`,
   khối ký. Câu chữ tự viết, **không chép** từ biểu mẫu của bất kỳ tổ chức nào.
2. `main()` ghi cả hai file. Chạy script, commit file `.docx` sinh ra
   (`.gitignore` đã cho phép `sample_*.docx`).
3. Mở rộng `test_sample_assets.py`: file tồn tại; `analyze` trả
   `repeat_groups == ["features", "items"]` (sau khi sắp xếp); không cảnh báo.

**Phụ thuộc:** 3.3. **Cỡ:** S.

---

### Điểm dừng 3

- [ ] Toàn bộ test xanh; scanner sạch.
- [ ] Tải các mẫu thật (Task 0.3) lên bản sao; trang chi tiết mẫu không có cảnh báo.
- [ ] Sinh 3 hợp đồng mẫu của Điểm dừng 2, in ra, so từng số với trang chi tiết.
- [ ] Người phụ trách nghiệp vụ duyệt bản in.

---

## Đợt 4 — Phụ lục

Chặn bởi câu hỏi 1 và 3 của Task 0.1. Không bắt đầu khi chưa có câu trả lời.

### Task 4.1: Đánh số phụ lục

**File:** `app/services/numbering.py`, `app/services/settings_service.py`,
`templates/settings/index.html`, `app/routers/settings.py`,
`tests/integration/test_number_format_compat.py` (thêm test, không sửa test cũ)

**Các bước:**

1. Trong `numbering.py`:

```python
DEFAULT_ADDENDUM_FORMAT = "PL{seq}/{parent}"
ADDENDUM_PLACEHOLDERS = ("seq", "parent")

def validate_addendum_format(fmt: str) -> str      # bắt buộc có cả {seq} và {parent}
def build_addendum_number(parent_number: str, seq: str, fmt: Optional[str] = None) -> str
def next_addendum_seq(used: set[int]) -> str       # dùng lại next_seq
```

   Không đổi `PLACEHOLDERS`, `validate_format`, `build_number`: khuôn hợp đồng
   và khuôn phụ lục là hai thứ riêng.
2. AppSetting `addendum_number_format`; ô nhập ở trang Cài đặt, kiểm bằng
   `validate_addendum_format` trước khi lưu.
3. Test: khuôn mặc định; khuôn tùy biến; thiếu `{parent}` → lỗi; số thứ tự lấp
   khoảng trống như hợp đồng.

**Phụ thuộc:** Không. **Cỡ:** S.

---

### Task 4.2: Phụ lục bổ sung gói

**File:** tạo `app/services/addendum_service.py`, sửa `app/routers/contracts.py`,
tạo `templates/contracts/addendum_form.html`, sửa `templates/contracts/detail.html`,
tạo `tests/integration/test_addendum.py`

**Các bước:**

1. Service:

```python
class AddendumError(ValueError): ...

def addendum_defaults(db, parent_id: int, kind: str, start: date) -> dict
    # số phụ lục gợi ý, ngày bắt đầu, ngày kết thúc, số tháng, danh sách gói chọn được
def create_addendum(db, parent_id: int, kind: str, form_data: dict) -> Contract
```

   Luật của `kind == "addon"`:
   - Hợp đồng cha phải có `doc_kind == "contract"` và
     `status in lifecycle.IN_SERVICE_STATUSES`; sai → `AddendumError`.
   - `start_date` nằm trong `[parent.start_date, parent.end_date]`.
   - `end_date = parent.end_date`; `month_count = pricing.addon_months(start_date, parent.end_date)`.
     Bỏ qua mọi giá trị số tháng form gửi lên.
   - Chỉ nhận gói `kind != "main"`, cùng `group_code` với gói chính của hợp đồng cha.
     Vì `_build_lines` đòi dòng đầu là gói chính, thêm tham số
     `require_main: bool = True` cho `_build_lines` và gọi với `False` ở đây.
   - Tạo bằng cách dựng `form_data` rồi gọi `create_draft`, sau đó gán
     `parent_contract_id`, `doc_kind = "addendum_addon"`, `customer_id` của cha.
   - Ghi `ContractEvent` trên **cả hai** bản ghi (`_add_event`).

2. Route (đặt trước route `/{contract_id}` dạng GET không hậu tố không cần, vì
   các route này có hậu tố riêng):

```
GET  /contracts/{contract_id}/addendum/new?kind=addon
POST /contracts/{contract_id}/addendum/new
```

3. `addendum_form.html`: form gọn — số phụ lục, ngày ký, ngày bắt đầu (đổi ngày
   thì gọi lại để lấy số tháng), mẫu Word (chỉ loại "Phụ lục"), bảng dòng dùng
   lại `refreshPreview` của Task 2.5. Tách phần JavaScript bảng dòng của
   `form.html` ra `static/js/contract_items.js` trong task này để hai form dùng chung.

4. Trang chi tiết hợp đồng cha: bảng "Phụ lục" (số, loại, trạng thái, tổng, liên
   kết) và nút "Tạo phụ lục bổ sung" chỉ hiện khi cha đủ điều kiện. Trang chi
   tiết phụ lục: liên kết về hợp đồng cha.

5. Test: tạo hợp lệ → số tháng đúng theo `addon_months`, ngày kết thúc bằng cha;
   cha ở `Draft` → lỗi; ngày bắt đầu sau ngày hết hạn cha → lỗi; chọn gói chính →
   lỗi; phụ lục đi được vòng đời `Generated → Sent → Signed` bằng
   `update_status`; xóa hợp đồng cha khi còn phụ lục → bị từ chối với thông báo
   rõ (thêm kiểm tra này vào `delete_contract`).

**Tiêu chí chấp nhận:**
- [ ] Số tháng của phụ lục không bao giờ vượt thời gian còn lại của hợp đồng cha.
- [ ] Phụ lục dùng chung vòng đời, chứng từ, sinh file với hợp đồng.
- [ ] Không xóa được hợp đồng còn phụ lục.

**Phụ thuộc:** 2.5, 3.2, 4.1. **Cỡ:** L — tách (a) service + test, (b) route + giao diện.

---

### Task 4.3: Gia hạn

**File:** `app/services/addendum_service.py`, `app/routers/contracts.py`,
`templates/contracts/addendum_form.html`, `tests/integration/test_addendum.py`

**Các bước:**

1. `addendum_defaults(kind="renewal")`:
   - `start_date = parent.end_date + 1 ngày`; số tháng mặc định = của gói.
   - Dòng gợi ý: từng dòng của cha; sản phẩm đã ngừng bán thì đổi sang
     `replaced_by_id` (đi theo chuỗi thay thế tối đa 5 bước, chống vòng lặp);
     không có gói thay thế → dòng trống, người dùng phải chọn.
   - Giá lấy từ **danh mục hiện tại**, không chép giá cũ.
   - Trả thêm `comparison`: tổng cũ theo tháng, tổng mới theo tháng, chênh lệch.
2. `create_addendum(kind="renewal")`: `doc_kind = "addendum_renewal"`, dòng đầu
   phải là gói chính (`require_main=True`).
3. Form hiện bảng so sánh giá cũ / mới ngay trên nút lưu.
4. Test: cha `legacy` một dòng, sản phẩm có `replaced_by_id` → gợi ý gói mới với
   giá mới; không có gói thay thế → dòng trống và `create_addendum` từ chối khi
   chưa chọn; ngày bắt đầu đúng; `comparison` đúng số.

**Phụ thuộc:** 4.2. **Cỡ:** M.

---

### Task 4.4: Báo cáo hết hạn hiểu hợp đồng đã được gia hạn

**File:** `app/services/dashboard_service.py`, `app/services/lifecycle.py`,
`tests/integration/test_expiry_reporting.py`

**Các bước:**

1. Trong `lifecycle.py` thêm hằng
   `RENEWAL_COUNTS_STATUSES` = các trạng thái của bản gia hạn được coi là "đã xử
   lý" (từ `Signed` trở đi; dựng từ các hằng đang có, không liệt kê tay).
2. Trong `dashboard_service`, mọi truy vấn "sắp hết hạn" loại hợp đồng có ít nhất
   một bản con `doc_kind == "addendum_renewal"` ở trạng thái trên. Viết điều
   kiện này **một lần** thành hàm `_not_renewed_filter()` rồi dùng lại.
3. Kiểm `get_stats`/các tổng doanh số: phụ lục là một dòng `contracts` nên đã
   được cộng một lần. Viết test khẳng định điều đó (cha 10 triệu + phụ lục 2
   triệu → tổng 12 triệu, không phải 14 hay 10).
4. Test: hợp đồng còn 10 ngày, có bản gia hạn `Signed` → không nằm trong danh
   sách sắp hết hạn; bản gia hạn còn `Draft` → vẫn nằm trong danh sách.

**Phụ thuộc:** 4.3. **Cỡ:** S.

---

### Task 4.5: Sinh văn bản kèm theo từ một mẫu bất kỳ

**Mô tả:** Biên bản bàn giao và các văn bản phụ được sinh theo yêu cầu, giống
cách biểu 08a đang làm, nhưng cho mọi mẫu.

**File:** `app/routers/contracts.py`, tạo `app/services/attachment_service.py`,
`templates/contracts/detail.html`, tạo `tests/integration/test_attachments.py`

**Các bước:**

1. `attachment_service.render_attachment(db, contract_id, template_id) -> tuple[bytes, str]`:
   nạp hợp đồng, khách, mẫu; `build_render_context` + `build_render_lists` +
   `render_docx(..., lists=lists)`; tên file bằng `make_output_filename` với
   `contract_type` của mẫu. Không ghi file, không đổi trạng thái hợp đồng.
2. Route `GET /contracts/{contract_id}/attachment?template_id=` trả file tải về.
   Chỉ nhận mẫu loại "Nghiệm thu", "Bàn giao", "Thanh lý", "Khác"; loại khác → 400.
   Xem `download_form_08a` để chép cách trả `Content-Disposition`.
3. Trang chi tiết: dropdown các mẫu hợp lệ + nút "Sinh văn bản".
4. Test: sinh được; mẫu loại "Hợp đồng" → 400; trạng thái và `output_file_path`
   của hợp đồng không đổi sau khi gọi.

**Phụ thuộc:** 3.2. **Cỡ:** S.

---

### Điểm dừng 4

- [ ] Toàn bộ test xanh; scanner sạch.
- [ ] Trên bản sao: một hợp đồng đang phục vụ → tạo phụ lục bổ sung giữa kỳ →
      sinh file; một hợp đồng cũ sắp hết hạn → gia hạn sang gói mới → sinh file.
- [ ] Duyệt nghiệp vụ.

---

## Đợt 5 — Làm theo lô, báo giá, báo cáo

### Task 5.1: Sinh hàng loạt theo dòng

**File:** `app/services/batch_generator.py`, `app/routers/batch.py`,
`templates/batch/form.html`, `templates/batch/preview.html`,
thêm test vào `tests/integration/test_multi_item_contract.py`

**Các bước:**

1. Form: chọn gói chính + tích các gói bổ sung áp chung cho cả lô (dùng lại
   `product-options`). Bỏ ô `unit_price_vat`.
2. `shared_params["items"]`: danh sách cùng cấu trúc `items_json` nhưng **không
   có** `quantity`. Với từng khách, số lượng tính bằng luật `default_quantity`
   của Task 2.3; cột ghi đè số lượng ở trang xem trước giữ nguyên.
3. `_build_contract_from_params`: khi có `items` → gọi `_build_lines` +
   `sum_lines`, điền hợp đồng và các `ContractItem` đúng như `create_draft`.
   Để không có hai bản sao logic, tách phần "điền `Contract` từ `lines`" của
   `create_draft` thành `contract_service.apply_lines(contract, lines, totals)`
   và gọi từ cả hai nơi.
4. Khách có gói tính theo hồ sơ mà `record_count` rỗng → dòng đó báo lỗi rõ ở
   trang xem trước, không chặn các khách khác.
5. File Excel của batch (`make_batch_template_xlsx`, `import_excel_mapping`):
   thêm cột `main_code`, `addon_codes` (các mã cách nhau dấu `;`).
6. Test: lô 3 khách (số cơ sở khác nhau) → 3 hợp đồng `items`, tổng đúng theo
   từng khách; một khách thiếu `record_count` → 2 thành công, 1 lỗi có thông báo.

**Phụ thuộc:** 2.5. **Cỡ:** L — tách (a) service + `apply_lines`, (b) form + Excel.

---

### Task 5.2: Báo giá nhiều dòng

**File:** `app/routers/quotation.py`, `templates/quotation/form.html`,
`tests/integration/test_generic_quotation.py`

**Các bước:**

1. Form: gói chính + gói bổ sung, dùng `contract_items.js`.
2. `_build_context` nhận `lines`, `totals`: thêm `TOTAL_PRE_VAT`, `TOTAL_VAT`;
   `TOTAL_AMOUNT`, `AMOUNT_TEXT`, `PRICING_DESCRIPTION` lấy từ `totals` /
   `pricing.describe`. Các khóa đơn (`UNIT_PRICE_PRE_VAT`, `VAT_RATE`,
   `UNIT_PRICE_VAT`, `QUANTITY`) lấy từ dòng gói chính để mẫu báo giá cũ vẫn dùng được.
3. `render_docx(..., lists={"items": ..., "features": ...})`.
4. `product_price_with_vat` giữ nguyên chữ ký và kết quả: test hiện có
   (`440_909` thuế 8 → `476_182`) phải vẫn xanh.
5. Test mới: báo giá 2 dòng với mẫu có bảng `ITEM_*` → file có 2 dòng dữ liệu,
   tổng đúng.

**Phụ thuộc:** 3.2. **Cỡ:** M.

---

### Task 5.3: Xuất Excel có chi tiết dòng

**File:** `app/routers/contracts.py` (`export_contracts`), test mới trong
`tests/integration/test_multi_item_contract.py`

**Các bước:** thêm cột "Trước thuế", "Thuế", "Loại văn bản", "Hợp đồng gốc" vào
sheet hiện có; thêm sheet "Chi tiết dòng" (số hợp đồng, TT, dịch vụ, ĐVT, số
lượng, số kỳ, đơn giá, ba cột tiền). Không đổi thứ tự các cột đang có. Test mở
file kết quả bằng `openpyxl` và kiểm số dòng của sheet mới.

**Phụ thuộc:** 2.1. **Cỡ:** S.

---

### Task 5.4: Dọn công thức cũ

**Mô tả:** Sau 5.1 và 5.2, kiểm kê các công thức tiền còn lại.

**Các bước:**

1. Tìm trong `ContractForge/` các mẫu: `* (1 +`, `/ 100`, `unit_price_vat *`,
   `recalcPrice`, `priceOfOption`.
2. Được phép còn lại, kèm chú thích `# legacy:` giải thích:
   - nhánh không có `items_json` của `create_draft`;
   - `_auto_pricing_description` cho hợp đồng `legacy`;
   - `quotation.product_price_with_vat`;
   - `seed._backfill_contract_items`.
3. Mọi chỗ khác: xóa hoặc chuyển sang gọi `pricing`.
4. Thêm test canh gác `tests/unit/test_pricing_single_source.py`: đọc mọi file
   `.html` trong `templates/` và khẳng định không chứa `recalcPrice` hay
   `priceOfOption`.

**Phụ thuộc:** 5.1, 5.2. **Cỡ:** S.

---

## Đợt 6 — Phát hành

### Task 6.1: Tài liệu, phiên bản, bản dựng

**File:** `HUONG_DAN.md`, `ContractForge/HUONG_DAN.md`, `CHANGELOG.md`,
`README.md`, `ContractForge/app/version.py`, `pyproject.toml`

**Các bước:**

1. `HUONG_DAN.md`: thêm mục quản lý danh mục, nhập Excel, lập hợp đồng nhiều
   dòng, phụ lục, bảng placeholder mới và cách đặt dòng lặp trong mẫu Word.
2. `CHANGELOG.md`: mục phiên bản mới, viết theo hành vi người dùng thấy.
3. Tăng phiên bản ở **cả** `app/version.py` và `pyproject.toml`
   (`tests/unit/test_version_consistency.py` canh việc này).
4. Chạy `build_exe.bat`, rồi làm hết `tests/smoke/windows_release_checklist.md`
   trên bản sao dữ liệu thật.

**Phụ thuộc:** tất cả. **Cỡ:** S.

---

## 9. Kiểm chứng trên dữ liệu thật

Làm sau Task 2.1 và trước mỗi lần chạy bản mới trên máy thật. Mọi bước chạy trên
**bản sao**; DB gốc không được mở để ghi.

1. Ghi hash DB gốc:

```bash
python -c "import hashlib,sys;print(hashlib.sha256(open('data/contract_manager.db','rb').read()).hexdigest())"
```

2. Sao DB sang thư mục tạm bằng `sqlite3.Connection.backup` (không cần tắt app
   đang chạy). Sao cả `uploads/`.
3. Trước khi migrate, xuất từ bản sao ra CSV: `id, contract_number, status,
   total_amount, amount_text, unit_price_pre_vat, vat_rate, unit_price_vat,
   end_date` của mọi hợp đồng.
4. Chạy app với `CONTRACTFORGE_DATA_ROOT` trỏ vào thư mục bản sao, chờ khởi động
   xong, tắt.
5. Xuất lại CSV như bước 3 và so hai file: phải giống hệt từng byte.
6. Kiểm bằng SQL trên bản sao:

```sql
SELECT COUNT(*) FROM contracts c
WHERE (SELECT COUNT(*) FROM contract_items i WHERE i.contract_id = c.id) <> 1;
-- phải là 0

SELECT COUNT(*) FROM contracts c JOIN contract_items i ON i.contract_id = c.id
WHERE COALESCE(c.total_amount, 0) <> i.amount_total
   OR i.amount_pre_vat + i.vat_amount <> i.amount_total;
-- phải là 0
```

7. Mở app trên bản sao, sinh lại 3 hợp đồng cũ thuộc các sản phẩm khác nhau, so
   nội dung với file đã phát hành.
8. Tính lại hash DB gốc: phải bằng bước 1.

Viết các bước 3, 5, 6 thành `scripts/verify_items_migration.py <thư mục bản sao>`
trong Task 2.1 để lặp lại được. Script chỉ đọc, không chứa tên hay số của tổ chức nào.

---

## 10. Rủi ro

| Rủi ro | Mức | Cách giảm |
|---|---|---|
| Migration làm đổi tiền hợp đồng cũ | Cao | Task 2.1 không gán `total_amount`; test so trước/sau; mục 9; tự sao lưu trước khi ghi |
| Form và server tính lệch nhau | Cao | Form không tính, chỉ gọi `price-preview`; test khẳng định xem trước = lưu thật |
| Mẫu Word thật đặt sai dòng lặp | Trung bình | Analyzer cảnh báo (3.3); in thử ở Điểm dừng 3 |
| Câu hỏi nghiệp vụ trả lời muộn | Trung bình | Đợt 1–3 không phụ thuộc; Đợt 4 chặn rõ ràng |
| Dữ liệu tổ chức lọt vào repo | Cao | Chạy scanner mỗi commit; ví dụ luôn hư cấu; mẫu thật chỉ ở `data/` |
| Số kiểu "1.000.000" và "82.5" bị đọc nhầm | Trung bình | Mục 0.5; `to_decimal` cho dữ liệu máy; test `"82.5"` ở 1.1 và 2.2 |
| Gói kỳ 12 tháng mua thiếu kỳ (ví dụ 6 tháng) | Thấp | Hiện chia tỷ lệ (`period_count = 0,5`); chốt với nghiệp vụ, nếu phải làm tròn lên thì sửa một chỗ trong `period_count` |
| Task giao diện 2.5 phình to | Trung bình | Đã có điểm tách (a)/(b); không gộp với 2.4 |

## 11. Câu hỏi còn mở

1. Bán kèm hai sản phẩm: gói bổ sung trong cùng hợp đồng, hay hai hợp đồng riêng?
2. Gói cao nhất đã gồm bản cơ bản của sản phẩm kèm theo: có bán thêm gói kèm không?
3. Gia hạn bằng phụ lục hay hợp đồng mới? (Task 4.3 đang làm theo phụ lục.)
4. Khuôn số hợp đồng và số phụ lục theo biểu mẫu mới.
5. Cột "Số lượng" trong bảng giá trị là số cơ sở hay số cơ sở × số tháng?
6. Gói tính theo kỳ 12 tháng khi mua thiếu kỳ: chia tỷ lệ hay tính tròn kỳ?
7. Có theo dõi phí bán hàng trong app không? (Chưa nằm trong kế hoạch.)
