"""
Router: /templates — CRUD mẫu hợp đồng + phân tích placeholder.

Routes:
  GET  /templates                   → danh sách
  GET  /templates/new               → form upload
  POST /templates/new               → xử lý upload + phân tích
  GET  /templates/{id}              → chi tiết + bảng placeholder/mapping
  GET  /templates/{id}/edit         → form sửa metadata
  POST /templates/{id}/edit         → lưu metadata
  POST /templates/{id}/mapping      → lưu field_mapping đã sửa
  POST /templates/{id}/reanalyze    → phân tích lại
  POST /templates/{id}/toggle       → bật/tắt
  GET  /templates/{id}/download     → tải file .docx về
"""

import os
from fastapi import APIRouter, Request, Form, UploadFile, File, HTTPException
from fastapi.responses import HTMLResponse, RedirectResponse, FileResponse
from fastapi.templating import Jinja2Templates

from app.database import SessionLocal
from app.paths import TEMPLATES_DIR
from app.services.template_service import (
    list_templates, get_template, get_by_code,
    create_template, update_metadata, update_mapping,
    reanalyze, toggle_active, generate_next_code,
    CONTRACT_TYPES, ALL_CANONICAL_FIELDS,
)

router = APIRouter(prefix="/templates", tags=["templates"])
templates_env = Jinja2Templates(directory=str(TEMPLATES_DIR))


# ─── Flash helpers ───────────────────────────────────────────────────────────

def _flash_redirect(url: str, msg: str, msg_type: str = "success") -> RedirectResponse:
    sep = "&" if "?" in url else "?"
    return RedirectResponse(f"{url}{sep}msg={msg}&msg_type={msg_type}", status_code=303)


def _extract_flash(request: Request) -> dict:
    return {
        "flash_msg":  request.query_params.get("msg", ""),
        "flash_type": request.query_params.get("msg_type", "success"),
    }


# ─── LIST ────────────────────────────────────────────────────────────────────

@router.get("", response_class=HTMLResponse)
def template_list(request: Request, active_only: int = 0):
    db = SessionLocal()
    try:
        tpls = list_templates(db, active_only=bool(active_only))
        # Group by contract_type
        grouped: dict[str, list] = {}
        for t in tpls:
            grouped.setdefault(t.contract_type, []).append(t)
    finally:
        db.close()

    return templates_env.TemplateResponse(request, "contract_templates/list.html", {
        "page_title": "Mẫu hợp đồng",
        "grouped": grouped,
        "total": len(tpls),
        "active_only": bool(active_only),
        **_extract_flash(request),
    })


# ─── NEW (upload form + submit) ──────────────────────────────────────────────

@router.get("/new", response_class=HTMLResponse)
def new_form(request: Request):
    db = SessionLocal()
    try:
        next_code = generate_next_code(db)
    finally:
        db.close()

    return templates_env.TemplateResponse(request, "contract_templates/form.html", {
        "page_title": "Mẫu hợp đồng",
        "form_title": "Upload mẫu hợp đồng mới",
        "action": "/templates/new",
        "tpl": None,
        "next_code": next_code,
        "contract_types": CONTRACT_TYPES,
        "errors": [],
    })


@router.post("/new")
async def new_submit(
    request: Request,
    file: UploadFile = File(...),
    code: str = Form(""),
    name: str = Form(...),
    contract_type: str = Form(...),
    description: str = Form(""),
):
    file_bytes = await file.read()

    db = SessionLocal()
    try:
        tpl, analysis = create_template(
            db, code=code, name=name, contract_type=contract_type,
            file_bytes=file_bytes, original_filename=file.filename,
            description=description,
        )
    except ValueError as e:
        db.close()
        db2 = SessionLocal()
        nxt = generate_next_code(db2)
        db2.close()
        return templates_env.TemplateResponse(request, "contract_templates/form.html", {
            "page_title": "Mẫu hợp đồng",
            "form_title": "Upload mẫu hợp đồng mới",
            "action": "/templates/new",
            "tpl": None,
            "next_code": nxt,
            "contract_types": CONTRACT_TYPES,
            "errors": [str(e)],
            "prefill": {"code": code, "name": name,
                        "contract_type": contract_type, "description": description},
        }, status_code=422)
    finally:
        db.close()

    ph_count = len(analysis.placeholders)
    warn_count = len(analysis.split_run_warnings)
    msg = f"Upload thành công. Phát hiện {ph_count} placeholder"
    if warn_count:
        msg += f" ({warn_count} split-run warning)"

    return _flash_redirect(f"/templates/{tpl.id}", msg, "success")


# ─── DETAIL ──────────────────────────────────────────────────────────────────

@router.get("/{template_id}", response_class=HTMLResponse)
def template_detail(request: Request, template_id: int):
    db = SessionLocal()
    try:
        tpl = get_template(db, template_id)
        if not tpl:
            raise HTTPException(status_code=404, detail="Không tìm thấy mẫu hợp đồng")
        # Build display data: list of (placeholder, canonical, is_mapped)
        ph_rows = [
            {
                "placeholder": ph,
                "canonical": tpl.field_mapping.get(ph, ""),
                "mapped": bool(tpl.field_mapping.get(ph, "")),
            }
            for ph in tpl.detected_placeholders
        ]
        mapped_count = sum(1 for r in ph_rows if r["mapped"])
    finally:
        db.close()

    return templates_env.TemplateResponse(request, "contract_templates/detail.html", {
        "page_title": "Mẫu hợp đồng",
        "tpl": tpl,
        "ph_rows": ph_rows,
        "mapped_count": mapped_count,
        "total_ph": len(ph_rows),
        "canonical_fields": ALL_CANONICAL_FIELDS,
        **_extract_flash(request),
    })


# ─── EDIT METADATA ───────────────────────────────────────────────────────────

@router.get("/{template_id}/edit", response_class=HTMLResponse)
def edit_form(request: Request, template_id: int):
    db = SessionLocal()
    try:
        tpl = get_template(db, template_id)
        if not tpl:
            raise HTTPException(status_code=404, detail="Không tìm thấy mẫu hợp đồng")
    finally:
        db.close()

    return templates_env.TemplateResponse(request, "contract_templates/form.html", {
        "page_title": "Mẫu hợp đồng",
        "form_title": f"Sửa thông tin — {tpl.name}",
        "action": f"/templates/{template_id}/edit",
        "tpl": tpl,
        "next_code": tpl.code,
        "contract_types": CONTRACT_TYPES,
        "errors": [],
    })


@router.post("/{template_id}/edit")
async def edit_submit(
    request: Request,
    template_id: int,
    name: str = Form(...),
    contract_type: str = Form(...),
    description: str = Form(""),
):
    db = SessionLocal()
    try:
        tpl = update_metadata(db, template_id, name=name,
                              contract_type=contract_type, description=description)
    except (ValueError, LookupError) as e:
        db2 = SessionLocal()
        orig = get_template(db2, template_id)
        db2.close()
        db.close()
        return templates_env.TemplateResponse(request, "contract_templates/form.html", {
            "page_title": "Mẫu hợp đồng",
            "form_title": "Sửa thông tin",
            "action": f"/templates/{template_id}/edit",
            "tpl": orig,
            "next_code": orig.code if orig else "",
            "contract_types": CONTRACT_TYPES,
            "errors": [str(e)],
        }, status_code=422)
    finally:
        db.close()

    return _flash_redirect(f"/templates/{tpl.id}", "Đã cập nhật thông tin template", "success")


# ─── UPDATE MAPPING ──────────────────────────────────────────────────────────

@router.post("/{template_id}/mapping")
async def save_mapping(request: Request, template_id: int):
    """
    Nhận form data dạng: mapping[PLACEHOLDER_NAME] = canonical_field_value
    """
    form = await request.form()
    mapping: dict[str, str] = {}
    for key, value in form.items():
        if key.startswith("mapping[") and key.endswith("]"):
            ph = key[8:-1]   # strip "mapping[" and "]"
            mapping[ph] = str(value).strip()

    db = SessionLocal()
    try:
        tpl = update_mapping(db, template_id, mapping)
    except (ValueError, LookupError) as e:
        raise HTTPException(status_code=400, detail=str(e))
    finally:
        db.close()

    mapped = sum(1 for v in tpl.field_mapping.values() if v)
    total = len(tpl.field_mapping)
    return _flash_redirect(
        f"/templates/{template_id}",
        f"Đã lưu mapping ({mapped}/{total} trường đã map)",
        "success",
    )


# ─── REANALYZE ───────────────────────────────────────────────────────────────

@router.post("/{template_id}/reanalyze")
def do_reanalyze(request: Request, template_id: int):
    db = SessionLocal()
    try:
        tpl, result = reanalyze(db, template_id)
    except (LookupError, FileNotFoundError, ValueError) as e:
        raise HTTPException(status_code=400, detail=str(e))
    finally:
        db.close()

    return _flash_redirect(
        f"/templates/{template_id}",
        f"Phân tích lại thành công — {len(result.placeholders)} placeholder",
        "success",
    )


# ─── TOGGLE ──────────────────────────────────────────────────────────────────

@router.post("/{template_id}/toggle")
def toggle(request: Request, template_id: int):
    db = SessionLocal()
    try:
        tpl = toggle_active(db, template_id)
    except LookupError as e:
        raise HTTPException(status_code=404, detail=str(e))
    finally:
        db.close()

    status_text = "kích hoạt" if tpl.is_active else "tạm ngưng"
    return _flash_redirect(
        f"/templates/{template_id}",
        f"Đã {status_text} template {tpl.code}",
        "success" if tpl.is_active else "warning",
    )


# ─── DOWNLOAD ────────────────────────────────────────────────────────────────

@router.get("/{template_id}/download")
def download(template_id: int):
    db = SessionLocal()
    try:
        tpl = get_template(db, template_id)
        if not tpl:
            raise HTTPException(status_code=404, detail="Không tìm thấy template")
        if not tpl.file_path or not os.path.exists(tpl.file_path):
            raise HTTPException(status_code=404, detail="File không tồn tại trên server")
        file_path = tpl.file_path
        file_name = tpl.file_name or os.path.basename(file_path)
    finally:
        db.close()

    return FileResponse(
        path=file_path,
        filename=file_name,
        media_type="application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    )
