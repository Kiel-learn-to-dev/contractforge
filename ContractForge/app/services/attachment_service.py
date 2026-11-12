"""attachment_service.py — sinh văn bản kèm theo (biên bản nghiệm thu, bàn giao, thanh lý, khác)
từ một mẫu bất kỳ của loại được phép, cho một hợp đồng.

Chỉ đọc: không ghi file, không đổi trạng thái hay đường dẫn file của hợp đồng.
"""

from __future__ import annotations

import os

from sqlalchemy.orm import Session

from app.models.contract import Contract
from app.models.contract_template import ContractTemplate
from app.services.docx_renderer import build_render_context, build_render_lists, render_docx
from app.utils.file_naming import make_output_filename

#: Loại mẫu được phép sinh như văn bản kèm theo. Mẫu hợp đồng và báo giá không nằm ở đây.
ATTACHMENT_TYPES = ("Nghiệm thu", "Bàn giao", "Thanh lý", "Khác")


class AttachmentError(ValueError):
    """Mẫu không thuộc loại văn bản kèm theo."""


def render_attachment(db: Session, contract_id: int, template_id: int) -> tuple[bytes, str]:
    contract = db.get(Contract, contract_id)
    if contract is None:
        raise LookupError("Không tìm thấy hợp đồng.")
    template = db.get(ContractTemplate, template_id)
    if template is None:
        raise LookupError("Không tìm thấy mẫu.")
    if template.contract_type not in ATTACHMENT_TYPES:
        raise AttachmentError(
            f"Mẫu loại '{template.contract_type}' không sinh được như văn bản kèm theo. "
            f"Chỉ dùng được các loại: {', '.join(ATTACHMENT_TYPES)}.")
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
