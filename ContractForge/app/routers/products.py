from fastapi import APIRouter, Request
from fastapi.responses import HTMLResponse
from fastapi.templating import Jinja2Templates

from app.database import SessionLocal
from app.paths import TEMPLATES_DIR
from app.models.product import Product

router = APIRouter(prefix="/products", tags=["products"])
templates = Jinja2Templates(directory=str(TEMPLATES_DIR))


@router.get("", response_class=HTMLResponse)
def product_list(request: Request):
    db = SessionLocal()
    try:
        products = db.query(Product).order_by(Product.is_active.desc(), Product.name.asc()).all()
    finally:
        db.close()

    return templates.TemplateResponse(request, "products/list.html", {
        "page_title": "Sản phẩm",
        "products": products,
    })
