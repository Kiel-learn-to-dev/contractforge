from urllib.parse import quote

from fastapi import APIRouter, Request
from fastapi.responses import HTMLResponse, RedirectResponse
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


def _form_context(db, *, title: str, action: str, values: dict,
                  errors=None, product_id=None):
    """Dữ liệu chung cho form thêm và form sửa."""
    templates_active = (
        db.query(ContractTemplate)
        .filter(ContractTemplate.is_active == True)  # noqa: E712
        .order_by(ContractTemplate.name)
        .all()
    )
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

    return templates.TemplateResponse(request, "products/list.html", {
        "page_title": "Sản phẩm",
        "products": products,
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
