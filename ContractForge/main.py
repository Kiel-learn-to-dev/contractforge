from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.responses import RedirectResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates

from app.database import init_db, SessionLocal
from app.models import Customer, CustomerUnit  # ensure all tables registered
from app.paths import STATIC_DIR, TEMPLATES_DIR, ensure_runtime_dirs
from app.security import LocalOriginMiddleware
from app.version import __version__

# ── Phải import TRƯỚC khi bất kỳ router nào tạo Jinja2Templates ──────────
import app.templating_patcher  # noqa: F401 — auto-inject party_b_defaults vào mọi templates instance
# ─────────────────────────────────────────────────────────────────────────

from app.routers.customers  import router as customers_router
from app.routers.templates  import router as templates_router
from app.routers.contracts  import router as contracts_router
from app.routers.batch      import router as batch_router
from app.routers.dashboard  import router as dashboard_router
from app.routers.products   import router as products_router
from app.routers.settings   import router as settings_router
from app.routers.quotation  import router as quotation_router
from app.utils.safe_console import safe_print

ensure_runtime_dirs()


@asynccontextmanager
async def lifespan(_app: FastAPI):
    """Chuẩn bị thư mục runtime, schema, seed, và quét hết hạn khi khởi động.

    Dùng lifespan thay cho `@app.on_event("startup")` — FastAPI đã khai tử
    on_event và cảnh báo của nó làm hỏng bộ test (pyproject đặt filterwarnings
    = error).
    """
    ensure_runtime_dirs()
    init_db()
    db = SessionLocal()
    try:
        from app.models.seed import run_all_seeds, run_migrations
        run_migrations(db)
        run_all_seeds(db)
        from app.services.template_service import (
            seed_from_project_files, cleanup_orphan_template_files,
        )
        seed_from_project_files(db)
        cleanup_orphan_template_files(db)
        from app.services.dashboard_service import auto_update_contract_statuses
        counts = auto_update_contract_statuses(db)
        if any(counts.values()):
            safe_print(f"  [startup] Auto-status update: {counts}")
    finally:
        db.close()

    yield


app = FastAPI(title="Quản lý Hợp đồng", version=__version__, lifespan=lifespan)

# Ứng dụng một người dùng, không đăng nhập → ranh giới bảo mật là ranh giới máy.
# Launcher chỉ bind 127.0.0.1; middleware này chặn nốt đường còn lại: một trang
# web bất kỳ đang mở trên trình duyệt vẫn gửi được form POST tới localhost.
app.add_middleware(LocalOriginMiddleware)

app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")
templates = Jinja2Templates(directory=str(TEMPLATES_DIR))


app.include_router(dashboard_router)
app.include_router(customers_router)
app.include_router(templates_router)
app.include_router(contracts_router)
app.include_router(batch_router)
app.include_router(products_router)
app.include_router(settings_router)
app.include_router(quotation_router)


@app.get("/")
def root():
    from app.services.settings_service import is_organization_profile_complete
    db = SessionLocal()
    try:
        if not is_organization_profile_complete(db):
            return RedirectResponse(url="/settings?setup=1", status_code=302)
    finally:
        db.close()
    return RedirectResponse(url="/dashboard", status_code=302)


@app.get("/health")
def health():
    return {"status": "ok", "version": __version__}
