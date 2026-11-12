"""attachment_service.py — sinh văn bản kèm theo (biên bản nghiệm thu, bàn giao, thanh lý, khác)
từ một mẫu bất kỳ của loại được phép, cho một hợp đồng.

Chỉ đọc: không ghi file, không đổi trạng thái hay đường dẫn file của hợp đồng.
"""

from __future__ import annotations

import io
import os
import zipfile

from sqlalchemy.orm import Session

from app.models.contract import Contract
from app.models.contract_template import ContractTemplate
from app.services.docx_renderer import build_render_context, build_render_lists, render_docx
from app.utils.file_naming import make_output_filename

#: Loại mẫu được phép sinh như văn bản kèm theo. Mẫu hợp đồng và báo giá không nằm ở đây.
from app.services.template_service import (
    ATTACHMENT_TYPES, ROLE_ATTACHMENT, TemplateMismatch, check_template_for, group_of_contract,
)


class AttachmentError(ValueError):
    """Mẫu không thuộc loại văn bản kèm theo."""


def render_attachment(db: Session, contract_id: int, template_id: int) -> tuple[bytes, str]:
    contract = db.get(Contract, contract_id)
    if contract is None:
        raise LookupError("Không tìm thấy hợp đồng.")
    template = db.get(ContractTemplate, template_id)
    if template is None:
        raise LookupError("Không tìm thấy mẫu.")
    try:
        check_template_for(template, group_of_contract(contract), ROLE_ATTACHMENT)
    except TemplateMismatch as exc:
        raise AttachmentError(str(exc)) from exc
    if not template.file_path or not os.path.exists(template.file_path):
        raise FileNotFoundError("File mẫu không tồn tại trên đĩa.")

    customer = contract.customer
    ctx = build_render_context(contract, customer, template=template, db=db)
    lists = build_render_lists(contract)
    data = render_docx(template.file_path, ctx, lists=lists)
    fname = make_output_filename(
        contract_number=contract.contract_number,
        customer_code=customer.code,
        contract_type=template.contract_type,
    )
    return data, fname


def resolve_attachment_templates(db: Session, raw_ids, contract: Contract) -> list[ContractTemplate]:
    """Kiểm tra toàn bộ lựa chọn trước khi render bất kỳ văn bản nào.

    Bỏ trùng nhưng giữ thứ tự chọn. Sai loại hoặc khác nhóm sản phẩm của hợp đồng
    thì ném AttachmentError.
    """
    ids: list[int] = []
    for raw in raw_ids or ():
        try:
            ids.append(int(raw))
        except (TypeError, ValueError):
            raise AttachmentError("Mã văn bản kèm theo không hợp lệ.")
    templates: list[ContractTemplate] = []
    for tid in dict.fromkeys(ids):
        tpl = db.get(ContractTemplate, tid)
        if tpl is None:
            raise AttachmentError(f"Không tìm thấy văn bản kèm theo #{tid}.")
        if not tpl.is_active:
            raise AttachmentError(f"Mẫu '{tpl.name}' đang tắt.")
        try:
            check_template_for(tpl, group_of_contract(contract), ROLE_ATTACHMENT)
        except TemplateMismatch as exc:
            raise AttachmentError(str(exc)) from exc
        if not tpl.file_path or not os.path.exists(tpl.file_path):
            raise AttachmentError(f"File của mẫu '{tpl.name}' không tồn tại trên đĩa.")
        templates.append(tpl)
    return templates


def render_attachments(db: Session, contract_id: int,
                       templates: list[ContractTemplate]) -> list[tuple[str, bytes]]:
    """Render trong bộ nhớ. Không ghi file, không đổi trạng thái. Lỗi một mẫu thì ném AttachmentError."""
    out: list[tuple[str, bytes]] = []
    for tpl in templates:
        try:
            data, fname = render_attachment(db, contract_id, tpl.id)
        except Exception as exc:
            raise AttachmentError(f"Không sinh được '{tpl.name}': {exc}") from exc
        out.append((fname, data))
    return out


def build_bundle_zip(items: list[tuple[str, bytes]]) -> bytes:
    """Đóng gói các văn bản thành một file ZIP trong bộ nhớ. Tên trùng thì thêm _2, _3."""
    buf = io.BytesIO()
    used: set[str] = set()
    with zipfile.ZipFile(buf, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for fname, data in items:
            name, n = fname, 2
            while name in used:
                stem, dot, ext = fname.rpartition(".")
                name = f"{stem}_{n}.{ext}" if dot else f"{fname}_{n}"
                n += 1
            used.add(name)
            zf.writestr(name, data)
    return buf.getvalue()
