"""
Model: ContractEvent — Timeline sự kiện của hợp đồng.

Dùng để ghi lại toàn bộ lịch sử vòng đời:
  tạo mới → sinh file → gửi → ký → gia hạn → thanh lý → v.v.

Chuẩn bị cho Pha 2 (timeline view), nhưng bảng cần có ngay từ V1
để không phải migrate sau.
"""

from datetime import datetime
from sqlalchemy import Column, Integer, String, DateTime, Text, ForeignKey
from sqlalchemy.orm import relationship
from app.database import Base


class ContractEvent(Base):
    __tablename__ = "contract_events"

    id = Column(Integer, primary_key=True, index=True)

    contract_id = Column(Integer, ForeignKey("contracts.id"), nullable=False, index=True)
    contract = relationship("Contract", back_populates="events")

    event_type = Column(String(50), nullable=False,
                        comment="created | generated | sent | signed | activated | "
                                "expiring_soon_alert | expired | renewed | terminated | note")
    description = Column(Text, nullable=True,
                         comment="Mô tả chi tiết sự kiện, VD: 'Sinh file lúc 09:32, người thực hiện: admin'")
    actor = Column(String(100), nullable=True,
                   comment="Người thực hiện (username hoặc 'system')")
    meta_json = Column(Text, nullable=True,
                       comment="JSON metadata bổ sung, VD: tên file sinh ra, batch job id...")

    occurred_at = Column(DateTime, default=datetime.utcnow, nullable=False, index=True)

    def __repr__(self):
        return f"<ContractEvent [{self.event_type}] contract_id={self.contract_id}>"
