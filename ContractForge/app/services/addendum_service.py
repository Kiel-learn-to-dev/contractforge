"""addendum_service.py — phụ lục của hợp đồng (Đợt 4).

Phụ lục là một bản ghi `contracts` có `parent_contract_id`. Nó dùng chung vòng đời,
chứng từ, đánh số và sinh file với hợp đồng; không có bảng song song.

Phụ lục bổ sung (kind "addon"): mua thêm một gói bổ sung giữa kỳ. Thời hạn của gói
là phần còn lại của hợp đồng gốc, làm tròn lên theo tháng, và không bao giờ vượt ngày
hết hạn của gốc. Số lượng mặc định lấy theo nguồn số lượng của gói, như hợp đồng.
"""

from __future__ import annotations

import json
from datetime import date, timedelta
from decimal import Decimal
from typing import Optional

from sqlalchemy.orm import Session

from app.models.contract import Contract
from app.models.contract_template import ContractTemplate
from app.models.product import Product
from app.services import lifecycle, numbering, pricing
from app.services.contract_service import (
    _add_event, _default_quantity, _parse_items_json, _safe_parse_date, create_draft,
)
from app.services.settings_service import get_setting
from app.services.template_service import (
    ROLE_ADDON, ROLE_RENEWAL, TemplateMismatch, check_template_for, default_template_for,
    group_of_contract,
)

KIND_ADDON = "addon"
DOC_KIND_ADDON = "addendum_addon"


class AddendumError(ValueError):
    """Không lập được phụ lục. Thông báo đọc được trên form."""


def _appendix_template_id(db: Session, raw, parent: Contract, role: str) -> Optional[int]:
    """Mẫu phụ lục là tùy chọn. Không chọn thì lấy mẫu riêng của nhóm nếu có đúng một.

    Mẫu chọn tay phải đúng vai trò và đúng nhóm của hợp đồng gốc.
    """
    group = group_of_contract(parent)
    if not raw:
        default = default_template_for(db, group, role)
        return default.id if default else None
    try:
        template = db.get(ContractTemplate, int(raw))
    except (TypeError, ValueError):
        template = None
    if template is None:
        raise AddendumError("Mẫu phụ lục không tồn tại.")
    try:
        check_template_for(template, group, role)
    except TemplateMismatch as exc:
        raise AddendumError(str(exc)) from exc
    return template.id


def _parent_or_raise(db: Session, parent_id: int) -> Contract:
    parent = db.get(Contract, parent_id)
    if parent is None:
        raise AddendumError("Không tìm thấy hợp đồng gốc.")
    if (parent.doc_kind or "contract") != "contract":
        raise AddendumError("Chỉ hợp đồng gốc mới có phụ lục.")
    if parent.status not in lifecycle.IN_SERVICE_STATUSES:
        raise AddendumError("Hợp đồng gốc chưa ở trạng thái đang phục vụ.")
    if parent.end_date is None or parent.start_date is None:
        raise AddendumError("Hợp đồng gốc chưa có đủ ngày bắt đầu và ngày hết hạn.")
    return parent


def can_add_addendum(contract: Contract) -> bool:
    """Để trang chi tiết quyết định có hiện nút "Tạo phụ lục" hay không."""
    return ((contract.doc_kind or "contract") == "contract"
            and contract.status in lifecycle.IN_SERVICE_STATUSES
            and contract.end_date is not None)


def _format(db: Session) -> str:
    return get_setting(db, "addendum_number_format", "") or numbering.DEFAULT_ADDENDUM_FORMAT


def _used_seqs(db: Session, parent: Contract, fmt: str) -> set[int]:
    """Các số thứ tự phụ lục đã dùng của một hợp đồng gốc, đọc ngược từ khuôn."""
    used: set[int] = set()
    children = db.query(Contract).filter(Contract.parent_contract_id == parent.id).all()
    for child in children:
        seq = numbering.parse_addendum_seq(child.contract_number or "", fmt)
        if seq is not None:
            used.add(seq)
    return used


def _addon_candidates(parent: Contract, db: Session) -> list[Product]:
    """Gói bổ sung đang bán, cùng nhóm với gói chính của hợp đồng gốc."""
    group = parent.product.group_code if parent.product else None
    if not group:
        return []
    return (db.query(Product)
            .filter(Product.group_code == group, Product.kind != "main",
                    Product.is_active == True)  # noqa: E712
            .order_by(Product.sort_order, Product.name)
            .all())


def addendum_defaults(db: Session, parent_id: int, kind: str, start: date) -> dict:
    """Số gợi ý, ngày, số tháng và danh sách gói chọn được cho một phụ lục mới."""
    parent = _parent_or_raise(db, parent_id)
    if kind != KIND_ADDON:
        raise AddendumError("Loại phụ lục này chưa được hỗ trợ.")
    if start < parent.start_date:
        raise AddendumError("Ngày bắt đầu trước ngày hiệu lực của hợp đồng gốc.")
    months = pricing.addon_months(start, parent.end_date)  # ném PricingError nếu start > hết hạn
    fmt = _format(db)
    seq = numbering.next_addendum_seq(_used_seqs(db, parent, fmt))
    return {
        "number": numbering.build_addendum_number(parent.contract_number, seq, fmt),
        "start_date": start,
        "end_date": parent.end_date,
        "month_count": months,
        "candidates": _addon_candidates(parent, db),
    }


def create_addendum(db: Session, parent_id: int, kind: str, form_data: dict) -> Contract:
    """Lập phụ lục bổ sung. Trả về bản ghi phụ lục đã lưu.

    `form_data` gồm: start_date, sign_date, template_id (tùy chọn, phải là mẫu Phụ lục),
    items_json (danh sách {"product_id"} — mỗi gói là một gói bổ sung cùng nhóm).
    Số tháng và ngày hết hạn không lấy từ form: tính từ hợp đồng gốc.
    """
    parent = _parent_or_raise(db, parent_id)
    if kind != KIND_ADDON:
        raise AddendumError("Loại phụ lục này chưa được hỗ trợ.")

    start = _safe_parse_date(form_data.get("start_date"))
    if start is None:
        raise AddendumError("Ngày bắt đầu không hợp lệ.")
    if start < parent.start_date:
        raise AddendumError("Ngày bắt đầu trước ngày hiệu lực của hợp đồng gốc.")
    try:
        months = pricing.addon_months(start, parent.end_date)
    except pricing.PricingError as exc:
        raise AddendumError(str(exc)) from exc

    items = _parse_items_json(form_data.get("items_json"))
    if not items:
        raise AddendumError("Chọn ít nhất một gói bổ sung.")

    group = parent.product.group_code if parent.product else None
    filled = []
    for idx, item in enumerate(items, start=1):
        try:
            product = db.get(Product, int(item.get("product_id")))
        except (TypeError, ValueError):
            product = None
        if product is None or product.kind == "main" or not group or product.group_code != group:
            raise AddendumError(
                f"Dòng {idx}: chỉ nhận gói bổ sung cùng nhóm với gói chính của hợp đồng gốc.")
        line = dict(item)
        if line.get("quantity") in (None, ""):
            qty = _default_quantity(product, parent.customer, parent.unit_count or 1)
            if not qty:
                raise AddendumError(f"Dòng {idx}: thiếu số hồ sơ của khách hàng.")
            line["quantity"] = qty
        filled.append(line)

    template_id = _appendix_template_id(db, form_data.get("template_id"), parent, ROLE_ADDON)

    fmt = _format(db)
    number = numbering.build_addendum_number(
        parent.contract_number, numbering.next_addendum_seq(_used_seqs(db, parent, fmt)), fmt)
    form = {
        "contract_number": number,
        "customer_id": str(parent.customer_id),
        "product_id": str(filled[0]["product_id"]),
        "template_id": str(template_id) if template_id else "",
        "sign_date": form_data.get("sign_date") or date.today().isoformat(),
        "start_date": start.isoformat(),
        "end_date": parent.end_date.isoformat(),
        "month_count": str(months),
        "sign_location": parent.sign_location or "",
        "items_json": json.dumps(filled),
    }
    child = create_draft(db, form, require_main=False, template_role=ROLE_ADDON,
                         template_group=group_of_contract(parent))
    child.parent_contract_id = parent.id
    child.doc_kind = DOC_KIND_ADDON
    _add_event(db, child.id, "addendum", f"Phụ lục bổ sung của {parent.contract_number}", "user")
    _add_event(db, parent.id, "addendum", f"Đã lập phụ lục {number}", "user")
    db.commit()
    db.refresh(child)
    return child


# ─── Gia hạn ─────────────────────────────────────────────────────────────────
# Gia hạn có hai cách, người dùng chọn: phụ lục gia hạn (doc_kind addendum_renewal, gắn
# parent_contract_id) hoặc hợp đồng mới (doc_kind contract, gắn renewed_from_id). Cả hai
# đều được 4.4 coi là đã gia hạn.

KIND_RENEWAL = "renewal"
DOC_KIND_RENEWAL = "addendum_renewal"
MAX_REPLACEMENT_HOPS = 5
MODE_APPENDIX = "addendum"
MODE_NEW_CONTRACT = "contract"


def _current_product(db: Session, product: Optional[Product]) -> Optional[Product]:
    """Gói đang bán thay cho một gói: đi theo replaced_by_id tối đa MAX_REPLACEMENT_HOPS bước.

    Có vòng lặp hoặc không có gói thay thế thì trả None: dòng đó phải do người dùng chọn.
    """
    seen: set[int] = set()
    cur = product
    hops = 0
    while cur is not None and not cur.is_active:
        if cur.id in seen or hops >= MAX_REPLACEMENT_HOPS:
            return None
        seen.add(cur.id)
        hops += 1
        cur = db.get(Product, cur.replaced_by_id) if cur.replaced_by_id else None
    return cur


def renewal_lines(db: Session, parent: Contract) -> list[dict]:
    """Dòng gợi ý cho gia hạn, theo từng dòng của hợp đồng gốc. Giá là giá danh mục hiện tại."""
    rows = []
    for item in sorted(parent.items, key=lambda i: i.sort_order or 0):
        old = db.get(Product, item.product_id) if item.product_id else None
        rows.append({"old_product": old, "product": _current_product(db, old)})
    return rows


def renewal_comparison(db: Session, parent: Contract, lines: list[dict]) -> dict:
    """Tổng cũ và mới theo tháng, và chênh lệch. Không tính lại tiền của hợp đồng gốc."""
    old_total = Decimal(str(parent.total_amount or 0))
    months_old = parent.month_count or 1
    old_per_month = pricing.round_money(old_total / Decimal(months_old))
    new_per_month = Decimal("0")
    for row in lines:
        product = row["product"]
        if product is None:
            continue
        line = pricing.price_line(
            description=product.name, quantity=1, months=1,
            unit_price_pre_vat=product.default_price or 0,
            vat_rate=product.default_vat_rate or 0,
            price_period_months=product.price_period_months or 1,
        )
        new_per_month += line.amount_total
    new_rounded = pricing.round_money(new_per_month)
    return {
        "old_per_month": old_per_month,
        "new_per_month": new_rounded,
        "diff": new_rounded - old_per_month,
    }


def renewal_defaults(db: Session, parent_id: int) -> dict:
    """Ngày bắt đầu (sau ngày hết hạn gốc), số tháng mặc định, dòng gợi ý và so sánh giá."""
    parent = _parent_or_raise(db, parent_id)
    start = parent.end_date + timedelta(days=1)
    main_product = parent.product
    months = (main_product.default_duration_months if main_product else None) or parent.month_count or 12
    lines = renewal_lines(db, parent)
    return {
        "start_date": start,
        "month_count": months,
        "lines": lines,
        "comparison": renewal_comparison(db, parent, lines),
        "candidates": (db.query(Product).filter(Product.is_active == True)  # noqa: E712
                       .order_by(Product.name).all()),
    }


def _renewal_items(db: Session, items: list[dict]) -> list[dict]:
    """Kiểm dòng gia hạn: có ít nhất một dòng, và mọi dòng phải là gói đang bán."""
    if not items:
        raise AddendumError("Chọn ít nhất gói chính cho gia hạn.")
    for idx, item in enumerate(items, start=1):
        try:
            product = db.get(Product, int(item.get("product_id")))
        except (TypeError, ValueError):
            product = None
        if product is None or not product.is_active:
            raise AddendumError(f"Dòng {idx}: chưa chọn gói đang bán cho gia hạn.")
    return items


def _fill_quantities(db: Session, parent: Contract, items: list[dict]) -> list[dict]:
    """Dòng gia hạn chưa có số lượng: lấy theo nguồn số lượng của gói, như phụ lục bổ sung."""
    filled = []
    for idx, item in enumerate(items, start=1):
        line = dict(item)
        if line.get("quantity") in (None, ""):
            product = db.get(Product, int(line["product_id"]))
            qty = _default_quantity(product, parent.customer, parent.unit_count or 1)
            if not qty:
                raise AddendumError(f"Dòng {idx}: thiếu số hồ sơ của khách hàng.")
            line["quantity"] = qty
        filled.append(line)
    return filled


def create_renewal_appendix(db: Session, parent_id: int, form_data: dict) -> Contract:
    """Phụ lục gia hạn: bắt đầu sau ngày hết hạn gốc, dòng đầu là gói chính."""
    parent = _parent_or_raise(db, parent_id)
    start = _safe_parse_date(form_data.get("start_date"))
    if start is None:
        raise AddendumError("Ngày bắt đầu không hợp lệ.")
    if start <= parent.end_date:
        raise AddendumError("Gia hạn phải bắt đầu sau ngày hết hạn của hợp đồng gốc.")
    try:
        months = int(form_data.get("month_count") or 0)
    except (TypeError, ValueError):
        raise AddendumError("Số tháng không hợp lệ.")
    if months < 1:
        raise AddendumError("Số tháng gia hạn phải từ 1 trở lên.")
    items = _fill_quantities(db, parent, _renewal_items(db, _parse_items_json(form_data.get("items_json"))))
    template_id = _appendix_template_id(db, form_data.get("template_id"), parent, ROLE_RENEWAL)
    fmt = _format(db)
    number = numbering.build_addendum_number(
        parent.contract_number, numbering.next_addendum_seq(_used_seqs(db, parent, fmt)), fmt)
    form = {
        "contract_number": number,
        "customer_id": str(parent.customer_id),
        "product_id": str(items[0]["product_id"]),
        "template_id": str(template_id) if template_id else "",
        "sign_date": form_data.get("sign_date") or date.today().isoformat(),
        "start_date": start.isoformat(),
        "month_count": str(months),
        "sign_location": parent.sign_location or "",
        "items_json": json.dumps(items),
    }
    child = create_draft(db, form, require_main=True, template_role=ROLE_RENEWAL,
                         template_group=group_of_contract(parent))
    child.parent_contract_id = parent.id
    child.doc_kind = DOC_KIND_RENEWAL
    _add_event(db, child.id, "renewal", f"Phụ lục gia hạn của {parent.contract_number}", "user")
    _add_event(db, parent.id, "renewal", f"Đã lập phụ lục gia hạn {number}", "user")
    db.commit()
    db.refresh(child)
    return child


def create_renewal_contract(db: Session, parent_id: int, form_data: dict) -> Contract:
    """Gia hạn bằng hợp đồng mới: một hợp đồng bình thường, liên kết về hợp đồng cũ."""
    parent = _parent_or_raise(db, parent_id)
    number = (form_data.get("contract_number") or "").strip()
    if not number:
        raise AddendumError("Nhập số hợp đồng mới.")
    start = _safe_parse_date(form_data.get("start_date"))
    if start is None or start <= parent.end_date:
        raise AddendumError("Hợp đồng gia hạn phải bắt đầu sau ngày hết hạn của hợp đồng cũ.")
    items = _fill_quantities(db, parent, _renewal_items(db, _parse_items_json(form_data.get("items_json"))))
    first = db.get(Product, int(items[0]["product_id"]))
    template_raw = form_data.get("template_id") or (first.default_template_id if first else None)
    form = {
        "contract_number": number,
        "customer_id": str(parent.customer_id),
        "product_id": str(items[0]["product_id"]),
        "template_id": str(template_raw) if template_raw else "",
        "sign_date": form_data.get("sign_date") or date.today().isoformat(),
        "start_date": start.isoformat(),
        "month_count": str(form_data.get("month_count") or ""),
        "sign_location": parent.sign_location or "",
        "items_json": json.dumps(items),
    }
    new = create_draft(db, form, require_main=True)
    new.renewed_from_id = parent.id
    _add_event(db, new.id, "renewal", f"Gia hạn từ hợp đồng {parent.contract_number}", "user")
    _add_event(db, parent.id, "renewal", f"Đã gia hạn bằng hợp đồng {number}", "user")
    db.commit()
    db.refresh(new)
    return new
