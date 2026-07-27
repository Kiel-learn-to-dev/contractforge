"""
Model: Customer — Hồ sơ khách hàng / đơn vị ký hợp đồng (Bên A).

Trạng thái khách hàng (3 mức):
  is_deleted=False, is_active=True  → Đang hoạt động
  is_deleted=False, is_active=False → Tạm ngưng
  is_deleted=True                   → Đã xóa (thùng rác) — ẩn khỏi mọi giao diện
"""

from datetime import datetime, date as _date
from sqlalchemy import Column, Integer, String, Boolean, DateTime, Date, Text
from sqlalchemy.orm import relationship
from app.database import Base


class Customer(Base):
    __tablename__ = "customers"

    id = Column(Integer, primary_key=True, index=True)

    # --- Nhận diện ---
    code = Column(String(50), unique=True, nullable=False, index=True,
                  comment="Mã khách hàng nội bộ, VD: KH-001")
    legal_name = Column(String(300), nullable=False,
                        comment="Tên pháp lý đầy đủ — dùng trong hợp đồng")
    short_name = Column(String(100), nullable=True,
                        comment="Tên ngắn tiện tra cứu")
    customer_type = Column(String(50), nullable=False, default="Trạm y tế",
                           comment="Trạm y tế | TTYT | Đơn vị quản lý | Khác")

    # --- Địa chỉ ---
    address  = Column(String(400), nullable=True)
    ward     = Column(String(100), nullable=True, comment="Xã / Phường / Thị trấn")
    district = Column(String(100), nullable=True,
                      comment="Trường cũ từ mô hình 3 cấp; giữ lại để tương thích dữ liệu")
    province = Column(String(100), nullable=True, default="Đồng Nai")

    # --- Pháp lý ---
    tax_code             = Column(String(30),  nullable=True)
    representative_name   = Column(String(150), nullable=True)
    representative_gender = Column(String(10),  nullable=True,
                                   comment="Giới tính đại diện: Nam | Nữ | None")
    representative_title  = Column(String(100), nullable=True)

    # --- Ngân hàng ---
    bank_account     = Column(String(50),  nullable=True)
    bank_name        = Column(String(200), nullable=True)
    beneficiary_name = Column(String(200), nullable=True)

    # --- Liên hệ ---
    contact_name = Column(String(150), nullable=True)
    phone        = Column(String(30),  nullable=True)
    email        = Column(String(150), nullable=True)

    # --- Trạng thái ---
    is_active  = Column(Boolean, default=True,  nullable=False,
                        comment="False = tạm ngưng, ẩn khỏi form tạo hợp đồng")
    is_deleted = Column(Boolean, default=False, nullable=False,
                        comment="Soft-delete — ẩn khỏi toàn bộ giao diện")
    deleted_at = Column(DateTime, nullable=True,
                        comment="Thời điểm soft-delete")

    # --- Thanh toán ---
    bill_main_station   = Column(Boolean, default=True, nullable=False)
    budget_relation_code = Column(String(50), nullable=True)
    kcb_code            = Column(String(30),  nullable=True,
                                  comment="Mã cơ sở khám chữa bệnh BHYT")
    establishment_decision = Column(String(100), nullable=True,
                                     comment="Số quyết định thành lập, VD: 1234/QĐ-UBND")
    establishment_date     = Column(Date, nullable=True,
                                     comment="Ngày thành lập / ngày ban hành QĐ")

    notes = Column(Text, nullable=True)

    created_at = Column(DateTime, default=datetime.utcnow, nullable=False)
    updated_at = Column(DateTime, default=datetime.utcnow,
                        onupdate=datetime.utcnow, nullable=False)

    # --- Relationships ---
    sub_units = relationship(
        "CustomerUnit", back_populates="customer",
        cascade="all, delete-orphan",
        order_by="CustomerUnit.sort_order, CustomerUnit.id",
    )
    documents = relationship(
        "CustomerDocument", back_populates="customer",
        cascade="all, delete-orphan",
        order_by="CustomerDocument.doc_type, CustomerDocument.id",
    )

    # --- Helpers ---
    def full_address(self) -> str:
        parts = [p for p in [self.address, self.ward, self.province] if p]
        return ", ".join(parts)

    @property
    def total_units(self) -> int:
        count = 1 if self.bill_main_station else 0
        count += sum(1 for u in self.sub_units if u.is_active and u.include_in_billing)
        return count

    def unit_names_list(self) -> list[str]:
        names = []
        if self.bill_main_station:
            names.append(self.short_name or self.legal_name)
        for u in self.sub_units:
            if u.is_active and u.include_in_billing:
                names.append(u.name)
        return names

    def __repr__(self):
        return f"<Customer {self.code} — {self.short_name or self.legal_name}>"
