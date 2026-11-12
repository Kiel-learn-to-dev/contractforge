"""Form sửa mẫu: nhãn cũ không bị đổi ngầm; lưu form không làm mất nhóm sản phẩm của mẫu."""


def test_edit_form_keeps_a_legacy_label_as_the_first_choice(client, make_template):
    tpl = make_template(name="Phụ lục cũ", contract_type="Phụ lục")

    resp = client.get(f"/templates/{tpl.id}/edit")

    assert resp.status_code == 200
    assert 'value="Phụ lục"' in resp.text
    assert resp.text.index('value="Phụ lục"') < resp.text.index('value="Hợp đồng"')


def test_saving_the_edit_form_keeps_the_product_group(client, db_session, make_template):
    from app.models.contract_template import ContractTemplate

    tpl = make_template(name="Mẫu có nhóm", contract_type="Hợp đồng", product_group="GRP-K")

    resp = client.post(f"/templates/{tpl.id}/edit", data={
        "name": "Mẫu có nhóm (đổi tên)", "contract_type": "Hợp đồng", "description": ""},
        follow_redirects=False)

    assert resp.status_code in (302, 303)
    db_session.expire_all()
    saved = db_session.get(ContractTemplate, tpl.id)
    assert saved.name == "Mẫu có nhóm (đổi tên)"
    assert saved.product_group == "GRP-K"
