"""
Router: /quotation — Hợp đồng báo giá

GET  /quotation           → form nhập thông tin báo giá
POST /quotation/generate  → sinh file .docx và trả về download
"""

import re
import unicodedata
from datetime import date
from decimal import Decimal, ROUND_HALF_UP
from urllib.parse import quote

from fastapi import APIRouter, Request, Form
from fastapi.responses import HTMLResponse, Response
from fastapi.templating import Jinja2Templates
from sqlalchemy.orm import joinedload

from app.database import SessionLocal
from app.models.customer import Customer
from app.models.product import Product
from app.models.contract_template import ContractTemplate
from app.paths import TEMPLATES_DIR
from app.services import contract_service, pricing
from app.services.docx_renderer import apply_template_mapping, lists_from_lines, render_docx, _fmt_number
from app.services.template_service import QUOTATION_TYPE, ROLE_QUOTATION, check_template_for
from app.utils.amount_to_words_vi import amount_to_words
from app.utils.date_helpers import split_date_parts, parse_date_input

router = APIRouter(prefix="/quotation", tags=["quotation"])
templates = Jinja2Templates(directory=str(TEMPLATES_DIR))

def list_quotation_products(db) -> list[Product]:
    """List active products that can provide quotation price and VAT data."""
    return (
        db.query(Product)
        .filter(Product.is_active == True)
        .order_by(Product.name, Product.code)
        .all()
    )


def list_quotation_templates(db) -> list[ContractTemplate]:
    """Mẫu DOCX dùng được cho báo giá — CHỈ mẫu mang nhãn báo giá.

    Trước đây hàm này trả về mọi mẫu đang bật, kể cả mẫu hợp đồng và biểu nghiệm
    thu. Kết hợp với đoạn JS tự chọn `product.default_template_id` (vốn trỏ vào
    mẫu HỢP ĐỒNG của sản phẩm), chọn sản phẩm xong là ô mẫu tự nhảy sang mẫu hợp
    đồng — bấm sinh file ra một bản hợp đồng mang tên "BaoGia_...docx".
    """
    return (
        db.query(ContractTemplate)
        .filter(
            ContractTemplate.is_active == True,
            ContractTemplate.contract_type == QUOTATION_TYPE,
        )
        .order_by(ContractTemplate.name, ContractTemplate.code)
        .all()
    )


def product_price_with_vat(product: Product) -> Decimal:
    """Calculate one unit's VAT-inclusive price from product configuration."""
    price = Decimal(str(product.default_price or 0))
    vat = Decimal(str(product.default_vat_rate or 0))
    return (price * (Decimal("1") + vat / Decimal("100"))).quantize(
        Decimal("1"), rounding=ROUND_HALF_UP
    )


def resolve_quotation_selection(db, product_id: int, template_id: int):
    """Resolve an active product and active template selected by the user."""
    product = (
        db.query(Product)
        .filter(Product.id == product_id, Product.is_active == True)
        .first()
    )
    if product is None:
        raise ValueError("Sản phẩm không tồn tại hoặc đã ngừng sử dụng.")
    if product.default_price is None or Decimal(str(product.default_price)) <= 0:
        raise ValueError("Sản phẩm chưa được cấu hình đơn giá hợp lệ.")

    template = (
        db.query(ContractTemplate)
        .filter(
            ContractTemplate.id == template_id,
            ContractTemplate.is_active == True,
        )
        .first()
    )
    if template is None:
        raise ValueError("Mẫu không tồn tại hoặc đã ngừng sử dụng.")
    # Chặn ở tầng server, không chỉ ở dropdown: form cũ còn mở trong tab khác,
    # nút Back, hay mẫu bị đổi nhãn sau khi trang đã tải đều gửi lên được một
    # template_id trỏ vào mẫu hợp đồng. Sinh ra file sai loại mà vẫn đặt tên
    # "BaoGia_..." là kiểu lỗi người dùng chỉ phát hiện sau khi đã gửi cho khách.
    if template.contract_type != QUOTATION_TYPE:
        raise ValueError(
            f"'{template.name}' là mẫu {template.contract_type}, không phải mẫu "
            f"{QUOTATION_TYPE}. Hãy chọn một mẫu {QUOTATION_TYPE}."
        )
    # Mẫu báo giá cũng phải đúng nhóm sản phẩm đã chọn (ném ValueError như trên).
    check_template_for(template, product.group_code, ROLE_QUOTATION)
    return product, template


def _fmt_currency(n: Decimal) -> str:
    """Format số tiền VNĐ: 24000000 → '24.000.000'."""
    return f"{int(n):,}".replace(",", ".")


def _build_context_from_lines(customer, main_product, quote_date, month_count, lines, totals) -> dict:
    """Ngữ cảnh báo giá theo dòng. Tiền lấy từ pricing; khóa đơn lấy từ dòng gói chính."""
    ctx = {}
    ctx["PARTY_A_NAME"] = customer.legal_name or ""
    parts = split_date_parts(quote_date)
    ctx["QUOTE_DATE"]       = quote_date.strftime("%d/%m/%Y")
    ctx["QUOTE_DATE_DAY"]   = parts["day"]
    ctx["QUOTE_DATE_MONTH"] = parts["month"]
    ctx["QUOTE_DATE_YEAR"]  = parts["year"]
    main = lines[0]
    ctx["UNIT_COUNT"]  = _fmt_number(main.quantity)
    ctx["MONTH_COUNT"] = str(month_count)
    ctx["PRODUCT_CODE"] = main_product.code or ""
    ctx["PRODUCT_NAME"] = main_product.name or ""
    ctx["UNIT_PRICE_PRE_VAT"] = _fmt_currency(main.unit_price_pre_vat)
    ctx["VAT_RATE"] = f"{main.vat_rate.normalize()}%"
    ctx["UNIT_PRICE_VAT"] = _fmt_currency(
        pricing.unit_price_with_vat(main.unit_price_pre_vat, main.vat_rate))
    ctx["QUANTITY"] = _fmt_number(main.quantity * main.period_count)
    ctx["TOTAL_PRE_VAT"] = _fmt_currency(totals.pre_vat)
    ctx["TOTAL_VAT"]     = _fmt_currency(totals.vat)
    ctx["TOTAL_AMOUNT"]  = _fmt_currency(totals.total)
    ctx["AMOUNT_TEXT"]   = totals.amount_text
    ctx["PRICING_UNITS"] = "; ".join(customer.unit_names_list())
    ctx["PRICING_DESCRIPTION"] = pricing.describe(lines, totals)
    return ctx


def _items_context_and_lists(db, customer, quote_date, month_count, raw_items: str):
    """Đường báo giá nhiều dòng: cùng _build_lines và sum_lines với hợp đồng."""
    items = contract_service._parse_items_json(raw_items)
    lines = contract_service._build_lines(db, items, month_count)
    totals = pricing.sum_lines(lines)
    main_product = db.get(Product, lines[0].product_id)
    feature_lists = []
    for line in lines:
        p = db.get(Product, line.product_id) if line.product_id else None
        feature_lists.append(p.feature_list if p else [])
    lists = lists_from_lines(lines, feature_lists)
    ctx = _build_context_from_lines(customer, main_product, quote_date, month_count, lines, totals)
    return ctx, lists


def _content_disposition(filename: str) -> str:
    """
    Sinh Content-Disposition header đúng chuẩn RFC 5987 cho filename có Unicode.

    HTTP/1.1 yêu cầu header value phải latin-1-safe. Tên file tiếng Việt chứa
    ký tự ngoài latin-1 nên phải dùng cả hai tham số:
      - filename=  : ASCII fallback (NFD → bỏ combining diacritics), cho client cũ
      - filename*= : UTF-8 percent-encoded (RFC 5987), cho browser hiện đại

    File content (nội dung .docx) không bị ảnh hưởng — chỉ header tên file thay đổi.
    """
    # ASCII fallback: decompose NFD → strip combining diacritics → thay ký tự lạ
    nfkd = unicodedata.normalize('NFKD', filename)
    ascii_name = re.sub(r'[^\w\-.]', '_', nfkd.encode('ascii', 'ignore').decode('ascii'))
    # UTF-8 encoded: giữ nguyên tiếng Việt, percent-encode cho header
    utf8_name = quote(filename.encode('utf-8'), safe='-._()')
    return f"attachment; filename=\"{ascii_name}\"; filename*=UTF-8''{utf8_name}"


def _build_context(
    customer: Customer,
    product: Product,
    quote_date: date,
    month_count: int,
) -> dict:
    """Build a neutral quotation context from customer and product data."""
    ctx = {}

    # Thông tin khách hàng
    ctx["PARTY_A_NAME"] = customer.legal_name or ""

    # Ngày báo giá
    parts = split_date_parts(quote_date)
    ctx["QUOTE_DATE"]       = quote_date.strftime("%d/%m/%Y")
    ctx["QUOTE_DATE_DAY"]   = parts["day"]
    ctx["QUOTE_DATE_MONTH"] = parts["month"]
    ctx["QUOTE_DATE_YEAR"]  = parts["year"]

    # Số cơ sở và số tháng
    unit_count = customer.total_units
    ctx["UNIT_COUNT"]  = str(unit_count)
    ctx["MONTH_COUNT"] = str(month_count)

    # Sản phẩm và đơn giá
    unit_price_pre_vat = Decimal(str(product.default_price or 0))
    vat_rate = Decimal(str(product.default_vat_rate or 0))
    unit_price_vat = product_price_with_vat(product)
    ctx["PRODUCT_CODE"] = product.code or ""
    ctx["PRODUCT_NAME"] = product.name or ""
    ctx["UNIT_PRICE_PRE_VAT"] = _fmt_currency(unit_price_pre_vat)
    ctx["VAT_RATE"] = f"{vat_rate.normalize()}%"
    ctx["UNIT_PRICE_VAT"] = _fmt_currency(unit_price_vat)

    # Tổng tiền
    # legacy: báo giá một gói không có dòng (đường cũ, product_price_with_vat)
    total = unit_price_vat * Decimal(unit_count) * Decimal(month_count)
    ctx["TOTAL_AMOUNT"] = _fmt_currency(total)

    # Bằng chữ
    try:
        ctx["AMOUNT_TEXT"] = amount_to_words(total)
    except Exception:
        ctx["AMOUNT_TEXT"] = ""

    # Tổng số đơn vị-tháng
    ctx["QUANTITY"] = str(unit_count * month_count)

    # Danh sách tên cơ sở
    names = customer.unit_names_list()

    # Danh sách đơn vị dùng cho mẫu có placeholder chi tiết
    ctx["PRICING_UNITS"] = "; ".join(names)

    # PRICING_DESCRIPTION: mô tả công thức đầy đủ
    unit_detail = f" ({'; '.join(names)})" if len(names) > 1 else ""
    ctx["PRICING_DESCRIPTION"] = (
        f"Số tiền = Đơn giá có VAT ({_fmt_currency(unit_price_vat)} VNĐ) "
        f"× {unit_count} đơn vị{unit_detail} "
        f"× {month_count} tháng = {_fmt_currency(total)} VNĐ"
    )

    return ctx


# ─── GET /quotation ─────────────────────────────────────────────────────────

@router.get("", response_class=HTMLResponse)
def quotation_form(request: Request):
    db = SessionLocal()
    try:
        customers = (
            db.query(Customer)
            .options(joinedload(Customer.sub_units))
            .filter(Customer.is_active == True, Customer.is_deleted == False)
            .order_by(Customer.code)
            .all()
        )
        customer_units = {c.id: c.total_units for c in customers}
        today_str = date.today().isoformat()

        products = list_quotation_products(db)
        quotation_templates = list_quotation_templates(db)

        return templates.TemplateResponse("quotation/form.html", {
            "request": request,
            "page_title": "Hợp đồng báo giá",
            "customers": customers,
            "customer_units": customer_units,
            "products": products,
            "quotation_templates": quotation_templates,
            "today": today_str,
            "flash_msg": request.query_params.get("msg", ""),
            "flash_type": request.query_params.get("msg_type", "danger"),
        })
    finally:
        db.close()


# ─── POST /quotation/generate ────────────────────────────────────────────────

@router.post("/generate")
def quotation_generate(
    request: Request,
    product_id:    int  = Form(...),
    template_id:   int  = Form(...),
    customer_id:   int  = Form(...),
    quote_date:    str  = Form(...),
    month_count:   int  = Form(...),
    items_json:    str  = Form(""),
):
    db = SessionLocal()
    try:
        # Validate customer
        customer = (
            db.query(Customer)
            .options(joinedload(Customer.sub_units))
            .filter(Customer.id == customer_id)
            .first()
        )
        if not customer:
            return _error_redirect(request, "Không tìm thấy khách hàng.")

        try:
            product, template = resolve_quotation_selection(db, product_id, template_id)
        except ValueError as exc:
            return _error_redirect(request, str(exc))

        # Validate quote_date
        try:
            qdate = parse_date_input(quote_date)
            if qdate is None:
                raise ValueError("empty")
        except Exception:
            return _error_redirect(request, "Ngày báo giá không hợp lệ.")

        # Validate month_count
        if month_count < 1 or month_count > 120:
            return _error_redirect(request, "Số tháng phải từ 1 đến 120.")

        if not template.file_path:
            return _error_redirect(request, "Mẫu đã chọn chưa có file DOCX.")

        # Build context và render. items_json có dòng → báo giá nhiều dòng; không có → đường cũ.
        raw_items = items_json if isinstance(items_json, str) else ""
        if raw_items.strip():
            ctx, lists = _items_context_and_lists(db, customer, qdate, month_count, raw_items)
            ctx = apply_template_mapping(ctx, template)
        else:
            ctx = apply_template_mapping(
                _build_context(customer, product, qdate, month_count), template
            )
            lists = None
        docx_bytes = render_docx(template.file_path, ctx, lists=lists)

        # Tên file output
        slug = (customer.short_name or customer.legal_name or "KH").replace(" ", "_")[:30]
        filename = f"BaoGia_{product.code}_{slug}_{qdate.strftime('%Y%m%d')}.docx"

        return Response(
            content=docx_bytes,
            media_type="application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            headers={"Content-Disposition": _content_disposition(filename)},
        )

    except Exception as e:
        return _error_redirect(request, f"Lỗi khi sinh file: {e}")
    finally:
        db.close()


def _error_redirect(request: Request, msg: str):
    from fastapi.responses import RedirectResponse
    return RedirectResponse(
        f"/quotation?msg={msg}&msg_type=danger",
        status_code=303,
    )
