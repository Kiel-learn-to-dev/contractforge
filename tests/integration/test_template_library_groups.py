"""Thư viện mẫu và danh mục theo nhóm sản phẩm (B6). Nhóm GRP-A, GRP-B hư cấu.

Mẫu Word dựng trong thư mục tạm. Kiểm bằng HTTP và dữ liệu DB.
"""

import io
import re
from urllib.parse import unquote

import docx
import pytest


def _docx_bytes(paragraphs):
    doc = docx.Document()
    for text in paragraphs:
        doc.add_paragraph(text)
    buf = io.BytesIO()
    doc.save(buf)
    return buf.getvalue()


HANDOVER = ["BIÊN BẢN BÀN GIAO DỊCH VỤ", "Bên A: {{PARTY_A_NAME}}", "Hợp đồng {{CONTRACT_NUMBER}}"]
ADDON = ["PHỤ LỤC BỔ SUNG", "Căn cứ hợp đồng số {{PARENT_CONTRACT_NUMBER}}", "Số {{CONTRACT_NUMBER}}"]
RENEWAL = ["PHỤ LỤC GIA HẠN", "Căn cứ hợp đồng số {{PARENT_CONTRACT_NUMBER}}",
           "Từ {{START_DATE}} đến {{END_DATE}}"]


def _product_form(**overrides):
    base = {
        "code": "LIB-P", "name": "Gói thư viện", "product_type": "Dịch vụ", "group_code": "",
        "kind": "main", "unit_label": "", "default_price": "500.000", "default_vat_rate": "10",
        "price_period_months": "1", "quantity_source": "units", "min_quantity": "",
        "quantity_step": "", "account_limit": "", "default_duration_months": "12",
        "default_template_id": "", "contract_number_format": "", "replaced_by_id": "",
        "sort_order": "0", "is_active": "1",
    }
    base.update(overrides)
    return base


@pytest.fixture()
def group_a(make_product):
    return make_product(code="LIB-A", name="Gói nhóm A", group_code="GRP-A", kind="main",
                        default_price=500_000, default_vat_rate=10)


def test_form_creates_a_template_with_its_group(client, db_session, group_a):
    from app.models.contract_template import ContractTemplate

    resp = client.post("/templates/new", data={
        "code": "", "name": "Biên bản nhóm A", "contract_type": "Bàn giao",
        "description": "", "product_group": "GRP-A"},
        files={"file": ("bb_nhom_a.docx", _docx_bytes(HANDOVER), "application/octet-stream")},
        follow_redirects=False)

    assert resp.status_code == 303
    db_session.expire_all()
    tpl = db_session.query(ContractTemplate).filter_by(name="Biên bản nhóm A").one()
    assert tpl.product_group == "GRP-A"


def test_form_refuses_a_group_that_no_product_has(client, db_session):
    from app.models.contract_template import ContractTemplate

    resp = client.post("/templates/new", data={
        "code": "", "name": "Mẫu nhóm lạ", "contract_type": "Hợp đồng",
        "description": "", "product_group": "GRP-KHONG-CO"},
        files={"file": ("nhom_la.docx", _docx_bytes(["Hợp đồng {{CONTRACT_NUMBER}}"]),
                        "application/octet-stream")},
        follow_redirects=False)

    assert resp.status_code == 422
    assert "Nhóm sản phẩm không tồn tại." in resp.text
    assert db_session.query(ContractTemplate).filter_by(name="Mẫu nhóm lạ").count() == 0


def test_library_lists_group_headings_with_shared_last(client, make_template, group_a):
    make_template(name="Mẫu nhóm A", contract_type="Hợp đồng", product_group="GRP-A")
    make_template(name="Mẫu chung", contract_type="Hợp đồng", product_group=None)

    page = client.get("/templates").text

    headings = [h.strip() for h in re.findall(r'<h6 class="text-uppercase[^"]*"[^>]*>(.*?)</h6>', page, re.S)]
    assert headings == ["GRP-A", "Dùng chung"]  # nhóm trước, mẫu dùng chung đứng cuối


def test_bulk_upload_takes_three_files_in_one_go_and_recognises_each_kind(
        client, db_session, group_a):
    from app.models.contract_template import ContractTemplate

    resp = client.post("/templates/bulk-upload", data={"product_group": "GRP-A"}, files=[
        ("files", ("bb_ban_giao.docx", _docx_bytes(HANDOVER), "application/octet-stream")),
        ("files", ("pl_bo_sung.docx", _docx_bytes(ADDON), "application/octet-stream")),
        ("files", ("pl_gia_han.docx", _docx_bytes(RENEWAL), "application/octet-stream")),
    ], follow_redirects=False)

    assert resp.status_code == 303
    db_session.expire_all()
    kinds = {t.name: (t.contract_type, t.product_group)
             for t in db_session.query(ContractTemplate).all()}
    assert kinds["bb ban giao"] == ("Bàn giao", "GRP-A")
    assert kinds["pl bo sung"] == ("Phụ lục bổ sung", "GRP-A")
    assert kinds["pl gia han"] == ("Phụ lục gia hạn", "GRP-A")


def test_bulk_upload_skips_identical_content(client, db_session, group_a):
    from app.models.contract_template import ContractTemplate

    data = _docx_bytes(HANDOVER)
    files = [("files", ("bb_ban_giao.docx", data, "application/octet-stream"))]
    client.post("/templates/bulk-upload", data={"product_group": "GRP-A"}, files=files,
                follow_redirects=False)
    resp = client.post("/templates/bulk-upload", data={"product_group": "GRP-A"}, files=files,
                       follow_redirects=False)

    assert resp.status_code == 303
    assert "bỏ qua" in unquote(resp.headers["location"])
    db_session.expire_all()
    assert db_session.query(ContractTemplate).count() == 1


def test_bulk_upload_one_broken_file_does_not_stop_the_others(client, db_session, group_a):
    from app.models.contract_template import ContractTemplate

    resp = client.post("/templates/bulk-upload", data={"product_group": "GRP-A"}, files=[
        ("files", ("hong.docx", b"khong phai docx", "application/octet-stream")),
        ("files", ("bb_ban_giao.docx", _docx_bytes(HANDOVER), "application/octet-stream")),
    ], follow_redirects=False)

    assert resp.status_code == 303
    db_session.expire_all()
    names = [t.name for t in db_session.query(ContractTemplate).all()]
    assert "bb ban giao" in names
    assert "hong" not in names


def test_product_with_a_default_template_of_another_group_is_refused(db_session, make_template):
    from app.services import product_service

    other = make_template(name="HĐ nhóm B", contract_type="Hợp đồng", product_group="GRP-B")

    with pytest.raises(product_service.ProductError) as exc:
        product_service.create_product(db_session, _product_form(
            group_code="GRP-A", default_template_id=str(other.id)))

    assert any("GRP-B" in e for e in exc.value.errors)


def test_product_catalog_group_row_shows_missing_roles(client, make_product):
    make_product(code="SUM-A", name="Gói tóm tắt", group_code="GRP-SUM", kind="main")

    page = client.get("/products").text

    assert "Phụ lục bổ sung: thiếu" in page
    assert "Hợp đồng: thiếu" in page


def test_product_form_offers_only_its_group_and_shared_contract_templates(client, make_template,
                                                                          make_product):
    product = make_product(code="FM-A", name="Gói form", group_code="GRP-FORM", kind="main")
    own = make_template(name="HĐ nhóm form", contract_type="Hợp đồng", product_group="GRP-FORM")
    other = make_template(name="HĐ nhóm khác", contract_type="Hợp đồng", product_group="GRP-OTHER")
    shared = make_template(name="HĐ chung form", contract_type="Hợp đồng", product_group=None)

    page = client.get(f"/products/{product.id}/edit").text

    assert f'value="{own.id}"' in page
    assert f'value="{shared.id}"' in page
    assert f'value="{other.id}"' not in page
