"""Task 3.2 — context và danh sách cho hợp đồng theo dòng.

Dữ liệu hoàn toàn hư cấu. Mẫu Word được dựng trong thư mục tạm của test.
"""

import io
from decimal import Decimal

import docx
import pytest


def _template_docx(path, *, with_rows=True):
    doc = docx.Document()
    doc.add_paragraph("Hợp đồng số {{CONTRACT_NUMBER}} — {{PRODUCT_NAME}}")
    doc.add_paragraph("Bên A: {{PARTY_A_NAME}}, ĐT {{PARTY_A_PHONE}}")
    doc.add_paragraph("Bên B: {{PARTY_B_NAME}}, ĐT {{PARTY_B_PHONE}}")
    if with_rows:
        t = doc.add_table(rows=3, cols=4)
        for i, h in enumerate(["TT", "Dịch vụ", "Số lượng", "Thành tiền"]):
            t.rows[0].cells[i].text = h
        for i, v in enumerate(["{{ITEM_NO}}", "{{ITEM_NAME}}", "{{ITEM_QTY}}", "{{ITEM_AMOUNT_TOTAL}}"]):
            t.rows[1].cells[i].text = v
        t.rows[2].cells[0].text = "Tổng"
        t.rows[2].cells[3].text = "{{TOTAL_PRE_VAT}}"
        f = doc.add_table(rows=2, cols=2)
        f.rows[0].cells[0].text = "STT"
        f.rows[0].cells[1].text = "Chức năng"
        f.rows[1].cells[0].text = "{{FEATURE_NO}}"
        f.rows[1].cells[1].text = "{{FEATURE_NAME}}"
    doc.save(str(path))
    return path


def _all_text(document):
    parts = [p.text for p in document.paragraphs]
    for table in document.tables:
        for row in table.rows:
            for cell in row.cells:
                parts.append(cell.text)
    return parts


def test_build_render_lists_orders_lines_and_dedupes_features(db_session, make_customer,
                                                              make_product):
    from app.services.contract_service import create_draft
    from app.services.docx_renderer import build_render_lists

    customer = make_customer()
    main = make_product(code="RL-MAIN", name="Gói chính RL", kind="main", group_code="RL-G",
                        unit_label="cơ sở", default_price=500_000, default_vat_rate=10)
    addon = make_product(code="RL-ADD", name="Gói kèm RL", kind="addon", group_code="RL-G",
                         unit_label="cơ sở", default_price=150_000, default_vat_rate=10)
    main.feature_list = [{"no": 1, "name": "Đăng nhập", "description": "Mô tả A"},
                         {"no": 2, "name": "Báo cáo", "description": "Mô tả B"}]
    addon.feature_list = [{"no": 1, "name": "Báo cáo", "description": "Mô tả B"},
                          {"no": 2, "name": "Xuất file", "description": "Mô tả C"}]
    db_session.commit()

    contract = create_draft(db_session, {
        "contract_number": "HD-RL-001", "customer_id": str(customer.id),
        "product_id": str(main.id), "start_date": "01/01/2026", "month_count": "12",
        "items_json": '[{"product_id": %d, "quantity": "2"}, {"product_id": %d, "quantity": "1"}]'
                      % (main.id, addon.id),
    })
    db_session.expire_all()
    lists = build_render_lists(db_session.get(type(contract), contract.id))

    assert [r["ITEM_NO"] for r in lists["items"]] == ["1", "2"]
    assert [r["ITEM_NAME"] for r in lists["items"]] == ["Gói chính RL", "Gói kèm RL"]
    assert lists["items"][0]["ITEM_QTY"] == "24"          # số lượng × số kỳ
    assert lists["items"][0]["ITEM_PRICE"] == "500.000"
    assert lists["items"][0]["ITEM_VAT_RATE"] == "10%"
    assert lists["items"][0]["ITEM_AMOUNT_PRE_VAT"] == "12.000.000"
    assert lists["items"][0]["ITEM_AMOUNT_TOTAL"] == "13.200.000"
    assert [r["FEATURE_NO"] for r in lists["features"]] == ["1", "2", "3"]
    assert [r["FEATURE_NAME"] for r in lists["features"]] == ["Đăng nhập", "Báo cáo", "Xuất file"]


def test_generated_items_contract_fills_rows_and_totals(db_session, make_customer, make_product,
                                                        make_template, data_root):
    from app.services.contract_service import create_draft, generate_docx

    tpl_path = _template_docx(data_root / "items_template.docx")
    template = make_template(file_path=str(tpl_path), contract_type="Hợp đồng")
    customer = make_customer(legal_name="Công ty hư cấu A", phone="090-000-0001")
    main = make_product(code="GD-MAIN", name="Gói chính GD", kind="main", group_code="GD-G",
                        unit_label="cơ sở", default_price=500_000, default_vat_rate=10)
    addon = make_product(code="GD-ADD", name="Gói kèm GD", kind="addon", group_code="GD-G",
                         unit_label="cơ sở", default_price=150_000, default_vat_rate=10)
    main.feature_list = [{"no": 1, "name": "Chức năng một", "description": "x"}]
    db_session.commit()

    contract = create_draft(db_session, {
        "contract_number": "HD-GD-001", "customer_id": str(customer.id),
        "product_id": str(main.id), "template_id": str(template.id),
        "start_date": "01/01/2026", "month_count": "12",
        "items_json": '[{"product_id": %d, "quantity": "2"}, {"product_id": %d, "quantity": "1"}]'
                      % (main.id, addon.id),
    })
    contract = generate_docx(db_session, contract.id)

    rendered = docx.Document(contract.output_file_path)
    text = "\n".join(_all_text(rendered))
    assert "{{" not in text
    assert "Hợp đồng số HD-GD-001" in text
    assert "ĐT 090-000-0001" in text
    rows = [[c.text for c in r.cells] for r in rendered.tables[0].rows]
    assert rows[1] == ["1", "Gói chính GD", "24", "13.200.000"]
    assert rows[2] == ["2", "Gói kèm GD", "12", "1.980.000"]
    assert rows[3][0] == "Tổng" and rows[3][3] == "13.800.000"  # {{TOTAL_PRE_VAT}} trong mẫu này
    feature_rows = [[c.text for c in r.cells] for r in rendered.tables[1].rows]
    assert feature_rows[1] == ["1", "Chức năng một"]
    assert len(feature_rows) == 2


def test_legacy_contract_with_plain_template_still_generates(db_session, make_customer,
                                                             make_contract, make_template,
                                                             data_root):
    from app.services.contract_service import generate_docx

    tpl_path = _template_docx(data_root / "plain_template.docx", with_rows=False)
    template = make_template(file_path=str(tpl_path), contract_type="Hợp đồng")
    customer = make_customer(legal_name="Công ty hư cấu B")
    contract = make_contract(customer=customer, template_id=template.id,
                             contract_number="HD-LEG-RND-1", total_amount=Decimal("1100000"))

    contract = generate_docx(db_session, contract.id)

    rendered = docx.Document(contract.output_file_path)
    assert "Hợp đồng số HD-LEG-RND-1" in "\n".join(_all_text(rendered))
    assert contract.pricing_model == "legacy"


def test_party_b_contact_settings_round_trip_through_the_form(client, db_session):
    from app.services.settings_service import get_party_b_settings

    resp = client.post("/settings/party-b", data={
        "party_b_name": "Đơn vị hư cấu",
        "party_b_phone": "028-000-0000",
        "party_b_contact_name": "Người liên hệ",
        "party_b_contact_email": "lienhe@example.invalid",
    }, follow_redirects=False)

    assert resp.status_code == 303
    db_session.expire_all()
    pbs = get_party_b_settings(db_session)
    assert pbs["phone"] == "028-000-0000"
    assert pbs["contact_name"] == "Người liên hệ"
    assert pbs["contact_email"] == "lienhe@example.invalid"


def test_party_b_contact_keys_default_to_blank(db_session):
    from app.services.settings_service import get_party_b_settings

    pbs = get_party_b_settings(db_session)
    assert pbs["phone"] == ""
    assert pbs["contact_name"] == ""
    assert pbs["contact_email"] == ""
