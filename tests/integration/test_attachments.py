"""Task 4.5 — sinh văn bản kèm theo từ một mẫu bất kỳ của loại được phép.

Dữ liệu hoàn toàn hư cấu. Mẫu Word dựng trong thư mục tạm.
"""

import io

import docx
import pytest


def _attachment_template(path):
    doc = docx.Document()
    doc.add_paragraph("Biên bản cho hợp đồng {{CONTRACT_NUMBER}} — {{PARTY_A_NAME}}")
    doc.save(str(path))
    return path


def test_attachment_renders_from_an_allowed_template(db_session, make_customer, make_contract,
                                                     make_template, tmp_path):
    from app.services.attachment_service import render_attachment

    customer = make_customer(legal_name="Công ty văn bản hư cấu")
    contract = make_contract(customer=customer, contract_number="HD-ATT-1")
    template = make_template(file_path=str(_attachment_template(tmp_path / "att.docx")),
                             contract_type="Nghiệm thu")

    data, fname = render_attachment(db_session, contract.id, template.id)

    out = docx.Document(io.BytesIO(data))
    text = "\n".join(p.text for p in out.paragraphs)
    assert "HD-ATT-1" in text and "Công ty văn bản hư cấu" in text
    assert "{{" not in text
    assert fname.endswith(".docx")


def test_contract_template_is_refused_with_400(client, db_session, make_customer, make_contract,
                                               make_template, tmp_path):
    customer = make_customer()
    contract = make_contract(customer=customer, contract_number="HD-ATT-2")
    template = make_template(file_path=str(_attachment_template(tmp_path / "hd.docx")),
                             contract_type="Hợp đồng")

    resp = client.get(f"/contracts/{contract.id}/attachment?template_id={template.id}")
    assert resp.status_code == 400


def test_attachment_download_works_and_changes_nothing(client, db_session, make_customer,
                                                       make_contract, make_template, tmp_path):
    from app.models.contract import ContractStatus

    customer = make_customer()
    contract = make_contract(customer=customer, contract_number="HD-ATT-3",
                             status=ContractStatus.Signed, output_file_path="/nowhere/keep.docx")
    template = make_template(file_path=str(_attachment_template(tmp_path / "bg.docx")),
                             contract_type="Bàn giao")

    resp = client.get(f"/contracts/{contract.id}/attachment?template_id={template.id}")

    assert resp.status_code == 200
    assert "attachment" in resp.headers["content-disposition"]
    db_session.expire_all()
    reloaded = db_session.get(type(contract), contract.id)
    assert reloaded.status == ContractStatus.Signed
    assert reloaded.output_file_path == "/nowhere/keep.docx"


def test_detail_page_offers_attachment_templates(client, make_customer, make_contract,
                                                 make_template, tmp_path):
    customer = make_customer()
    contract = make_contract(customer=customer, contract_number="HD-ATT-4")
    make_template(file_path=str(_attachment_template(tmp_path / "ng.docx")),
                  contract_type="Nghiệm thu", name="Mẫu nghiệm thu hư cấu")

    resp = client.get(f"/contracts/{contract.id}")

    assert resp.status_code == 200
    assert "Mẫu nghiệm thu hư cấu" in resp.text
    assert f"/contracts/{contract.id}/attachment" in resp.text
