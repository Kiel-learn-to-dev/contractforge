"""Explicit reusable issuer profiles; no quote prices or recipients are persisted."""
from html.parser import HTMLParser
from io import BytesIO
import json

from docx import Document
import pytest
from sqlalchemy import create_engine, inspect, text
from sqlalchemy.orm import Session


def issuer(**overrides):
    values = dict(issuer_name="Đơn vị Báo Giá Mẫu", issuer_address="Địa chỉ nhập riêng",
                  issuer_tax_code="0300000099", issuer_representative="Nguyễn Văn Mẫu",
                  issuer_title="Giám đốc", issuer_phone="0900000000", issuer_email="contact@example.test")
    values.update(overrides)
    return values


def test_save_profile_reuses_exact_duplicate_and_survives_new_session(client, db_engine):
    from app.models.quotation_issuer import QuotationIssuer
    first = client.post("/quotation/manual/issuers", data=issuer())
    assert first.status_code == 201
    data = first.json()["issuer"]
    assert data["name"] == issuer()["issuer_name"]
    repeat = client.post("/quotation/manual/issuers", data=issuer(issuer_name="  Đơn vị Báo Giá Mẫu  "))
    assert repeat.status_code == 200 and repeat.json()["issuer"]["id"] == data["id"]
    with Session(db_engine) as db:
        profiles = db.query(QuotationIssuer).all()
        assert len(profiles) == 1 and profiles[0].phone == "0900000000"
    page = client.get("/quotation/manual")
    assert f'value="{data["id"]}"' in page.text
    assert "Đơn vị Báo Giá Mẫu" in page.text
    class Inputs(HTMLParser):
        def handle_starttag(self, tag, attrs):
            attrs = dict(attrs)
            if tag == "input" and attrs.get("id") == "issuer_name":
                assert attrs.get("value") == ""
    Inputs().feed(page.text)


def test_saving_changed_details_does_not_overwrite_saved_profile(client, db_engine):
    from app.models.quotation_issuer import QuotationIssuer
    first = client.post("/quotation/manual/issuers", data=issuer()).json()["issuer"]
    second = client.post("/quotation/manual/issuers", data=issuer(issuer_phone="0911111111"))
    assert second.status_code == 201 and second.json()["issuer"]["id"] != first["id"]
    with Session(db_engine) as db:
        assert db.get(QuotationIssuer, first["id"]).phone == "0900000000"


def test_save_whitelists_identity_and_escapes_html(client, db_engine):
    from app.models.quotation_issuer import QuotationIssuer
    payload = issuer(issuer_name='Đơn vị <script>alert("test")</script>',
                     items_json="DO-NOT-STORE", recipient_name="DO-NOT-STORE", price="777123")
    response = client.post("/quotation/manual/issuers", data=payload)
    assert response.status_code == 201
    assert set(response.json()["issuer"]) == {"id", "name", "address", "tax_code", "representative", "title", "phone", "email"}
    with Session(db_engine) as db:
        profile = db.query(QuotationIssuer).one()
        assert not hasattr(profile, "price") and not hasattr(profile, "items_json")
    page = client.get("/quotation/manual")
    assert '<script>alert("test")</script>' not in page.text
    assert "DO-NOT-STORE" not in page.text


@pytest.mark.parametrize("values", [issuer(issuer_name="  "), issuer(issuer_address="x"*501)])
def test_invalid_profile_is_not_saved(client, db_engine, values):
    from app.models.quotation_issuer import QuotationIssuer
    response = client.post("/quotation/manual/issuers", data=values)
    assert response.status_code == 422 and response.json()["error"]
    with Session(db_engine) as db:
        assert db.query(QuotationIssuer).count() == 0


def test_quote_generation_uses_current_fields_without_modifying_saved_snapshot(client, db_engine):
    from app.models.quotation_issuer import QuotationIssuer
    saved = client.post("/quotation/manual/issuers", data=issuer()).json()["issuer"]
    values = issuer(issuer_name="Tên chỉnh riêng cho lần báo giá này", issuer_phone="0922222222")
    values.update(saved_issuer_id=saved["id"], layout="compact", recipient_name="Khách hàng Mẫu",
                  quote_date="2026-11-24", items_json=json.dumps([
                      dict(name="Dịch vụ", unit="Gói", quantity=1, periods=1, price="123456", vat=0)]))
    response = client.post("/quotation/manual/generate", data=values)
    assert response.status_code == 200
    doc = Document(BytesIO(response.content))
    content = "\n".join([p.text for p in doc.paragraphs] + [c.text for t in doc.tables for r in t.rows for c in r.cells])
    assert "Tên chỉnh riêng cho lần báo giá này" in content and "123.456" in content
    with Session(db_engine) as db:
        profile = db.get(QuotationIssuer, saved["id"])
        assert profile.name == "Đơn vị Báo Giá Mẫu" and profile.phone == "0900000000"


def test_schema_addition_preserves_existing_tables_and_is_idempotent(tmp_path):
    import app.models
    from app.database import Base
    engine = create_engine(f"sqlite:///{(tmp_path/'old.db').as_posix()}")
    try:
        with engine.begin() as conn:
            conn.execute(text("CREATE TABLE existing_contracts (id INTEGER PRIMARY KEY, amount NUMERIC)"))
            conn.execute(text("INSERT INTO existing_contracts VALUES (1, 123456)"))
        Base.metadata.create_all(engine)
        with Session(engine) as db:
            from app.services.quotation_issuer_service import save_issuer
            profile, created = save_issuer(db, issuer())
            profile_id = profile.id
            assert created
        Base.metadata.create_all(engine)
        with engine.connect() as conn:
            assert conn.execute(text("SELECT amount FROM existing_contracts WHERE id=1")).scalar_one() == 123456
            assert conn.execute(text("SELECT id FROM quotation_issuers")).scalar_one() == profile_id
        assert "quotation_issuers" in inspect(engine).get_table_names()
    finally:
        engine.dispose()


def test_cross_origin_profile_save_is_blocked(client):
    response = client.post("/quotation/manual/issuers", data=issuer(), headers={"Origin":"https://example.test"})
    assert response.status_code == 403
