"""Bật/tắt mẫu bằng ô tích ngay trong danh sách mẫu.

Mẫu tắt vẫn nằm trong thư viện (không xóa), chỉ không còn hiện khi chọn mẫu hay tick văn bản.
Dữ liệu hoàn toàn hư cấu.
"""

import docx
import pytest


@pytest.fixture()
def attachment_template(make_template, tmp_path):
    path = tmp_path / "bb_tick.docx"
    d = docx.Document()
    d.add_paragraph("Biên bản {{CONTRACT_NUMBER}}")
    d.save(str(path))
    return make_template(file_path=str(path), contract_type="Bàn giao", name="Biên bản tick hư cấu")


def test_list_shows_a_checkbox_per_template(client, attachment_template):
    page = client.get("/templates").text
    assert f'id="tpl-on-{attachment_template.id}"' in page
    assert f'action="/templates/{attachment_template.id}/toggle"' in page


def test_checkbox_toggles_and_returns_to_the_list(client, db_session, attachment_template):
    from app.models.contract_template import ContractTemplate

    resp = client.post(f"/templates/{attachment_template.id}/toggle", data={"next": "/templates"},
                       follow_redirects=False)

    assert resp.status_code == 303
    assert resp.headers["location"].startswith("/templates")
    assert not resp.headers["location"].startswith(f"/templates/{attachment_template.id}")
    db_session.expire_all()
    assert db_session.get(ContractTemplate, attachment_template.id).is_active is False


def test_off_template_is_hidden_from_pickers_and_kept_in_the_library(
        client, db_session, attachment_template, make_product):
    from app.models.contract_template import ContractTemplate

    prod = make_product(code="TG-P", name="Gói bật tắt", kind="main",
                        default_price=1000, default_vat_rate=10)
    tid = attachment_template.id

    def offered():
        data = client.get(f"/contracts/product-options?product_id={prod.id}").json()
        return tid in [a["id"] for a in data["templates"]["attachments"]]

    assert offered()

    client.post(f"/templates/{tid}/toggle", data={"next": "/templates"}, follow_redirects=False)

    assert not offered()
    assert db_session.get(ContractTemplate, tid) is not None  # vẫn còn trong thư viện

    client.post(f"/templates/{tid}/toggle", data={"next": "/templates"}, follow_redirects=False)
    assert offered()


def test_toggle_refuses_a_foreign_redirect(client, attachment_template):
    resp = client.post(f"/templates/{attachment_template.id}/toggle",
                       data={"next": "https://example.invalid/"}, follow_redirects=False)
    assert resp.headers["location"].startswith(f"/templates/{attachment_template.id}")
