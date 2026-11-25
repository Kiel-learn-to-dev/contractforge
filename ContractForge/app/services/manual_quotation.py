"""Independent quotation input. Never reads catalog prices or organization settings."""
from decimal import DecimalException
import json
import re

from app.paths import ASSETS_DIR
from app.services import pricing
from app.services.docx_renderer import _fmt_number, lists_from_lines
from app.utils.date_helpers import parse_date_input

LAYOUTS = (
    ("compact", "Báo giá gọn", "Bảng giá và thông tin chính, ưu tiên một trang."),
    ("formal", "Báo giá trang trọng", "Đầu văn bản, nội dung chào giá và chữ ký."),
    ("landscape", "Báo giá bảng ngang", "Bảng rộng, tách rõ số lượng, kỳ và thuế."),
    ("features", "Báo giá kèm chức năng", "Bảng giá và phạm vi chức năng của dịch vụ."),
    ("terms", "Báo giá kèm điều kiện cung cấp", "Triển khai, hỗ trợ và điều kiện thanh toán."),
)
LAYOUT_KEYS = frozenset(item[0] for item in LAYOUTS)
ISSUER_FIELDS = ("name", "address", "tax_code", "representative", "title", "phone", "email")


def template_path(layout):
    if layout not in LAYOUT_KEYS:
        raise pricing.PricingError("Mẫu báo giá không hợp lệ.")
    return ASSETS_DIR / "default_templates" / f"sample_quote_{layout}.docx"


def parse_rows(raw):
    try:
        data = json.loads(raw or "[]")
    except (ValueError, TypeError):
        raise pricing.PricingError("Danh sách hạng mục không hợp lệ.")
    if not isinstance(data, list) or not 1 <= len(data) <= 100 or any(not isinstance(row, dict) for row in data):
        raise pricing.PricingError("Báo giá cần từ 1 đến 100 hạng mục.")
    return data


def _number(value, label, *, money=False):
    raw = str(value if value is not None else "").strip().replace(" ", "")
    if "," in raw:
        raw = raw.replace(".", "").replace(",", ".")
    elif money and re.fullmatch(r"\d{1,3}(?:\.\d{3})+", raw):
        raw = raw.replace(".", "")
    result = pricing.to_decimal(raw, label)
    if not result.is_finite():
        raise pricing.PricingError(f"{label} phải là số hữu hạn.")
    return result


def build_lines(raw):
    lines = []
    for index, row in enumerate(parse_rows(raw), 1):
        name = str(row.get("name") or "").strip()
        if not name:
            raise pricing.PricingError(f"Hạng mục {index} thiếu tên.")
        if len(name) > 500:
            raise pricing.PricingError(f"Tên hạng mục {index} quá dài (tối đa 500 ký tự).")
        try:
            lines.append(pricing.price_line(
                description=name, unit_label=str(row.get("unit") or "").strip(),
                quantity=_number(row.get("quantity"), f"Số lượng dòng {index}"),
                months=_number(row.get("periods"), f"Số kỳ dòng {index}"),
                unit_price_pre_vat=_number(row.get("price"), f"Đơn giá dòng {index}", money=True),
                vat_rate=_number(row.get("vat"), f"Thuế suất dòng {index}"),
            ))
        except DecimalException:
            raise pricing.PricingError(f"Giá trị dòng {index} quá lớn hoặc không hợp lệ.")
    return lines, pricing.sum_lines(lines)


def _clean(values, key):
    return str(values.get(key) or "").strip()


def _unit_price_text(value):
    # Keep fractional unit prices: only the line amount is rounded to VND.
    result = format(value, ",f")
    if "." in result:
        result = result.rstrip("0").rstrip(".")
    return result.replace(",", "_").replace(".", ",").replace("_", ".")


def build_document(values, features=None):
    template_path(values.get("layout"))
    if not _clean(values, "issuer_name"):
        raise pricing.PricingError("Nhập tên đơn vị báo giá.")
    if not _clean(values, "recipient_name"):
        raise pricing.PricingError("Nhập tên khách hàng nhận báo giá.")
    try:
        quote_date = parse_date_input(values.get("quote_date", ""))
        valid_until = parse_date_input(values.get("valid_until", "")) if _clean(values, "valid_until") else None
    except (ValueError, TypeError):
        raise pricing.PricingError("Ngày báo giá hoặc ngày hết hiệu lực không hợp lệ.")
    if quote_date is None:
        raise pricing.PricingError("Ngày báo giá không hợp lệ.")
    if _clean(values, "valid_until") and valid_until is None:
        raise pricing.PricingError("Ngày hết hiệu lực không hợp lệ.")
    if valid_until and valid_until < quote_date:
        raise pricing.PricingError("Ngày hết hiệu lực không được trước ngày báo giá.")
    lines, totals = build_lines(values.get("items_json", ""))
    ctx = {f"QUOTE_ISSUER_{field.upper()}": _clean(values, f"issuer_{field}") for field in ISSUER_FIELDS}
    ctx.update({
        "PARTY_A_NAME": _clean(values, "recipient_name"),
        "PARTY_A_ADDRESS": _clean(values, "recipient_address"),
        "PARTY_A_TAX_CODE": _clean(values, "recipient_tax_code"),
        "PARTY_A_CONTACT_NAME": _clean(values, "recipient_contact"),
        "QUOTE_DATE": quote_date.strftime("%d/%m/%Y"),
        "QUOTE_DATE_DAY": str(quote_date.day), "QUOTE_DATE_MONTH": str(quote_date.month),
        "QUOTE_DATE_YEAR": str(quote_date.year),
        "QUOTE_VALID_UNTIL": valid_until.strftime("%d/%m/%Y") if valid_until else "",
        "TOTAL_PRE_VAT": pricing.fmt_money(totals.pre_vat),
        "TOTAL_VAT": pricing.fmt_money(totals.vat), "TOTAL_AMOUNT": pricing.fmt_money(totals.total),
        "AMOUNT_TEXT": totals.amount_text,
    })
    for key, field in (("QUOTE_NUMBER", "quote_number"), ("QUOTE_PLACE", "place"),
                       ("QUOTE_SUBJECT", "subject"), ("QUOTE_PAYMENT_TERMS", "payment_terms"),
                       ("QUOTE_DELIVERY_TERMS", "delivery_terms"), ("QUOTE_SUPPORT_TERMS", "support_terms"),
                       ("QUOTE_NOTE", "note")):
        ctx[key] = _clean(values, field)
    lists = lists_from_lines(lines, [features or []])
    for row, line in zip(lists["items"], lines):
        row["ITEM_QTY"] = _fmt_number(line.quantity)
        row["ITEM_PERIODS"] = _fmt_number(line.period_count)
        row["ITEM_PRICE"] = _unit_price_text(line.unit_price_pre_vat)
    return ctx, lists
