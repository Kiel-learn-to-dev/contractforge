"""addendum_service.py — phụ lục của hợp đồng (Đợt 4).

Phụ lục là một bản ghi `contracts` có `parent_contract_id`. Nó dùng chung vòng đời,
chứng từ, đánh số và sinh file với hợp đồng; không có bảng song song.

Phụ lục bổ sung (kind "addon"): mua thêm một gói bổ sung giữa kỳ. Thời hạn của gói
là phần còn lại của hợp đồng gốc, làm tròn lên theo tháng, và không bao giờ vượt ngày
hết hạn của gốc. Số lượng mặc định lấy theo nguồn số lượng của gói, như hợp đồng.
"""

from __future__ import annotations

import json
import re
from datetime import date
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

KIND_ADDON = "addon"
DOC_KIND_ADDON = "addendum_addon"
APPENDIX_TEMPLATE_TYPE = "Phụ lục"


class AddendumError(ValueError):
    """Không lập được phụ lục. Thông báo đọc được trên form."""


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
    pattern = (re.escape(fmt)
               .replace(re.escape("{seq}"), r"(\d+)")
               .replace(re.escape("{parent}"), ".*"))
    used: set[int] = set()
    children = db.query(Contract).filter(Contract.parent_contract_id == parent.id).all()
    for child in children:
        m = re.fullmatch(pattern, child.contract_number or "")
        if m:
            used.add(int(m.group(1)))
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

    template_id = form_data.get("template_id") or None
    if template_id:
        template = db.get(ContractTemplate, int(template_id))
        if template is None or template.contract_type != APPENDIX_TEMPLATE_TYPE:
            raise AddendumError(f"Chọn mẫu loại '{APPENDIX_TEMPLATE_TYPE}' cho phụ lục.")

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
    child = create_draft(db, form, require_main=False)
    child.parent_contract_id = parent.id
    child.doc_kind = DOC_KIND_ADDON
    _add_event(db, child.id, "addendum", f"Phụ lục bổ sung của {parent.contract_number}", "user")
    _add_event(db, parent.id, "addendum", f"Đã lập phụ lục {number}", "user")
    db.commit()
    db.refresh(child)
    return child
