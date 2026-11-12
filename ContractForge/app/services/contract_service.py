"""
contract_service.py — Business logic cho module Hợp đồng.

Chức năng:
  - list_contracts: lọc + phân trang
  - get_contract: tra cứu
  - create_draft: tạo bản nháp từ form
  - generate_docx: render file .docx từ bản nháp, lưu file, cập nhật status
  - update_status: chuyển trạng thái
  - list_for_customer: danh sách theo KH
  - build_render_context_from_form: xây context từ form data (không cần object đầy đủ)
  - get_stats: số liệu cho dashboard
"""

import json
import os
import re
import unicodedata
from datetime import date, datetime
from decimal import Decimal, InvalidOperation
from typing import Optional

from sqlalchemy.orm import Session
from sqlalchemy import or_

from app.models.contract import Contract, ContractStatus
from app.models.contract_event import ContractEvent
from app.models.contract_item import ContractItem
from app.models.customer import Customer
from app.models.contract_template import ContractTemplate
from app.models.product import Product
from app.services import lifecycle, numbering, pricing
from app.services.docx_renderer import render_docx, build_render_context, build_render_lists
from app.utils.amount_to_words_vi import amount_to_words, amount_to_words_chan
from app.utils.date_helpers import (
    parse_date_input, compute_end_date, months_between, format_date_vn
)
from app.utils.file_naming import make_output_filename

from app.paths import OUTPUT_CONTRACTS_DIR

OUTPUT_DIR = str(OUTPUT_CONTRACTS_DIR)


# ─── CRUD helpers ────────────────────────────────────────────────────────────

def get_contract(db: Session, contract_id: int) -> Optional[Contract]:
    return db.query(Contract).filter(Contract.id == contract_id).first()


def get_by_number(db: Session, contract_number: str) -> Optional[Contract]:
    return db.query(Contract).filter(
        Contract.contract_number == contract_number
    ).first()


def make_contract_slug(name: str) -> str:
    """
    Chuyển tên khách hàng → slug hợp đồng: viết hoa, không dấu, không khoảng trắng.
    VD: "Công ty TNHH An Bình"     → "CTYTNHHANBINH"
        "Trạm Y tế xã Sông Xanh"   → "TRAMYTEXASONGXANH"
    """
    # Rút gọn tên đơn vị y tế thành TYT
    unit_replacements = [
        (r'\btrạm\s+y\s+tế\b',        'TYT'),
        (r'\btrung\s+tâm\s+y\s+tế\b', 'TYT'),
        (r'\bphòng\s+y\s+tế\b',        'TYT'),
    ]
    cleaned = name
    for pattern, replacement in unit_replacements:
        cleaned = re.sub(pattern, replacement, cleaned, flags=re.IGNORECASE)

    # Bỏ các từ đơn vị hành chính
    admin_words = [
        r'\bxã\b', r'\bphường\b', r'\bthị\s*trấn\b',
        r'\bthị\s*xã\b', r'\bthành\s*phố\b',
        r'\bquận\b', r'\bhuyện\b', r'\btỉnh\b',
    ]
    for pattern in admin_words:
        cleaned = re.sub(pattern, '', cleaned, flags=re.IGNORECASE)

    # Xử lý Đ/đ trước NFD (không decompose được bằng NFD thông thường)
    cleaned = cleaned.replace('Đ', 'D').replace('đ', 'd')

    # Normalize NFD → remove diacritics → ASCII only
    nfkd = unicodedata.normalize('NFD', cleaned)
    ascii_str = ''.join(c for c in nfkd
                        if unicodedata.category(c) != 'Mn' and ord(c) < 128)
    slug = re.sub(r'[^A-Za-z0-9]', '', ascii_str)
    return slug.upper()


def resolve_number_format(db: Session, product_id=None) -> str:
    """Khuôn số hợp đồng áp dụng cho một sản phẩm.

    Khuôn là dữ liệu trên bản ghi sản phẩm, không phải nhánh if trong code —
    xem app/services/numbering.py. Sản phẩm chưa cấu hình thì dùng khuôn mặc
    định trung tính.
    """
    # product_id có thể tới từ form (chuỗi), từ router (int), hoặc thiếu hẳn.
    try:
        pid = int(product_id or 0)
    except (TypeError, ValueError):
        pid = 0

    if pid:
        product = db.query(Product).filter(Product.id == pid).first()
        if product and (product.contract_number_format or "").strip():
            return product.contract_number_format
    return numbering.DEFAULT_FORMAT


def get_next_contract_seq(db: Session, customer_id: int,
                          number_format: str = "",
                          year: int = 0) -> str:
    """
    Tìm số thứ tự nhỏ nhất chưa dùng cho (customer, khuôn số, year).
    - Bộ đếm riêng theo khuôn: cùng KH nhưng khác khuôn thì đếm độc lập.
    - Lấp khoảng trống: nếu 01 đã xóa và 02 còn, kết quả là 01.
    - Sang năm mới reset về 01.
    """
    from datetime import date as _date
    if not year:
        year = _date.today().year

    pattern = numbering.like_pattern(number_format or numbering.DEFAULT_FORMAT, year)

    existing = db.query(Contract.contract_number).filter(
        Contract.customer_id == customer_id,
        Contract.contract_number.like(pattern),
    ).all()

    used = {
        seq for (num,) in existing
        if (seq := numbering.seq_of(num)) is not None
    }
    return numbering.next_seq(used)


def build_contract_number(seq: str, slug: str,
                          number_format: str = "", year: int = 0) -> str:
    """
    Ghép số hợp đồng theo khuôn — nguồn sự thật duy nhất cho form lẻ, batch
    và preview (tránh lặp công thức ở nhiều nơi).
    """
    return numbering.build_number(
        seq, slug, number_format or numbering.DEFAULT_FORMAT, year or date.today().year
    )



_SORT_MAP = {
    "created_at":      Contract.created_at,
    "sign_date":       Contract.sign_date,
    "end_date":        Contract.end_date,
    "total_amount":    Contract.total_amount,
    "contract_number": Contract.contract_number,
    "status":          Contract.status,
}


def _build_contract_query(
    db: Session,
    q: str = "",
    status: str = "",
    customer_id: Optional[int] = None,
    product_id: Optional[int] = None,
    alert_filter: str = "",
    year: int = 0,
    date_from=None,
    date_to=None,
    date_field: str = "sign_date",
):
    """Xây SQLAlchemy query đã áp filters, chưa paginate/sort."""
    from datetime import date as _date, timedelta
    query = db.query(Contract)

    if q:
        kw = f"%{q.strip()}%"
        cust_ids = db.query(Customer.id).filter(
            or_(Customer.legal_name.ilike(kw), Customer.short_name.ilike(kw))
        ).subquery()
        query = query.filter(
            or_(
                Contract.contract_number.ilike(kw),
                Contract.notes.ilike(kw),
                Contract.customer_id.in_(cust_ids),
            )
        )
    if status:
        query = query.filter(Contract.status == status)
    if customer_id:
        query = query.filter(Contract.customer_id == customer_id)
    if product_id:
        query = query.filter(Contract.product_id == product_id)
    if year:
        from sqlalchemy import extract
        query = query.filter(extract("year", Contract.sign_date) == year)
    if date_from or date_to:
        col = Contract.end_date if date_field == "end_date" else Contract.sign_date
        if date_from:
            query = query.filter(col >= date_from)
        if date_to:
            query = query.filter(col <= date_to)
    if alert_filter == "unsent_10d":
        cutoff = _date.today() - timedelta(days=10)
        query = query.filter(
            Contract.status == ContractStatus.Sent,
            Contract.updated_at < cutoff,
        )
    elif alert_filter == "expiring_60":
        today = _date.today()
        d60 = today + timedelta(days=lifecycle.EXPIRING_WARN_DAYS)
        query = query.filter(
            Contract.status.in_(lifecycle.ACTIVE_LIFECYCLE_STATUSES),
            Contract.end_date.isnot(None),
            Contract.end_date >= today,
            Contract.end_date <= d60,
        )
    elif alert_filter == "all_active":
        # Tất cả hợp đồng đang thực sự hiệu lực (không bao gồm Signed chờ kích hoạt)
        query = query.filter(Contract.status.in_(lifecycle.IN_SERVICE_STATUSES))
    return query


def list_contracts(
    db: Session,
    q: str = "",
    status: str = "",
    customer_id: Optional[int] = None,
    product_id: Optional[int] = None,
    alert_filter: str = "",
    year: int = 0,
    date_from=None,
    date_to=None,
    date_field: str = "sign_date",
    page: int = 1,
    per_page: int = 25,
    sort_by: str = "created_at",
    sort_dir: str = "desc",
):
    """Trả về (rows, total) — rows là list Contract."""
    query = _build_contract_query(
        db, q=q, status=status, customer_id=customer_id,
        product_id=product_id, alert_filter=alert_filter,
        year=year, date_from=date_from, date_to=date_to, date_field=date_field,
    )
    total = query.count()
    from sqlalchemy.orm import joinedload
    col = _SORT_MAP.get(sort_by, Contract.created_at)
    order = col.desc() if sort_dir == "desc" else col.asc()
    rows = (
        query.options(joinedload(Contract.customer))
        .order_by(order)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all()
    )
    return rows, total


def sum_contracts(
    db: Session,
    q: str = "",
    status: str = "",
    customer_id: Optional[int] = None,
    product_id: Optional[int] = None,
    alert_filter: str = "",
    year: int = 0,
    date_from=None,
    date_to=None,
    date_field: str = "sign_date",
) -> float:
    """Tổng total_amount của kết quả lọc hiện tại."""
    from sqlalchemy import func
    query = _build_contract_query(
        db, q=q, status=status, customer_id=customer_id,
        product_id=product_id, alert_filter=alert_filter,
        year=year, date_from=date_from, date_to=date_to, date_field=date_field,
    )
    result = query.with_entities(func.sum(Contract.total_amount)).scalar()
    return float(result or 0)


def get_stats(db: Session) -> dict:
    """Thống kê nhanh cho dashboard."""
    all_contracts = db.query(Contract).all()
    today = date.today()

    stats = {s.value: 0 for s in ContractStatus}
    expiring = 0
    for c in all_contracts:
        stats[c.status.value] = stats.get(c.status.value, 0) + 1
        if c.status in lifecycle.ACTIVE_LIFECYCLE_STATUSES and \
                lifecycle.is_expiring_soon(c.end_date, today):
            expiring += 1
    stats["expiring_soon_count"] = expiring
    return stats


# ─── Dòng hợp đồng (items_json) ──────────────────────────────────────────────

def _parse_items_json(raw) -> list[dict]:
    """Trả [] khi rỗng. Ném ValueError khi không phải danh sách các đối tượng."""
    if raw is None or str(raw).strip() == "":
        return []
    try:
        data = json.loads(str(raw))
    except (TypeError, ValueError):
        raise ValueError("Dữ liệu dòng hợp đồng không đọc được.")
    if not isinstance(data, list) or not all(isinstance(x, dict) for x in data):
        raise ValueError("Dữ liệu dòng hợp đồng phải là danh sách các dòng.")
    return data


def _item_decimal(item: dict, key: str, fallback, label: str) -> Decimal:
    """Đọc số máy trong một dòng; nếu thiếu thì lấy giá trị mặc định của gói."""
    raw = item.get(key)
    if raw in (None, ""):
        if fallback is None:
            raise pricing.PricingError(f"Thiếu {label}.")
        raw = fallback
    return pricing.to_decimal(raw, label)


def _build_lines(db: Session, raw_items: list[dict], month_count: int,
                 require_main: bool = True) -> list[pricing.Line]:
    """Đổi dữ liệu items_json thành các dòng đã tính tiền.

    Mọi lỗi dữ liệu đều thành ValueError có số dòng để đọc được trên form.
    `require_main=False` dùng cho phụ lục bổ sung: mọi dòng phải là gói bổ sung.
    """
    lines: list[pricing.Line] = []
    main_group = None
    for idx, item in enumerate(raw_items, start=1):
        raw_pid = item.get("product_id")
        try:
            product = db.get(Product, int(raw_pid)) if raw_pid not in (None, "") else None
        except (TypeError, ValueError):
            product = None
        if product is None:
            raise ValueError(f"Dòng {idx}: gói {raw_pid} không tồn tại.")

        if require_main and idx == 1:
            if product.kind != "main":
                raise ValueError("Dòng 1 phải là gói chính.")
            main_group = product.group_code
        else:
            if product.kind == "main":
                raise ValueError(f"Dòng {idx}: chỉ dòng đầu tiên được là gói chính.")
            if require_main and (product.group_code is None or product.group_code != main_group):
                raise ValueError(f"Dòng {idx}: gói bổ sung phải cùng nhóm với gói chính.")

        try:
            line = pricing.price_line(
                description=str(item.get("description") or "").strip() or product.name,
                unit_label=product.unit_label or "",
                quantity=pricing.normalize_quantity(
                    item.get("quantity"), product.min_quantity, product.quantity_step),
                months=month_count,
                unit_price_pre_vat=_item_decimal(item, "unit_price_pre_vat",
                                                 product.default_price, "đơn giá"),
                vat_rate=_item_decimal(item, "vat_rate", product.default_vat_rate or 0,
                                       "thuế suất"),
                price_period_months=product.price_period_months or 1,
                amount_pre_vat_override=item.get("amount_pre_vat_override") or None,
                product_id=product.id,
                note=str(item.get("note") or "").strip(),
            )
        except pricing.PricingError as exc:
            raise ValueError(f"Dòng {idx}: {exc}") from exc
        lines.append(line)
    return lines


def _payment_mode(raw) -> Optional[str]:
    value = str(raw or "").strip().lower()
    return value if value in ("prepaid", "postpaid") else None


def apply_lines(contract: Contract, lines: list, totals) -> None:
    """Đặt các trường tiền của hợp đồng theo dòng đã tính.

    Dùng chung cho nhập tay (create_draft) và sinh hàng loạt. Không ghi đè số chữ
    hay mô tả đã có sẵn.
    """
    main = lines[0]
    contract.pricing_model = "items"
    contract.product_id = main.product_id
    contract.unit_count = int(main.quantity)
    contract.unit_price_pre_vat = main.unit_price_pre_vat
    contract.vat_rate = main.vat_rate
    contract.unit_price_vat = pricing.unit_price_with_vat(main.unit_price_pre_vat, main.vat_rate)
    contract.quantity = main.quantity * main.period_count
    contract.total_amount = totals.total
    contract.total_pre_vat = totals.pre_vat
    contract.total_vat = totals.vat
    if not (contract.amount_text or "").strip():
        contract.amount_text = totals.amount_text or None
    if not (contract.pricing_description or "").strip():
        contract.pricing_description = pricing.describe(lines, totals)


def add_line_rows(db: Session, contract: Contract, lines: list) -> None:
    """Ghi các dòng bảng giá trị. Gọi sau khi hợp đồng đã có id (sau flush)."""
    for order, line in enumerate(lines):
        db.add(ContractItem(
            contract_id=contract.id, product_id=line.product_id, sort_order=order,
            description=line.description, unit_label=line.unit_label or None,
            quantity=line.quantity, period_count=line.period_count,
            unit_price_pre_vat=line.unit_price_pre_vat, vat_rate=line.vat_rate,
            amount_pre_vat=line.amount_pre_vat, vat_amount=line.vat_amount,
            amount_total=line.amount_total, note=line.note or None,
        ))
    db.flush()


def _dec_str(value) -> str:
    """Số → chuỗi máy, không kèm số 0 thừa: Decimal('500000.00') → '500000'."""
    if value is None:
        return ""
    d = pricing.to_decimal(value)
    return format(d.normalize(), "f")


def _default_quantity(product: Product, customer: Optional[Customer], unit_count: int) -> str:
    """Số lượng mặc định theo nguồn số lượng của gói. Rỗng nếu chưa biết."""
    source = product.quantity_source or "units"
    if source == "units":
        return _dec_str(max(int(unit_count or 1), 1))
    if source == "records":
        if customer is None or not customer.record_count:
            return ""
        qty = pricing.normalize_quantity(customer.record_count,
                                         product.min_quantity, product.quantity_step)
        return _dec_str(qty)
    return "1"


def _product_brief(product: Product, customer: Optional[Customer], unit_count: int) -> dict:
    return {
        "id": product.id,
        "code": product.code,
        "name": product.name,
        "unit_label": product.unit_label or "",
        "price": _dec_str(product.default_price),
        "vat": _dec_str(product.default_vat_rate or 0),
        "price_period_months": product.price_period_months or 1,
        "quantity_source": product.quantity_source or "units",
        "default_quantity": _default_quantity(product, customer, unit_count),
        "template_id": product.default_template_id,
    }


def product_options(db: Session, product_id: int, customer_id: int, unit_count: int) -> dict:
    """Gói chính đã chọn và các gói bổ sung cùng nhóm đang bán.

    Không tìm thấy gói chính thì trả main = None và danh sách rỗng.
    """
    product = db.get(Product, product_id) if product_id else None
    customer = db.get(Customer, customer_id) if customer_id else None
    if product is None:
        return {"main": None, "addons": []}

    addons: list[dict] = []
    if product.group_code:
        rows = (
            db.query(Product)
            .filter(Product.group_code == product.group_code,
                    Product.id != product.id,
                    Product.kind != "main",
                    Product.is_active == True)  # noqa: E712
            .order_by(Product.sort_order, Product.name)
            .all()
        )
        addons = [_product_brief(p, customer, unit_count) for p in rows]

    return {"main": _product_brief(product, customer, unit_count), "addons": addons}


def price_preview(db: Session, items, month_count) -> dict:
    """Tính các dòng và tổng như create_draft sẽ tính, nhưng không ghi gì.

    Dùng chung _build_lines và pricing.sum_lines với create_draft, nên xem trước
    và lưu thật luôn ra cùng một số. Lỗi dữ liệu là ValueError.
    """
    if not isinstance(items, list):
        raise ValueError("Danh sách dòng phải là một mảng.")
    try:
        months = int(month_count)
    except (TypeError, ValueError):
        raise ValueError("Số tháng không hợp lệ.")
    if months < 1:
        raise ValueError("Phải có số tháng để tính tiền.")

    lines = _build_lines(db, items, months)
    totals = pricing.sum_lines(lines)
    return {
        "lines": [
            {
                "description": line.description,
                "unit_label": line.unit_label,
                "quantity": _dec_str(line.quantity),
                "period_count": _dec_str(line.period_count),
                "unit_price_pre_vat": _dec_str(line.unit_price_pre_vat),
                "vat_rate": _dec_str(line.vat_rate),
                "amount_pre_vat": _dec_str(line.amount_pre_vat),
                "vat_amount": _dec_str(line.vat_amount),
                "amount_total": _dec_str(line.amount_total),
            }
            for line in lines
        ],
        "totals": {
            "pre_vat": _dec_str(totals.pre_vat),
            "vat": _dec_str(totals.vat),
            "total": _dec_str(totals.total),
            "amount_text": totals.amount_text,
        },
        "description": pricing.describe(lines, totals),
    }


# ─── Create draft ─────────────────────────────────────────────────────────────

def create_draft(db: Session, form_data: dict, *, require_main: bool = True) -> Contract:
    """
    Tạo bản nháp hợp đồng từ form data.
    Validates + computes derived fields.
    Raises ValueError cho lỗi validation.
    """
    # Validate required
    contract_number = (form_data.get("contract_number") or "").strip()
    if not contract_number:
        raise ValueError("Số hợp đồng không được để trống.")

    customer_id = form_data.get("customer_id")
    if not customer_id:
        raise ValueError("Phải chọn khách hàng.")
    try:
        customer_id = int(customer_id)
    except (ValueError, TypeError):
        raise ValueError("customer_id không hợp lệ.")

    template_id = form_data.get("template_id")
    try:
        template_id = int(template_id) if template_id else None
    except (ValueError, TypeError):
        template_id = None

    product_id = form_data.get("product_id")
    try:
        product_id = int(product_id) if product_id else None
    except (ValueError, TypeError):
        product_id = None

    if get_by_number(db, contract_number):
        raise ValueError(f"Số hợp đồng '{contract_number}' đã tồn tại.")

    # Parse dates
    sign_date        = _safe_parse_date(form_data.get("sign_date"))
    start_date       = _safe_parse_date(form_data.get("start_date"))
    end_date         = _safe_parse_date(form_data.get("end_date"))
    acceptance_date  = _safe_parse_date(form_data.get("acceptance_date"))
    liquidation_date = _safe_parse_date(form_data.get("liquidation_date"))

    # Tính month_count nếu chưa có
    month_count = _safe_int(form_data.get("month_count"))
    if month_count is None and start_date and end_date:
        month_count = months_between(start_date, end_date)
    elif month_count and start_date and not end_date:
        end_date = compute_end_date(start_date, month_count)

    # Giá trị
    unit_count        = _safe_int(form_data.get("unit_count")) or 1
    unit_price_pre_vat = _safe_decimal(form_data.get("unit_price_pre_vat"))
    vat_rate          = _safe_decimal(form_data.get("vat_rate")) or Decimal("0")
    unit_price_vat    = _safe_decimal(form_data.get("unit_price_vat"))
    total_amount      = _safe_decimal(form_data.get("total_amount"))

    # Auto-compute unit_price_vat nếu thiếu
    if unit_price_pre_vat is not None and unit_price_vat is None:
        # legacy: nhánh không có items_json giữ công thức cũ, số đã lưu không được tính lại
        unit_price_vat = unit_price_pre_vat * (1 + vat_rate / 100)

    # Quantity = unit_count × month_count
    quantity = _safe_decimal(form_data.get("quantity"))
    if quantity is None and month_count:
        quantity = Decimal(unit_count) * Decimal(month_count)

    # Total amount auto-compute
    if total_amount is None and unit_price_vat and quantity:
        # legacy: nhánh không có items_json
        total_amount = unit_price_vat * quantity
    elif total_amount is None and unit_price_pre_vat and quantity:
        total_amount = unit_price_pre_vat * quantity

    # Dòng theo gói (items_json). Không có items_json thì bỏ qua khối này hoàn toàn,
    # và hợp đồng đi đúng đường cũ ở trên.
    raw_items = _parse_items_json(form_data.get("items_json"))
    items_lines = None
    items_totals = None
    if raw_items:
        if not month_count:
            raise ValueError("Phải có số tháng để tính tiền.")
        items_lines = _build_lines(db, raw_items, month_count, require_main=require_main)
        items_totals = pricing.sum_lines(items_lines)

    # Số tiền bằng chữ
    amount_text = (form_data.get("amount_text") or "").strip()
    if not amount_text and total_amount:
        try:
            amount_text = amount_to_words(total_amount)
        except Exception:
            amount_text = ""

    contract = Contract(
        contract_number=contract_number,
        status=ContractStatus.Draft,
        customer_id=customer_id,
        product_id=product_id,
        template_id=template_id,
        sign_date=sign_date,
        start_date=start_date,
        end_date=end_date,
        month_count=month_count,
        sign_location=(form_data.get("sign_location") or "").strip() or None,
        unit_count=unit_count,
        quantity=quantity,
        unit_price_pre_vat=unit_price_pre_vat,
        vat_rate=vat_rate,
        unit_price_vat=unit_price_vat,
        total_amount=total_amount,
        amount_text=amount_text or None,
        pricing_description=(form_data.get("pricing_description") or "").strip() or None,
        party_b_representative=(form_data.get("party_b_representative") or "").strip() or None,
        party_b_authorization_number=(form_data.get("party_b_authorization_number") or "").strip() or None,
        party_b_authorization_date=(form_data.get("party_b_authorization_date") or "").strip() or None,
        notes=(form_data.get("notes") or "").strip() or None,
        acceptance_date=acceptance_date,
        liquidation_date=liquidation_date,
    )

    # Cơ sở áp dụng — danh sách tên cơ sở được tích chọn trên form (JSON array)
    selected_names = _parse_selected_units(form_data.get("selected_units"))
    if selected_names:
        contract.selected_unit_names = selected_names

    if items_lines is not None:
        apply_lines(contract, items_lines, items_totals)
        contract.payment_mode = _payment_mode(form_data.get("payment_mode"))
        contract.payment_cycle_months = _safe_int(form_data.get("payment_cycle_months"))

    db.add(contract)
    db.flush()

    if items_lines is not None:
        add_line_rows(db, contract, items_lines)

    # Ghi event
    _add_event(db, contract.id, "created", "Tạo bản nháp hợp đồng", "system")
    db.commit()
    db.refresh(contract)
    return contract


# ─── Generate DOCX ────────────────────────────────────────────────────────────

def generate_docx(db: Session, contract_id: int, attachment_ids=(),
                  bundle: Optional[list] = None) -> Contract:
    """
    Render file .docx từ template + dữ liệu hợp đồng.
    Cập nhật status → Generated, lưu đường dẫn file vào DB.

    `attachment_ids`: các mẫu văn bản kèm theo được chọn cùng lúc. Tất cả được kiểm tra
    và render trong bộ nhớ trước khi ghi file hay đổi trạng thái: một lỗi thì hủy cả lượt.
    `bundle`: nếu truyền list, sẽ nhận (tên file, bytes) của hợp đồng chính rồi các văn bản kèm theo.
    Raises LookupError, FileNotFoundError, ValueError.
    """
    contract = get_contract(db, contract_id)
    if not contract:
        raise LookupError(f"Không tìm thấy hợp đồng id={contract_id}")

    if not contract.template_id:
        raise ValueError("Hợp đồng chưa chọn mẫu hợp đồng (template).")

    template = db.query(ContractTemplate).filter(
        ContractTemplate.id == contract.template_id
    ).first()
    if not template:
        raise ValueError("Mẫu hợp đồng không tồn tại.")
    if not template.file_path or not os.path.exists(template.file_path):
        raise FileNotFoundError(
            f"File template không tìm thấy trên disk: {template.file_path!r}"
        )

    from app.services.attachment_service import resolve_attachment_templates, render_attachments
    attachment_templates = resolve_attachment_templates(db, attachment_ids)

    customer = db.query(Customer).filter(Customer.id == contract.customer_id).first()
    if not customer:
        raise LookupError(f"Khách hàng id={contract.customer_id} không tồn tại.")

    # Luôn reload customer với joinedload để tránh lazy-load partial sau db.expire
    from sqlalchemy.orm import joinedload as _jl
    customer = (
        db.query(Customer)
        .options(_jl(Customer.sub_units))
        .filter(Customer.id == contract.customer_id)
        .first()
    )

    # db.refresh đảm bảo contract object có dữ liệu mới nhất từ DB
    db.refresh(contract)

    # Auto-sinh pricing_description nếu form để trống (tương đương batch_generator)
    # Không commit sớm để tránh expire các object trong session
    auto_pricing_desc = None
    if contract.pricing_model == "legacy" and not (contract.pricing_description or "").strip():
        from app.services.batch_generator import _auto_pricing_description
        params = {
            "unit_price_vat": str(contract.unit_price_vat or ""),
            "unit_count":     str(contract.unit_count or ""),
            "month_count":    str(contract.month_count or ""),
        }
        auto_pricing_desc = _auto_pricing_description(
            customer, params, names=contract.selected_unit_names or None
        ) or None

    # Build context
    ctx = build_render_context(contract, customer, template=template, db=db)
    lists = build_render_lists(contract)

    # Render hết trong bộ nhớ trước khi chạm vào render_data, file hay trạng thái
    try:
        rendered_bytes = render_docx(template.file_path, ctx, lists=lists)
    except Exception as e:
        raise ValueError(f"Lỗi khi render .docx: {e}")
    attachments = render_attachments(db, contract.id, attachment_templates)

    # Snapshot render_data vào contract (kèm dữ liệu dòng lặp để tái tạo được)
    contract.render_data = {**ctx, "_lists": lists}

    # Lưu file output
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    fname = make_output_filename(
        contract_number=contract.contract_number,
        customer_code=customer.code,
        contract_type=template.contract_type,
    )
    fpath = os.path.join(OUTPUT_DIR, fname)
    if bundle is not None:
        bundle.append((fname, rendered_bytes))
        bundle.extend(attachments)

    # Ghi file mới TRƯỚC khi cập nhật DB — nếu write fail thì DB không đổi (safe)
    with open(fpath, "wb") as f:
        f.write(rendered_bytes)

    # Lưu đường dẫn file cũ để xóa sau khi commit thành công
    old_file_path = contract.output_file_path

    contract.output_file_path = fpath
    contract.output_file_name = fname
    contract.status = ContractStatus.Generated
    # Áp dụng auto-generated pricing_description nếu có (không commit sớm trước đó)
    if auto_pricing_desc:
        contract.pricing_description = auto_pricing_desc

    _add_event(db, contract.id, "generated",
               f"Sinh file {fname}", "system")

    try:
        db.commit()
    except Exception as commit_err:
        # Commit fail → xóa file mới vừa ghi để tránh file rác, giữ nguyên trạng thái DB
        try:
            os.remove(fpath)
        except OSError:
            pass
        raise ValueError(f"Lỗi khi lưu thông tin hợp đồng: {commit_err}")

    # Commit thành công → xóa file .docx cũ (nếu khác file mới)
    if old_file_path and old_file_path != fpath and os.path.exists(old_file_path):
        try:
            os.remove(old_file_path)
        except OSError:
            pass  # Không chặn flow nếu xóa file cũ thất bại
    db.refresh(contract)
    return contract


# ─── Status update ────────────────────────────────────────────────────────────

# Luật vòng đời sống ở app/services/lifecycle.py. Tên này được re-export vì
# routers/contracts.py import nó để dựng danh sách "chuyển tiếp được" trên UI.
VALID_TRANSITIONS = lifecycle.VALID_TRANSITIONS


def _apply_status_change(
    db: Session,
    contract: Contract,
    new_status,
    actor: str = "user",
    note: str = "",
) -> Contract:
    """Kiểm tra rồi áp dụng một bước chuyển trạng thái — **không commit**.

    Đây là lối đi duy nhất được phép ghi `Contract.status`. Cả đổi từng hợp đồng
    lẫn đổi hàng loạt đều đi qua đây, nên không thể có đường vòng nào bỏ qua
    kiểm tra hợp lệ, chứng từ bắt buộc, hay ghi lịch sử.

    Người gọi chịu trách nhiệm commit — nhờ vậy thao tác hàng loạt gói được
    nhiều hợp đồng vào một giao dịch duy nhất.
    """
    new_st = lifecycle.validate_transition(contract, new_status)

    old_status = contract.status.value
    contract.status = new_st
    desc = f"Chuyển trạng thái {old_status} → {new_st.value}"
    if note:
        desc += f": {note}"
    _add_event(db, contract.id, "status_changed", desc, actor)
    return contract


def update_status(
    db: Session,
    contract_id: int,
    new_status: str,
    actor: str = "user",
    note: str = "",
) -> Contract:
    contract = get_contract(db, contract_id)
    if not contract:
        raise LookupError(f"Không tìm thấy hợp đồng id={contract_id}")

    _apply_status_change(db, contract, new_status, actor=actor, note=note)
    db.commit()
    db.refresh(contract)
    return contract


# ─── Internal helpers ────────────────────────────────────────────────────────


# Các cột trỏ tới file thuộc riêng một hợp đồng. Xoá hợp đồng thì xoá luôn.
# KHÔNG bao gồm hồ sơ giấy tờ khách hàng (CustomerDocument) — thứ đó thuộc về
# khách hàng, dùng chung cho nhiều hợp đồng, nên phải sống lâu hơn từng hợp đồng.
_CONTRACT_OWNED_FILE_COLUMNS = (
    "output_file_path",     # bản .docx đã sinh
    "signed_pdf_path",      # bản scan hợp đồng đã ký
    "invoice_pdf_path",     # hoá đơn
    "payment_slip_path",    # uỷ nhiệm chi
)


def _delete_owned_files(contract: Contract) -> int:
    """Xoá các file thuộc riêng hợp đồng này khỏi đĩa. Trả về số file đã xoá.

    Best-effort: file khoá hay đã bị xoá tay không được chặn việc xoá bản ghi —
    cùng cách xử lý với file cũ trong `generate_docx()`. Bản ghi DB là nguồn sự
    thật; file mồ côi còn lại chỉ chiếm chỗ, còn bản ghi mồ côi thì làm hỏng
    danh sách.
    """
    removed = 0
    for column in _CONTRACT_OWNED_FILE_COLUMNS:
        path = getattr(contract, column, None)
        if not path:
            continue
        try:
            if os.path.exists(path):
                os.remove(path)
                removed += 1
        except OSError:
            pass
    return removed


def delete_contract(db: Session, contract_id: int) -> bool:
    """Xóa 1 hợp đồng, kèm mọi file thuộc riêng nó. Trả về True nếu thành công."""
    c = db.query(Contract).filter(Contract.id == contract_id).first()
    if not c:
        return False
    _refuse_if_has_addenda([c])
    _delete_owned_files(c)
    db.delete(c)
    db.commit()
    return True


def _refuse_if_has_addenda(contracts) -> None:
    """Không xóa hợp đồng gốc khi còn phụ lục: nói rõ phải xóa phụ lục trước."""
    for c in contracts:
        n = len(c.addenda or [])
        if n:
            raise ValueError(
                f"Không xóa được hợp đồng {c.contract_number}: còn {n} phụ lục. "
                "Hãy xóa các phụ lục trước."
            )


def bulk_delete(db: Session, ids: list[int]) -> int:
    """Xóa nhiều hợp đồng kèm file của chúng. Trả về số bản ghi đã xóa."""
    contracts = db.query(Contract).filter(Contract.id.in_(ids)).all() if ids else []
    _refuse_if_has_addenda(contracts)  # kiểm hết trước khi xóa cái nào
    for c in contracts:
        _delete_owned_files(c)
        db.delete(c)
    db.commit()
    return len(contracts)


def bulk_update_status(db: Session, ids: list[int], new_status: str,
                       actor: str = "user") -> int:
    """Cập nhật trạng thái nhiều hợp đồng. Trả về số bản ghi đã cập nhật.

    Áp dụng **đúng** bộ luật của `update_status()` — trước đây hàm này gán thẳng
    `c.status`, nên chọn nhiều hợp đồng ở màn hình danh sách là nhảy được từ
    'Đã sinh file' sang 'Đã thanh toán' mà không cần hoá đơn lẫn uỷ nhiệm chi,
    và không để lại dấu vết nào trong lịch sử.

    Chính sách giao dịch: **tất cả hoặc không có gì**. Nếu bất kỳ hợp đồng nào
    trong lô không hợp lệ, không hợp đồng nào bị đổi. Với thao tác một người dùng
    trên một danh sách vừa chọn bằng mắt, sửa lại lựa chọn rồi bấm lần nữa dễ hiểu
    hơn nhiều so với "12 cái xong, 3 cái lỗi, tự đoán xem cái nào".

    Raises:
        ValueError: nếu trạng thái đích không hợp lệ, hoặc có hợp đồng không
            chuyển được — kèm danh sách số hợp đồng và lý do.
    """
    new_st = lifecycle.parse_status(new_status)

    contracts = db.query(Contract).filter(Contract.id.in_(ids)).all() if ids else []
    found = {c.id for c in contracts}
    missing = [cid for cid in ids if cid not in found]

    problems: list[str] = [f"#{cid}: không tìm thấy" for cid in missing]
    for c in contracts:
        try:
            lifecycle.validate_transition(c, new_st)
        except ValueError as exc:
            problems.append(f"{c.contract_number}: {exc}")

    if problems:
        # Không đụng vào DB. Session chưa có thay đổi nào để rollback.
        raise ValueError(
            f"Không cập nhật hợp đồng nào. {len(problems)} hợp đồng không hợp lệ — "
            + "; ".join(problems[:5])
            + (f" (và {len(problems) - 5} lỗi khác)" if len(problems) > 5 else "")
        )

    for c in contracts:
        _apply_status_change(db, c, new_st, actor=actor)
    db.commit()
    return len(contracts)


def _add_event(db: Session, contract_id: int, event_type: str,
               description: str, actor: str):
    ev = ContractEvent(
        contract_id=contract_id,
        event_type=event_type,
        description=description,
        actor=actor,
    )
    db.add(ev)


def _safe_parse_date(v) -> Optional[date]:
    try:
        return parse_date_input(v)
    except Exception:
        return None


def _parse_selected_units(raw) -> list[str]:
    """Parse hidden field 'selected_units' (JSON array tên cơ sở) → list[str] sạch."""
    if not raw or not str(raw).strip():
        return []
    try:
        data = json.loads(raw)
    except (ValueError, TypeError):
        return []
    if not isinstance(data, list):
        return []
    out = []
    for n in data:
        s = str(n).strip()
        if s:
            out.append(s)
    return out


def _safe_int(v) -> Optional[int]:
    if v is None or str(v).strip() == "":
        return None
    try:
        return int(str(v).strip().replace(",", "").replace(".", ""))
    except Exception:
        return None


def _safe_decimal(v) -> Optional[Decimal]:
    if v is None or str(v).strip() == "":
        return None
    try:
        # Remove VN thousand separators (dấu chấm) trước khi parse
        cleaned = str(v).strip().replace(".", "").replace(",", ".")
        return Decimal(cleaned)
    except InvalidOperation:
        return None
