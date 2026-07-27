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
