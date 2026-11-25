"""Explicit issuer snapshots: never saves quote prices or organization defaults."""
from hashlib import sha256
import json

from sqlalchemy.exc import IntegrityError

from app.models.quotation_issuer import QuotationIssuer
from app.services.manual_quotation import ISSUER_FIELDS


class IssuerError(ValueError):
    pass


def list_issuers(db):
    return db.query(QuotationIssuer).order_by(QuotationIssuer.name, QuotationIssuer.id).all()


def serialize_issuer(profile):
    return {"id": profile.id, **{field: getattr(profile, field) for field in ISSUER_FIELDS}}


def save_issuer(db, values):
    fields = {field: str(values.get(f"issuer_{field}") or "").strip() for field in ISSUER_FIELDS}
    if not fields["name"]:
        raise IssuerError("Nhập tên đơn vị báo giá trước khi lưu.")
    if any(len(value) > 500 for value in fields.values()):
        raise IssuerError("Mỗi ô thông tin đơn vị báo giá tối đa 500 ký tự.")
    fingerprint = sha256(json.dumps(fields, ensure_ascii=False, sort_keys=True).encode("utf-8")).hexdigest()
    existing = db.query(QuotationIssuer).filter_by(fingerprint=fingerprint).first()
    if existing is not None:
        return existing, False
    profile = QuotationIssuer(**fields, fingerprint=fingerprint)
    db.add(profile)
    try:
        db.commit()
    except IntegrityError:
        # Another open app can save the identical profile at the same time.
        db.rollback()
        existing = db.query(QuotationIssuer).filter_by(fingerprint=fingerprint).first()
        if existing is None:
            raise
        return existing, False
    db.refresh(profile)
    return profile, True
