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

import hashlib
import os
from urllib.parse import quote

from fastapi import APIRouter, Request, Form, UploadFile, File, HTTPException
from fastapi.responses import HTMLResponse, RedirectResponse, FileResponse
from fastapi.templating import Jinja2Templates

from app.database import SessionLocal
from app.models.product import Product
from app.paths import TEMPLATES_DIR
from app.services.template_analyzer import analyze
from app.services.template_service import (
    list_templates, get_template, get_by_code,
    create_template, update_metadata, update_mapping,
    reanalyze, toggle_active, generate_next_code,
    CONTRACT_TYPES, ALL_CANONICAL_FIELDS,
    _document_heading, _existing_template_with_bytes, _template_kind,
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


#: Nhãn nhóm của mẫu dùng chung (không thuộc sản phẩm nào). Luôn xếp cuối.
SHARED_GROUP_LABEL = "Dùng chung"


def _group_choices(db) -> list[str]:
    """Các nhóm sản phẩm đang có (mọi trạng thái sản phẩm), đã sắp xếp."""
    rows = (db.query(Product.group_code)
            .filter(Product.group_code.isnot(None), Product.group_code != "")
            .distinct().order_by(Product.group_code).all())
    return [r[0] for r in rows]


def _check_group(raw, choices) -> str:
    """Nhóm gửi lên phải là một nhóm đang có. Rỗng nghĩa là dùng chung."""
    value = (raw or "").strip()
    if value and value not in choices:
        raise ValueError("Nhóm sản phẩm không tồn tại.")
    return value


# ─── LIST ────────────────────────────────────────────────────────────────────

@router.get("", response_class=HTMLResponse)
def template_list(request: Request, active_only: int = 0):
    db = SessionLocal()
    try:
        tpls = list_templates(db, active_only=bool(active_only))
        # Nhóm sản phẩm trước, rồi loại. Mẫu dùng chung nằm dưới khóa riêng và đứng cuối.
        flat: dict[str, dict[str, list]] = {}
        for t in tpls:
            label = t.product_group or SHARED_GROUP_LABEL
            flat.setdefault(label, {}).setdefault(t.contract_type, []).append(t)
        grouped: dict[str, dict[str, list]] = {
            k: flat[k] for k in sorted(k for k in flat if k != SHARED_GROUP_LABEL)}
        if SHARED_GROUP_LABEL in flat:
            grouped[SHARED_GROUP_LABEL] = flat[SHARED_GROUP_LABEL]
        group_choices = _group_choices(db)
    finally:
        db.close()

    return templates_env.TemplateResponse(request, "contract_templates/list.html", {
        "page_title": "Mẫu hợp đồng",
        "grouped": grouped,
        "group_choices": group_choices,
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
        group_choices = _group_choices(db)
    finally:
        db.close()

    return templates_env.TemplateResponse(request, "contract_templates/form.html", {
        "page_title": "Mẫu hợp đồng",
        "form_title": "Upload mẫu hợp đồng mới",
        "action": "/templates/new",
        "tpl": None,
        "next_code": next_code,
        "contract_types": CONTRACT_TYPES,
        "group_choices": group_choices,
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
    product_group: str = Form(""),
):
    file_bytes = await file.read()

    db = SessionLocal()
    try:
        group = _check_group(product_group, _group_choices(db))
        tpl, analysis = create_template(
            db, code=code, name=name, contract_type=contract_type,
            file_bytes=file_bytes, original_filename=file.filename,
            description=description, product_group=group,
        )
    except ValueError as e:
        db.close()
        db2 = SessionLocal()
        nxt = generate_next_code(db2)
        choices = _group_choices(db2)
        db2.close()
        return templates_env.TemplateResponse(request, "contract_templates/form.html", {
            "page_title": "Mẫu hợp đồng",
            "form_title": "Upload mẫu hợp đồng mới",
            "action": "/templates/new",
            "tpl": None,
            "next_code": nxt,
            "contract_types": CONTRACT_TYPES,
            "group_choices": choices,
            "errors": [str(e)],
            "prefill": {"code": code, "name": name,
                        "contract_type": contract_type, "description": description,
                        "product_group": product_group},
        }, status_code=422)
    finally:
        db.close()

    ph_count = len(analysis.placeholders)
    warn_count = len(analysis.split_run_warnings)
    msg = f"Upload thành công. Phát hiện {ph_count} placeholder"
    if warn_count:
        msg += f" ({warn_count} split-run warning)"

    return _flash_redirect(f"/templates/{tpl.id}", msg, "success")


# ─── BULK UPLOAD (nhiều file, một nhóm) ──────────────────────────────────────

@router.post("/bulk-upload")
async def bulk_upload(files: list[UploadFile] = File(...), product_group: str = Form("")):
    """Tải nhiều mẫu một lượt cho cùng một nhóm. Loại nhận ra từ nội dung; trùng nội dung
    thì bỏ qua. Một file lỗi không chặn các file khác."""
    db = SessionLocal()
    try:
        try:
            group = _check_group(product_group, _group_choices(db))
        except ValueError as exc:
            return _flash_redirect("/templates", str(exc), "danger")

        lines: list[str] = []
        created = 0
        for upload in files:
            fname = upload.filename or ""
            data = await upload.read()
            name = os.path.splitext(os.path.basename(fname))[0].replace("_", " ").strip() or fname
            try:
                analysis = analyze(data)
                if not analysis.ok:
                    raise ValueError(analysis.error or "không đọc được file")
                kind = _template_kind(analysis.placeholders, _document_heading(data))
                existing = _existing_template_with_bytes(db, hashlib.sha256(data).hexdigest())
                if existing is not None:
                    lines.append(f"{fname}: bỏ qua, trùng với '{existing.name}'")
                    continue
                create_template(db, code="", name=name, contract_type=kind, file_bytes=data,
                                original_filename=fname, product_group=group)
                created += 1
                lines.append(f"{fname}: {kind}")
            except Exception as exc:
                db.rollback()
                lines.append(f"{fname}: lỗi — {exc}")
    finally:
        db.close()

    summary = f"Đã tải {created}/{len(files)} mẫu. " + "; ".join(lines)
    return _flash_redirect("/templates", quote(summary[:400], safe=""),
                           "success" if created == len(files) else "warning")


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
        from app.services.template_analyzer import analyze
        analysis = analyze(tpl.file_path) if tpl.file_path else None
    finally:
        db.close()

    return templates_env.TemplateResponse(request, "contract_templates/detail.html", {
        "page_title": "Mẫu hợp đồng",
        "tpl": tpl,
        "ph_rows": ph_rows,
        "mapped_count": mapped_count,
        "total_ph": len(ph_rows),
        "canonical_fields": ALL_CANONICAL_FIELDS,
        "repeat_groups": analysis.repeat_groups if analysis and analysis.ok else [],
        "row_warnings": analysis.row_warnings if analysis and analysis.ok else [],
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
        group_choices = _group_choices(db)
    finally:
        db.close()

    return templates_env.TemplateResponse(request, "contract_templates/form.html", {
        "page_title": "Mẫu hợp đồng",
        "form_title": f"Sửa thông tin — {tpl.name}",
        "action": f"/templates/{template_id}/edit",
        "tpl": tpl,
        "next_code": tpl.code,
        "contract_types": _type_choices(tpl.contract_type),
        "group_choices": group_choices,
        "errors": [],
    })


def _type_choices(current: str | None) -> list[str]:
    """Danh sách loại để chọn. Nhãn cũ đang có (ví dụ "Phụ lục") được giữ lên đầu, không bị đổi ngầm."""
    if current and current not in CONTRACT_TYPES:
        return [current, *CONTRACT_TYPES]
    return list(CONTRACT_TYPES)


@router.post("/{template_id}/edit")
async def edit_submit(
    request: Request,
    template_id: int,
    name: str = Form(...),
    contract_type: str = Form(...),
    description: str = Form(""),
    product_group: str = Form(""),
):
    db = SessionLocal()
    try:
        group = _check_group(product_group, _group_choices(db))
        tpl = update_metadata(db, template_id, name=name, contract_type=contract_type,
                              description=description, product_group=group)
    except (ValueError, LookupError) as e:
        db2 = SessionLocal()
        orig = get_template(db2, template_id)
        choices = _group_choices(db2)
        db2.close()
        db.close()
        return templates_env.TemplateResponse(request, "contract_templates/form.html", {
            "page_title": "Mẫu hợp đồng",
            "form_title": "Sửa thông tin",
            "action": f"/templates/{template_id}/edit",
            "tpl": orig,
            "next_code": orig.code if orig else "",
            "contract_types": _type_choices(orig.contract_type if orig else None),
            "group_choices": choices,
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
async def toggle(request: Request, template_id: int):
    form = await request.form()
    # Ô bật/tắt trong danh sách gửi next="/templates" để ở lại danh sách.
    # Chỉ nhận đường dẫn nội bộ thuộc /templates.
    next_url = str(form.get("next", "")).strip()
    back = next_url if next_url.startswith("/templates") and "//" not in next_url else f"/templates/{template_id}"
    db = SessionLocal()
    try:
        tpl = toggle_active(db, template_id)
    except LookupError as e:
        raise HTTPException(status_code=404, detail=str(e))
    finally:
        db.close()

    status_text = "kích hoạt" if tpl.is_active else "tạm ngưng"
    return _flash_redirect(
        back,
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
