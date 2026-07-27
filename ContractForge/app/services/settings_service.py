"""
settings_service.py — helper đọc/ghi AppSetting.
"""
from sqlalchemy.orm import Session
from app.models.app_setting import AppSetting

# ── Default neutral organization profile ────────────────────────────────────
# A public installation must never prefill another organization's identity.
# Existing AppSetting rows always take precedence and are never overwritten.
PARTY_B_DEFAULTS = {
    "party_b_representative":       "",
    "party_b_authorization_number": "",
    "party_b_authorization_date":   "",
    "party_b_name":                 "",
    "party_b_address":              "",
    "party_b_bank_account":         "",
    "party_b_bank_name":            "",
    "party_b_beneficiary":          "",
    "party_b_tax_code":             "",
    "party_b_title":                "",
}


def get_setting(db: Session, key: str, default: str = "") -> str:
    row = db.query(AppSetting).filter(AppSetting.key == key).first()
    if row is None:
        return default
    return row.value or default


def set_setting(db: Session, key: str, value: str) -> None:
    row = db.query(AppSetting).filter(AppSetting.key == key).first()
    if row:
        row.value = value
    else:
        db.add(AppSetting(key=key, value=value))
    db.commit()


def is_organization_profile_complete(db: Session) -> bool:
    """Return whether the minimum identity needed for a new contract exists."""
    return bool(
        get_setting(db, "party_b_name").strip()
        and get_setting(db, "party_b_representative").strip()
    )


def get_party_b_settings(db: Session) -> dict:
    """Trả về dict đầy đủ thông tin Bên B (dùng cho Jinja2 global và render context)."""
    def _g(key):
        return get_setting(db, key, PARTY_B_DEFAULTS.get(key, ""))
    return {
        # Giấy ủy quyền
        "representative":       _g("party_b_representative"),
        "authorization_number": _g("party_b_authorization_number"),
        "authorization_date":   _g("party_b_authorization_date"),
        # Thông tin tổ chức
        "name":         _g("party_b_name"),
        "address":      _g("party_b_address"),
        "bank_account": _g("party_b_bank_account"),
        "bank_name":    _g("party_b_bank_name"),
        "beneficiary":  _g("party_b_beneficiary"),
        "tax_code":     _g("party_b_tax_code"),
        "title":        _g("party_b_title"),
    }
