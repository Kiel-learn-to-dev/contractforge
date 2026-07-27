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
from app.models.customer import Customer
from app.models.contract_template import ContractTemplate
from app.services.docx_renderer import render_docx, build_render_context
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
    VD: "Trạm Y tế xã Phước An"          → "TYTPHUOCAN"
        "Trạm Y tế xã Đại Phước"         → "TYTDAIPHUOC"
        "Trung tâm Y tế phường Tam Phước" → "TYTTAMPHUOC"
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


def get_next_contract_seq(db: Session, customer_id: int,
                          contract_type: str = "YTCS",
                          year: int = 0) -> str:
    """
    Tìm số thứ tự nhỏ nhất chưa dùng cho (customer, contract_type, year).
    - Đi theo sản phẩm + năm: cùng KH nhưng khác sản phẩm thì bộ đếm riêng.
    - Lấp khoảng trống: nếu 01 đã xóa và 02 còn, kết quả là 01.
    - Sang năm mới reset về 01.
    """
    from datetime import date as _date
    if not year:
        year = _date.today().year

    # Xác định pattern dựa trên loại hợp đồng
    if contract_type == "HSSK":
        # Format: {seq}/VIETTEL-{slug}/HSSK/{year}
        pattern = f"%/HSSK/{year}"
    else:
        # Format: {seq}/KDGP/DNI-{slug}/{year}
        pattern = f"%/KDGP/DNI-%/{year}"

    existing = db.query(Contract.contract_number).filter(
        Contract.customer_id == customer_id,
        Contract.contract_number.like(pattern),
    ).all()

    # Extract seq numbers from existing contracts
    used = set()
    for (num,) in existing:
        if num:
            part = num.split("/")[0].strip()
            try:
                used.add(int(part))
            except ValueError:
                pass

    # Find smallest unused positive integer
    seq = 1
    while seq in used:
        seq += 1
    return str(seq).zfill(2)


def contract_type_from_code(code: str) -> str:
    """Xác định loại hợp đồng (YTCS | HSSK) từ mã template hoặc số hợp đồng."""
    return "HSSK" if "HSSK" in (code or "").upper() else "YTCS"


def build_contract_number(seq: str, slug: str,
                          contract_type: str = "YTCS", year: int = 0) -> str:
    """
    Ghép số hợp đồng chuẩn theo loại — nguồn sự thật duy nhất cho cả form lẻ,
    batch và preview (tránh lặp công thức ở nhiều nơi).
      HSSK: {seq}/VIETTEL-{slug}/HSSK/{year}
      YTCS: {seq}/KDGP/DNI-{slug}/{year}
    """
    if not year:
        year = date.today().year
    if contract_type == "HSSK":
        return f"{seq}/VIETTEL-{slug}/HSSK/{year}"
    return f"{seq}/KDGP/DNI-{slug}/{year}"



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
        d60 = today + timedelta(days=60)
        query = query.filter(
            Contract.status.in_([
                ContractStatus.Active, ContractStatus.Signed,
                ContractStatus.PaidActive, ContractStatus.ExpiringSoon,
            ]),
            Contract.end_date.isnot(None),
            Contract.end_date >= today,
            Contract.end_date <= d60,
        )
    elif alert_filter == "all_active":
        # Tất cả hợp đồng đang thực sự hiệu lực (không bao gồm Signed chờ kích hoạt)
        query = query.filter(
            Contract.status.in_([
                ContractStatus.PaidActive, ContractStatus.Active,
                ContractStatus.Invoiced, ContractStatus.ExpiringSoon,
            ])
        )
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
        if c.end_date and c.status in (ContractStatus.Active, ContractStatus.Signed,
                                          ContractStatus.PaidActive, ContractStatus.Invoiced):
            delta = (c.end_date - today).days
            if 0 < delta <= 30:
                expiring += 1
    stats["expiring_soon_count"] = expiring
    return stats


# ─── Create draft ─────────────────────────────────────────────────────────────

def create_draft(db: Session, form_data: dict) -> Contract:
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
        unit_price_vat = unit_price_pre_vat * (1 + vat_rate / 100)

    # Quantity = unit_count × month_count
    quantity = _safe_decimal(form_data.get("quantity"))
    if quantity is None and month_count:
        quantity = Decimal(unit_count) * Decimal(month_count)

    # Total amount auto-compute
    if total_amount is None and unit_price_vat and quantity:
        total_amount = unit_price_vat * quantity
    elif total_amount is None and unit_price_pre_vat and quantity:
        total_amount = unit_price_pre_vat * quantity

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

    db.add(contract)
    db.flush()

    # Ghi event
    _add_event(db, contract.id, "created", "Tạo bản nháp hợp đồng", "system")
    db.commit()
    db.refresh(contract)
    return contract


# ─── Generate DOCX ────────────────────────────────────────────────────────────

def generate_docx(db: Session, contract_id: int) -> Contract:
    """
    Render file .docx từ template + dữ liệu hợp đồng.
    Cập nhật status → Generated, lưu đường dẫn file vào DB.
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
    if not (contract.pricing_description or "").strip():
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

    # Snapshot render_data vào contract
    contract.render_data = ctx

    # Render
    try:
        rendered_bytes = render_docx(template.file_path, ctx)
    except Exception as e:
        raise ValueError(f"Lỗi khi render .docx: {e}")

    # Lưu file output
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    fname = make_output_filename(
        contract_number=contract.contract_number,
        customer_code=customer.code,
        contract_type=template.contract_type,
    )
    fpath = os.path.join(OUTPUT_DIR, fname)

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

VALID_TRANSITIONS = {
    ContractStatus.Draft:        {ContractStatus.Generated},
    ContractStatus.Generated:    {ContractStatus.Sent, ContractStatus.Signed},
    ContractStatus.Sent:         {ContractStatus.Signed},
    ContractStatus.Signed:       {ContractStatus.Invoiced, ContractStatus.Terminated},
    ContractStatus.Invoiced:     {ContractStatus.PaidActive, ContractStatus.Terminated},
    ContractStatus.Active:       {ContractStatus.Invoiced, ContractStatus.PaidActive, ContractStatus.Terminated},  # legacy
    ContractStatus.PaidActive:   {ContractStatus.ExpiringSoon, ContractStatus.Expired, ContractStatus.Terminated},
    ContractStatus.ExpiringSoon: {ContractStatus.PaidActive, ContractStatus.Expired, ContractStatus.Terminated},
    ContractStatus.Expired:      {ContractStatus.Terminated},
    ContractStatus.Terminated:   set(),
}


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

    try:
        new_st = ContractStatus(new_status)
    except ValueError:
        raise ValueError(f"Trạng thái không hợp lệ: {new_status!r}")

    allowed = VALID_TRANSITIONS.get(contract.status, set())
    if new_st not in allowed:
        raise ValueError(
            f"Không thể chuyển từ {contract.status.value} sang {new_st.value}. "
            f"Cho phép: {[s.value for s in allowed] or 'không có'}"
        )

    # Yêu cầu file trước khi chuyển sang trạng thái thanh toán
    if new_st == ContractStatus.Invoiced and not contract.invoice_pdf_path:
        raise ValueError("Vui lòng upload file hóa đơn PDF trước khi chuyển sang 'Đã xuất hóa đơn'.")
    if new_st == ContractStatus.PaidActive and not contract.payment_slip_path:
        raise ValueError("Vui lòng upload file ủy nhiệm chi trước khi chuyển sang 'Đã thanh toán'.")

    old_status = contract.status.value
    contract.status = new_st
    desc = f"Chuyển trạng thái {old_status} → {new_st.value}"
    if note:
        desc += f": {note}"
    _add_event(db, contract.id, "status_changed", desc, actor)
    db.commit()
    db.refresh(contract)
    return contract


# ─── Internal helpers ────────────────────────────────────────────────────────


def delete_contract(db: Session, contract_id: int) -> bool:
    """Xóa 1 hợp đồng. Trả về True nếu thành công."""
    c = db.query(Contract).filter(Contract.id == contract_id).first()
    if not c:
        return False
    db.delete(c)
    db.commit()
    return True


def bulk_delete(db: Session, ids: list[int]) -> int:
    """Xóa nhiều hợp đồng. Trả về số bản ghi đã xóa."""
    deleted = 0
    for cid in ids:
        c = db.query(Contract).filter(Contract.id == cid).first()
        if c:
            db.delete(c)
            deleted += 1
    db.commit()
    return deleted


def bulk_update_status(db: Session, ids: list[int], new_status: str) -> int:
    """Cập nhật trạng thái nhiều hợp đồng. Trả về số bản ghi đã cập nhật."""
    valid = {s.value for s in ContractStatus}
    if new_status not in valid:
        raise ValueError(f"Trạng thái không hợp lệ: {new_status}")
    updated = 0
    for cid in ids:
        c = db.query(Contract).filter(Contract.id == cid).first()
        if c:
            c.status = ContractStatus(new_status)
            updated += 1
    db.commit()
    return updated


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
