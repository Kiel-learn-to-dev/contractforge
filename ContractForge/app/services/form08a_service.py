"""
form08a_service.py — Sinh "Mẫu số 08a" (Bảng xác định giá trị khối lượng
công việc hoàn thành) theo từng hợp đồng.

Khác với hợp đồng chính (lưu file vào DB), biểu 08A được sinh on-the-fly
từ dữ liệu hợp đồng + template đóng gói sẵn (assets/default_templates/template_08A.docx).
Không lưu trạng thái, luôn phản ánh dữ liệu mới nhất.

Hàm công khai:
    render_form_08a(db, contract_id) -> (bytes, filename)
"""

import os
import re

from sqlalchemy.orm import Session, joinedload

from app.models.contract import Contract
from app.models.customer import Customer
from app.models.product import Product
from app.paths import FORM_08A_TEMPLATE
from app.services.docx_renderer import render_docx, build_render_context
from app.utils.file_naming import sanitize_for_filename


def _work_content(contract: Contract, product) -> str:
    """Nội dung công việc trong biểu 08A — nhãn ngắn gọn của sản phẩm.

    Lấy thẳng từ danh mục sản phẩm, bỏ phần viết tắt trong ngoặc ở cuối tên
    ("Phần mềm Quản lý Kho (WMS)" → "Phần mềm Quản lý Kho"), vì trong biểu 08A
    mã viết tắt nội bộ không có ý nghĩa với bên duyệt chi.

    Bản trước dò chuỗi mã sản phẩm để trả về nhãn viết cứng cho hai sản phẩm cụ
    thể. Cách đó khoá mã nguồn vào một danh mục, và sản phẩm mới thì rơi vào
    nhánh dự phòng. Muốn đổi nhãn: sửa tên sản phẩm trong danh mục.
    """
    name = (getattr(product, "name", "") or "").strip()
    if not name:
        return "Phần mềm"
    return re.sub(r"\s*\([^()]*\)\s*$", "", name).strip() or name


def build_08a_context(db: Session, contract: Contract, customer: Customer,
                      product=None) -> dict:
    """Xây context cho biểu 08A: mở rộng context hợp đồng chuẩn."""
    ctx = build_render_context(contract, customer, product=product, db=db)

    # Đơn vị sử dụng ngân sách — VIẾT HOA tên pháp lý
    ctx["BUDGET_UNIT_NAME"] = (customer.legal_name or "").upper()
    ctx["BUDGET_UNIT_CODE"] = (customer.budget_relation_code or "").strip()

    # Nội dung công việc
    ctx["WORK_CONTENT"] = _work_content(contract, product)

    # Chức danh chữ ký — VIẾT HOA
    ctx["PARTY_A_TITLE_UPPER"] = (ctx.get("PARTY_A_TITLE") or "").upper()
    ctx["PARTY_B_TITLE_UPPER"] = (ctx.get("PARTY_B_TITLE") or "").upper()
    return ctx


def render_form_08a(db: Session, contract_id: int) -> tuple[bytes, str]:
    """
    Render biểu 08A cho 1 hợp đồng.

    Returns: (file_bytes, filename)
    Raises: LookupError / FileNotFoundError / ValueError
    """
    contract = db.query(Contract).filter(Contract.id == contract_id).first()
    if not contract:
        raise LookupError(f"Không tìm thấy hợp đồng id={contract_id}")

    template_path = str(FORM_08A_TEMPLATE)
    if not os.path.exists(template_path):
        raise FileNotFoundError(
            "Không tìm thấy file mẫu biểu 08A (template_08A.docx) trong hệ thống."
        )

    customer = (
        db.query(Customer)
        .options(joinedload(Customer.sub_units))
        .filter(Customer.id == contract.customer_id)
        .first()
    )
    if not customer:
        raise LookupError(f"Khách hàng id={contract.customer_id} không tồn tại.")

    product = (
        db.query(Product).filter(Product.id == contract.product_id).first()
        if contract.product_id else None
    )

    ctx = build_08a_context(db, contract, customer, product)

    try:
        data = render_docx(template_path, ctx)
    except Exception as e:
        raise ValueError(f"Lỗi khi render biểu 08A: {e}")

    safe_num = sanitize_for_filename(contract.contract_number or str(contract_id), max_len=30)
    cust_part = sanitize_for_filename(customer.code or "KH", max_len=15)
    filename = f"08A_{cust_part}_{safe_num}.docx"
    return data, filename
