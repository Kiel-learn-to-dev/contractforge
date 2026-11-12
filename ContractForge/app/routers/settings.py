from fastapi import APIRouter, Request, Form
from fastapi.responses import HTMLResponse, RedirectResponse
from fastapi.templating import Jinja2Templates

from app.database import SessionLocal
from app.paths import TEMPLATES_DIR
from app.services.settings_service import get_party_b_settings, get_setting, set_setting
from app.services.numbering import (
    DEFAULT_ADDENDUM_FORMAT, InvalidNumberFormat, validate_addendum_format,
)
from app.services.customer_service import list_deleted_customers

router = APIRouter()
templates = Jinja2Templates(directory=str(TEMPLATES_DIR))


@router.get("/settings", response_class=HTMLResponse)
def settings_page(request: Request):
    db = SessionLocal()
    try:
        pbs = get_party_b_settings(db)
        addendum_format = get_setting(db, "addendum_number_format", DEFAULT_ADDENDUM_FORMAT)
        deleted_customers = list_deleted_customers(db)
    finally:
        db.close()
    return templates.TemplateResponse("settings/index.html", {
        "request": request,
        "page_title": "Cài đặt",
        "party_b": pbs,
        "addendum_format": addendum_format,
        "deleted_customers": deleted_customers,
        "flash_msg": request.query_params.get("flash"),
        "flash_type": request.query_params.get("ft", "success"),
        "setup_required": request.query_params.get("setup") == "1",
    })


@router.post("/settings/party-b", response_class=HTMLResponse)
def save_party_b(
    request: Request,
    # ── Thông tin tổ chức ──────────────────────────────────────────────────
    party_b_name:         str = Form(""),
    party_b_address:      str = Form(""),
    party_b_bank_account: str = Form(""),
    party_b_bank_name:    str = Form(""),
    party_b_beneficiary:  str = Form(""),
    party_b_tax_code:     str = Form(""),
    party_b_title:        str = Form(""),
    # ── Giấy ủy quyền ──────────────────────────────────────────────────────
    party_b_representative:       str = Form(""),
    party_b_authorization_number: str = Form(""),
    party_b_authorization_date:   str = Form(""),
    # ── Đầu mối liên hệ ────────────────────────────────────────────────────
    party_b_phone:          str = Form(""),
    party_b_contact_name:   str = Form(""),
    party_b_contact_email:  str = Form(""),
):
    db = SessionLocal()
    try:
        for key, val in [
            ("party_b_name",                 party_b_name),
            ("party_b_address",              party_b_address),
            ("party_b_bank_account",         party_b_bank_account),
            ("party_b_bank_name",            party_b_bank_name),
            ("party_b_beneficiary",          party_b_beneficiary),
            ("party_b_tax_code",             party_b_tax_code),
            ("party_b_title",                party_b_title),
            ("party_b_representative",       party_b_representative),
            ("party_b_authorization_number", party_b_authorization_number),
            ("party_b_authorization_date",   party_b_authorization_date),
            ("party_b_phone",                party_b_phone),
            ("party_b_contact_name",         party_b_contact_name),
            ("party_b_contact_email",        party_b_contact_email),
        ]:
            set_setting(db, key, val.strip())
    finally:
        db.close()
    return RedirectResponse(
        url="/settings?flash=Đã lưu thông tin Bên B thành công.&ft=success",
        status_code=303,
    )


@router.post("/settings/addendum-format", response_class=HTMLResponse)
def save_addendum_format(addendum_number_format: str = Form("")):
    fmt = addendum_number_format.strip()
    try:
        validate_addendum_format(fmt)
    except InvalidNumberFormat as exc:
        return RedirectResponse(
            url=f"/settings?flash={str(exc)}&ft=danger",
            status_code=303,
        )
    db = SessionLocal()
    try:
        set_setting(db, "addendum_number_format", fmt)
    finally:
        db.close()
    return RedirectResponse(
        url="/settings?flash=Đã lưu khuôn số phụ lục.&ft=success",
        status_code=303,
    )
