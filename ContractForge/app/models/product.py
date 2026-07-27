"""
Model: Product — Danh mục phần mềm / dịch vụ có thể bán.

Blueprint §5.2: mỗi sản phẩm có giá chuẩn, VAT mặc định,
thời hạn mặc định và template hợp đồng mặc định.
"""

from datetime import datetime
from sqlalchemy import Column, Integer, String, Boolean, DateTime, Numeric, ForeignKey
from sqlalchemy.orm import relationship
from app.database import Base


class Product(Base):
    __tablename__ = "products"

    id = Column(Integer, primary_key=True, index=True)

    code = Column(String(50), unique=True, nullable=False, index=True,
                  comment="Mã sản phẩm, VD: SW-CRM, SW-HRM")
    name = Column(String(200), nullable=False,
                  comment="Tên hiển thị trong giao diện và hợp đồng")
    product_type = Column(String(80), nullable=False, default="Phần mềm",
                          comment="Phần mềm | Dịch vụ | Gói hỗ trợ | Thuê dịch vụ | Đào tạo | Triển khai")

    # Giá & thuế mặc định (có thể override khi tạo hợp đồng cụ thể)
    default_price = Column(Numeric(18, 0), nullable=True,
                           comment="Đơn giá mặc định chưa VAT (VNĐ)")
    default_vat_rate = Column(Numeric(5, 2), nullable=True, default=0,
                              comment="Thuế VAT mặc định, VD: 0 | 8 | 10 (phần trăm)")
    default_duration_months = Column(Integer, nullable=True, default=12,
                                     comment="Số tháng mặc định của 1 chu kỳ hợp đồng")

    # Template hợp đồng gợi ý cho sản phẩm này
    default_template_id = Column(Integer, ForeignKey("contract_templates.id"),
                                  nullable=True)
    default_template = relationship("ContractTemplate", foreign_keys=[default_template_id])

    # Khuôn số hợp đồng của sản phẩm này — dữ liệu, không phải code.
    # Ô trống = dùng mặc định của hệ thống. Xem app/services/numbering.py.
    contract_number_format = Column(String(200), nullable=True,
                                    comment="Khuôn số HĐ, VD: {seq}/{slug}/{year}")

    is_active = Column(Boolean, default=True, nullable=False,
                       comment="False = ẩn khỏi form tạo hợp đồng")

    created_at = Column(DateTime, default=datetime.utcnow, nullable=False)
    updated_at = Column(DateTime, default=datetime.utcnow,
                        onupdate=datetime.utcnow, nullable=False)

    def __repr__(self):
        return f"<Product {self.code} — {self.name}>"
