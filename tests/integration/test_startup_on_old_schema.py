"""Khởi động trên DB cũ chưa có cột contract_templates.product_group.

Chạy đúng lifespan của main.py (init_db → migration → seed → dọn file mồ côi → cập nhật trạng thái).
Migration phải chạy trước mọi truy vấn mẫu, nếu không mọi bản cài cũ sẽ không khởi động được.
"""

import asyncio
import warnings

from sqlalchemy import text


def test_startup_on_a_database_without_the_product_group_column(
        db_engine, db_session, make_template, monkeypatch, data_root):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", DeprecationWarning)
        import main

    import app.database as database
    from app.models.contract_template import ContractTemplate

    tpl = make_template(name="Mẫu từ DB cũ", contract_type="Hợp đồng")
    tpl_id = tpl.id
    db_session.commit()

    # DB thật: không có cột (và chỉ mục của cột) product_group.
    with db_engine.begin() as conn:
        conn.execute(text("DROP INDEX IF EXISTS ix_contract_templates_product_group"))
        conn.execute(text("ALTER TABLE contract_templates DROP COLUMN product_group"))
    db_session.expire_all()

    monkeypatch.setattr(main, "init_db", database.init_db)
    monkeypatch.setattr(main, "SessionLocal", database.SessionLocal)
    # template_service giữ đường dẫn thư mục mẫu từ lúc import: trỏ về thư mục tạm,
    # để dọn file mồ côi của lần khởi động không chạm vào data/ thật.
    import app.services.template_service as template_service
    monkeypatch.setattr(template_service, "UPLOAD_DIR", str(data_root / "uploads" / "templates"))

    async def start_and_stop():
        async with main.lifespan(main.app):
            pass

    asyncio.run(start_and_stop())

    with db_engine.connect() as conn:
        cols = [r[1] for r in conn.execute(text("PRAGMA table_info(contract_templates)"))]
    assert "product_group" in cols

    db_session.expire_all()
    kept = db_session.get(ContractTemplate, tpl_id)
    assert kept.name == "Mẫu từ DB cũ"
    assert kept.product_group is None
