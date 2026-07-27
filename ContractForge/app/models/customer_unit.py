"""
Model: CustomerUnit — Đơn vị con của khách hàng (điểm trạm, phòng khám vệ tinh...).

Mỗi Customer có thể có 0..N CustomerUnit.
unit_count trên contract form = 1 (trạm chính) + len(sub_units).
"""

from datetime import datetime
from sqlalchemy import Column, Integer, String, Boolean, DateTime, Text, ForeignKey
from sqlalchemy.orm import relationship
from app.database import Base


UNIT_TYPES = ["Điểm trạm", "Phòng khám", "Trạm y tế lưu động", "Khác"]


class CustomerUnit(Base):
    __tablename__ = "customer_units"

    id         = Column(Integer, primary_key=True, index=True)
    customer_id = Column(Integer, ForeignKey("customers.id", ondelete="CASCADE"),
                         nullable=False, index=True)

    name       = Column(String(300), nullable=False, comment="Tên đơn vị con, VD: Điểm trạm Bàu Hàm 2")
    unit_type  = Column(String(50), nullable=False, default="Điểm trạm",
                        comment="Điểm trạm | Phòng khám | ...")
    address    = Column(String(400), nullable=True)
    notes      = Column(Text, nullable=True)
    is_active          = Column(Boolean, default=True, nullable=False)
    include_in_billing = Column(Boolean, default=True, nullable=False,
                                comment="True = tính điểm trạm này vào số cơ sở hợp đồng")
    sort_order         = Column(Integer, default=0, nullable=False)

    created_at = Column(DateTime, default=datetime.utcnow, nullable=False)
    updated_at = Column(DateTime, default=datetime.utcnow,
                        onupdate=datetime.utcnow, nullable=False)

    # relationship
    customer   = relationship("Customer", back_populates="sub_units")

    def __repr__(self):
        return f"<CustomerUnit {self.unit_type}: {self.name}>"
