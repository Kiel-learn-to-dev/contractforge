"""Ranh giới bảo mật của ứng dụng cục bộ (OPEN_SOURCE_DESKTOP_PLAN.md Task 11).

App không có đăng nhập, nên ranh giới bảo mật chính là ranh giới máy. Bộ test
này khoá bốn thứ: chỉ loopback, chặn cross-origin, không XSS lưu trữ, và không
sửa được tài nguyên con của khách hàng khác.
"""

from __future__ import annotations

from pathlib import Path

import pytest
from fastapi.testclient import TestClient


@pytest.fixture()
def client(db_engine, monkeypatch):
    """TestClient nối vào ĐÚNG database tạm mà các fixture đang ghi vào.

    Router dùng `from app.database import SessionLocal`, tức đã giữ tham chiếu
    hàm từ lúc import. Vá thuộc tính trên module `app.database` không đổi được
    tham chiếu đó, nên phải vá từng module đang giữ nó — cùng cách
    test_generic_quotation.py và test_organization_profile.py đang làm, chỉ là
    quét toàn bộ vì bài test này chạm nhiều router.

    Dùng sessionmaker thật (không phải `lambda: db_session`) để mỗi request có
    session riêng — router nào cũng `db.close()` trong finally, sẽ đóng mất
    session dùng chung.
    """
    import sys

    from sqlalchemy.orm import sessionmaker

    import main  # phải import trước khi quét: nó kéo theo mọi router

    factory = sessionmaker(autocommit=False, autoflush=False, bind=db_engine)
    for name, module in list(sys.modules.items()):
        if (name == "main" or name.startswith("app.")) and hasattr(module, "SessionLocal"):
            monkeypatch.setattr(module, "SessionLocal", factory, raising=False)

    return TestClient(main.app)


# ─── Chặn cross-origin ───────────────────────────────────────────────────────

def test_cross_origin_post_is_refused(client):
    resp = client.post(
        "/contracts/bulk-action",
        data={"bulk_action": "delete", "selected_ids": ["1"]},
        headers={"Origin": "https://evil.example"},
        follow_redirects=False,
    )
    assert resp.status_code == 403


def test_cross_origin_referer_is_refused(client):
    """Trình duyệt bỏ Origin ở vài form cũ nhưng vẫn gửi Referer."""
    resp = client.post(
        "/contracts/bulk-action",
        data={"bulk_action": "delete", "selected_ids": ["1"]},
        headers={"Referer": "https://evil.example/attack.html"},
        follow_redirects=False,
    )
    assert resp.status_code == 403


@pytest.mark.parametrize("origin", [
    "http://localhost:8000",
    "http://127.0.0.1:8888",
    "http://localhost",
])
def test_local_origins_are_allowed(client, origin):
    resp = client.post(
        "/contracts/bulk-action",
        data={"bulk_action": "", "selected_ids": []},
        headers={"Origin": origin},
        follow_redirects=False,
    )
    assert resp.status_code != 403


def test_reads_are_never_blocked(client):
    """GET không đổi dữ liệu — chặn nó chỉ làm phiền, không tăng an toàn."""
    resp = client.get("/health", headers={"Origin": "https://evil.example"})
    assert resp.status_code == 200


def test_requests_without_origin_still_work(client):
    """Launcher, script, và bộ test gọi thẳng, không có Origin lẫn Referer."""
    resp = client.post(
        "/contracts/bulk-action",
        data={"bulk_action": "", "selected_ids": []},
        follow_redirects=False,
    )
    assert resp.status_code != 403


@pytest.mark.parametrize("url,expected", [
    ("http://localhost:8000", True),
    ("http://127.0.0.1:9000/x", True),
    ("http://[::1]:8000", True),
    ("https://evil.example", False),
    ("http://localhost.evil.example", False),   # tiền tố lừa
    ("http://127.0.0.1.evil.example", False),
    ("", False),
])
def test_origin_classification(db_engine, url, expected):
    from app.security import is_local_origin
    assert is_local_origin(url) is expected


# ─── XSS lưu trữ ─────────────────────────────────────────────────────────────

XSS = '<script>window.__pwned=1</script>'


def test_customer_name_is_escaped_in_lists(client, make_customer):
    make_customer(legal_name=f"Công ty {XSS}", short_name=f"CT {XSS}")

    body = client.get("/customers").text

    assert "<script>window.__pwned" not in body
    assert "&lt;script&gt;" in body


def test_search_api_returns_data_not_markup(client, make_customer):
    """/api/search trả JSON thô; việc thoát HTML là của lớp DOM.

    Hợp đồng giữa hai bên: API trả đúng chuỗi người dùng đã lưu, không thêm
    không bớt, và base.html gắn nó bằng textContent. Test này chốt nửa đầu —
    nửa sau do test_search_dropdown_never_builds_markup_from_user_data chốt.
    """
    name = f"BV {XSS}"
    make_customer(legal_name="Bệnh viện Đa khoa", short_name=name)

    resp = client.get("/api/search", params={"q": "Bệnh"})

    assert resp.headers["content-type"].startswith("application/json")
    labels = [r["label"] for r in resp.json()["results"]]
    assert name in labels, f"phải trả đúng chuỗi đã lưu, nhận được {labels}"


def test_search_dropdown_never_builds_markup_from_user_data():
    """base.html phải gắn nhãn kết quả bằng textContent, không nối innerHTML.

    Đây là kiểm tra ở mức mã nguồn vì lỗ hổng nằm trong JavaScript phía client,
    thứ bộ test Python không chạy được. Nối `r.label` vào một chuỗi innerHTML là
    dựng lại đúng lỗ XSS lưu trữ vừa vá.
    """
    base = (Path(__file__).resolve().parents[2]
            / "ContractForge" / "templates" / "base.html")
    source = base.read_text(encoding="utf-8")

    offenders = [
        stripped
        for stripped in (line.strip() for line in source.splitlines())
        if not stripped.startswith("//")          # chú thích được nhắc tới cả hai
        and "innerHTML" in stripped
        and ("r.label" in stripped or "r.sub" in stripped)
    ]
    assert not offenders, f"nhãn kết quả tìm kiếm bị nối vào innerHTML: {offenders}"

    assert "labelEl.textContent = r.label" in source
    assert "subEl.textContent = r.sub" in source


def test_batch_page_does_not_inline_unescaped_customer_html(client, make_customer):
    """Trang sinh hàng loạt nhúng JSON khách hàng vào khối <script>."""
    make_customer(legal_name="Trạm </script><script>window.__pwned=1</script>",
                  short_name="Trạm hiểm")

    body = client.get("/batch").text

    # `| tojson` escape < > & nên chuỗi đóng thẻ không thoát ra được.
    assert "</script><script>window.__pwned" not in body


def test_dashboard_chart_data_is_escaped(client, make_product, make_contract):
    from app.models.contract import ContractStatus

    product = make_product(name="SP </script><script>window.__pwned=1</script>")
    make_contract(product=product, status=ContractStatus.PaidActive)

    body = client.get("/dashboard").text

    assert "</script><script>window.__pwned" not in body


# ─── Quyền sở hữu tài nguyên con ─────────────────────────────────────────────

def test_sub_unit_cannot_be_stolen_across_customers(db_engine, db_session,
                                                    make_customer):
    """Sửa id đơn vị con trong form không được ghi đè đơn vị của KH khác."""
    from app.models.customer_unit import CustomerUnit
    from app.services.customer_service import save_sub_units

    victim = make_customer(code="KH-VICTIM")
    attacker = make_customer(code="KH-ATTACK")

    save_sub_units(db_session, victim.id, [{"name": "Trạm của nạn nhân"}])
    stolen = db_session.query(CustomerUnit).filter(
        CustomerUnit.customer_id == victim.id).one()

    # Kẻ tấn công gửi id của đơn vị thuộc khách hàng khác.
    save_sub_units(db_session, attacker.id,
                   [{"id": stolen.id, "name": "ĐÃ BỊ CHIẾM"}])

    db_session.refresh(stolen)
    assert stolen.name == "Trạm của nạn nhân"
    assert stolen.customer_id == victim.id

    # Vẫn tạo được đơn vị mới cho chính kẻ tấn công — không phải chặn cứng.
    attacker_units = db_session.query(CustomerUnit).filter(
        CustomerUnit.customer_id == attacker.id).all()
    assert [u.name for u in attacker_units] == ["ĐÃ BỊ CHIẾM"]


def test_sub_unit_update_still_works_for_the_owner(db_engine, db_session,
                                                   make_customer):
    from app.models.customer_unit import CustomerUnit
    from app.services.customer_service import save_sub_units

    customer = make_customer()
    save_sub_units(db_session, customer.id, [{"name": "Trạm A"}])
    unit = db_session.query(CustomerUnit).filter(
        CustomerUnit.customer_id == customer.id).one()

    save_sub_units(db_session, customer.id, [{"id": unit.id, "name": "Trạm A đổi tên"}])

    db_session.refresh(unit)
    assert unit.name == "Trạm A đổi tên"


# ─── Dọn file khi xoá ────────────────────────────────────────────────────────

def _touch(tmp_path: Path, name: str) -> str:
    path = tmp_path / name
    path.write_bytes(b"noi dung gia lap")
    return str(path)


def test_deleting_a_contract_removes_its_files(db_engine, db_session,
                                               make_contract, tmp_path):
    from app.services.contract_service import delete_contract

    contract = make_contract()
    paths = {
        "output_file_path": _touch(tmp_path, "hop_dong.docx"),
        "signed_pdf_path": _touch(tmp_path, "da_ky.pdf"),
        "invoice_pdf_path": _touch(tmp_path, "hoa_don.pdf"),
        "payment_slip_path": _touch(tmp_path, "uy_nhiem_chi.pdf"),
    }
    for column, path in paths.items():
        setattr(contract, column, path)
    db_session.commit()

    assert delete_contract(db_session, contract.id) is True

    for path in paths.values():
        assert not Path(path).exists(), f"còn sót file nhạy cảm: {path}"


def test_bulk_delete_removes_files_too(db_engine, db_session, make_contract, tmp_path):
    from app.services.contract_service import bulk_delete

    contracts = []
    for i in range(3):
        c = make_contract()
        c.output_file_path = _touch(tmp_path, f"hd_{i}.docx")
        contracts.append(c)
    db_session.commit()
    paths = [c.output_file_path for c in contracts]

    assert bulk_delete(db_session, [c.id for c in contracts]) == 3

    assert not any(Path(p).exists() for p in paths)


def test_delete_survives_a_missing_file(db_engine, db_session, make_contract):
    """File bị xoá tay từ trước không được chặn việc xoá bản ghi."""
    from app.services.contract_service import delete_contract

    contract = make_contract()
    contract.output_file_path = "Z:/khong/ton/tai/hop_dong.docx"
    db_session.commit()

    assert delete_contract(db_session, contract.id) is True


def test_customer_documents_survive_contract_deletion(db_engine, db_session,
                                                      make_customer, make_contract,
                                                      make_document, tmp_path):
    """Hồ sơ giấy tờ thuộc về khách hàng, không thuộc hợp đồng nào."""
    from app.models.customer_document import CustomerDocument
    from app.services.contract_service import delete_contract

    customer = make_customer()
    doc_path = _touch(tmp_path, "giay_phep.pdf")
    make_document(customer=customer, file_path=doc_path)
    contract = make_contract(customer=customer)

    delete_contract(db_session, contract.id)

    assert Path(doc_path).exists()
    assert db_session.query(CustomerDocument).filter(
        CustomerDocument.customer_id == customer.id).count() == 1
