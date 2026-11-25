"""Five actual quotation layouts with independent, explicitly entered prices."""
from datetime import date

from fastapi import APIRouter, Request
from fastapi.responses import HTMLResponse, JSONResponse, RedirectResponse, Response
from fastapi.templating import Jinja2Templates
from sqlalchemy.orm import joinedload
from sqlalchemy.exc import SQLAlchemyError

from app.database import SessionLocal
from app.models.customer import Customer
from app.models.product import Product
from app.paths import TEMPLATES_DIR
from app.routers.quotation import _content_disposition
from app.services import manual_quotation as svc, pricing
from app.services import quotation_issuer_service as issuer_service
from app.services.docx_renderer import render_docx

router = APIRouter(prefix="/quotation/manual", tags=["quotation"])
templates = Jinja2Templates(directory=str(TEMPLATES_DIR))
DOCX_MEDIA = "application/vnd.openxmlformats-officedocument.wordprocessingml.document"


def _render_form(request, values=None, error="", status_code=200):
    values = dict(values or {})
    values.setdefault("layout", request.query_params.get("layout", "compact"))
    values.setdefault("quote_date", date.today().isoformat())
    try:
        rows = svc.parse_rows(values.get("items_json", ""))
    except pricing.PricingError:
        rows = [{"name": "", "unit": "", "quantity": "1", "periods": "1", "price": "", "vat": "0"}]
    with SessionLocal() as db:
        customers = db.query(Customer).options(joinedload(Customer.sub_units)).filter(
            Customer.is_active == True, Customer.is_deleted == False).order_by(Customer.code).all()
        products = db.query(Product).filter(Product.is_active == True, Product.kind == "main").order_by(Product.name).all()
        customer_data = {str(c.id): dict(name=c.legal_name, address=c.full_address(),
                                        tax_code=c.tax_code or "", contact=c.contact_name or "") for c in customers}
        product_data = {str(p.id): dict(name=p.name, unit=p.unit_label or "", features=p.feature_list) for p in products}
        saved_issuers = [issuer_service.serialize_issuer(profile) for profile in issuer_service.list_issuers(db)]
        return templates.TemplateResponse("quotation/manual_form.html", {
            "request": request, "page_title": "Báo giá", "layouts": svc.LAYOUTS,
            "values": values, "rows": rows, "customers": customers, "products": products,
            "customer_data": customer_data, "product_data": product_data, "error": error,
            "issuer_fields": svc.ISSUER_FIELDS,
            "saved_issuers": saved_issuers,
        }, status_code=status_code)


@router.get("", response_class=HTMLResponse)
def manual_form(request: Request):
    if request.query_params.get("layout") == "legacy":
        return RedirectResponse("/quotation/legacy", status_code=303)
    return _render_form(request)


@router.get("/templates/{layout}")
def download_template(layout: str):
    try:
        path = svc.template_path(layout)
    except pricing.PricingError:
        return JSONResponse({"error": "Không có mẫu báo giá này."}, status_code=404)
    return Response(path.read_bytes(), media_type=DOCX_MEDIA,
                    headers={"Content-Disposition": _content_disposition(f"MauBaoGia_{layout}.docx")})


@router.post("/preview")
async def preview(request: Request):
    values = dict(await request.form())
    try:
        _, totals = svc.build_lines(values.get("items_json", ""))
        return {"pre_vat": pricing.fmt_money(totals.pre_vat), "vat": pricing.fmt_money(totals.vat),
                "total": pricing.fmt_money(totals.total), "amount_text": totals.amount_text}
    except pricing.PricingError as exc:
        return JSONResponse({"error": str(exc)}, status_code=422)


@router.post("/issuers")
async def save_issuer(request: Request):
    values = dict(await request.form())
    try:
        with SessionLocal() as db:
            profile, created = issuer_service.save_issuer(db, values)
            return JSONResponse({"issuer": issuer_service.serialize_issuer(profile), "created": created},
                                status_code=201 if created else 200)
    except issuer_service.IssuerError as exc:
        return JSONResponse({"error": str(exc)}, status_code=422)
    except SQLAlchemyError:
        return JSONResponse({"error": "Chưa lưu được đơn vị báo giá. Vui lòng thử lại."}, status_code=503)


@router.post("/generate")
async def generate(request: Request):
    values = dict(await request.form())
    try:
        features = []
        with SessionLocal() as db:
            if values.get("product_id"):
                try:
                    product_id = int(values["product_id"])
                except (ValueError, TypeError):
                    raise pricing.PricingError("Sản phẩm không hợp lệ.")
                product = db.query(Product).filter(Product.id == product_id, Product.is_active == True,
                                                    Product.kind == "main").first()
                if product is None:
                    raise pricing.PricingError("Sản phẩm không tồn tại hoặc đã ngừng sử dụng.")
                features = product.feature_list
        if str(values.get("features_text", "")).strip():
            features = []
            for raw in values["features_text"].splitlines():
                name, _, description = raw.partition("|")
                if name.strip():
                    features.append({"name": name.strip(), "description": description.strip()})
        context, lists = svc.build_document(values, features)
        content = render_docx(str(svc.template_path(values.get("layout"))), context, lists=lists,
                              omit_empty_tables={"features"})
        return Response(content, media_type=DOCX_MEDIA, headers={"Content-Disposition": _content_disposition(
            f"BaoGia_{values['layout']}_{values.get('quote_date', '')}.docx")})
    except pricing.PricingError as exc:
        return _render_form(request, values, str(exc), status_code=422)
    except (OSError, ValueError) as exc:
        return _render_form(request, values, f"Không sinh được báo giá: {exc}", status_code=422)
