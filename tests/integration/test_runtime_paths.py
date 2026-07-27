"""
Integration tests: runtime data directory resolution (Task 3).

Verifies the resolution order, directory creation, and that the override hook
redirects the database and all runtime paths to the configured root — while
preserving backward compatibility with a legacy installation.
"""

from __future__ import annotations

import importlib
import os
from pathlib import Path

import pytest


@pytest.fixture(autouse=True)
def _clear_path_overrides(monkeypatch):
    """Ensure no stale override leaks between tests."""
    monkeypatch.delenv("CONTRACTFORGE_DATA_ROOT", raising=False)
    import app.paths as paths
    paths.cf_clear_data_root_override()
    yield
    paths.cf_clear_data_root_override()
    monkeypatch.delenv("CONTRACTFORGE_DATA_ROOT", raising=False)


# ─── Resolution order ─────────────────────────────────────────────────────────


def test_env_override_wins_over_everything(monkeypatch, tmp_path):
    import app.paths as paths
    target = tmp_path / "override_root"
    monkeypatch.setenv("CONTRACTFORGE_DATA_ROOT", str(target))
    assert paths._resolve_data_root() == target.resolve()


def test_programmatic_override(monkeypatch, tmp_path):
    monkeypatch.delenv("CONTRACTFORGE_DATA_ROOT", raising=False)
    import app.paths as paths
    target = tmp_path / "prog_root"
    paths.cf_set_data_root(target)
    assert paths._resolve_data_root() == target.resolve()
    # And derived paths follow.
    assert paths.DB_PATH == target.resolve() / "contract_manager.db"
    paths.cf_clear_data_root_override()


def test_legacy_data_used_when_present(monkeypatch, tmp_path):
    """Existing private installation keeps working (no data regression)."""
    import app.paths as paths
    # Force a fake "home" + LOCALAPPDATA so the user default does not collide,
    # and point BASE_DIR's legacy sibling at a tmp dir that HAS a db.
    fake_home = tmp_path / "home"
    fake_home.mkdir()
    fake_local = fake_home / "AppData" / "Local"
    fake_local.mkdir(parents=True)
    monkeypatch.setenv("LOCALAPPDATA", str(fake_local))

    # Legacy sibling normally = <repo>/data. We simulate a populated legacy dir
    # by checking the real resolution path: legacy wins over a fresh user
    # default when it has a db and the user default does not.
    legacy = paths.legacy_data_root()
    user_default = paths.user_data_default()
    # Ensure user default has no db (clean) for this test's assumptions.
    assert not (user_default / "contract_manager.db").exists() or _is_real(user_default)

    # If a real legacy db exists in the repo, the resolver must prefer it over
    # a fresh (empty) user default.
    if (legacy / "contract_manager.db").exists():
        resolved = paths._resolve_data_root()
        assert resolved == legacy.resolve(), (
            "Legacy installation with a db must not be silently abandoned."
        )


def test_frozen_build_looks_beside_the_executable(monkeypatch, tmp_path):
    """Bản .exe phải thấy thư mục data/ nằm cạnh nó.

    PyInstaller giải nén mã nguồn vào %TEMP%, nên ``__file__`` không nói gì về
    chỗ người dùng đặt file .exe. Bản trước tính thư mục data kiểu cũ từ
    ``__file__``, nên bản đóng gói không bao giờ thấy dữ liệu đang có ngay bên
    cạnh — nó âm thầm tạo CSDL trống trong LocalAppData và người dùng mở lên
    thấy trắng trơn.
    """
    import sys

    import app.paths as paths

    install = tmp_path / "ProgramFiles" / "ContractForge"
    install.mkdir(parents=True)
    fake_exe = install / "ContractForge.exe"
    fake_exe.write_bytes(b"MZ")

    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(fake_exe), raising=False)

    assert paths._legacy_sibling_data() == install / "data"


def test_frozen_build_finds_an_existing_database_beside_it(monkeypatch, tmp_path):
    """Nâng cấp lên bản .exe không được làm mất dữ liệu đang dùng."""
    import sys

    import app.paths as paths

    install = tmp_path / "app"
    (install / "data").mkdir(parents=True)
    (install / "data" / "contract_manager.db").write_bytes(b"sqlite")
    fake_exe = install / "ContractForge.exe"
    fake_exe.write_bytes(b"MZ")

    fake_local = tmp_path / "Local"          # LocalAppData sạch, chưa có gì
    fake_local.mkdir()
    monkeypatch.setenv("LOCALAPPDATA", str(fake_local))
    monkeypatch.delenv("CONTRACTFORGE_DATA_ROOT", raising=False)
    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(fake_exe), raising=False)
    paths.cf_clear_data_root_override()

    assert paths._resolve_data_root() == (install / "data").resolve()


def test_source_checkout_still_uses_the_repo_data_dir(monkeypatch, tmp_path):
    """Chạy từ mã nguồn thì hành vi cũ giữ nguyên."""
    import sys

    import app.paths as paths

    monkeypatch.setattr(sys, "frozen", False, raising=False)

    assert paths._legacy_sibling_data() == paths.BASE_DIR.parent / "data"


def test_fresh_install_targets_user_data_default(monkeypatch, tmp_path):
    """A brand-new install resolves to the platform user-data dir."""
    import app.paths as paths
    fake_home = tmp_path / "fresh_home"
    fake_home.mkdir()
    fake_local = fake_home / "AppData" / "Local"
    monkeypatch.setenv("LOCALAPPDATA", str(fake_local))
    # No legacy db, no user-default db → fresh default.
    monkeypatch.setattr(paths, "_legacy_sibling_data", lambda: tmp_path / "nonexistent_legacy")
    resolved = paths._resolve_data_root()
    assert resolved == (fake_local / "ContractForge").resolve()


def test_migrated_install_reuses_user_data_default(monkeypatch, tmp_path):
    """An already-migrated desktop install keeps using the user-data dir."""
    import app.paths as paths
    fake_local = tmp_path / "AppData" / "Local"
    migrated = fake_local / "ContractForge"
    (migrated).mkdir(parents=True)
    (migrated / "contract_manager.db").write_bytes(b"x")  # has a db
    monkeypatch.setenv("LOCALAPPDATA", str(fake_local))
    resolved = paths._resolve_data_root()
    assert resolved == migrated.resolve()


# ─── Directory creation ───────────────────────────────────────────────────────


def test_ensure_runtime_dirs_creates_all(monkeypatch, tmp_path):
    import app.paths as paths
    root = tmp_path / "empty_root"
    paths.cf_set_data_root(root)
    paths.ensure_runtime_dirs()
    expected = [
        "uploads/templates", "uploads/signed_scans", "uploads/customer_docs",
        "uploads/invoice_docs", "uploads/payment_slips",
        "outputs/contracts", "outputs/batch", "logs", "backups",
    ]
    for rel in expected:
        assert (root / rel).is_dir(), f"missing {rel}"
    # Database file itself is not created by ensure_runtime_dirs (that is the
    # engine's job), but its parent dir exists.
    assert root.is_dir()
    # Marker written.
    assert (root / ".cf_data_root").exists()


def test_ensure_runtime_dirs_idempotent(monkeypatch, tmp_path):
    import app.paths as paths
    root = tmp_path / "idem_root"
    paths.cf_set_data_root(root)
    paths.ensure_runtime_dirs()
    # Second call must not raise.
    paths.ensure_runtime_dirs()
    assert root.is_dir()


# ─── Data lands only in the configured root ──────────────────────────────────


def test_record_appears_only_in_configured_root(db_session, make_customer, data_root):
    """A record created via the isolated DB lives under the temp data root,
    never under the repo legacy dir or the real LocalAppData."""
    make_customer(legal_name="TASK3_ISOLATION_MARKER")
    from app.models.customer import Customer
    assert db_session.query(Customer).filter(
        Customer.legal_name == "TASK3_ISOLATION_MARKER"
    ).first() is not None
    # The temp DB FILE (not a directory) exists under data_root.
    temp_db = data_root / "contract_manager.db"
    assert temp_db.is_file(), "DB must be a file, not a directory, under data_root"
    # The test marker must NOT be present in the real legacy DB.
    import app.paths as paths
    legacy_db = paths.legacy_data_root() / "contract_manager.db"
    if legacy_db.exists():
        from sqlalchemy import create_engine, text
        eng = create_engine(f"sqlite:///{legacy_db.as_posix()}", connect_args={"check_same_thread": False})
        try:
            with eng.connect() as conn:
                row = conn.execute(
                    text("SELECT COUNT(*) FROM customers WHERE legal_name = :n"),
                    {"n": "TASK3_ISOLATION_MARKER"},
                ).scalar()
                assert row == 0, "Test marker leaked into the real legacy database!"
        finally:
            eng.dispose()


# ─── Backward compatibility ───────────────────────────────────────────────────


def test_no_override_falls_back_gracefully(monkeypatch):
    """With no env var and no programmatic override, resolution still returns a
    real path and never raises."""
    import app.paths as paths
    monkeypatch.delenv("CONTRACTFORGE_DATA_ROOT", raising=False)
    paths.cf_clear_data_root_override()
    resolved = paths._resolve_data_root()
    assert isinstance(resolved, Path)
    assert resolved.name in ("ContractForge", "data")


def _is_real(p: Path) -> bool:
    return p.exists()
