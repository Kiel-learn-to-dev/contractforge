"""
Model: ContractItem — một dòng trong bảng giá trị của hợp đồng.

Mỗi dòng là ảnh chụp tại thời điểm lập: tên gói, đơn vị tính, đơn giá và thuế
suất được chép vào dòng, nên sửa danh mục về sau không làm đổi hợp đồng đã có.

Hợp đồng `legacy` có đúng một dòng, chép từ số đã lưu (xem seed.py). Hợp đồng
`items` có một dòng cho mỗi gói chính / gói bổ sung, tính bởi pricing.py.
"""

from sqlalchemy import Column, ForeignKey, Integer, Numeric, String, Text
from sqlalchemy.orm import relationship

from app.database import Base


class ContractItem(Base):
    __tablename__ = "contract_items"

    id = Column(Integer, primary_key=True, index=True)
    contract_id = Column(Integer, ForeignKey("contracts.id", ondelete="CASCADE"),
                         nullable=False, index=True)
    product_id = Column(Integer, ForeignKey("products.id"), nullable=True)
    sort_order = Column(Integer, nullable=False, default=0)
    description = Column(String(300), nullable=False)
    unit_label = Column(String(50), nullable=True)
    quantity = Column(Numeric(18, 2), nullable=False)
    period_count = Column(Numeric(10, 4), nullable=False)
    unit_price_pre_vat = Column(Numeric(18, 2), nullable=True)
    vat_rate = Column(Numeric(5, 2), nullable=False, default=0)
    amount_pre_vat = Column(Numeric(18, 0), nullable=False)
    vat_amount = Column(Numeric(18, 0), nullable=False)
    amount_total = Column(Numeric(18, 0), nullable=False)
    note = Column(Text, nullable=True)

    contract = relationship("Contract", back_populates="items")
    product = relationship("Product")

    def __repr__(self):
        return f"<ContractItem #{self.id} contract={self.contract_id} {self.description!r}>"
