"""Luật "mẫu nào dùng cho hợp đồng nào" — chỉ ở app/services/template_service.py.

Nhóm sản phẩm là hư cấu: GRP-A, GRP-B.
"""
from datetime import date

import pytest

from app.services import template_service as ts


def _contract_with_parent(make_contract, parent, product):
    return make_contract(customer=parent.customer, product=product,
                         parent_contract_id=parent.id, doc_kind="addendum_addon",
                         contract_number="PL01/" + parent.contract_number,
                         sign_date=date(2027, 2, 1), start_date=date(2027, 2, 1),
                         end_date=date(2027, 12, 31))


# ─── has_role ────────────────────────────────────────────────────────────────

def test_free_labels_and_contract_label_are_contract_templates():
    assert ts.has_role("Hợp đồng", ts.ROLE_CONTRACT)
    assert ts.has_role("Nhãn tự do của khách", ts.ROLE_CONTRACT)
    assert not ts.has_role("Bàn giao", ts.ROLE_CONTRACT)


def test_legacy_appendix_label_serves_both_appendix_roles():
    assert ts.has_role("Phụ lục", ts.ROLE_ADDON)
    assert ts.has_role("Phụ lục", ts.ROLE_RENEWAL)


def test_renewal_label_is_not_an_addon():
    assert ts.has_role("Phụ lục gia hạn", ts.ROLE_RENEWAL)
    assert not ts.has_role("Phụ lục gia hạn", ts.ROLE_ADDON)


def test_handover_is_an_attachment_and_quotation_is_only_a_quotation():
    assert ts.has_role("Bàn giao", ts.ROLE_ATTACHMENT)
    assert ts.has_role("Báo giá", ts.ROLE_QUOTATION)
    assert not ts.has_role("Báo giá", ts.ROLE_ATTACHMENT)
    assert not ts.has_role("Báo giá", ts.ROLE_CONTRACT)


# ─── templates_for / default_template_for ───────────────────────────────────

@pytest.fixture()
def shelf(make_template):
    return {
        "a_handover": make_template(name="Bàn giao A", contract_type="Bàn giao", product_group="GRP-A"),
        "b_handover": make_template(name="Bàn giao B", contract_type="Bàn giao", product_group="GRP-B"),
        "shared_ack": make_template(name="Nghiệm thu chung", contract_type="Nghiệm thu", product_group=None),
        "a_off": make_template(name="Bàn giao A tắt", contract_type="Bàn giao",
                               product_group="GRP-A", is_active=False),
    }


def test_templates_for_group_keeps_own_and_shared_and_drops_other_group_and_off(db_session, shelf):
    names = [t.name for t in ts.templates_for(db_session, "GRP-A", ts.ROLE_ATTACHMENT)]
    assert "Bàn giao A" in names
    assert "Nghiệm thu chung" in names
    assert "Bàn giao B" not in names
    assert "Bàn giao A tắt" not in names


def test_own_group_template_comes_before_shared_ones(db_session, shelf):
    names = [t.name for t in ts.templates_for(db_session, "GRP-A", ts.ROLE_ATTACHMENT)]
    assert names.index("Bàn giao A") < names.index("Nghiệm thu chung")


def test_no_group_offers_only_shared_templates(db_session, shelf):
    names = [t.name for t in ts.templates_for(db_session, None, ts.ROLE_ATTACHMENT)]
    assert names == ["Nghiệm thu chung"]


def test_default_is_the_single_own_template(db_session, make_template):
    own = make_template(name="Hợp đồng A", contract_type="Hợp đồng", product_group="GRP-A")
    assert ts.default_template_for(db_session, "GRP-A", ts.ROLE_CONTRACT).id == own.id


def test_default_is_none_when_group_has_two_own_templates(db_session, make_template):
    make_template(name="Hợp đồng A1", contract_type="Hợp đồng", product_group="GRP-A")
    make_template(name="Hợp đồng A2", contract_type="Hợp đồng", product_group="GRP-A")
    assert ts.default_template_for(db_session, "GRP-A", ts.ROLE_CONTRACT) is None


def test_default_is_none_when_only_shared_templates_exist(db_session, make_template):
    make_template(name="Hợp đồng chung", contract_type="Hợp đồng", product_group=None)
    assert ts.default_template_for(db_session, "GRP-A", ts.ROLE_CONTRACT) is None


# ─── check_template_for ──────────────────────────────────────────────────────

def test_matching_group_passes(make_template):
    tpl = make_template(contract_type="Hợp đồng", product_group="GRP-A")
    ts.check_template_for(tpl, "GRP-A", ts.ROLE_CONTRACT)


def test_shared_template_passes_for_any_group(make_template):
    tpl = make_template(contract_type="Hợp đồng", product_group=None)
    ts.check_template_for(tpl, "GRP-A", ts.ROLE_CONTRACT)
    ts.check_template_for(tpl, "GRP-B", ts.ROLE_CONTRACT)


def test_other_group_is_refused(make_template):
    tpl = make_template(contract_type="Hợp đồng", product_group="GRP-B")
    with pytest.raises(ts.TemplateMismatch):
        ts.check_template_for(tpl, "GRP-A", ts.ROLE_CONTRACT)


def test_wrong_role_is_refused(make_template):
    tpl = make_template(contract_type="Bàn giao", product_group=None)
    with pytest.raises(ts.TemplateMismatch):
        ts.check_template_for(tpl, "GRP-A", ts.ROLE_CONTRACT)


def test_group_template_used_for_a_product_without_group_is_refused(make_template):
    tpl = make_template(contract_type="Hợp đồng", product_group="GRP-A")
    with pytest.raises(ts.TemplateMismatch):
        ts.check_template_for(tpl, None, ts.ROLE_CONTRACT)


# ─── group_of_contract ───────────────────────────────────────────────────────

def test_addendum_takes_the_group_of_its_parent_contract(db_session, make_product, make_contract):
    main = make_product(code="GA-MAIN", name="Chính A", group_code="GRP-A", kind="main")
    addon = make_product(code="GA-ADD", name="Bổ sung A", group_code="GRP-A", kind="addon")
    parent = make_contract(product=main, contract_number="HD-GA-1")
    child = _contract_with_parent(make_contract, parent, addon)
    db_session.expire_all()

    assert ts.group_of_contract(child) == "GRP-A"
    assert ts.group_of_contract(parent) == "GRP-A"


def test_product_without_group_gives_none(db_session, make_product, make_contract):
    prod = make_product(code="NG-1", name="Không nhóm")
    contract = make_contract(product=prod)
    assert ts.group_of_contract(contract) is None
