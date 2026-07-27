"""
paths.py — runtime data directory resolution for ContractForge.

Single source of truth for where the application stores its database, uploads,
generated outputs, and logs. Keeps CODE and DATA strictly separated
(OPEN_SOURCE_DESKTOP_PLAN.md AD-4).

Resolution order for the data root
----------------------------------
1. **Explicit override** — ``CONTRACTFORGE_DATA_ROOT`` env var, or the
   ``cf_set_data_root(...)`` helper. Used by tests, the migration tooling, and
   power users. Highest priority.
2. **Existing user-data dir** — ``%LOCALAPPDATA%\\ContractForge`` if it already
   contains a database. Honors a migrated desktop installation.
3. **Legacy sibling ``data/``** — ``<repo>/data`` if it already contains a
   database. Honors an existing private installation that has not been migrated
   yet (Checkpoint A guarantee: no data regression on upgrade).
4. **Fresh default** — ``%LOCALAPPDATA%\\ContractForge`` for a brand-new
   desktop install or first run from source.

This order means:
  * A brand-new user gets isolated LocalAppData data (never writes to the
    installation/source directory).
  * An existing user who upgrades keeps working off their current data until
    they explicitly run the migration (Task 4) — and even then the legacy copy
    is never deleted automatically.
  * Tests override via the env var and never touch either real location.

On non-Windows platforms the default falls back to ``~/.local/share/ContractForge``
(XDG-style), keeping the source portable (plan §3 allows macOS/Linux packaging
later; the source remains portable now).
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent

# Marker file written into a data root once it has been initialized as the
# active runtime location. Lets us distinguish "this is a real ContractForge
# data dir" from "an empty directory that happens to exist".
_DATA_MARKER = ".cf_data_root"

# Override hook (highest priority). May be set by tests, migration tooling, or
# a power user. When set, ALL other resolution steps are skipped.
_DATA_ROOT_OVERRIDE: Path | None = None


def _user_data_default() -> Path:
    """Platform-specific default application-data directory.

    Windows: %LOCALAPPDATA%\\ContractForge  (per plan AD-4)
    macOS:   ~/Library/Application Support/ContractForge
    Linux:   ~/.local/share/ContractForge   (XDG_DATA_HOME if set)
    """
    if sys.platform == "win32":
        base = os.environ.get("LOCALAPPDATA")
        if base:
            return Path(base) / "ContractForge"
        # Fallback if LOCALAPPDATA is somehow missing.
        return Path.home() / "AppData" / "Local" / "ContractForge"
    if sys.platform == "darwin":
        return Path.home() / "Library" / "Application Support" / "ContractForge"
    # POSIX / Linux
    xdg = os.environ.get("XDG_DATA_HOME")
    if xdg:
        return Path(xdg) / "ContractForge"
    return Path.home() / ".local" / "share" / "ContractForge"


def _legacy_sibling_data() -> Path:
    """The old source-relative data location: <repo>/data."""
    return BASE_DIR.parent / "data"


def _has_active_db(root: Path) -> bool:
    """True if ``root`` looks like a real ContractForge data dir (has a DB)."""
    return (root / "contract_manager.db").exists()


def cf_set_data_root(path: str | os.PathLike | None) -> None:
    """Programmatically set (or clear, if None) the data-root override.

    Intended for the launcher and migration tooling. Tests use the env var
    instead so resolution happens at import time before the app is loaded.
    """
    global _DATA_ROOT_OVERRIDE
    _DATA_ROOT_OVERRIDE = Path(path).resolve() if path else None
    _refresh_derived_paths()


def cf_clear_data_root_override() -> None:
    """Clear the programmatic override (env var still wins if set)."""
    cf_set_data_root(None)


def _resolve_data_root() -> Path:
    """Return the runtime data root, applying the resolution order above."""
    # 1. Explicit override (env var first, then programmatic).
    env_override = os.environ.get("CONTRACTFORGE_DATA_ROOT")
    if env_override:
        return Path(env_override).resolve()
    if _DATA_ROOT_OVERRIDE is not None:
        return _DATA_ROOT_OVERRIDE

    user_default = _user_data_default()
    legacy = _legacy_sibling_data()

    # 2. Already-migrated desktop install.
    if _has_active_db(user_default):
        return user_default
    # 3. Existing legacy installation — keep working, do not regress.
    if _has_active_db(legacy):
        return legacy
    # 4. Fresh install → isolated user-data dir.
    return user_default


# ── Resolved paths (module-level constants, for backward compatibility) ───────
# These are computed once at import. Code that captured them at import time
# (e.g. services that store ``OUTPUT_DIR = str(OUTPUT_CONTRACTS_DIR)``) keeps
# working. The launcher / migration tooling calls ``cf_set_data_root`` and then
# ``_refresh_derived_paths`` to update these in place when redirecting at runtime.
DATA_DIR = _resolve_data_root()

TEMPLATES_DIR = BASE_DIR / "templates"
STATIC_DIR = BASE_DIR / "static"
ASSETS_DIR = BASE_DIR / "assets"

def resolve_form_08a_template(assets_dir: Path) -> Path:
    """Chọn mẫu biểu 08a: bản riêng của cơ quan nếu có, không thì bản mẫu.

    Biểu 08a được render on-the-fly chứ không phải mẫu người dùng tải lên, nên
    phải luôn có một file. Repo loại trừ mọi .docx trong assets vì chúng mang
    tiêu đề thư thật; bản mẫu hư cấu `sample_form_08a.docx` được commit để bản
    clone sạch vẫn dùng được chức năng này.

    Bản riêng luôn thắng — nâng cấp không được đổi giấy tờ đang phát hành.
    """
    private = assets_dir / "default_templates" / "template_08A.docx"
    if private.exists():
        return private
    return assets_dir / "default_templates" / "sample_form_08a.docx"


# Acceptance-form template (rendered on demand, not a user-managed template).
FORM_08A_TEMPLATE = resolve_form_08a_template(ASSETS_DIR)

# The SQLite database filename inside the data root (single source of truth).
DB_FILENAME = "contract_manager.db"

UPLOADS_DIR = DATA_DIR / "uploads"
UPLOAD_TEMPLATES_DIR = DATA_DIR / "uploads" / "templates"
SIGNED_SCANS_DIR = DATA_DIR / "uploads" / "signed_scans"     # PDF scan hợp đồng đã ký
CUSTOMER_DOCS_DIR = DATA_DIR / "uploads" / "customer_docs"  # Hồ sơ giấy tờ khách hàng
INVOICE_DOCS_DIR = DATA_DIR / "uploads" / "invoice_docs"    # Hóa đơn PDF
PAYMENT_SLIPS_DIR = DATA_DIR / "uploads" / "payment_slips"  # Ủy nhiệm chi (PDF/ảnh)
OUTPUTS_DIR = DATA_DIR / "outputs"
OUTPUT_CONTRACTS_DIR = DATA_DIR / "outputs" / "contracts"
OUTPUT_BATCH_DIR = DATA_DIR / "outputs" / "batch"
LOGS_DIR = DATA_DIR / "logs"
BACKUPS_DIR = DATA_DIR / "backups"
DB_PATH = DATA_DIR / DB_FILENAME

# The complete set of runtime directories that must exist for the app to run.
_RUNTIME_DIRS: tuple[Path, ...] = (
    DATA_DIR,
    UPLOADS_DIR,
    UPLOAD_TEMPLATES_DIR,
    SIGNED_SCANS_DIR,
    CUSTOMER_DOCS_DIR,
    INVOICE_DOCS_DIR,
    PAYMENT_SLIPS_DIR,
    OUTPUTS_DIR,
    OUTPUT_CONTRACTS_DIR,
    OUTPUT_BATCH_DIR,
    LOGS_DIR,
    BACKUPS_DIR,
)


def _refresh_derived_paths() -> None:
    """Recompute DATA_DIR and every derived path in place.

    Called after ``cf_set_data_root`` redirects the data root at runtime (e.g.
    the launcher pointing at LocalAppData). Module-level constants are mutated
    so already-imported services that captured them by reference stay in sync.
    """
    global DATA_DIR
    DATA_DIR = _resolve_data_root()

    # Mutate module attributes in place. We reassign the names so that code
    # reading ``app.paths.UPLOAD_TEMPLATES_DIR`` at call-time sees the new value.
    g = globals()
    g["DATA_DIR"] = DATA_DIR
    new_paths = {
        "UPLOADS_DIR": DATA_DIR / "uploads",
        "UPLOAD_TEMPLATES_DIR": DATA_DIR / "uploads" / "templates",
        "SIGNED_SCANS_DIR": DATA_DIR / "uploads" / "signed_scans",
        "CUSTOMER_DOCS_DIR": DATA_DIR / "uploads" / "customer_docs",
        "INVOICE_DOCS_DIR": DATA_DIR / "uploads" / "invoice_docs",
        "PAYMENT_SLIPS_DIR": DATA_DIR / "uploads" / "payment_slips",
        "OUTPUTS_DIR": DATA_DIR / "outputs",
        "OUTPUT_CONTRACTS_DIR": DATA_DIR / "outputs" / "contracts",
        "OUTPUT_BATCH_DIR": DATA_DIR / "outputs" / "batch",
        "LOGS_DIR": DATA_DIR / "logs",
        "BACKUPS_DIR": DATA_DIR / "backups",
        "DB_PATH": DATA_DIR / DB_FILENAME,
    }
    g.update(new_paths)
    # Refresh the runtime-DIRECTORIES tuple. This must contain only directories,
    # never the DB file (DB_PATH) — ensure_runtime_dirs() mkdir's every entry,
    # and mkdir'ing the DB path would create it as a directory and break the
    # engine. Keep this list in sync with the module-level _RUNTIME_DIRS.
    g["_RUNTIME_DIRS"] = (
        DATA_DIR,
        new_paths["UPLOADS_DIR"],
        new_paths["UPLOAD_TEMPLATES_DIR"],
        new_paths["SIGNED_SCANS_DIR"],
        new_paths["CUSTOMER_DOCS_DIR"],
        new_paths["INVOICE_DOCS_DIR"],
        new_paths["PAYMENT_SLIPS_DIR"],
        new_paths["OUTPUTS_DIR"],
        new_paths["OUTPUT_CONTRACTS_DIR"],
        new_paths["OUTPUT_BATCH_DIR"],
        new_paths["LOGS_DIR"],
        new_paths["BACKUPS_DIR"],
    )


def ensure_runtime_dirs() -> None:
    """Create every runtime directory that does not yet exist.

    Idempotent and safe to call repeatedly. Writes the data-root marker so a
    freshly-created root is recognizable as an active ContractForge data dir.
    """
    for p in _RUNTIME_DIRS:
        p.mkdir(parents=True, exist_ok=True)
    # Mark this directory as a ContractForge data root (best-effort).
    try:
        marker = DATA_DIR / _DATA_MARKER
        if not marker.exists():
            marker.write_text("ContractForge runtime data root\n", encoding="utf-8")
    except OSError:
        pass


def active_data_root() -> Path:
    """Return the currently-resolved data root (read-only convenience)."""
    return _resolve_data_root()


def legacy_data_root() -> Path:
    """Return the legacy source-relative data root (for migration detection)."""
    return _legacy_sibling_data()


def user_data_default() -> Path:
    """Return the platform default user-data root (for migration target)."""
    return _user_data_default()
