"""product_service.py — logic của danh mục sản phẩm / gói cước.

Router chỉ gọi vào đây. Mọi quy tắc kiểm dữ liệu nằm trong `clean_form`.
Giá do người gõ đọc bằng `contract_service._safe_decimal` (kiểu "500.000");
thuế suất và các ô số nguyên đọc bằng `Decimal(str(v))` / `int`, vì người dùng
gõ "8.5" nghĩa là 8,5%.
"""

from __future__ import annotations

from decimal import Decimal, InvalidOperation
from typing import Optional

from app.models.contract_template import ContractTemplate
from app.models.product import PRODUCT_KINDS, QUANTITY_SOURCES, Product
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

    price_raw = _safe_decimal(data.get("default_price"))
    if price_raw is not None and price_raw < 0:
        errors.append("Đơn giá không được âm.")
    out["default_price"] = price_raw

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

    template_id = _optional_int(data, "default_template_id", errors, "Mẫu hợp đồng")
    if template_id is not None and db.get(ContractTemplate, template_id) is None:
        errors.append("Mẫu hợp đồng mặc định không tồn tại.")
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

    if errors:
        raise ProductError(errors)
    return out


def create_product(db, data: dict) -> Product:
    cleaned = clean_form(db, data)
    product = Product(**cleaned, is_active=True)
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
    }
