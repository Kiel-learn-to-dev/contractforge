"""
app/templating_patcher.py — Monkeypatch Jinja2Templates để auto-inject globals.

Import module này TRƯỚC khi bất kỳ router nào được import trong main.py.
Sau khi import, MỌI Jinja2Templates() mới tạo sẽ tự động có party_b_defaults.

Cơ chế: Wrap Jinja2Templates.__init__ để gọi _register_globals sau mỗi lần khởi tạo.
"""

from fastapi.templating import Jinja2Templates as _OriginalJinja2Templates

_original_init = _OriginalJinja2Templates.__init__


def _patched_init(self, *args, **kwargs):
    _original_init(self, *args, **kwargs)
    _register_globals(self)


def _register_globals(tmpl_instance):
    """Đăng ký tất cả globals cần thiết."""
    def _party_b_defaults():
        from app.database import SessionLocal
        from app.services.settings_service import get_party_b_settings, PARTY_B_DEFAULTS
        db = SessionLocal()
        try:
            return get_party_b_settings(db)
        except Exception:
            return {
                "representative":       PARTY_B_DEFAULTS["party_b_representative"],
                "authorization_number": PARTY_B_DEFAULTS["party_b_authorization_number"],
                "authorization_date":   PARTY_B_DEFAULTS["party_b_authorization_date"],
            }
        finally:
            db.close()

    tmpl_instance.env.globals["party_b_defaults"] = _party_b_defaults


# Apply patch
_OriginalJinja2Templates.__init__ = _patched_init
