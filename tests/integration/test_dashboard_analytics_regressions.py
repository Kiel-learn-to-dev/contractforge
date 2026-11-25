"""Regression checks for dashboard analytics that used to omit or miscount data."""

from datetime import date, datetime, timedelta
from decimal import Decimal


def _freeze_dashboard_today(monkeypatch, year=2026, month=11, day=25):
    from app.services import dashboard_service

    class FrozenDate(date):
        @classmethod
        def today(cls):
            return cls(year, month, day)

    monkeypatch.setattr(dashboard_service, "date", FrozenDate)


def test_expiry_forecast_includes_current_month_and_stops_at_month_twelve(
    db_session, make_contract, monkeypatch,
):
    from app.models.contract import ContractStatus
    from app.services.dashboard_service import get_monthly_expiry_forecast

    _freeze_dashboard_today(monkeypatch)
    make_contract(status=ContractStatus.PaidActive, end_date=date(2026, 11, 26))
    make_contract(status=ContractStatus.PaidActive, end_date=date(2027, 10, 31))
    make_contract(status=ContractStatus.PaidActive, end_date=date(2027, 11, 1))

    forecast = get_monthly_expiry_forecast(db_session)

    assert len(forecast) == 12
    assert forecast[0]["month"] == "2026-11"
    assert forecast[0]["count"] == 1
    assert forecast[-1]["month"] == "2027-10"
    assert forecast[-1]["count"] == 1  # October is the twelfth calendar month


def test_unclassified_product_value_is_included(db_session, make_contract):
    from app.models.contract import ContractStatus
    from app.services.dashboard_service import _contracts_by_product

    make_contract(status=ContractStatus.PaidActive, product_id=None,
                  total_amount=Decimal("123456"))

    unclassified = next(row for row in _contracts_by_product(db_session)
                        if row["name"] == "Không phân loại")

    assert unclassified["count"] == 1
    assert unclassified["total_value"] == Decimal("123456")


def test_monthly_stats_only_count_signed_and_expired_lifecycle_contracts(
    db_session, make_contract, monkeypatch,
):
    from app.models.contract import ContractStatus
    from app.services.dashboard_service import get_monthly_stats

    _freeze_dashboard_today(monkeypatch)
    today = date(2026, 11, 25)
    month_start = today.replace(day=1)
    this_month_end = date(2026, 11, 30)
    # Drafts can have a default sign_date and end_date, but neither means they
    # were signed or expired.
    make_contract(status=ContractStatus.Draft, sign_date=month_start,
                  end_date=month_start)
    make_contract(status=ContractStatus.Signed, sign_date=month_start,
                  end_date=this_month_end + timedelta(days=5))
    make_contract(status=ContractStatus.Expired, sign_date=month_start,
                  end_date=month_start)
    make_contract(status=ContractStatus.Terminated, sign_date=month_start,
                  end_date=month_start)
    make_contract(status=ContractStatus.Signed, sign_date=today + timedelta(days=1),
                  end_date=this_month_end + timedelta(days=5))
    # End date falls in this month but remains in the future; it is not expired.
    make_contract(status=ContractStatus.PaidActive, sign_date=month_start,
                  end_date=this_month_end)

    stats = get_monthly_stats(db_session)

    assert stats["signed"] == 4  # Draft and future-dated Signed are excluded
    assert stats["expired_count"] == 1


def test_monthly_created_range_includes_last_microsecond_of_month(
    db_session, make_contract, monkeypatch,
):
    from app.services.dashboard_service import get_monthly_stats

    _freeze_dashboard_today(monkeypatch)
    last_day = date(2026, 11, 30)
    last_instant = datetime.combine(last_day, datetime.max.time())
    make_contract(created_at=last_instant)

    assert get_monthly_stats(db_session)["new_contracts"] == 1


def test_generated_card_link_uses_the_same_file_presence_filter(db_session, make_contract):
    from app.services.contract_service import list_contracts
    from app.services.dashboard_service import get_dashboard_data

    make_contract(output_file_path="contracts/has-file.docx")
    make_contract(output_file_path=None)
    make_contract(output_file_path="")
    make_contract(output_file_path="   ")

    dashboard_count = get_dashboard_data(db_session)["summary"]["generated"]
    _, linked_count = list_contracts(db_session, alert_filter="generated")

    assert dashboard_count == linked_count == 1


def test_expiring_60_contract_filter_matches_dashboard_sections(
    db_session, make_contract,
):
    from app.models.contract import ContractStatus
    from app.services.contract_service import list_contracts
    from app.services.dashboard_service import get_dashboard_data

    today = date.today()
    make_contract(status=ContractStatus.PaidActive, end_date=today)
    make_contract(status=ContractStatus.PaidActive, end_date=today + timedelta(days=30))
    make_contract(status=ContractStatus.PaidActive, end_date=today + timedelta(days=60))
    make_contract(status=ContractStatus.PaidActive, end_date=today + timedelta(days=61))
    renewed_parent = make_contract(status=ContractStatus.PaidActive,
                                   end_date=today + timedelta(days=20))
    make_contract(status=ContractStatus.Signed, end_date=today + timedelta(days=400),
                  renewed_from_id=renewed_parent.id)
    addon_parent = make_contract(status=ContractStatus.PaidActive,
                                 end_date=today + timedelta(days=40))
    make_contract(status=ContractStatus.PaidActive, end_date=addon_parent.end_date,
                  doc_kind="addendum_addon", parent_contract_id=addon_parent.id)

    data = get_dashboard_data(db_session)
    _, linked_count = list_contracts(db_session, alert_filter="expiring_60")

    assert len(data["expiring_30"]) + len(data["expiring_31_60"]) == 3
    assert linked_count == 3
