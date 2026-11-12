"""Sinh hợp đồng cùng lúc với các văn bản kèm theo đã tick.

Dữ liệu hoàn toàn hư cấu. Mẫu Word dựng trong thư mục tạm.
"""

import io
import zipfile

import docx
import pytest


def _docx(path, text):
    doc = docx.Document()
    doc.add_paragraph(text)
    doc.save(str(path))
    return path


@pytest.fixture()
def setup_contract(db_session, make_customer, make_contract, make_template, tmp_path):
    from app.models.contract import ContractStatus

    customer = make_customer(legal_name="Công ty kèm theo hư cấu")
    main_tpl = make_template(file_path=str(_docx(tmp_path / "hd.docx", "Hợp đồng {{CONTRACT_NUMBER}}")),
                             contract_type="Hợp đồng", name="Mẫu hợp đồng hư cấu")
    contract = make_contract(customer=customer, contract_number="HD-KT-1",
                             status=ContractStatus.Draft, template_id=main_tpl.id)
    return {"contract": contract, "main": main_tpl, "tmp": tmp_path}


def _att(make_template, tmp_path, name, contract_type, text="Biên bản {{CONTRACT_NUMBER}}"):
    return make_template(file_path=str(_docx(tmp_path / f"{name}.docx", text)),
                         contract_type=contract_type, name=name)


def test_ticked_attachments_come_back_with_the_main_document(db_session, setup_contract,
                                                             make_template, data_root):
    from app.models.contract import ContractStatus
    from app.services.contract_service import generate_docx

    s = setup_contract
    a1 = _att(make_template, s["tmp"], "Biên bản bàn giao hư cấu", "Bàn giao")
    a2 = _att(make_template, s["tmp"], "Biên bản nghiệm thu hư cấu", "Nghiệm thu")
    bundle = []

    contract = generate_docx(db_session, s["contract"].id, [str(a1.id), str(a2.id)], bundle)

    assert contract.status == ContractStatus.Generated
    names = [name for name, _ in bundle]
    assert len(bundle) == 3  # hợp đồng chính + 2 văn bản
    assert contract.output_file_name == names[0]
    assert all(name != contract.output_file_name for name in names[1:])
    assert all(b"PK" == data[:2] for _, data in bundle)  # đều là docx (zip)


def test_one_broken_attachment_cancels_the_whole_run(db_session, setup_contract, make_template,
                                                     data_root):
    from app.models.contract import ContractStatus
    from app.services.contract_service import generate_docx

    s = setup_contract
    good = _att(make_template, s["tmp"], "Biên bản tốt hư cấu", "Bàn giao")
    broken = make_template(file_path=str(s["tmp"] / "khong_co.docx"), contract_type="Nghiệm thu",
                           name="Mẫu hỏng hư cấu")
    bundle = []

    with pytest.raises(ValueError):
        generate_docx(db_session, s["contract"].id, [str(good.id), str(broken.id)], bundle)

    db_session.expire_all()
    contract = db_session.get(type(s["contract"]), s["contract"].id)
    assert contract.status == ContractStatus.Draft
    assert contract.output_file_path is None
    assert bundle == []


def test_a_template_of_another_kind_is_refused_before_anything_is_written(
        db_session, setup_contract, make_template, data_root):
    from app.models.contract import ContractStatus
    from app.services.contract_service import generate_docx

    s = setup_contract
    wrong = make_template(file_path=str(_docx(s["tmp"] / "hd2.docx", "x")),
                          contract_type="Hợp đồng", name="Mẫu hợp đồng khác")

    with pytest.raises(ValueError, match="không dùng được ở đây"):
        generate_docx(db_session, s["contract"].id, [str(wrong.id)], [])

    db_session.expire_all()
    contract = db_session.get(type(s["contract"]), s["contract"].id)
    assert contract.status == ContractStatus.Draft


def test_generate_route_returns_a_zip_when_attachments_are_ticked(
        client, db_session, setup_contract, make_template, data_root):
    s = setup_contract
    a1 = _att(make_template, s["tmp"], "Biên bản route hư cấu", "Bàn giao")

    resp = client.post(f"/contracts/{s['contract'].id}/generate",
                       data={"attachment_ids": [str(a1.id)]})

    assert resp.status_code == 200
    assert resp.headers["content-type"] == "application/zip"
    assert len(zipfile.ZipFile(io.BytesIO(resp.content)).namelist()) == 2


def test_generate_route_without_ticks_keeps_the_old_redirect(client, db_session, setup_contract,
                                                             data_root):
    s = setup_contract

    resp = client.post(f"/contracts/{s['contract'].id}/generate", follow_redirects=False)

    assert resp.status_code == 303
    assert "/contracts/" in resp.headers["location"]


def test_contract_form_offers_only_attachment_kinds(client, make_template, make_product, tmp_path):
    # Form hợp đồng lấy mẫu theo sản phẩm qua /contracts/product-options, không in sẵn trong HTML.
    prod = make_product(code="OA-P", name="Gói tick", kind="main", default_price=1000, default_vat_rate=10)
    att = make_template(file_path=str(_docx(tmp_path / "a.docx", "x")), contract_type="Bàn giao",
                        name="Biên bản bàn giao hiện trên form")
    main = make_template(file_path=str(_docx(tmp_path / "b.docx", "x")), contract_type="Hợp đồng",
                         name="Mẫu hợp đồng không tick được")

    data = client.get(f"/contracts/product-options?product_id={prod.id}").json()

    att_ids = [a["id"] for a in data["templates"]["attachments"]]
    assert att.id in att_ids
    assert main.id not in att_ids


def test_biên_bản_is_kinded_by_its_heading_not_by_fields():
    from app.services.template_service import _template_kind

    assert _template_kind([], "Biểu mẫu 01 BIÊN BẢN BÀN GIAO VÀ NGHIỆM THU") == "Bàn giao"
    assert _template_kind([], "BIÊN BẢN NGHIỆM THU HOÀN THÀNH CUNG CẤP DỊCH VỤ") == "Nghiệm thu"
    assert _template_kind(["CONTRACT_NUMBER"], "") == "Hợp đồng"
    # Mẫu gia hạn thật có START_DATE và END_DATE; tiêu đề một mình không đủ để xếp gia hạn.
    assert _template_kind(["PARENT_CONTRACT_NUMBER", "START_DATE", "END_DATE"], "PHỤ LỤC GIA HẠN") == "Phụ lục gia hạn"
