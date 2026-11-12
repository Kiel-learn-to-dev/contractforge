"""Integration tests for the neutral organization profile (Task 5)."""

from __future__ import annotations

import warnings


def test_clean_database_has_no_prefilled_organization_profile(db_session):
    from app.services.settings_service import (
        get_party_b_settings,
        is_organization_profile_complete,
    )

    profile = get_party_b_settings(db_session)

    assert profile["name"] == ""
    assert profile["representative"] == ""
    assert not is_organization_profile_complete(db_session)


def test_saved_organization_profile_is_complete_and_preserved(db_session):
    from app.services.settings_service import (
        get_party_b_settings,
        is_organization_profile_complete,
        set_setting,
    )

    set_setting(db_session, "party_b_name", "Công ty Mẫu")
    set_setting(db_session, "party_b_representative", "Người Đại Diện")

    assert is_organization_profile_complete(db_session)
    assert get_party_b_settings(db_session)["name"] == "Công ty Mẫu"


def test_missing_new_profile_fields_do_not_overwrite_existing_settings(db_session):
    from app.services.settings_service import get_party_b_settings, set_setting

    set_setting(db_session, "party_b_name", "Tổ chức đã lưu")

    profile = get_party_b_settings(db_session)

    assert profile["name"] == "Tổ chức đã lưu"
    assert profile["bank_account"] == ""


def test_root_sends_new_installation_to_organization_setup(db_session, monkeypatch):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", DeprecationWarning)
        import main

    monkeypatch.setattr(main, "SessionLocal", lambda: db_session)

    response = main.root()

    assert response.headers["location"] == "/settings?setup=1"


def test_authorization_falls_back_to_saved_settings_when_contract_has_none(
        db_session, make_customer, make_product, make_contract):
    from datetime import date
    from app.services.docx_renderer import build_render_context
    from app.services.settings_service import set_setting

    set_setting(db_session, "party_b_authorization_number", "GU-CAI-DAT-01")
    set_setting(db_session, "party_b_authorization_date", "01/01/2027")
    customer = make_customer()
    product = make_product(code="AUTH-P", name="Gói ủy quyền")
    contract = make_contract(customer=customer, product=product, contract_number="HD-AUTH-1",
                             sign_date=date(2027, 1, 1), start_date=date(2027, 1, 1),
                             end_date=date(2027, 12, 31))
    db_session.expire_all()

    ctx = build_render_context(contract, customer, product, None, db=db_session)

    assert ctx["PARTY_B_AUTHORIZATION_NUMBER"] == "GU-CAI-DAT-01"
    assert ctx["PARTY_B_AUTHORIZATION_DATE"] == "01/01/2027"


def test_contract_own_authorization_wins_over_saved_settings(
        db_session, make_customer, make_product, make_contract):
    from datetime import date
    from app.services.docx_renderer import build_render_context
    from app.services.settings_service import set_setting

    set_setting(db_session, "party_b_authorization_number", "GU-CAI-DAT-01")
    customer = make_customer()
    product = make_product(code="AUTH-Q", name="Gói ủy quyền riêng")
    contract = make_contract(customer=customer, product=product, contract_number="HD-AUTH-2",
                             sign_date=date(2027, 1, 1), start_date=date(2027, 1, 1),
                             end_date=date(2027, 12, 31),
                             party_b_authorization_number="GU-RIENG-02")
    db_session.expire_all()

    ctx = build_render_context(contract, customer, product, None, db=db_session)

    assert ctx["PARTY_B_AUTHORIZATION_NUMBER"] == "GU-RIENG-02"
