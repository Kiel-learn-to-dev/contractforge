"""
file_naming.py — Quy tắc đặt tên file output .docx.

Quy tắc (từ blueprint §5.6):
  - Ổn định, dễ tìm, tránh trùng lặp
  - Format: {loại_hđ}_{mã_KH}_{số_hđ_safe}_{timestamp}.docx
  - Ví dụ: HOPDONG_KH001_HD001-2026_20260115_093045.docx

Hàm công khai:
  make_output_filename(contract_number, customer_code, contract_type, ts=None)
  sanitize_for_filename(s) → chuỗi an toàn cho tên file
"""

import re
import unicodedata
from datetime import datetime
from typing import Optional


def sanitize_for_filename(s: str, max_len: int = 40) -> str:
    """
    Chuyển chuỗi tùy ý thành phần an toàn cho tên file:
      - Bỏ dấu tiếng Việt (kể cả Đ/đ và các ký tự không qua NFD)
      - Thay khoảng trắng và ký tự đặc biệt bằng '_'
      - Viết hoa
      - Cắt bớt nếu quá dài
    """
    # Bảng thay thế thủ công cho các ký tự đặc biệt tiếng Việt
    # không phân giải được qua NFD (chủ yếu là Đ/đ)
    _MANUAL = str.maketrans("ĐđƠơƯư", "DdOoUu")
    s = s.translate(_MANUAL)

    # Normalize NFD rồi loại combining marks (dấu huyền, sắc, hỏi, ngã, nặng)
    normalized = unicodedata.normalize("NFD", s)
    ascii_str = "".join(c for c in normalized if unicodedata.category(c) != "Mn")

    # Giữ lại alphanumeric + gạch ngang
    safe = re.sub(r"[^\w\-]", "_", ascii_str)
    safe = re.sub(r"_+", "_", safe).strip("_")
    return safe[:max_len].upper()


def make_output_filename(
    contract_number: str,
    customer_code: str,
    contract_type: str = "",
    ts: Optional[datetime] = None,
) -> str:
    """
    Tạo tên file output .docx.

    Args:
        contract_number: VD "01/ACME/2026"
        customer_code:   VD "KH-001"
        contract_type:   Nhãn phân loại của mẫu, VD "Hợp đồng", "Phụ lục"
        ts:              Timestamp, mặc định now()

    Returns:
        Ví dụ: "HOPDONG_KH-001_01-ACME-2026_20260115_093045.docx"
    """
    if ts is None:
        ts = datetime.now()

    type_part = sanitize_for_filename(contract_type or "HD", max_len=10)
    cust_part = sanitize_for_filename(customer_code or "KH", max_len=15)
    num_part  = sanitize_for_filename(contract_number or "UNKNOWN", max_len=30)
    time_part = ts.strftime("%Y%m%d_%H%M%S")

    return f"{type_part}_{cust_part}_{num_part}_{time_part}.docx"
