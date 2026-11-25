"""Reusable independent issuer identities, separate from organization settings."""
from sqlalchemy import Column, Integer, String

from app.database import Base


class QuotationIssuer(Base):
    __tablename__ = "quotation_issuers"

    id = Column(Integer, primary_key=True)
    name = Column(String(500), nullable=False)
    address = Column(String(500), nullable=False, default="")
    tax_code = Column(String(500), nullable=False, default="")
    representative = Column(String(500), nullable=False, default="")
    title = Column(String(500), nullable=False, default="")
    phone = Column(String(500), nullable=False, default="")
    email = Column(String(500), nullable=False, default="")
    fingerprint = Column(String(64), nullable=False, unique=True)
