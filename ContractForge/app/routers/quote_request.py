"""
Router: /quotation/request — Yêu cầu báo giá

GET  /quotation/request                  → form
POST /quotation/request/generate         → sinh file .docx để gửi đối tác tự điền
POST /quotation/request/install-samples  → cài các mẫu hư cấu đi kèm vào danh mục mẫu

Văn bản này KHÔNG chứa giá: xem app/services/quote_request.py.
"""

import json
from datetime import date
from urllib.parse import urlencode

from fastapi import APIRouter, Form, Request
from fastapi.responses import HTMLResponse, RedirectResponse, Response
from fastapi.templating import Jinja2Templates
from sqlalchemy.orm import joinedload

from app.database import SessionLocal
from app.models.customer import Customer
from app.models.product import Product
from app.paths import TEMPLATES_DIR
from app.routers.quotation import _content_disposition
from app.services import quote_request as svc
from app.services.docx_renderer import apply_template_mapping, render_docx
from app.utils.date_helpers import parse_date_input

router = APIRouter(prefix="/quotation/request", tags=["quote-request"])
templates = Jinja2Templates(directory=str(TEMPLATES_DIR))

DOCX_MEDIA = "application/vnd.openxmlformats-officedocument.wordprocessingml.document"


def _back(msg: str, kind: str = "danger"):
    return RedirectResponse(
        f"/quotation/request?{urlencode({'msg': msg, 'msg_type': kind})}", status_code=303)


def _parse_addons(raw: str) -> list[dict]:
    """addons_json từ form: [{"product_id": 3, "name": "...", "qty": ""}]. Sai định dạng → lỗi đọc được."""
    if not (raw or "").strip():
        return []
    try:
        data = json.loads(raw)
    except ValueError:
        raise svc.QuoteRequestError("Danh sách gói bổ sung không hợp lệ.")
    if not isinstance(data, list):
        raise svc.QuoteRequestError("Danh sách gói bổ sung không hợp lệ.")
    out = []
    for entry in data:
        if not isinstance(entry, dict):
            raise svc.QuoteRequestError("Danh sách gói bổ sung không hợp lệ.")
        try:
            pid = int(entry.get("product_id"))
        except (TypeError, ValueError, OverflowError):
            raise svc.QuoteRequestError("Danh sách gói bổ sung không hợp lệ.")
        out.append({"product_id": pid, "name": str(entry.get("name") or ""),
                    "qty": str(entry["qty"] if entry.get("qty") is not None else "")})
    return out


def _render_form(request: Request, values=None, error="", status_code=200):
    values = values or {}
    try:
        selected_addons = {str(item["product_id"]): item
                           for item in _parse_addons(values.get("addons_json", ""))}
    except svc.QuoteRequestError:
        selected_addons = {}
    db = SessionLocal()
    try:
        customers = (
            db.query(Customer)
            .options(joinedload(Customer.sub_units))
            .filter(Customer.is_active == True, Customer.is_deleted == False)  # noqa: E712
            .order_by(Customer.code)
            .all()
        )
        products = (
            db.query(Product)
            .filter(Product.is_active == True)  # noqa: E712
            .order_by(Product.sort_order, Product.name, Product.code)
            .all()
        )
        request_templates = svc.list_request_templates_any_group(db)
        return templates.TemplateResponse("quotation/request_form.html", {
            "request": request,
            "page_title": "Yêu cầu báo giá",
            "customers": customers,
            "main_products": [p for p in products if p.kind == "main"],
            "addon_products": [p for p in products if p.kind != "main"],
            "request_templates": request_templates,
            "sample_count": len(svc.SAMPLE_REQUEST_TEMPLATES),
            "today": date.today().isoformat(),
            "values": values,
            "selected_addons": selected_addons,
            "flash_msg": error or request.query_params.get("msg", ""),
            "flash_type": "danger" if error else request.query_params.get("msg_type", "danger"),
        }, status_code=status_code)
    finally:
        db.close()


@router.get("", response_class=HTMLResponse)
def request_form(request: Request):
    return RedirectResponse("/quotation", status_code=303)


@router.post("/install-samples")
def install_samples():
    db = SessionLocal()
    try:
        try:
            created = svc.install_sample_templates(db)
        except (svc.QuoteRequestError, ValueError) as exc:
            return _back(str(exc))
        if not created:
            return _back("Các mẫu có sẵn đã được cài từ trước.", "info")
        return _back(f"Đã cài {len(created)} mẫu có sẵn.", "success")
    finally:
        db.close()


@router.post("/generate")
def request_generate(
    request: Request,
    customer_id: str = Form(""),
    product_id: str = Form(""),
    template_id: str = Form(""),
    request_date: str = Form(""),
    deadline: str = Form(""),
    month_count: str = Form(""),
    subject: str = Form(""),
    addressee: str = Form(""),
    note: str = Form(""),
    main_name: str = Form(""),
    main_qty: str = Form(""),
    addons_json: str = Form(""),
):
    values = {"customer_id": customer_id, "product_id": product_id, "template_id": template_id,
              "request_date": request_date, "deadline": deadline, "month_count": month_count,
              "subject": subject, "addressee": addressee, "note": note,
              "main_name": main_name, "main_qty": main_qty, "addons_json": addons_json}

    def invalid(message):
        return _render_form(request, values, message, status_code=422)

    numbers = {}
    for field, label in (("customer_id", "Khách hàng"), ("product_id", "Gói chính"),
                         ("template_id", "Mẫu DOCX"), ("month_count", "Số tháng")):
        try:
            numbers[field] = int(values[field])
        except (ValueError, TypeError):
            return invalid(f"{label} không hợp lệ hoặc chưa được chọn.")
    customer_id, product_id, template_id, month_count = (
        numbers[key] for key in ("customer_id", "product_id", "template_id", "month_count"))
    db = SessionLocal()
    try:
        customer = (
            db.query(Customer)
            .options(joinedload(Customer.sub_units))
            .filter(Customer.id == customer_id, Customer.is_deleted == False,
                    Customer.is_active == True)  # noqa: E712
            .first()
        )
        if customer is None:
            return invalid("Không tìm thấy khách hàng đang sử dụng.")

        try:
            rdate = parse_date_input(request_date)
            ddate = parse_date_input(deadline)
        except Exception:
            rdate = ddate = None
        if rdate is None:
            return invalid("Ngày lập không hợp lệ.")
        if ddate is None:
            return invalid("Hạn nhận báo giá không hợp lệ.")
        if ddate < rdate:
            return invalid("Hạn nhận báo giá không được trước ngày lập.")

        try:
            main, items, feature_lists = svc.resolve_request_items(
                db, customer, product_id, month_count,
                main_name=main_name, main_qty=main_qty, addons=_parse_addons(addons_json),
            )
            template = svc.resolve_request_template(db, template_id, main.group_code)
        except svc.QuoteRequestError as exc:
            return invalid(str(exc))

        ctx = svc.build_request_context(
            customer, rdate, ddate, month_count=month_count,
            addressee=addressee, note=note,
            subject=subject.strip() or f"Mời báo giá {items[0].name}",
        )
        ctx = apply_template_mapping(ctx, template)
        lists = svc.build_request_lists(items, feature_lists)
        docx_bytes = render_docx(template.file_path, ctx, lists=lists,
                                omit_empty_tables={"features"})

        slug = (customer.short_name or customer.legal_name or "KH").replace(" ", "_")[:30]
        filename = f"YeuCauBaoGia_{slug}_{rdate.strftime('%Y%m%d')}.docx"
        return Response(
            content=docx_bytes,
            media_type=DOCX_MEDIA,
            headers={"Content-Disposition": _content_disposition(filename)},
        )
    except Exception as exc:  # noqa: BLE001 — người dùng cần thấy lỗi trên form, không phải trang 500
        return invalid(f"Lỗi khi sinh file: {exc}")
    finally:
        db.close()
