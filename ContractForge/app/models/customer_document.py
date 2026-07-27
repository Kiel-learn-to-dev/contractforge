"""
Model: CustomerDocument — Hồ sơ giấy tờ đính kèm khách hàng.

doc_type:
  'cccd_front'    — CCCD mặt trước (người đại diện)
  'cccd_back'     — CCCD mặt sau  (người đại diện)
  'establishment' — Quyết định thành lập đơn vị
  'other'         — Giấy tờ khác (label do user đặt)

Quy tắc:
  - fixed types (cccd_front/back, establishment): tối đa 1 record/customer,
    upload mới tự ghi đè (router xử lý upsert).
  - 'other': không giới hạn, mỗi record có label riêng.

File lưu tại: data/uploads/customer_docs/{customer_id}/{filename}.{ext}
"""

from datetime import datetime
from sqlalchemy import Column, Integer, String, DateTime, ForeignKey
from sqlalchemy.orm import relationship
from app.database import Base


class CustomerDocument(Base):
    __tablename__ = "customer_documents"

    id          = Column(Integer, primary_key=True, index=True)
    customer_id = Column(Integer, ForeignKey("customers.id"),
                         nullable=False, index=True)
    doc_type    = Column(String(50),  nullable=False,
                         comment="cccd_front | cccd_back | establishment | other")
    label       = Column(String(200), nullable=True,
                         comment="Tên tự đặt — chỉ dùng khi doc_type='other'")
    file_path   = Column(String(500), nullable=False,
                         comment="Đường dẫn tuyệt đối trên disk")
    file_name   = Column(String(200), nullable=False,
                         comment="Tên file gốc khi upload")
    mime_hint   = Column(String(20),  nullable=True,
                         comment="'pdf' | 'image' — để template chọn icon")
    uploaded_at = Column(DateTime, default=datetime.utcnow, nullable=False)

    customer = relationship("Customer", back_populates="documents")

    def __repr__(self):
        return f"<CustomerDocument [{self.doc_type}] customer_id={self.customer_id}>"
