"""Giao diện hợp đồng theo nhóm (B5). Nhóm GRP-A, GRP-B hư cấu.

Kiểm bằng HTTP và HTML/JSON của máy chủ. Hành vi JS (đổi gói trên trình duyệt, tick sẵn
khi đổi gói) không có test tự động trong repo; người dùng kiểm ở Điểm dừng 1.
"""

import re
from datetime import date
from decimal import Decimal

import docx
import pytest


def _docx(path, text="Văn bản {{CONTRACT_NUMBER}}"):
    doc = docx.Document()
    doc.add_paragraph(text)
    doc.save(str(path))
    return str(path)


@pytest.fixture()
def ui_world(db_session, make_customer, make_product, make_template, make_contract, tmp_path):
    from app.models.contract import ContractStatus

    customer = make_customer(legal_name="Khách giao diện hư cấu")
    a_main = make_product(code="UI-A-MAIN", name="Chính A", kind="main", group_code="GRP-A",
                          unit_label="cơ sở", default_price=500_000, default_vat_rate=10,
                          default_duration_months=12)
    a_add = make_product(code="UI-A-ADD", name="Bổ sung A", kind="addon", group_code="GRP-A",
                         unit_label="cơ sở", default_price=150_000, default_vat_rate=10)
    b_main = make_product(code="UI-B-MAIN", name="Chính B", kind="main", group_code="GRP-B",
                          unit_label="cơ sở", default_price=700_000, default_vat_rate=10)

    def tpl(name, contract_type, group=None):
        return make_template(name=name, contract_type=contract_type, product_group=group,
                             file_path=_docx(tmp_path / f"{name.replace(' ', '_')}.docx"))

    t = {
        "a_c1": tpl("HĐ A một", "Hợp đồng", "GRP-A"),
        "a_c2": tpl("HĐ A hai", "Hợp đồng", "GRP-A"),
        "a_c3": tpl("HĐ A ba", "Hợp đồng", "GRP-A"),
        "b_c": tpl("HĐ B", "Hợp đồng", "GRP-B"),
        "a_hand": tpl("BB bàn giao A", "Bàn giao", "GRP-A"),
        "b_hand": tpl("BB bàn giao B", "Bàn giao", "GRP-B"),
        "a_ack": tpl("BB nghiệm thu A", "Nghiệm thu", "GRP-A"),
        "a_other": tpl("BB khác A", "Khác", "GRP-A"),
        "shared_hand": tpl("BB bàn giao chung", "Bàn giao", None),
        "a_add": tpl("PL bổ sung A", "Phụ lục bổ sung", "GRP-A"),
        "a_ren": tpl("PL gia hạn A", "Phụ lục gia hạn", "GRP-A"),
        "shared_appendix": tpl("Phụ lục chung cũ", "Phụ lục", None),
        "b_quote": tpl("Báo giá B", "Báo giá", "GRP-B"),
        "a_quote": tpl("Báo giá A", "Báo giá", "GRP-A"),
    }
    a_main.default_template_id = t["a_c2"].id
    db_session.commit()

    draft = make_contract(customer=customer, product=a_main, status=ContractStatus.Draft,
                          contract_number="HD-UI-DRAFT", template_id=t["a_c1"].id,
                          start_date=date(2027, 1, 1), end_date=date(2027, 12, 31),
                          month_count=12, unit_count=1)
    parent = make_contract(customer=customer, product=a_main, status=ContractStatus.Invoiced,
                           contract_number="HD-UI-P", template_id=t["a_c1"].id,
                           start_date=date(2027, 1, 1), end_date=date(2027, 12, 31),
                           month_count=12, unit_count=1, total_amount=Decimal("6600000"),
                           vat_rate=10, sign_location="Địa điểm hư cấu")
    return {"customer": customer, "a_main": a_main, "a_add": a_add, "b_main": b_main,
            "t": t, "draft": draft, "parent": parent}


def _options(client, product):
    return client.get(f"/contracts/product-options?product_id={product.id}").json()


# ─── product-options: mẫu theo nhóm ───────────────────────────────────────────

def test_product_options_lists_only_own_group_and_shared_contract_templates(client, ui_world):
    w, t = ui_world, ui_world["t"]
    contract_ids = [c["id"] for c in _options(client, w["a_main"])["templates"]["contract"]]

    assert t["a_c1"].id in contract_ids
    assert t["a_c2"].id in contract_ids
    assert t["b_c"].id not in contract_ids


def test_a_package_with_three_own_contract_templates_offers_all_three(client, ui_world):
    """Lỗi cũ: form chỉ hiện đúng một mẫu của gói (mẫu mặc định), các mẫu khác bị ẩn."""
    w, t = ui_world, ui_world["t"]
    contract_ids = [c["id"] for c in _options(client, w["a_main"])["templates"]["contract"]]

    assert {t["a_c1"].id, t["a_c2"].id, t["a_c3"].id} <= set(contract_ids)


def test_default_is_the_package_default_when_it_is_offered(client, ui_world):
    w, t = ui_world, ui_world["t"]
    assert _options(client, w["a_main"])["templates"]["default_id"] == t["a_c2"].id


def test_attachments_are_own_group_and_shared_only(client, ui_world):
    w, t = ui_world, ui_world["t"]
    att_ids = [a["id"] for a in _options(client, w["a_main"])["templates"]["attachments"]]

    assert t["a_hand"].id in att_ids
    assert t["a_ack"].id in att_ids
    assert t["shared_hand"].id in att_ids
    assert t["b_hand"].id not in att_ids


def test_preselect_flags_follow_q11(client, ui_world):
    w, t = ui_world, ui_world["t"]
    flags = {a["id"]: a["preselect"] for a in _options(client, w["a_main"])["templates"]["attachments"]}

    assert flags[t["a_hand"].id] is True     # bàn giao riêng của nhóm
    assert flags[t["a_ack"].id] is True      # nghiệm thu riêng của nhóm
    assert flags[t["shared_hand"].id] is False  # bàn giao dùng chung: không tick sẵn
    assert flags[t["a_other"].id] is False   # loại Khác: không tick sẵn


# ─── Form hợp đồng ───────────────────────────────────────────────────────────

def test_new_form_html_has_no_product_code_attributes_and_an_empty_attachment_box(client, ui_world):
    html = client.get("/contracts/new").text

    assert "data-product-code" not in html
    assert 'id="attachment-box"' in html
    assert '<input class="form-check-input" type="checkbox" name="attachment_ids"' not in html  # ô tick do JS dựng
    assert 'id="sel-template"' in html


def test_failed_submit_re_renders_template_and_attachment_choices(client, ui_world):
    w, t = ui_world, ui_world["t"]
    resp = client.post("/contracts/new", data={
        "contract_number": "",  # lỗi: số hợp đồng trống
        "customer_id": str(w["customer"].id),
        "product_id": str(w["a_main"].id),
        "template_id": str(t["a_c2"].id),
        "attachment_ids": [str(t["a_hand"].id), str(t["a_ack"].id)],
        "items_json": "[]",
    })

    assert resp.status_code == 422
    assert f'data-selected="{t["a_c2"].id}"' in resp.text
    assert f'data-selected="{t["a_hand"].id},{t["a_ack"].id}"' in resp.text
    assert 'data-prefilled="1"' in resp.text


# ─── Trang chi tiết hợp đồng ─────────────────────────────────────────────────

def test_detail_offers_only_the_contracts_group_attachments(client, ui_world):
    w, t = ui_world, ui_world["t"]
    html = client.get(f"/contracts/{w['draft'].id}").text

    assert f'id="gen-att-{t["a_hand"].id}"' in html
    assert f'id="gen-att-{t["a_ack"].id}"' in html
    assert f'id="gen-att-{t["b_hand"].id}"' not in html
    assert "Nhóm sản phẩm" in html
    assert "GRP-A" in html


# ─── Phụ lục và gia hạn ──────────────────────────────────────────────────────

def test_addendum_form_offers_group_addon_templates_and_selects_the_default(client, ui_world):
    w, t = ui_world, ui_world["t"]
    html = client.get(f"/contracts/{w['parent'].id}/addendum/new").text

    assert re.search(rf'<option value="{t["a_add"].id}"\s+selected>', html)
    assert f'value="{t["a_ren"].id}"' not in html   # phụ lục gia hạn không vào form bổ sung
    assert f'value="{t["b_hand"].id}"' not in html
    assert f'value="{t["shared_appendix"].id}"' in html  # nhãn "Phụ lục" cũ vẫn dùng được


def test_renewal_form_offers_group_renewal_templates_and_selects_the_default(client, ui_world):
    w, t = ui_world, ui_world["t"]
    html = client.get(f"/contracts/{w['parent'].id}/renewal/new").text

    assert re.search(rf'<option value="{t["a_ren"].id}"\s+selected>', html)
    assert f'value="{t["a_add"].id}"' not in html


# ─── Báo giá ─────────────────────────────────────────────────────────────────

def test_quotation_form_carries_group_attributes_for_products_and_templates(client, ui_world):
    html = client.get("/quotation").text

    assert 'data-group="GRP-A"' in html
    assert 'data-group="GRP-B"' in html


# ─── Luật văn bản kèm theo được tick sẵn (đơn vị) ────────────────────────────

def test_preselected_attachment_rule(make_template):
    from app.services.template_service import preselected_attachment

    assert preselected_attachment(make_template(contract_type="Bàn giao", product_group="GRP-A"))
    assert preselected_attachment(make_template(contract_type="Nghiệm thu", product_group="GRP-A"))
    assert not preselected_attachment(make_template(contract_type="Bàn giao", product_group=None))
    assert not preselected_attachment(make_template(contract_type="Khác", product_group="GRP-A"))
    assert not preselected_attachment(make_template(contract_type="Thanh lý", product_group="GRP-A"))
