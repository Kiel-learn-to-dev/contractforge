"""
AppSetting — key-value store cho cấu hình nội bộ.
Mỗi row: (key VARCHAR, value TEXT).
"""

from sqlalchemy import Column, String, Text
from app.database import Base


class AppSetting(Base):
    __tablename__ = "app_settings"

    key   = Column(String(100), primary_key=True)
    value = Column(Text, nullable=True, default="")

    def __repr__(self):
        return f"<AppSetting {self.key}={self.value!r}>"
