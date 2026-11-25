"""
quote_request.py — "Yêu cầu báo giá": văn bản khách hàng gửi đối tác để đối tác TỰ điền
thông tin đơn vị và đơn giá của chính họ rồi đóng dấu.

Ranh giới thiết kế (xem QUOTE_REQUEST_PLAN.md): ứng dụng chỉ chuẩn bị phần YÊU CẦU,
không bao giờ chuẩn bị phần GIÁ. Vì vậy module này cố ý KHÔNG import `pricing`, không
đọc `default_price` / `default_vat_rate`, và dữ liệu nó trả ra không có khóa nào về tiền.
Mẫu muốn hiện giá cũng không có gì để điền: hàng rào nằm ở cấu trúc dữ liệu, không dựa
vào việc ai đó nhớ kiểm tra. Test `test_quote_request.py` khóa điều này.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date
from decimal import Decimal, InvalidOperation
from hashlib import sha256
from pathlib import Path
from typing import Optional

from sqlalchemy.orm import Session

from app.models.contract_template import ContractTemplate
from app.models.customer import Customer
from app.models.product import Product
from app.paths import ASSETS_DIR
from app.services import template_service
from app.services.docx_renderer import _fmt_number
from app.services.template_service import (
    QUOTE_REQUEST_TYPE, ROLE_QUOTE_REQUEST, check_template_for, normalize_group,
)
from app.utils.date_helpers import split_date_parts

#: Chỗ trống in ra khi khách hàng chưa ghi "Kính gửi" — để viết tay sau khi in.
ADDRESSEE_BLANK = "……………………………………………"

MAX_MONTHS = 120


class QuoteRequestError(ValueError):
    """Dữ liệu form không hợp lệ; thông điệp đọc được trên form."""


@dataclass(frozen=True)
class RequestItem:
    """Một hạng mục yêu cầu báo giá. Không có trường tiền."""
    name: str
    unit_label: str
    quantity: Decimal
    period_count: Decimal
    product_id: Optional[int] = None


def _clean(value) -> str:
    return str(value if value is not None else "").strip()


# ─── Dữ liệu cho mẫu ─────────────────────────────────────────────────────────

def build_request_context(
    customer: Customer,
    request_date: date,
    deadline: Optional[date],
    *,
    month_count: int,
    addressee: str = "",
    subject: str = "",
    note: str = "",
) -> dict[str, str]:
    """Ngữ cảnh thay placeholder. Chỉ có thông tin bên mời báo giá và phạm vi yêu cầu."""
    parts = split_date_parts(request_date)
    ctx: dict[str, str] = {
        "PARTY_A_NAME": _clean(customer.legal_name),
        "PARTY_A_ADDRESS": _clean(customer.full_address()),
        "PARTY_A_TAX_CODE": _clean(customer.tax_code),
        "PARTY_A_REPRESENTATIVE": _clean(customer.representative_name),
        "PARTY_A_TITLE": _clean(customer.representative_title),
        "PARTY_A_PHONE": _clean(customer.phone),
        "PARTY_A_EMAIL": _clean(customer.email),
        "PARTY_A_CONTACT_NAME": _clean(customer.contact_name),
        "REQUEST_ISSUER_NAME": _clean(customer.legal_name).upper(),
        "REQUEST_DATE": request_date.strftime("%d/%m/%Y"),
        "REQUEST_DATE_DAY": parts["day"],
        "REQUEST_DATE_MONTH": parts["month"],
        "REQUEST_DATE_YEAR": parts["year"],
        "REQUEST_DEADLINE": deadline.strftime("%d/%m/%Y") if deadline else "",
        "REQUEST_PLACE": _clean(customer.province),
        "REQUEST_ADDRESSEE": _clean(addressee) or ADDRESSEE_BLANK,
        "REQUEST_SUBJECT": _clean(subject),
        "REQUEST_NOTE": _clean(note),
        "UNIT_COUNT": str(customer.total_units),
        "MONTH_COUNT": str(month_count),
        "PRICING_UNITS": "; ".join(customer.unit_names_list()),
    }
    return ctx


def build_request_lists(items: list[RequestItem],
                        feature_lists: list[list[dict]]) -> dict[str, list[dict[str, str]]]:
    """Dòng lặp cho bảng hạng mục và bảng chức năng. Không có ITEM_PRICE / ITEM_AMOUNT_*.

    `feature_lists` đi cùng thứ tự `items`. Chức năng gộp và bỏ trùng theo tên.
    ITEM_QTY = số lượng × số kỳ, cùng quy ước với hợp đồng.
    """
    rows: list[dict[str, str]] = []
    features: list[dict[str, str]] = []
    seen: set[str] = set()
    for no, item in enumerate(items, start=1):
        rows.append({
            "ITEM_NO": str(no),
            "ITEM_NAME": item.name,
            "ITEM_UNIT": item.unit_label,
            "ITEM_QTY": _fmt_number(item.quantity * item.period_count),
        })
    for feat_list in feature_lists:
        for feat in (feat_list or []):
            name = _clean(feat.get("name"))
            if not name or name in seen:
                continue
            seen.add(name)
            features.append({
                "FEATURE_NO": str(len(features) + 1),
                "FEATURE_NAME": name,
                "FEATURE_DESC": _clean(feat.get("description")),
            })
    return {"items": rows, "features": features}


# ─── Chọn hạng mục từ form ───────────────────────────────────────────────────

def _quantity_from(product: Product, raw, customer: Customer) -> Decimal:
    """Số lượng: ô nhập tay nếu có, không thì mặc định theo nguồn số lượng của gói."""
    from app.services.contract_service import _default_quantity

    text = _clean(raw) or _default_quantity(product, customer, customer.total_units)
    if not text:
        raise QuoteRequestError(f"Gói '{product.name}' cần nhập số lượng.")
    try:
        qty = Decimal(text.replace(",", "."))
    except InvalidOperation:
        raise QuoteRequestError(f"Số lượng của '{product.name}' không hợp lệ.")
    if not qty.is_finite():
        raise QuoteRequestError(f"Số lượng của '{product.name}' không hợp lệ.")
    if qty <= 0:
        raise QuoteRequestError(f"Số lượng của '{product.name}' phải lớn hơn 0.")
    return qty


def _period_count(product: Product, month_count: int) -> Decimal:
    period = Decimal(product.price_period_months if product.price_period_months is not None else 1)
    if not period.is_finite() or period <= 0:
        raise QuoteRequestError(f"Kỳ tính số lượng của '{product.name}' không hợp lệ.")
    return Decimal(month_count) / period


def resolve_request_items(
    db: Session,
    customer: Customer,
    main_product_id: int,
    month_count: int,
    *,
    main_name: str = "",
    main_qty: str = "",
    addons: Optional[list[dict]] = None,
) -> tuple[Product, list[RequestItem], list[list[dict]]]:
    """Gói chính + gói bổ sung đã chọn → (gói chính, hạng mục, danh sách chức năng từng dòng).

    `addons`: [{"product_id": int, "name": str, "qty": str}]. Gói bổ sung phải cùng nhóm
    với gói chính. Mọi lỗi dữ liệu là QuoteRequestError.
    """
    if not 1 <= month_count <= MAX_MONTHS:
        raise QuoteRequestError(f"Số tháng phải từ 1 đến {MAX_MONTHS}.")

    main = db.get(Product, main_product_id)
    if main is None or not main.is_active or main.kind != "main":
        raise QuoteRequestError("Sản phẩm chính không tồn tại hoặc đã ngừng sử dụng.")

    products: list[tuple[Product, str, str]] = [(main, main_name, main_qty)]
    seen = {main.id}
    for entry in addons or []:
        product = db.get(Product, entry.get("product_id"))
        if product is None or not product.is_active:
            raise QuoteRequestError("Gói bổ sung không tồn tại hoặc đã ngừng sử dụng.")
        if product.id in seen:
            continue
        if product.kind == "main":
            raise QuoteRequestError(f"'{product.name}' là gói chính, không thêm làm gói bổ sung.")
        if product.group_code is None or product.group_code != main.group_code:
            raise QuoteRequestError(f"Gói bổ sung '{product.name}' phải cùng nhóm với gói chính.")
        seen.add(product.id)
        products.append((product, entry.get("name", ""), entry.get("qty", "")))

    items: list[RequestItem] = []
    feature_lists: list[list[dict]] = []
    for product, name, qty in products:
        items.append(RequestItem(
            name=_clean(name) or product.name,
            unit_label=_clean(product.unit_label),
            quantity=_quantity_from(product, qty, customer),
            period_count=_period_count(product, month_count),
            product_id=product.id,
        ))
        feature_lists.append(product.feature_list)
    return main, items, feature_lists


# ─── Mẫu ─────────────────────────────────────────────────────────────────────

def list_request_templates(db: Session, group) -> list[ContractTemplate]:
    """Mẫu "Yêu cầu báo giá" đang bật, thuộc nhóm `group` hoặc dùng chung."""
    return template_service.templates_for(db, group, ROLE_QUOTE_REQUEST)


def list_request_templates_any_group(db: Session) -> list[ContractTemplate]:
    """Mọi mẫu "Yêu cầu báo giá" đang bật, mọi nhóm. Form lọc theo nhóm của gói chính ở trình duyệt;
    server vẫn kiểm lại bằng `resolve_request_template`."""
    rows = (
        db.query(ContractTemplate)
        .filter(ContractTemplate.is_active == True,  # noqa: E712
                ContractTemplate.contract_type == QUOTE_REQUEST_TYPE)
        .all()
    )
    rows.sort(key=lambda t: (normalize_group(t.product_group) is None, (t.name or "").lower()))
    return rows


def resolve_request_template(db: Session, template_id: int, group) -> ContractTemplate:
    """Mẫu phải tồn tại, đang bật, đúng loại và đúng nhóm; sai thì QuoteRequestError."""
    template = db.get(ContractTemplate, template_id)
    if template is None or not template.is_active:
        raise QuoteRequestError("Mẫu không tồn tại hoặc đã ngừng sử dụng.")
    try:
        check_template_for(template, normalize_group(group), ROLE_QUOTE_REQUEST)
    except template_service.TemplateMismatch as exc:
        # Chặn ở server, không chỉ ở dropdown: form cũ trong tab khác, nút Back hay mẫu
        # bị đổi nhãn sau khi trang đã tải đều gửi lên được một template_id sai loại.
        raise QuoteRequestError(str(exc))
    if not template.file_path:
        raise QuoteRequestError("Mẫu đã chọn chưa có file DOCX.")
    return template


#: Mẫu hư cấu đi kèm mã nguồn (scripts/make_sample_templates.py). Không tự nhập vào danh mục:
#: người dùng bấm "Cài mẫu có sẵn" thì mới nhập, để không nhét mẫu lạ vào danh sách của họ.
SAMPLE_REQUEST_TEMPLATES = (
    ("sample_request_letter.docx", "SAMPLE-RFQ-LETTER", "Thư mời báo giá (mẫu)",
     "Thư mời chính thức: quốc hiệu, kính gửi, bảng hạng mục và khối thông tin đối tác."),
    ("sample_request_form.docx", "SAMPLE-RFQ-FORM", "Phiếu khảo sát giá gọn (mẫu)",
     "Một trang: bảng hạng mục và khối thông tin đối tác, hợp để in nhanh."),
    ("sample_request_detailed.docx", "SAMPLE-RFQ-DETAILED", "Yêu cầu báo giá kèm yêu cầu chức năng (mẫu)",
     "Thư mời kèm bảng yêu cầu chức năng, điều kiện báo giá và khối thông tin đối tác."),
)


def install_sample_templates(db: Session) -> list[ContractTemplate]:
    """Cài mẫu mới, thêm bản riêng khi bố cục đổi; không ghi đè mẫu đã có."""
    folder = ASSETS_DIR / "default_templates"
    created: list[ContractTemplate] = []
    for filename, code, name, description in SAMPLE_REQUEST_TEMPLATES:
        path = folder / filename
        if not path.is_file():
            raise QuoteRequestError(f"Thiếu file mẫu đi kèm: {filename}.")
        file_bytes = path.read_bytes()
        existing = template_service.get_by_code(db, code)
        if existing is not None:
            try:
                if existing.file_path and Path(existing.file_path).read_bytes() == file_bytes:
                    continue
            except OSError:
                pass
            # Nội dung khác có thể là bản cũ hoặc mẫu người dùng đã sửa.
            # Cả hai trường hợp đều phải giữ nguyên, cài bản mới dưới mã riêng.
            code = f"{code}-{sha256(file_bytes).hexdigest()[:8].upper()}"
            if template_service.get_by_code(db, code) is not None:
                continue
            name = f"{name} — bố cục mới"
        template, _ = template_service.create_template(
            db=db, code=code, name=name, contract_type=QUOTE_REQUEST_TYPE,
            file_bytes=file_bytes, original_filename=filename, description=description,
        )
        created.append(template)
    return created
