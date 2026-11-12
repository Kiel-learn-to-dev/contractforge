import json
from decimal import Decimal
from urllib.parse import quote

from fastapi import APIRouter, File, Request, UploadFile
from fastapi.responses import HTMLResponse, RedirectResponse, Response
from fastapi.templating import Jinja2Templates

from app.database import SessionLocal
from app.paths import TEMPLATES_DIR
from app.models.contract_template import ContractTemplate
from app.models.product import Product
from app.services import product_service
from app.services.product_service import ProductError

router = APIRouter(prefix="/products", tags=["products"])
templates = Jinja2Templates(directory=str(TEMPLATES_DIR))


def _flash(url, msg, t="success"):
    sep = "&" if "?" in url else "?"
    # Mã hóa: tên sản phẩm có thể chứa "&" hoặc "#", làm gãy query string.
    return RedirectResponse(
        f"{url}{sep}msg={quote(msg, safe='')}&msg_type={t}", status_code=303)


def _fv(request):
    return {"flash_msg": request.query_params.get("msg", ""),
            "flash_type": request.query_params.get("msg_type", "success")}


def _contract_templates_for_form(db, group: str):
    """Mẫu hợp đồng cho ô "mẫu mặc định". Có nhóm: mẫu của nhóm và mẫu dùng chung.
    Chưa có nhóm: mọi mẫu hợp đồng, mẫu của nhóm nào thì ghi kèm tên nhóm."""
    from app.services.template_service import ROLE_CONTRACT, has_role, list_templates, templates_for
    if (group or "").strip():
        return templates_for(db, group.strip(), ROLE_CONTRACT)
    return [t for t in list_templates(db, active_only=True)
            if has_role(t.contract_type or "", ROLE_CONTRACT)]


def _group_summary(db, groups) -> dict:
    """Với mỗi nhóm: số mẫu riêng của từng vai trò, hoặc "thiếu" nếu chưa có."""
    from app.services.template_service import (
        ROLE_ADDON, ROLE_ATTACHMENT, ROLE_CONTRACT, ROLE_RENEWAL, templates_for,
    )
    roles = [("Hợp đồng", ROLE_CONTRACT), ("Phụ lục bổ sung", ROLE_ADDON),
             ("Phụ lục gia hạn", ROLE_RENEWAL), ("Kèm theo", ROLE_ATTACHMENT)]
    out = {}
    for g in groups:
        parts = []
        for label, role in roles:
            own = [t for t in templates_for(db, g, role, active_only=True) if t.product_group == g]
            parts.append((label, f"{len(own)} mẫu riêng" if own else "thiếu"))
        out[g] = parts
    return out


def _form_context(db, *, title: str, action: str, values: dict,
                  errors=None, product_id=None):
    """Dữ liệu chung cho form thêm và form sửa."""
    templates_active = _contract_templates_for_form(db, values.get("group_code") or "")
    other_products = [p for p in product_service.list_products(db) if p.id != product_id]
    return {
        "page_title": title,
        "form_title": title,
        "action": action,
        "values": values,
        "errors": errors or [],
        "templates": templates_active,
        "products": other_products,
        "kind_labels": product_service.KIND_LABELS,
        "source_labels": product_service.QUANTITY_SOURCE_LABELS,
    }


SHOW_CHOICES = ("active", "inactive", "all")


def _normalize_show(raw: str) -> str:
    return raw if raw in SHOW_CHOICES else "active"


@router.get("", response_class=HTMLResponse)
def product_list(request: Request, show: str = "active"):
    show = _normalize_show(show)
    db = SessionLocal()
    try:
        products = product_service.list_products(db)
        counts = product_service.in_service_counts(db)
    finally:
        db.close()

    if show == "active":
        products = [p for p in products if p.is_active]
    elif show == "inactive":
        products = [p for p in products if not p.is_active]

    groups = sorted({p.group_code for p in products if p.group_code})
    with_db = SessionLocal()
    try:
        group_summary = _group_summary(with_db, groups)
    finally:
        with_db.close()

    return templates.TemplateResponse(request, "products/list.html", {
        "page_title": "Sản phẩm",
        "products": products,
        "group_summary": group_summary,
        "counts": counts,
        "show": show,
        "kind_labels": product_service.KIND_LABELS,
        **_fv(request),
    })


@router.post("/{product_id}/toggle-active")
async def product_toggle_active(request: Request, product_id: int):
    form = await request.form()
    show = _normalize_show(str(form.get("show", "active")))

    db = SessionLocal()
    try:
        try:
            product = product_service.toggle_active(db, product_id)
        except ProductError:
            return HTMLResponse("Không tìm thấy sản phẩm.", status_code=404)
        name = product.name
        now_active = bool(product.is_active)
    finally:
        db.close()

    msg = f"Đã bán lại {name}" if now_active else f"Đã ngừng bán {name}"
    return _flash(f"/products?show={show}", msg)


# /new khai báo TRƯỚC các route có {product_id}.

@router.get("/new", response_class=HTMLResponse)
def product_new_form(request: Request):
    db = SessionLocal()
    try:
        context = _form_context(
            db, title="Thêm sản phẩm", action="/products/new",
            values={"kind": "main", "quantity_source": "units",
                    "price_period_months": "1", "default_vat_rate": "0",
                    "sort_order": "0"},
        )
    finally:
        db.close()
    return templates.TemplateResponse(request, "products/form.html", context)


@router.post("/new", response_class=HTMLResponse)
async def product_new_submit(request: Request):
    form = await request.form()
    data = dict(form)

    db = SessionLocal()
    try:
        try:
            product = product_service.create_product(db, data)
        except ProductError as exc:
            context = _form_context(
                db, title="Thêm sản phẩm", action="/products/new",
                values=data, errors=exc.errors,
            )
            return templates.TemplateResponse(
                request, "products/form.html", context, status_code=422)
        name = product.name
    finally:
        db.close()

    return _flash("/products", f"Đã thêm sản phẩm {name}")


# ─── Excel: tải mẫu, xuất, nhập (khai báo trước các route /{product_id}) ─────

XLSX_MEDIA = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"


def _xlsx_response(content: bytes, filename: str) -> Response:
    return Response(content=content, media_type=XLSX_MEDIA,
                    headers={"Content-Disposition": f'attachment; filename="{filename}"'})


def _revive_rows(rows: list) -> list[dict]:
    """Đọc lại dòng xem trước từ JSON. Giá và thuế phải về Decimal."""
    out = []
    for r in rows:
        if not isinstance(r, dict):
            raise ValueError("dòng không phải đối tượng")
        data = r.get("data")
        if isinstance(data, dict):
            data = dict(data)
            for key in ("default_price", "default_vat_rate"):
                if data.get(key) not in (None, ""):
                    data[key] = Decimal(str(data[key]))
        out.append({**r, "data": data})
    return out


@router.get("/template.xlsx")
def product_template_xlsx():
    return _xlsx_response(product_service.make_catalog_xlsx(None, with_data=False),
                          "product_template.xlsx")


@router.get("/export.xlsx")
def product_export_xlsx():
    db = SessionLocal()
    try:
        content = product_service.make_catalog_xlsx(db, with_data=True)
    finally:
        db.close()
    return _xlsx_response(content, "product_catalog.xlsx")


@router.get("/import", response_class=HTMLResponse)
def product_import_form(request: Request):
    return templates.TemplateResponse(request, "products/import.html", {
        "page_title": "Nhập danh mục",
        "rows": None,
        "errors": [],
        "columns": product_service.IMPORT_COLUMNS,
        **_fv(request),
    })


@router.post("/import", response_class=HTMLResponse)
async def product_import_preview(request: Request, file: UploadFile = File(...)):
    content = await file.read()
    db = SessionLocal()
    try:
        try:
            rows = product_service.parse_catalog_xlsx(db, content)
        except ProductError as exc:
            return templates.TemplateResponse(request, "products/import.html", {
                "page_title": "Nhập danh mục", "rows": None,
                "errors": exc.errors, "columns": product_service.IMPORT_COLUMNS,
            }, status_code=422)
    finally:
        db.close()

    summary = {
        "create": sum(1 for r in rows if r["action"] == "create"),
        "update": sum(1 for r in rows if r["action"] == "update"),
        "error": sum(1 for r in rows if r["action"] == "error"),
    }
    return templates.TemplateResponse(request, "products/import.html", {
        "page_title": "Nhập danh mục",
        "rows": rows,
        "summary": summary,
        "errors": [],
        "rows_json": json.dumps(rows, ensure_ascii=False, default=str),
        "columns": product_service.IMPORT_COLUMNS,
    })


@router.post("/import/apply")
async def product_import_apply(request: Request):
    form = await request.form()
    raw = str(form.get("rows_json", ""))

    try:
        rows = _revive_rows(json.loads(raw))
    except (ValueError, TypeError, ArithmeticError):
        return _flash("/products/import", "Dữ liệu xem trước không đọc được. Hãy tải file lên lại.", "danger")

    db = SessionLocal()
    try:
        try:
            created, updated = product_service.apply_catalog_rows(db, rows)
        except ProductError as exc:
            return _flash("/products/import",
                          "Chưa ghi file nào. " + "; ".join(exc.errors)[:500], "danger")
    finally:
        db.close()

    return _flash("/products", f"Đã tạo {created} sản phẩm, cập nhật {updated} sản phẩm.")


@router.get("/{product_id}/edit", response_class=HTMLResponse)
def product_edit_form(request: Request, product_id: int):
    db = SessionLocal()
    try:
        product = product_service.get_product(db, product_id)
        if product is None:
            return HTMLResponse("Không tìm thấy sản phẩm.", status_code=404)
        context = _form_context(
            db, title="Sửa sản phẩm", action=f"/products/{product_id}/edit",
            values=product_service.form_values(product), product_id=product_id,
        )
    finally:
        db.close()
    return templates.TemplateResponse(request, "products/form.html", context)


@router.post("/{product_id}/edit", response_class=HTMLResponse)
async def product_edit_submit(request: Request, product_id: int):
    form = await request.form()
    data = dict(form)

    db = SessionLocal()
    try:
        try:
            product = product_service.update_product(db, product_id, data)
        except ProductError as exc:
            if product_service.get_product(db, product_id) is None:
                return HTMLResponse("Không tìm thấy sản phẩm.", status_code=404)
            context = _form_context(
                db, title="Sửa sản phẩm", action=f"/products/{product_id}/edit",
                values=data, errors=exc.errors, product_id=product_id,
            )
            return templates.TemplateResponse(
                request, "products/form.html", context, status_code=422)
        name = product.name
    finally:
        db.close()

    return _flash("/products", f"Đã lưu sản phẩm {name}")
