"""Task 2.1 — bảng contract_items, cột mới của contracts, chuyển dữ liệu cũ.

Dữ liệu hoàn toàn hư cấu. Mục tiêu: mỗi hợp đồng có đúng một dòng chép từ số đã
lưu, KHÔNG có số nào bị tính lại, chạy lại không tạo dòng trùng, và xóa hợp đồng
thì xóa luôn dòng của nó.
"""

import glob
import sqlite3
from contextlib import closing
from decimal import Decimal


def _money_snapshot(db_session):
    from app.models.contract import Contract

    db_session.expire_all()
    return {
        c.id: (c.total_amount, c.amount_text, c.unit_price_pre_vat,
               c.unit_price_vat, c.vat_rate)
        for c in db_session.query(Contract).all()
    }


def _seed_three_legacy_contracts(make_product, make_contract):
    product = make_product(code="MG-LEG", name="Gói mẫu cũ", unit_label="cơ sở")
    contracts = [
        # Thuế 10, tổng tròn, có đơn giá và chữ.
        make_contract(product=product, total_amount=Decimal("1100000"), vat_rate=10,
                      unit_count=2, month_count=12, unit_price_pre_vat=Decimal("50000"),
                      unit_price_vat=Decimal("55000"), amount_text="Một triệu một trăm nghìn đồng"),
        # Thuế 8, tổng lẻ: pre-VAT phải suy ngược ra đúng và cộng lại đúng tổng.
        make_contract(product=product, total_amount=Decimal("17142542"), vat_rate=8,
                      unit_count=3, month_count=12, unit_price_pre_vat=None,
                      unit_price_vat=None, amount_text=None),
        # Tổng rỗng: dòng có amount_total = 0.
        make_contract(product=product, total_amount=None, vat_rate=0,
                      unit_count=1, month_count=6, unit_price_pre_vat=None,
                      unit_price_vat=None, amount_text=None),
    ]
    return contracts


def test_each_legacy_contract_gets_one_item_and_keeps_its_stored_amounts(
        db_session, make_product, make_contract):
    from app.models.contract import Contract
    from app.models.seed import run_migrations

    contracts = _seed_three_legacy_contracts(make_product, make_contract)
    before = _money_snapshot(db_session)

    run_migrations(db_session)

    db_session.expire_all()
    for contract in contracts:
        reloaded = db_session.get(Contract, contract.id)
        assert len(reloaded.items) == 1, contract.id
        item = reloaded.items[0]
        assert item.amount_pre_vat + item.vat_amount == item.amount_total
        assert item.amount_total == (reloaded.total_amount or 0)
        assert reloaded.total_pre_vat + reloaded.total_vat == (reloaded.total_amount or 0)
        assert reloaded.pricing_model == "legacy"
        assert reloaded.doc_kind == "contract"

    assert _money_snapshot(db_session) == before


def test_rounding_of_pre_vat_matches_vat_inclusive_total(db_session, make_product, make_contract):
    from app.models.contract import Contract
    from app.models.seed import run_migrations

    contracts = _seed_three_legacy_contracts(make_product, make_contract)
    run_migrations(db_session)

    db_session.expire_all()
    lossy = db_session.get(Contract, contracts[1].id)
    assert lossy.items[0].amount_pre_vat == Decimal("15872724")
    assert lossy.items[0].vat_amount == Decimal("1269818")
    assert lossy.items[0].amount_total == Decimal("17142542")


def test_running_migration_twice_does_not_duplicate_items(db_session, make_product, make_contract):
    from app.models.contract_item import ContractItem
    from app.models.seed import run_migrations

    _seed_three_legacy_contracts(make_product, make_contract)
    run_migrations(db_session)
    run_migrations(db_session)

    assert db_session.query(ContractItem).count() == 3


def test_migration_writes_a_backup_before_touching_legacy_contracts(
        db_session, make_product, make_contract, data_root):
    from app.models.seed import run_migrations

    _seed_three_legacy_contracts(make_product, make_contract)
    run_migrations(db_session)

    backups = glob.glob(str(data_root / "backups" / "contract_manager-before-contract-items-*.db"))
    assert len(backups) == 1
    with closing(sqlite3.connect(backups[0])) as conn:
        count = conn.execute("SELECT COUNT(*) FROM contracts").fetchone()[0]
    assert count == 3


def test_deleting_a_contract_deletes_its_items(db_session, make_product, make_contract):
    from app.models.contract_item import ContractItem
    from app.models.seed import run_migrations
    from app.services.contract_service import delete_contract

    contracts = _seed_three_legacy_contracts(make_product, make_contract)
    run_migrations(db_session)
    doomed = contracts[0].id

    assert delete_contract(db_session, doomed) is True
    assert db_session.query(ContractItem).filter(ContractItem.contract_id == doomed).count() == 0
    assert db_session.query(ContractItem).count() == 2


def test_customer_record_count_column_exists_and_defaults_to_null(db_session, make_customer):
    c = make_customer()
    assert c.record_count is None


def test_verify_script_passes_on_migrated_copy_and_refuses_the_repo_data_dir(
        db_session, make_product, make_contract, data_root):
    import importlib.util
    from pathlib import Path

    from app.models.seed import run_migrations

    script = Path(__file__).resolve().parents[2] / "scripts" / "verify_items_migration.py"
    spec = importlib.util.spec_from_file_location("verify_items_migration", script)
    verify = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(verify)

    _seed_three_legacy_contracts(make_product, make_contract)
    run_migrations(db_session)

    snapshot = data_root / "snapshot.csv"
    assert verify.main([str(data_root), "--snapshot", str(snapshot)]) == 0
    assert snapshot.read_text(encoding="utf-8").startswith("id,contract_number,status,")
    assert verify.main([str(verify.REPO_DATA_DIR)]) == 2
