"""
Model: Product — Danh mục phần mềm / dịch vụ có thể bán.

Blueprint §5.2: mỗi sản phẩm có giá chuẩn, VAT mặc định,
thời hạn mặc định và template hợp đồng mặc định.
"""

import json
from datetime import datetime
from sqlalchemy import Column, Integer, String, Boolean, DateTime, Numeric, ForeignKey, Text
from sqlalchemy.orm import relationship
from app.database import Base


PRODUCT_KINDS = ("main", "addon", "storage")
QUANTITY_SOURCES = ("units", "records", "manual")


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

    # Danh mục gói cước: gói chính / gói bổ sung, nhóm, đơn vị tính, kỳ giá.
    group_code = Column(String(50), nullable=True, index=True,
                        comment="Nhóm: gom gói chính với gói bổ sung của nó")
    kind = Column(String(20), nullable=False, default="main",
                  comment="main | addon | storage")
    unit_label = Column(String(50), nullable=True,
                        comment="Đơn vị tính in ra hợp đồng")
    price_period_months = Column(Integer, nullable=False, default=1,
                                 comment="Đơn giá áp cho bao nhiêu tháng")
    quantity_source = Column(String(20), nullable=False, default="units",
                             comment="units | records | manual")
    min_quantity = Column(Integer, nullable=True)
    quantity_step = Column(Integer, nullable=True)
    account_limit = Column(Integer, nullable=True)
    _feature_list = Column("feature_list", Text, nullable=True,
                           comment="JSON: [{no, name, description}]")
    replaced_by_id = Column(Integer, ForeignKey("products.id"), nullable=True)
    sort_order = Column(Integer, nullable=False, default=0)

    created_at = Column(DateTime, default=datetime.utcnow, nullable=False)
    updated_at = Column(DateTime, default=datetime.utcnow,
                        onupdate=datetime.utcnow, nullable=False)

    @property
    def feature_list(self) -> list:
        if not self._feature_list:
            return []
        try:
            value = json.loads(self._feature_list)
        except (ValueError, TypeError):
            return []
        return value if isinstance(value, list) else []

    @feature_list.setter
    def feature_list(self, value):
        self._feature_list = json.dumps(list(value or []), ensure_ascii=False)

    def __repr__(self):
        return f"<Product {self.code} — {self.name}>"
