"""product_service.py — logic của danh mục sản phẩm / gói cước.

Router chỉ gọi vào đây. Mọi quy tắc kiểm dữ liệu nằm trong `clean_form`.

Cách đọc số:
- Ô giá do người gõ trên form: `contract_service._safe_decimal` (kiểu "500.000").
- Ô giá trong Excel: số máy, ép bằng `Decimal(str(v))`. KHÔNG đi qua
  `_safe_decimal`, vì "82.5" sẽ thành 825.
- Thuế suất và các ô số nguyên: `Decimal(str(v))` / `int`, vì người dùng gõ
  "8.5" nghĩa là 8,5%.
"""

from __future__ import annotations

import io
from decimal import Decimal, InvalidOperation
from typing import Optional

from sqlalchemy import func

from app.models.contract import Contract
from app.models.contract_template import ContractTemplate
from app.models.product import PRODUCT_KINDS, QUANTITY_SOURCES, Product
from app.services import lifecycle
from app.services.contract_service import _safe_decimal
from app.services.numbering import InvalidNumberFormat, validate_format

KIND_LABELS = {
    "main": "Gói chính",
    "addon": "Gói bổ sung",
    "storage": "Gói lưu trữ",
}

QUANTITY_SOURCE_LABELS = {
    "units": "Theo số cơ sở đã tích",
    "records": "Theo số hồ sơ của khách",
    "manual": "Nhập tay khi lập hợp đồng",
}

#: Thứ tự cột của file Excel danh mục (Task 1.5). Dòng 1 là tiêu đề.
IMPORT_COLUMNS = [
    "code", "name", "product_type", "group_code", "kind", "unit_label",
    "default_price", "default_vat_rate", "price_period_months", "quantity_source",
    "min_quantity", "quantity_step", "account_limit", "default_duration_months",
    "contract_number_format", "is_active", "sort_order",
]

#: Nhập Excel không bao giờ ghi các cột này khi cập nhật sản phẩm đã có.
IMPORT_PRESERVED = frozenset({"default_template_id", "replaced_by_id", "feature_list"})

_TRUE_WORDS = {"1", "true", "yes", "y", "có", "x"}
_FALSE_WORDS = {"0", "false", "no", "n", "không"}


class ProductError(ValueError):
    """Dữ liệu sản phẩm không hợp lệ. `errors` chứa mọi lỗi tìm được."""

    def __init__(self, errors):
        self.errors = list(errors)
        super().__init__("; ".join(self.errors))


def _text(data: dict, key: str) -> str:
    return str(data.get(key) or "").strip()


def _optional_int(data: dict, key: str, errors: list, label: str) -> Optional[int]:
    raw = _text(data, key)
    if raw == "":
        return None
    try:
        return int(raw)
    except ValueError:
        errors.append(f"{label} phải là số nguyên.")
        return None


def _parse_bool(raw) -> Optional[bool]:
    """True/False, hoặc None nếu không hiểu. Ô trống = True (đang bán)."""
    if isinstance(raw, bool):
        return raw
    word = "" if raw is None else str(raw).strip().lower()
    if word == "":
        return True
    if word in _TRUE_WORDS:
        return True
    if word in _FALSE_WORDS:
        return False
    return None


def list_products(db, include_inactive: bool = True) -> list[Product]:
    """Sắp theo nhóm (không nhóm xuống cuối), gói chính trước, thứ tự, tên."""
    query = db.query(Product)
    if not include_inactive:
        query = query.filter(Product.is_active == True)  # noqa: E712
    products = query.all()
    return sorted(products, key=lambda p: (
        p.group_code is None,
        p.group_code or "",
        0 if p.kind == "main" else 1,
        p.sort_order or 0,
        p.name or "",
    ))


def get_product(db, product_id: int) -> Optional[Product]:
    return db.get(Product, product_id)


def clean_form(db, data: dict, product_id: Optional[int] = None) -> dict:
    """Kiểm và ép kiểu dữ liệu form. Trả dict các cột của Product.

    Giá trị `Decimal` được nhận nguyên, không đọc lại (dùng cho Excel và cho dữ
    liệu đã làm sạch gửi lại). Ô `is_active` chỉ có trong kết quả khi `data` có khóa đó.

    Ném ProductError chứa toàn bộ lỗi tìm được (thông báo tiếng Việt).
    """
    errors: list[str] = []
    out: dict = {}

    code = _text(data, "code").upper()
    if not code:
        errors.append("Mã sản phẩm không được để trống.")
    else:
        clash = db.query(Product).filter(Product.code == code)
        if product_id is not None:
            clash = clash.filter(Product.id != product_id)
        if clash.first() is not None:
            errors.append(f"Mã {code} đã tồn tại.")
    out["code"] = code

    name = _text(data, "name")
    if not name:
        errors.append("Tên sản phẩm không được để trống.")
    out["name"] = name

    product_type = _text(data, "product_type")
    if not product_type:
        errors.append("Loại sản phẩm không được để trống.")
    out["product_type"] = product_type

    kind = _text(data, "kind") or "main"
    if kind not in PRODUCT_KINDS:
        errors.append("Kiểu sản phẩm không hợp lệ.")
    out["kind"] = kind

    group_code = _text(data, "group_code") or None
    if kind != "main" and group_code is None:
        errors.append("Gói bổ sung phải thuộc một nhóm để ghép với gói chính.")
    out["group_code"] = group_code

    out["unit_label"] = _text(data, "unit_label") or None

    raw_price = data.get("default_price")
    if isinstance(raw_price, Decimal):
        price = raw_price
    else:
        price = _safe_decimal(raw_price)
    if price is not None and price < 0:
        errors.append("Đơn giá không được âm.")
    out["default_price"] = price

    vat_raw = _text(data, "default_vat_rate")
    if vat_raw == "":
        vat = Decimal("0")
    else:
        try:
            vat = Decimal(vat_raw)
        except InvalidOperation:
            vat = Decimal("-1")
            errors.append("Thuế suất phải là số, ví dụ 8 hoặc 8.5.")
        if not (0 <= vat <= 100):
            errors.append("Thuế suất phải trong khoảng 0 đến 100.")
    out["default_vat_rate"] = vat

    period = _optional_int(data, "price_period_months", errors, "Kỳ giá")
    if period is None:
        period = 1
    if period < 1:
        errors.append("Kỳ giá phải từ 1 tháng trở lên.")
    out["price_period_months"] = period

    source = _text(data, "quantity_source") or "units"
    if source not in QUANTITY_SOURCES:
        errors.append("Nguồn số lượng không hợp lệ.")
    out["quantity_source"] = source

    min_qty = _optional_int(data, "min_quantity", errors, "Số lượng tối thiểu")
    if min_qty is not None and min_qty < 0:
        errors.append("Số lượng tối thiểu không được âm.")
    out["min_quantity"] = min_qty

    step = _optional_int(data, "quantity_step", errors, "Bước làm tròn")
    if step is not None and step < 1:
        errors.append("Bước làm tròn phải từ 1 trở lên.")
    out["quantity_step"] = step

    account_limit = _optional_int(data, "account_limit", errors, "Số tài khoản")
    if account_limit is not None and account_limit < 0:
        errors.append("Số tài khoản không được âm.")
    out["account_limit"] = account_limit

    duration = _optional_int(data, "default_duration_months", errors, "Thời hạn mặc định")
    if duration is not None and duration < 1:
        errors.append("Thời hạn mặc định phải từ 1 tháng trở lên.")
    out["default_duration_months"] = duration

    if "feature_text" in data:  # chỉ form gửi ô này; nhập Excel giữ nguyên feature_list
        out["feature_list"] = parse_feature_text(str(data.get("feature_text") or ""))

    template_id = _optional_int(data, "default_template_id", errors, "Mẫu hợp đồng")
    template_row = db.get(ContractTemplate, template_id) if template_id is not None else None
    if template_id is not None and template_row is None:
        errors.append("Mẫu hợp đồng mặc định không tồn tại.")
    if template_row is not None and kind == "main":
        from app.services.template_service import ROLE_CONTRACT, TemplateMismatch, check_template_for
        try:
            check_template_for(template_row, group_code, ROLE_CONTRACT)
        except TemplateMismatch as exc:
            errors.append(str(exc))
    out["default_template_id"] = template_id

    fmt = _text(data, "contract_number_format")
    if fmt:
        try:
            validate_format(fmt)
        except InvalidNumberFormat as exc:
            errors.append(str(exc))
    out["contract_number_format"] = fmt or None

    replaced_by = _optional_int(data, "replaced_by_id", errors, "Gói thay thế")
    if replaced_by is not None:
        if product_id is not None and replaced_by == product_id:
            errors.append("Gói thay thế không được là chính sản phẩm này.")
        elif db.get(Product, replaced_by) is None:
            errors.append("Gói thay thế không tồn tại.")
    out["replaced_by_id"] = replaced_by

    sort_order = _optional_int(data, "sort_order", errors, "Thứ tự hiển thị")
    out["sort_order"] = sort_order if sort_order is not None else 0

    if "is_active" in data:
        active = _parse_bool(data.get("is_active"))
        if active is None:
            errors.append("Cột hiển thị (is_active) chỉ nhận 1 hoặc 0.")
        else:
            out["is_active"] = active

    if errors:
        raise ProductError(errors)
    return out


def create_product(db, data: dict) -> Product:
    cleaned = clean_form(db, data)
    product = Product(**{"is_active": True, **cleaned})
    db.add(product)
    db.commit()
    db.refresh(product)
    return product


def update_product(db, product_id: int, data: dict) -> Product:
    product = get_product(db, product_id)
    if product is None:
        raise ProductError(["Không tìm thấy sản phẩm."])
    cleaned = clean_form(db, data, product_id=product_id)
    for field, value in cleaned.items():
        setattr(product, field, value)
    db.commit()
    db.refresh(product)
    return product


def in_service_counts(db) -> dict[int, int]:
    """product_id → số hợp đồng đang trong thời gian phục vụ (một truy vấn)."""
    rows = (
        db.query(Contract.product_id, func.count(Contract.id))
        .filter(Contract.status.in_(lifecycle.IN_SERVICE_STATUSES))
        .group_by(Contract.product_id)
        .all()
    )
    return {pid: n for pid, n in rows if pid is not None}


def count_in_service(db, product_id: int) -> int:
    """Số hợp đồng đang phục vụ của sản phẩm này.

    Tập trạng thái lấy từ lifecycle.IN_SERVICE_STATUSES. Hợp đồng `Signed` chưa
    được tính là đang phục vụ (chưa xuất hóa đơn), theo định nghĩa ở lifecycle.
    """
    return (
        db.query(Contract)
        .filter(Contract.product_id == product_id,
                Contract.status.in_(lifecycle.IN_SERVICE_STATUSES))
        .count()
    )


def toggle_active(db, product_id: int) -> Product:
    """Ngừng bán hoặc bán lại. Không xóa, không đụng hợp đồng đã có."""
    product = get_product(db, product_id)
    if product is None:
        raise ProductError(["Không tìm thấy sản phẩm."])
    product.is_active = not bool(product.is_active)
    db.commit()
    db.refresh(product)
    return product


def parse_feature_text(text: str) -> list[dict]:
    """Mỗi dòng: "Tên chức năng | Mô tả" (mô tả tùy chọn). Dòng trống hoặc không có tên bị bỏ."""
    out: list[dict] = []
    for raw in (text or "").splitlines():
        name, _, desc = raw.partition("|")
        name = name.strip()
        if not name:
            continue
        out.append({"no": len(out) + 1, "name": name, "description": desc.strip()})
    return out


def feature_text(product: Product) -> str:
    """Ngược lại: danh sách chức năng của gói thành văn bản để điền sẵn form."""
    lines = []
    for feat in product.feature_list or []:
        name = str(feat.get("name", "")).strip()
        if not name:
            continue
        desc = str(feat.get("description", "") or "").strip()
        lines.append(f"{name} | {desc}" if desc else name)
    return "\n".join(lines)


def form_values(product: Product) -> dict:
    """Biến một sản phẩm thành dict chuỗi để điền sẵn form sửa."""
    def plain(v):
        if v is None:
            return ""
        if isinstance(v, Decimal):
            return format(v.normalize(), "f")
        return str(v)

    return {
        "code": product.code or "",
        "name": product.name or "",
        "product_type": product.product_type or "",
        "group_code": product.group_code or "",
        "kind": product.kind or "main",
        "unit_label": product.unit_label or "",
        "default_price": plain(product.default_price),
        "default_vat_rate": plain(product.default_vat_rate),
        "price_period_months": plain(product.price_period_months),
        "quantity_source": product.quantity_source or "units",
        "min_quantity": plain(product.min_quantity),
        "quantity_step": plain(product.quantity_step),
        "account_limit": plain(product.account_limit),
        "default_duration_months": plain(product.default_duration_months),
        "default_template_id": plain(product.default_template_id),
        "contract_number_format": product.contract_number_format or "",
        "replaced_by_id": plain(product.replaced_by_id),
        "sort_order": plain(product.sort_order),
        "feature_text": feature_text(product),
    }


# ─── Excel: xuất, nhập ───────────────────────────────────────────────────────

def _cell_text(v) -> str:
    """Ô Excel → chuỗi. Số nguyên không kèm ".0"."""
    if v is None:
        return ""
    if isinstance(v, bool):
        return "1" if v else "0"
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        return str(int(v)) if v.is_integer() else str(Decimal(str(v)))
    if isinstance(v, Decimal):
        return str(int(v)) if v == v.to_integral_value() else str(v)
    return str(v).strip()


def _cell_price(v) -> Optional[Decimal]:
    """Ô giá trong Excel: số máy. Chuỗi phải là số máy ("500000", "82.5")."""
    if v is None or (isinstance(v, str) and v.strip() == ""):
        return None
    if isinstance(v, bool):
        raise ProductError(["Đơn giá không được là giá trị đúng/sai."])
    if isinstance(v, (int, float, Decimal)):
        return Decimal(str(v))
    try:
        return Decimal(str(v).strip())
    except InvalidOperation:
        raise ProductError([f"Đơn giá không phải là số: {v!r}."])


def _excel_number(v):
    """Số để ghi vào ô: số nguyên khi có thể, không thì float."""
    if v is None:
        return None
    if isinstance(v, Decimal):
        return int(v) if v == v.to_integral_value() else float(v)
    return v


def _export_row(p: Product) -> list:
    return [
        p.code, p.name, p.product_type, p.group_code, p.kind, p.unit_label,
        _excel_number(p.default_price), _excel_number(p.default_vat_rate),
        p.price_period_months, p.quantity_source, p.min_quantity, p.quantity_step,
        p.account_limit, p.default_duration_months, p.contract_number_format,
        1 if p.is_active else 0, p.sort_order,
    ]


#: Hai dòng ví dụ hư cấu cho file mẫu.
_TEMPLATE_EXAMPLES = [
    ["EX-MAIN", "Gói chính mẫu", "Dịch vụ", "NHOM-MAU", "main", "cơ sở",
     500000, 10, 1, "units", 2000, 500, None, 12, "{seq}/{slug}/{year}", 1, 0],
    ["EX-ADDON", "Gói bổ sung mẫu", "Dịch vụ", "NHOM-MAU", "addon", "cơ sở",
     150000, 10, 1, "units", None, None, 3, 12, None, 1, 1],
]


def make_catalog_xlsx(db, with_data: bool) -> bytes:
    """File mẫu (tiêu đề + hai dòng ví dụ) hoặc xuất toàn bộ danh mục hiện có."""
    from openpyxl import Workbook

    wb = Workbook()
    try:
        ws = wb.active
        ws.title = "Danh mục"
        ws.append(IMPORT_COLUMNS)
        if with_data:
            for product in list_products(db):
                ws.append(_export_row(product))
        else:
            for example in _TEMPLATE_EXAMPLES:
                ws.append(example)
        buf = io.BytesIO()
        wb.save(buf)
    finally:
        wb.close()
    return buf.getvalue()


def _row_result(row_no, action, code, raw, data, error) -> dict:
    return {
        "row": row_no,
        "action": action,
        "code": code,
        "name": str(raw.get("name", "")),
        "data": data,
        "error": error,
    }


def parse_catalog_xlsx(db, file_bytes: bytes) -> list[dict]:
    """Đọc file, kiểm từng dòng. Không ghi gì vào DB.

    Mỗi phần tử: {"row", "action": create|update|error, "code", "name",
    "data": dict đã qua clean_form hoặc None, "error": chuỗi lỗi hoặc ""}.
    """
    from openpyxl import load_workbook

    try:
        wb = load_workbook(io.BytesIO(file_bytes), read_only=True, data_only=True)
    except Exception:
        raise ProductError(["Không đọc được file Excel (chỉ nhận .xlsx)."])
    try:
        all_rows = [list(r) for r in wb.active.iter_rows(values_only=True)]
    finally:
        wb.close()

    if not all_rows:
        raise ProductError(["File trống."])

    header = [_cell_text(h) for h in all_rows[0]]
    column_at = {name: i for i, name in enumerate(header) if name}
    missing = [c for c in IMPORT_COLUMNS if c not in column_at]
    if missing:
        raise ProductError([f"File thiếu cột: {', '.join(missing)}."])

    results: list[dict] = []
    seen_codes: set[str] = set()
    for row_no, cells in enumerate(all_rows[1:], start=2):
        if all(_cell_text(c) == "" for c in cells):
            continue

        def cell(name):
            i = column_at[name]
            return cells[i] if i < len(cells) else None

        raw = {col: _cell_text(cell(col)) for col in IMPORT_COLUMNS
               if col != "default_price"}
        code = raw["code"].upper()

        error = ""
        try:
            raw["default_price"] = _cell_price(cell("default_price"))
        except ProductError as exc:
            raw["default_price"] = None
            error = "; ".join(exc.errors)
        if not error and code and code in seen_codes:
            error = f"Mã {code} xuất hiện nhiều lần trong file."
        if code:
            seen_codes.add(code)
        if error:
            results.append(_row_result(row_no, "error", code, raw, None, error))
            continue

        existing = db.query(Product).filter(Product.code == code).first() if code else None
        try:
            cleaned = clean_form(db, raw, product_id=existing.id if existing else None)
        except ProductError as exc:
            results.append(_row_result(row_no, "error", code, raw, None,
                                       "; ".join(exc.errors)))
            continue
        action = "update" if existing else "create"
        results.append(_row_result(row_no, action, code, raw, cleaned, ""))
    return results


def apply_catalog_rows(db, rows: list[dict]) -> tuple[int, int]:
    """Ghi các dòng trong MỘT transaction. Trả (số tạo, số cập nhật).

    Nếu có dòng lỗi, hoặc một dòng không còn hợp lệ khi ghi, không ghi gì và ném
    ProductError. Mỗi dòng được kiểm lại bằng clean_form trước khi ghi.
    """
    lines_with_error = [r for r in rows if r.get("action") == "error"]
    if lines_with_error:
        raise ProductError([f"Dòng {r.get('row', '?')}: {r.get('error', '')}"
                            for r in lines_with_error])

    plan = []
    errors = []
    for r in rows:
        data = r.get("data") or {}
        code = str(data.get("code", "")).strip().upper()
        existing = db.query(Product).filter(Product.code == code).first() if code else None
        try:
            cleaned = clean_form(db, data, product_id=existing.id if existing else None)
        except ProductError as exc:
            errors.append(f"Dòng {r.get('row', '?')}: " + "; ".join(exc.errors))
            continue
        plan.append((existing, cleaned))
    if errors:
        raise ProductError(errors)

    created = updated = 0
    try:
        for existing, cleaned in plan:
            if existing is None:
                db.add(Product(**{"is_active": True, **cleaned}))
                created += 1
            else:
                for field, value in cleaned.items():
                    if field in IMPORT_PRESERVED:
                        continue
                    setattr(existing, field, value)
                updated += 1
        db.commit()
    except Exception:
        db.rollback()
        raise
    return created, updated
