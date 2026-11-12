"""Migration cột contract_templates.product_group: thêm một lần, chạy lại vẫn an toàn."""
from sqlalchemy import text


def test_migration_adds_product_group_and_keeps_old_templates(db_session, make_template):
    from app.models.contract_template import ContractTemplate
    from app.models.seed import run_migrations

    old = make_template(name="Mẫu cũ trước nhóm", contract_type="Hợp đồng")
    old_id = old.id
    # Đưa DB về trạng thái trước khi có cột (SQLite: bỏ chỉ mục trước khi bỏ cột).
    db_session.execute(text("DROP INDEX IF EXISTS ix_contract_templates_product_group"))
    db_session.execute(text("ALTER TABLE contract_templates DROP COLUMN product_group"))
    db_session.commit()
    cols_before = [r[1] for r in db_session.execute(text("PRAGMA table_info(contract_templates)"))]
    assert "product_group" not in cols_before

    run_migrations(db_session)
    cols_after = [r[1] for r in db_session.execute(text("PRAGMA table_info(contract_templates)"))]
    assert "product_group" in cols_after

    run_migrations(db_session)  # chạy lần hai: không lỗi, không thêm cột trùng

    db_session.expire_all()
    tpl = db_session.get(ContractTemplate, old_id)
    assert tpl.name == "Mẫu cũ trước nhóm"
    assert tpl.contract_type == "Hợp đồng"
    assert tpl.product_group is None
