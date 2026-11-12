"""Mọi khóa của build_render_context phải có trong CANONICAL_MAP.

Canh về sau: thêm placeholder mới vào renderer mà quên ánh xạ thì test này đỏ,
và trang chi tiết mẫu sẽ không báo "chưa ánh xạ" một cách sai.
"""
from datetime import date
from decimal import Decimal

from app.services.docx_renderer import build_render_context
from app.services.template_analyzer import CANONICAL_MAP


def test_every_render_key_has_a_canonical_mapping(db_session, make_customer, make_product,
                                                  make_contract):
    customer = make_customer()
    product = make_product(code="MAP-P", name="Gói ánh xạ", group_code="MAP-G")
    parent = make_contract(customer=customer, product=product, contract_number="HD-MAP-1",
                           sign_date=date(2027, 1, 1), start_date=date(2027, 1, 1),
                           end_date=date(2027, 12, 31), total_amount=Decimal("1000"))
    db_session.expire_all()

    ctx = build_render_context(parent, customer, product, None, db=db_session)

    missing = sorted(set(ctx) - set(CANONICAL_MAP))
    assert missing == []
