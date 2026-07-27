"""Con số trên dashboard phải đúng, và phải tự làm mới được.

Hai lỗi bộ test này chốt lại:

1. `/api/stats` luôn trả 500. Nó chỉ ép `total_value_active` về chuỗi, còn ba
   trường tiền thêm vào sau vẫn là `Decimal` — JSON không mã hoá được. Phía JS
   `.catch()` nuốt lỗi, nên "tự động làm mới mỗi 5 phút" chưa từng chạy và
   không ai biết.
2. Bản làm mới đếm thẻ "Đang hiệu lực" theo công thức khác với lúc render lần
   đầu, nên nếu nó có chạy thì sau 5 phút con số sẽ tự nhảy sang một giá trị
   khác mà dữ liệu không hề đổi.
"""

from __future__ import annotations

import json
import re
from datetime import date, timedelta
from decimal import Decimal

import pytest
from fastapi.testclient import TestClient


@pytest.fixture()
def client(db_engine, monkeypatch):
    """Client nối vào đúng database tạm mà fixture đang ghi vào."""
    import sys

    from sqlalchemy.orm import sessionmaker

    import main

    factory = sessionmaker(autocommit=False, autoflush=False, bind=db_engine)
    for name, module in list(sys.modules.items()):
        if (name == "main" or name.startswith("app.")) and hasattr(module, "SessionLocal"):
            monkeypatch.setattr(module, "SessionLocal", factory, raising=False)
    return TestClient(main.app)


@pytest.fixture()
def portfolio(db_engine, db_session, make_customer, make_contract):
    """Một danh mục nhỏ nhưng đủ mọi nhóm trạng thái và tiền."""
    from app.models.contract import ContractStatus

    far = date.today() + timedelta(days=300)
    soon = date.today() + timedelta(days=10)

    made = {
        "signed_far": make_contract(status=ContractStatus.Signed,
                                    end_date=far, total_amount=Decimal("1000000")),
        "signed_soon": make_contract(status=ContractStatus.Signed,
                                     end_date=soon, total_amount=Decimal("2000000")),
        "invoiced": make_contract(status=ContractStatus.Invoiced,
                                  end_date=far, total_amount=Decimal("4000000")),
        "paid": make_contract(status=ContractStatus.PaidActive,
                              end_date=far, total_amount=Decimal("8000000")),
        "paid_soon": make_contract(status=ContractStatus.PaidActive,
                                   end_date=soon, total_amount=Decimal("16000000")),
        # Nhiễu: chưa ký và đã kết thúc, không được tính vào đâu cả.
        "draft": make_contract(status=ContractStatus.Draft,
                               end_date=far, total_amount=Decimal("99000000")),
        "terminated": make_contract(status=ContractStatus.Terminated,
                                    end_date=far, total_amount=Decimal("77000000")),
    }
    return made


# ─── Endpoint phải sống ──────────────────────────────────────────────────────

def test_api_stats_returns_json_not_a_server_error(client, portfolio):
    """Lỗi gốc: Decimal không mã hoá được sang JSON → 500 mọi lúc."""
    response = client.get("/api/stats")

    assert response.status_code == 200, response.text
    assert response.headers["content-type"].startswith("application/json")
    json.loads(response.text)          # phải phân tích được


def test_api_stats_serialises_every_money_field(client, portfolio):
    """Thêm một chỉ số tiền mới mà quên ép kiểu là endpoint chết lần nữa."""
    summary = client.get("/api/stats").json()["summary"]

    money_keys = [k for k in summary if k.startswith("value_") or k.startswith("total_value")]
    assert money_keys, "summary phải có chỉ số tiền"
    for key in money_keys:
        assert isinstance(summary[key], (str, int, float)), (
            f"{key} chưa được ép kiểu — JSON sẽ vỡ"
        )


# ─── Con số phải đúng ────────────────────────────────────────────────────────

def test_counts_match_the_underlying_data(client, portfolio):
    summary = client.get("/api/stats").json()["summary"]

    assert summary["total_contracts"] == 7
    assert summary["in_force"] == 5          # signed x2 + invoiced + paid x2
    assert summary["in_service"] == 3        # bỏ Signed
    assert summary["signed"] == 2
    assert summary["invoiced"] == 1
    assert summary["paid_active"] == 2
    assert summary["draft"] == 1
    assert summary["terminated"] == 1
    assert summary["expiring_soon"] == 2     # một Signed + một PaidActive, còn 10 ngày


def test_money_split_adds_up_to_the_total(client, portfolio):
    summary = client.get("/api/stats").json()["summary"]

    total = Decimal(summary["total_value_active"])
    parts = (Decimal(summary["value_paid"])
             + Decimal(summary["value_invoiced"])
             + Decimal(summary["value_unpaid"]))

    assert total == Decimal("31000000"), "1M + 2M + 4M + 8M + 16M"
    assert parts == total, "ba dòng tách tiền phải cộng đúng bằng tổng"


def test_terminated_and_draft_money_is_excluded(client, portfolio):
    """Hợp đồng chưa ký hoặc đã kết thúc không được cộng vào doanh thu."""
    summary = client.get("/api/stats").json()["summary"]

    assert Decimal(summary["total_value_active"]) == Decimal("31000000")
    assert "99000000" not in summary["total_value_active"]
    assert "77000000" not in summary["total_value_active"]


# ─── Làm mới không được mâu thuẫn với trang ──────────────────────────────────

def _rendered(page: str, attribute: str) -> dict[str, str]:
    return dict(re.findall(rf'data-{attribute}="(\w+)"[^>]*>([\d.,]+)<', page))


def test_refresh_agrees_with_the_first_render(client, portfolio):
    """Không có dữ liệu nào đổi giữa hai lần gọi — con số phải y hệt.

    Đây chính là lỗi thứ hai: thẻ "Đang hiệu lực" render lần đầu bằng
    `in_service`, nhưng bản làm mới cộng `active + invoiced + expiring_soon`,
    nên sau 5 phút con số tự nhảy dù dữ liệu đứng yên.
    """
    page = client.get("/dashboard").text
    cards = _rendered(page, "stat")
    fresh = client.get("/api/stats").json()["cards"]

    assert cards, "trang phải có thẻ mang data-stat"
    for key, shown in cards.items():
        assert str(fresh[key]) == shown.replace(".", ""), (
            f"thẻ '{key}': trang hiện {shown}, làm mới lại đổi thành {fresh[key]}"
        )


def test_money_refresh_agrees_with_the_first_render(client, portfolio):
    page = client.get("/dashboard").text
    shown = _rendered(page, "money")
    fresh = client.get("/api/stats").json()["money"]

    assert shown, "phần tiền phải gắn data-money để làm mới được"
    for key, value in shown.items():
        assert str(fresh[key]) == value, (
            f"'{key}': trang hiện {value}, làm mới lại đổi thành {fresh[key]}"
        )


def test_expiring_soon_is_an_overlay_not_a_separate_bucket(client, portfolio):
    """`expiring_soon` nằm TRONG in_force — cộng thêm là đếm trùng."""
    summary = client.get("/api/stats").json()["summary"]
    cards = client.get("/api/stats").json()["cards"]

    assert summary["expiring_soon"] <= summary["in_force"]
    assert cards["active"] == summary["in_service"], (
        "thẻ 'Đang hiệu lực' phải là in_service, không phải một phép cộng tay"
    )


def test_numbers_follow_the_data_when_it_changes(client, db_session, portfolio):
    """Làm mới phải phản ánh thay đổi thật, không phải trả lại bản chụp cũ."""
    from app.models.contract import ContractStatus
    from app.services.contract_service import update_status

    before = client.get("/api/stats").json()["summary"]
    assert before["invoiced"] == 1

    contract = portfolio["signed_far"]
    contract.invoice_pdf_path = "uploads/invoice_docs/hd.pdf"
    db_session.commit()
    update_status(db_session, contract.id, "Invoiced")

    after = client.get("/api/stats").json()["summary"]

    assert after["invoiced"] == 2
    assert after["signed"] == 1
    assert Decimal(after["value_invoiced"]) == Decimal("5000000")   # 4M + 1M
    assert Decimal(after["value_unpaid"]) == Decimal("2000000")     # còn mỗi signed_soon
    # Tổng không đổi: hợp đồng chỉ chuyển nhóm, không rời khỏi vòng đời.
    assert Decimal(after["total_value_active"]) == Decimal(before["total_value_active"])
