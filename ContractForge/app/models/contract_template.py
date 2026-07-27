"""
Model: ContractTemplate — File .docx mẫu và metadata của nó.

Blueprint §5.3:
  - Hỗ trợ 2 chế độ: placeholder | legacy_anchor
  - Lưu đường dẫn file, danh sách placeholder phát hiện được,
    và mapping field canonical.

Lưu ý: detected_placeholders và field_mapping lưu dạng JSON text
để tránh phụ thuộc vào JSON column type của SQLite.
"""

import json
from datetime import datetime
from sqlalchemy import Column, Integer, String, Boolean, DateTime, Text
from app.database import Base


class ContractTemplate(Base):
    __tablename__ = "contract_templates"

    id = Column(Integer, primary_key=True, index=True)

    code = Column(String(50), unique=True, nullable=False, index=True,
                  comment="Mã template nội bộ, VD: TPL-YTCS-2024")
    name = Column(String(200), nullable=False,
                  comment="Tên hiển thị trong giao diện")
    contract_type = Column(String(80), nullable=False,
                           comment="YTCS | HSSK | Phụ lục | Nghiệm thu | Thanh lý | Khác")

    # File storage
    file_path = Column(String(500), nullable=True,
                       comment="Đường dẫn tương đối trong uploads/templates/")
    file_name = Column(String(200), nullable=True,
                       comment="Tên file gốc khi upload")

    # Chế độ xử lý
    mode = Column(String(20), nullable=False, default="placeholder",
                  comment="placeholder | legacy_anchor")

    # Kết quả phân tích template (lưu JSON)
    _detected_placeholders = Column("detected_placeholders", Text, nullable=True,
                                     comment='JSON array, VD: ["CONTRACT_NUMBER", "PARTY_A_NAME"]')
    _field_mapping = Column("field_mapping", Text, nullable=True,
                             comment='JSON object mapping placeholder → canonical field')

    description = Column(Text, nullable=True)
    is_active = Column(Boolean, default=True, nullable=False)

    created_at = Column(DateTime, default=datetime.utcnow, nullable=False)
    updated_at = Column(DateTime, default=datetime.utcnow,
                        onupdate=datetime.utcnow, nullable=False)

    # --- JSON helpers ---
    @property
    def detected_placeholders(self) -> list:
        if not self._detected_placeholders:
            return []
        return json.loads(self._detected_placeholders)

    @detected_placeholders.setter
    def detected_placeholders(self, value: list):
        self._detected_placeholders = json.dumps(value, ensure_ascii=False)

    @property
    def field_mapping(self) -> dict:
        if not self._field_mapping:
            return {}
        return json.loads(self._field_mapping)

    @field_mapping.setter
    def field_mapping(self, value: dict):
        self._field_mapping = json.dumps(value, ensure_ascii=False)

    def __repr__(self):
        return f"<ContractTemplate {self.code} [{self.mode}]>"
