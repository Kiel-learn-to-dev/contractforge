"""Yêu cầu báo giá: văn bản gửi đối tác để họ TỰ điền thông tin và đơn giá.

Điểm cốt lõi cần khóa: dữ liệu ứng dụng đưa vào văn bản KHÔNG BAO GIỜ có tiền. Xem
QUOTE_REQUEST_PLAN.md và app/services/quote_request.py.
"""

from __future__ import annotations

import re
from datetime import date
from io import BytesIO

import pytest
from docx import Document

# Giá cố ý lẻ và dễ nhận ra: nếu nó lọt vào văn bản thì quét chuỗi sẽ thấy.
PRICE = 777_123
PRICE_FORMS = ("777.123", "777,123", "777123")
MONEY_WORDS = ("PRICE", "AMOUNT", "TOTAL", "VAT", "BANK", "BUDGET")


def _all_text(docx_bytes: bytes) -> str:
    """Toàn bộ chữ trong file: đoạn, bảng, đầu trang, chân trang."""
    doc = Document(BytesIO(docx_bytes))
    chunks = [p.text for p in doc.paragraphs]
    for table in doc.tables:
        for row in table.rows:
            chunks.extend(cell.text for cell in row.cells)
    for section in doc.sections:
        for part in (section.header, section.footer):
            chunks.extend(p.text for p in part.paragraphs)
    return "\n".join(chunks)


def _form(customer, product, template, **over):
    data = {
        "customer_id": customer.id,
        "product_id": product.id,
        "template_id": template.id,
        "request_date": "2026-07-11",
        "deadline": "2026-07-20",
        "month_count": 12,
        "subject": "",
        "addressee": "",
        "note": "",
        "main_name": "",
        "main_qty": "",
        "addons_json": "",
    }
    data.update(over)
    return data


@pytest.fixture()
def samples(db_session):
    from app.services.quote_request import install_sample_templates

    return install_sample_templates(db_session)


# ─── Hàng rào: không có tiền ─────────────────────────────────────────────────

def test_context_and_lists_carry_no_money(db_session, make_customer, make_product):
    from app.services.quote_request import (
        build_request_context, build_request_lists, resolve_request_items,
    )

    customer = make_customer(tax_code="0312345678", bank_account="9999")
    product = make_product(default_price=PRICE, default_vat_rate=8,
                           feature_list=[{"name": "Chức năng A", "description": "Mô tả A"}])
    _, items, features = resolve_request_items(db_session, customer, product.id, 12)

    ctx = build_request_context(customer, date(2026, 7, 11), date(2026, 7, 20), month_count=12)
    lists = build_request_lists(items, features)

    for key in ctx:
        assert not any(word in key for word in MONEY_WORDS), key
    assert set(lists["items"][0]) == {"ITEM_NO", "ITEM_NAME", "ITEM_UNIT", "ITEM_QTY"}
    blob = repr(ctx) + repr(lists)
    assert not any(form in blob for form in PRICE_FORMS)
    assert "9999" not in blob  # số tài khoản của khách hàng cũng không đi vào văn bản gửi đối tác


def test_context_blank_fields_are_empty_not_none(make_customer):
    from app.services.quote_request import ADDRESSEE_BLANK, build_request_context

    customer = make_customer(tax_code=None, phone=None, email=None, contact_name=None,
                             representative_name=None)
    ctx = build_request_context(customer, date(2026, 7, 11), None, month_count=6)

    for key in ("PARTY_A_TAX_CODE", "PARTY_A_PHONE", "PARTY_A_EMAIL",
                "PARTY_A_CONTACT_NAME", "PARTY_A_REPRESENTATIVE", "REQUEST_DEADLINE"):
        assert ctx[key] == ""
    assert "None" not in repr(ctx)
    assert ctx["REQUEST_ADDRESSEE"] == ADDRESSEE_BLANK  # chỗ để viết tay


# ─── Chọn hạng mục ───────────────────────────────────────────────────────────

def test_default_quantity_follows_units_and_period(db_session, make_customer, make_product):
    from app.services.quote_request import build_request_lists, resolve_request_items

    customer = make_customer()  # một trạm chính, không đơn vị con
    product = make_product(unit_label="Đơn vị", price_period_months=1)
    _, items, features = resolve_request_items(db_session, customer, product.id, 12)

    assert items[0].unit_label == "Đơn vị"
    assert build_request_lists(items, features)["items"][0]["ITEM_QTY"] == "12"


def test_item_name_override_and_manual_quantity(db_session, make_customer, make_product):
    from app.services.quote_request import resolve_request_items

    customer = make_customer()
    product = make_product(name="Tên nội bộ")
    _, items, _ = resolve_request_items(
        db_session, customer, product.id, 6, main_name="Phần mềm quản lý tương đương", main_qty="3")

    assert items[0].name == "Phần mềm quản lý tương đương"
    assert items[0].quantity == 3


@pytest.mark.parametrize("raw", ["NaN", "sNaN", "Infinity", "-Infinity"])
def test_request_rejects_non_finite_quantity(db_session, make_customer, make_product, raw):
    from app.services.quote_request import QuoteRequestError, resolve_request_items

    customer = make_customer()
    product = make_product()
    with pytest.raises(QuoteRequestError, match="Số lượng.*không hợp lệ"):
        resolve_request_items(db_session, customer, product.id, 12, main_qty=raw)


@pytest.mark.parametrize("period", [0, -1, -12])
def test_request_rejects_invalid_catalog_period(db_session, make_customer, make_product, period):
    from app.services.quote_request import QuoteRequestError, resolve_request_items

    customer = make_customer()
    product = make_product(price_period_months=period)
    with pytest.raises(QuoteRequestError, match="Kỳ.*không hợp lệ"):
        resolve_request_items(db_session, customer, product.id, 12)


def test_addon_json_zero_quantity_is_not_replaced_by_default(
        db_session, make_customer, make_product):
    from app.routers.quote_request import _parse_addons
    from app.services.quote_request import QuoteRequestError, resolve_request_items

    customer = make_customer()
    main = make_product(group_code="G1")
    addon = make_product(group_code="G1", kind="addon")
    addons = _parse_addons('[{"product_id": %d, "qty": 0}]' % addon.id)
    with pytest.raises(QuoteRequestError, match="Số lượng.*lớn hơn 0"):
        resolve_request_items(db_session, customer, main.id, 12, addons=addons)


def test_addon_rules(db_session, make_customer, make_product):
    from app.services.quote_request import QuoteRequestError, resolve_request_items

    customer = make_customer()
    main = make_product(group_code="G1", kind="main")
    ok = make_product(group_code="G1", kind="addon", name="Gói thêm")
    other_group = make_product(group_code="G2", kind="addon")
    another_main = make_product(group_code="G1", kind="main")

    _, items, _ = resolve_request_items(
        db_session, customer, main.id, 12, addons=[{"product_id": ok.id}])
    assert [i.name for i in items][1] == "Gói thêm"

    with pytest.raises(QuoteRequestError, match="cùng nhóm"):
        resolve_request_items(db_session, customer, main.id, 12,
                              addons=[{"product_id": other_group.id}])
    with pytest.raises(QuoteRequestError, match="gói chính"):
        resolve_request_items(db_session, customer, main.id, 12,
                              addons=[{"product_id": another_main.id}])


def test_main_must_be_active_main_and_months_in_range(db_session, make_customer, make_product):
    from app.services.quote_request import QuoteRequestError, resolve_request_items

    customer = make_customer()
    inactive = make_product(is_active=False)
    addon = make_product(kind="addon")
    ok = make_product()

    for pid in (inactive.id, addon.id, 999_999):
        with pytest.raises(QuoteRequestError, match="Sản phẩm chính"):
            resolve_request_items(db_session, customer, pid, 12)
    for months in (0, 121):
        with pytest.raises(QuoteRequestError, match="Số tháng"):
            resolve_request_items(db_session, customer, ok.id, months)


# ─── Loại mẫu ────────────────────────────────────────────────────────────────

def test_request_templates_are_kept_apart_from_quotation_and_contract(
        db_session, make_template, make_product):
    from app.routers.quotation import list_quotation_templates, resolve_quotation_selection
    from app.services import template_service
    from app.services.quote_request import (
        QuoteRequestError, list_request_templates, resolve_request_template,
    )

    request_tpl = make_template(contract_type="Yêu cầu báo giá")
    quotation_tpl = make_template(contract_type="Báo giá")
    contract_tpl = make_template(contract_type="Hợp đồng")

    assert [t.id for t in list_request_templates(db_session, None)] == [request_tpl.id]
    assert request_tpl.id not in [t.id for t in list_quotation_templates(db_session)]
    assert request_tpl.id not in [
        t.id for t in template_service.templates_for(db_session, None, template_service.ROLE_CONTRACT)]

    assert resolve_request_template(db_session, request_tpl.id, None).id == request_tpl.id
    for wrong in (quotation_tpl, contract_tpl):
        with pytest.raises(QuoteRequestError):
            resolve_request_template(db_session, wrong.id, None)

    product = make_product()
    with pytest.raises(ValueError):  # trang báo giá cũng từ chối mẫu yêu cầu
        resolve_quotation_selection(db_session, product.id, request_tpl.id)


def test_request_template_must_match_product_group(db_session, make_template):
    from app.services.quote_request import QuoteRequestError, resolve_request_template

    tpl = make_template(contract_type="Yêu cầu báo giá", product_group="G1")
    assert resolve_request_template(db_session, tpl.id, "G1").id == tpl.id
    with pytest.raises(QuoteRequestError, match="nhóm"):
        resolve_request_template(db_session, tpl.id, "G2")


# ─── Mẫu có sẵn ──────────────────────────────────────────────────────────────

def test_install_samples_is_idempotent_and_money_free(db_session, samples):
    from app.services.quote_request import SAMPLE_REQUEST_TEMPLATES, install_sample_templates

    assert len(samples) == len(SAMPLE_REQUEST_TEMPLATES) == 3
    assert install_sample_templates(db_session) == []  # lần hai không tạo thêm

    for tpl in samples:
        assert tpl.contract_type == "Yêu cầu báo giá"
        for placeholder in tpl.detected_placeholders:
            assert not any(word in placeholder for word in MONEY_WORDS), (tpl.code, placeholder)


def test_install_new_sample_layout_preserves_previously_installed_copy(db_session, samples):
    from pathlib import Path
    from app.services.quote_request import install_sample_templates

    old = samples[0]
    path = Path(old.file_path)
    document = Document(path)
    document.add_paragraph("Nội dung đã chỉnh của người dùng")
    document.save(path)
    original = path.read_bytes()
    added = install_sample_templates(db_session)
    assert len(added) == 1
    assert added[0].id != old.id
    assert added[0].code.startswith(old.code + "-")
    assert added[0].contract_type == "Yêu cầu báo giá"
    assert path.read_bytes() == original
    assert install_sample_templates(db_session) == []


def test_analyzer_classifies_request_template_by_its_placeholders():
    from app.services.template_service import _template_kind

    assert _template_kind(["REQUEST_DATE", "ITEM_NAME"]) == "Yêu cầu báo giá"
    assert _template_kind(["QUOTE_DATE", "ITEM_NAME"]) == "Báo giá"


# ─── Sinh file qua giao diện ─────────────────────────────────────────────────

@pytest.mark.parametrize("code", ["SAMPLE-RFQ-LETTER", "SAMPLE-RFQ-FORM", "SAMPLE-RFQ-DETAILED"])
def test_generate_each_sample_has_no_price_and_no_leftover_placeholder(
        client, db_session, make_customer, make_product, samples, code):
    from app.services.template_service import get_by_code

    customer = make_customer(legal_name="Đơn vị Mời Báo Giá", short_name="Đơn vị Mời",
                             tax_code="0300000001", bank_account="123456789")
    product = make_product(
        name="Phần mềm quản lý", default_price=PRICE, default_vat_rate=8, unit_label="Đơn vị",
        feature_list=[{"name": "Quản lý hồ sơ", "description": "Lưu trữ hồ sơ"}])
    template = get_by_code(db_session, code)

    resp = client.post("/quotation/request/generate", data=_form(customer, product, template))

    assert resp.status_code == 200, resp.text[:300]
    assert "wordprocessingml" in resp.headers["content-type"]
    assert "YeuCauBaoGia" in resp.headers["content-disposition"]
    text = _all_text(resp.content)

    assert "{{" not in text and "}}" not in text
    assert "Đơn vị Mời Báo Giá" in text
    assert "Phần mềm quản lý" in text
    assert "20/07/2026" in text  # hạn nhận
    assert not any(form in text for form in PRICE_FORMS)
    assert "123456789" not in text  # tài khoản ngân hàng của khách hàng
    assert "tự xác định" in text  # chân trang: giá do đối tác tự xác định
    assert "None" not in text


@pytest.mark.parametrize("code", ["SAMPLE-RFQ-LETTER", "SAMPLE-RFQ-DETAILED"])
def test_letter_header_table_gets_no_extra_party_rows(
        client, db_session, make_customer, make_product, samples, code):
    """Bộ nhân dòng bên A của renderer nhận bảng có PARTY_A_NAME ở ô đầu là bảng bên A và chèn
    thêm "Mã số thuế". Bảng đầu thư không được rơi vào trường hợp đó."""
    from app.services.template_service import get_by_code

    customer = make_customer(tax_code="0300000001")
    product = make_product()
    resp = client.post("/quotation/request/generate",
                       data=_form(customer, product, get_by_code(db_session, code)))

    head = Document(BytesIO(resp.content)).tables[0]
    assert len(head.rows) == 1
    assert head.rows[0].cells[0].text == customer.legal_name.upper()


def test_detailed_sample_lists_features_and_others_do_not(
        client, db_session, make_customer, make_product, samples):
    from app.services.template_service import get_by_code

    customer = make_customer()
    with_features = make_product(feature_list=[{"name": "Chức năng Đặc Biệt", "description": ""}])
    detailed = get_by_code(db_session, "SAMPLE-RFQ-DETAILED")
    letter = get_by_code(db_session, "SAMPLE-RFQ-LETTER")

    on = client.post("/quotation/request/generate", data=_form(customer, with_features, detailed))
    off = client.post("/quotation/request/generate", data=_form(customer, with_features, letter))
    assert "Chức năng Đặc Biệt" in _all_text(on.content)
    assert "Chức năng Đặc Biệt" not in _all_text(off.content)

    empty = make_product(feature_list=[])
    again = client.post("/quotation/request/generate", data=_form(customer, empty, detailed))
    assert again.status_code == 200
    assert "{{" not in _all_text(again.content)  # bảng chức năng rỗng: dòng mẫu biến mất
    assert not any("Chức năng" in cell.text for table in Document(BytesIO(again.content)).tables
                   for row in table.rows for cell in row.cells)
    assert "Yêu cầu chức năng" not in _all_text(again.content)


def test_generate_with_addon_item(client, db_session, make_customer, make_product, samples):
    import json

    from app.services.template_service import get_by_code

    customer = make_customer()
    main = make_product(group_code="G1", kind="main", name="Gói chính A")
    addon = make_product(group_code="G1", kind="addon", name="Gói thêm B")
    template = get_by_code(db_session, "SAMPLE-RFQ-LETTER")
    addons = json.dumps([{"product_id": addon.id, "name": "Mô-đun bổ sung B", "qty": "5"}])

    resp = client.post("/quotation/request/generate",
                       data=_form(customer, main, template, addons_json=addons))

    text = _all_text(resp.content)
    assert "Gói chính A" in text and "Mô-đun bổ sung B" in text


def test_generate_rejects_bad_input_back_to_form(
        client, db_session, make_customer, make_product, make_template, samples):
    from app.services.template_service import get_by_code

    customer = make_customer()
    product = make_product()
    good = get_by_code(db_session, "SAMPLE-RFQ-FORM")
    quotation_tpl = make_template(contract_type="Báo giá")

    cases = {
        "deadline before date": _form(customer, product, good, deadline="2026-07-01"),
        "wrong template type": _form(customer, product, quotation_tpl),
        "bad month": _form(customer, product, good, month_count=0),
        "unknown customer": _form(customer, product, good, customer_id=999_999),
        "bad addons": _form(customer, product, good, addons_json="not json"),
        "infinite addon id": _form(customer, product, good,
                                   addons_json='[{"product_id": Infinity}]'),
    }
    for name, data in cases.items():
        resp = client.post("/quotation/request/generate", data=data, follow_redirects=False)
        assert resp.status_code == 422, name
        assert 'id="request-form"' in resp.text, name


@pytest.mark.parametrize("addon_quantity", [None, "Infinity", 0])
def test_generate_rejects_invalid_quantities_with_readable_error(
        client, db_session, make_customer, make_product, samples, addon_quantity):
    import json
    from app.services.template_service import get_by_code

    customer = make_customer()
    main = make_product(group_code="G1")
    addon = make_product(group_code="G1", kind="addon")
    template = get_by_code(db_session, "SAMPLE-RFQ-FORM")
    fields = ({"main_qty": "NaN"} if addon_quantity is None else {
        "addons_json": json.dumps([{"product_id": addon.id, "qty": addon_quantity}])})
    response = client.post("/quotation/request/generate",
                           data=_form(customer, main, template, **fields), follow_redirects=False)
    assert response.status_code == 422
    assert "Số lượng" in response.text
    assert "Lỗi khi sinh file" not in response.text


@pytest.mark.parametrize("invalid", [{"deadline": "2026-07-01"}, {"month_count": "abc"},
                                     {"customer_id": ""}, {"main_qty": "NaN"}])
def test_validation_error_preserves_form_values(
        client, db_session, make_customer, make_product, samples, invalid):
    import json
    from html.parser import HTMLParser
    from app.services.template_service import get_by_code

    class Controls(HTMLParser):
        def __init__(self, html):
            super().__init__()
            self.inputs = {}
            self.selected = []
            self.feed(html)

        def handle_starttag(self, tag, attrs):
            attrs = dict(attrs)
            if tag == "input":
                self.inputs[attrs.get("id")] = attrs
            if tag == "option" and "selected" in attrs:
                self.selected.append(attrs["value"])

    customer = make_customer()
    main = make_product(group_code="G1")
    addon = make_product(group_code="G1", kind="addon")
    template = get_by_code(db_session, "SAMPLE-RFQ-DETAILED")
    payload = _form(customer, main, template, month_count="6", main_name='Dịch vụ <mẫu> "A"',
                    main_qty="1,5", addressee="Đối tác A", subject="Mời chào giá",
                    note="Ghi chú giữ lại", addons_json=json.dumps([
                        {"product_id": addon.id, "name": "Bổ sung tùy chỉnh", "qty": "3,5"}]))
    payload.update(invalid)
    response = client.post("/quotation/request/generate", data=payload)
    assert response.status_code == 422
    controls = Controls(response.text)
    for field in ("main_name", "main_qty", "month_count", "request_date", "deadline",
                  "addressee", "subject"):
        assert controls.inputs[field]["value"] == payload[field]
    assert str(main.id) in controls.selected and str(template.id) in controls.selected
    assert "checked" in controls.inputs[f"addon-{addon.id}"]
    assert "Ghi chú giữ lại" in response.text
    assert 'value="Bổ sung tùy chỉnh"' in response.text
    assert 'value="3,5"' in response.text
    assert 'value="Dịch vụ <mẫu>' not in response.text
    payload.update(customer_id=customer.id, month_count="6", deadline="2026-07-20", main_qty="1,5")
    fixed = client.post("/quotation/request/generate", data=payload)
    assert fixed.status_code == 200
    assert "wordprocessingml" in fixed.headers["content-type"]
    assert "Ghi chú giữ lại" in _all_text(fixed.content)


def test_form_page_and_install_samples_route(client, db_session, make_customer, make_product):
    page = client.get("/quotation/request", follow_redirects=False)
    assert page.status_code == 303
    assert page.headers["location"] == "/quotation"

    resp = client.post("/quotation/request/install-samples", follow_redirects=False)
    assert resp.status_code == 303
    assert "msg_type=success" in resp.headers["location"]

    again = client.get("/quotation/request")
    assert "Cài 3 mẫu có sẵn" not in again.text
    assert 'value="compact"' in again.text

    repeat = client.post("/quotation/request/install-samples", follow_redirects=False)
    assert "msg_type=info" in repeat.headers["location"]


def test_sample_files_are_shipped_and_free_of_money_placeholders():
    from app.paths import ASSETS_DIR
    from app.services.quote_request import SAMPLE_REQUEST_TEMPLATES
    from app.services.template_analyzer import analyze

    for filename, *_ in SAMPLE_REQUEST_TEMPLATES:
        result = analyze(str(ASSETS_DIR / "default_templates" / filename))
        assert result.ok, filename
        assert not [p for p in result.placeholders if any(w in p for w in MONEY_WORDS)], filename
        assert re.search(r"REQUEST_DATE", " ".join(result.placeholders)), filename
