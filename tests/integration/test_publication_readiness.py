"""Checklist §12 của OPEN_SOURCE_DESKTOP_PLAN.md, chạy tự động.

Một checklist trong file Markdown chỉ đúng vào ngày có người ngồi tick nó. Những
mục kiểm được bằng máy thì nằm ở đây, để một commit vô ý làm lộ dữ liệu riêng
hay mở cổng ra LAN sẽ bị bắt ngay chứ không đợi tới lúc phát hành.

Các mục còn lại — ảnh chụp màn hình, giấy phép thư viện đi kèm — vẫn phải xem
tay và nằm trong tests/smoke/windows_release_checklist.md.
"""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
APP = ROOT / "ContractForge"


def _git(*args: str) -> str:
    result = subprocess.run(["git", "-C", str(ROOT), *args],
                            capture_output=True, text=True, encoding="utf-8")
    return result.stdout


@pytest.fixture(scope="module")
def tracked_files() -> list[str]:
    files = [p for p in _git("ls-files").splitlines() if p]
    if not files:
        pytest.skip("chưa khởi tạo Git")
    return files


@pytest.fixture(scope="module")
def history_paths() -> set[str]:
    paths = {p for p in _git("log", "--all", "--pretty=format:", "--name-only").splitlines() if p}
    if not paths:
        pytest.skip("chưa có commit nào")
    return paths


# ─── Không rò rỉ dữ liệu ─────────────────────────────────────────────────────

def test_data_directory_never_entered_history(history_paths):
    """Xoá khỏi cây làm việc là chưa đủ — Git nhớ mãi."""
    leaked = sorted(p for p in history_paths if p.startswith("data/"))
    assert not leaked, f"data/ đã lọt vào lịch sử: {leaked[:5]}"


@pytest.mark.parametrize("suffix", [
    ".db", ".sqlite", ".sqlite3", ".log", ".exe", ".zip", ".pdf", ".bak",
])
def test_no_runtime_artifact_is_tracked(tracked_files, suffix):
    offenders = [p for p in tracked_files if p.lower().endswith(suffix)]
    assert not offenders, f"file {suffix} được theo dõi: {offenders[:5]}"


@pytest.mark.parametrize("segment", [
    "uploads/", "outputs/", "signed_scans", "invoice_docs",
    "payment_slips", "customer_docs", "backups/",
])
def test_no_customer_document_is_tracked(tracked_files, segment):
    offenders = [p for p in tracked_files if segment in p]
    assert not offenders, f"chứng từ được theo dõi: {offenders[:5]}"


def test_only_fictional_docx_is_tracked(tracked_files):
    """Mẫu Word thật mang tiêu đề thư và tên đơn vị của cơ quan."""
    docx = [p for p in tracked_files if p.lower().endswith(".docx")]
    branded = [p for p in docx if not Path(p).name.startswith("sample_")]
    assert not branded, f"mẫu chưa trung tính được theo dõi: {branded}"


# ─── Bản cài đặt sạch không có dữ liệu ai ────────────────────────────────────

def test_seed_creates_no_customers():
    seed = (APP / "app" / "models" / "seed.py").read_text(encoding="utf-8")
    assert "Customer(" not in seed, "seed không được tạo sẵn khách hàng nào"


def test_organization_profile_starts_empty(db_engine, db_session):
    """Bản cài đặt mới phải hiện màn hình thiết lập, không điền sẵn đơn vị nào."""
    from app.models.seed import run_all_seeds
    from app.services.settings_service import is_organization_profile_complete

    run_all_seeds(db_session)

    assert is_organization_profile_complete(db_session) is False


def test_seeded_products_are_fictional(db_engine, db_session):
    from app.models.product import Product
    from app.models.seed import run_all_seeds

    run_all_seeds(db_session)

    products = db_session.query(Product).all()
    assert products, "bản cài đặt mới nên có sản phẩm mẫu để dùng thử"
    for product in products:
        assert not product.contract_number_format, (
            "sản phẩm mẫu không được mang khuôn số của tổ chức nào"
        )


# ─── Ranh giới cục bộ ────────────────────────────────────────────────────────

@pytest.mark.parametrize("relative", [
    "ContractForge.pyw",
    "ContractForge/chay_windows.bat",
    "ContractForge/chay_mac_linux.sh",
    "ContractForge/app/desktop.py",
    ".github/workflows/quality.yml",
])
def test_nothing_binds_to_all_interfaces(relative):
    """App không có đăng nhập — mở ra LAN là mở toàn bộ dữ liệu."""
    source = (ROOT / relative).read_text(encoding="utf-8", errors="ignore")
    assert "0.0.0.0" not in source, f"{relative} vẫn bind mọi interface"


def test_cross_origin_policy_is_installed():
    main_py = (APP / "main.py").read_text(encoding="utf-8")
    assert "LocalOriginMiddleware" in main_py


def test_no_template_pipes_data_through_safe():
    """`| safe` trong <script> là đường vào XSS — dùng `| tojson`."""
    offenders: list[str] = []
    for template in (APP / "templates").rglob("*.html"):
        for number, line in enumerate(template.read_text(encoding="utf-8").splitlines(), 1):
            stripped = line.strip()
            if stripped.startswith(("//", "{#", "<!--", "*")):
                continue                      # chú thích giải thích vì sao không dùng
            if "|safe" in stripped or "| safe" in stripped:
                offenders.append(f"{template.name}:{number}")
    assert not offenders, f"còn |safe: {offenders}"


# ─── Siêu dữ liệu phát hành ──────────────────────────────────────────────────

def test_dependencies_are_bounded():
    """Bản phát hành cần chặn trên, nếu không một bản nâng cấp lớn sẽ làm vỡ."""
    requirements = (APP / "requirements.txt").read_text(encoding="utf-8")
    assert "<" in requirements, "requirements.txt không có chặn trên nào"


@pytest.mark.parametrize("relative", [
    "LICENSE", "README.md", "CONTRIBUTING.md", "SECURITY.md", "CHANGELOG.md",
])
def test_publication_metadata_exists(relative):
    path = ROOT / relative
    assert path.is_file(), f"thiếu {relative}"
    assert path.read_text(encoding="utf-8").strip(), f"{relative} rỗng"


def test_public_content_scanner_passes():
    """Cổng cuối cùng — đúng bài kiểm tra mà CI chạy."""
    result = subprocess.run(
        ["python", str(ROOT / "scripts" / "check_public_repo.py"), "--root", str(ROOT)],
        capture_output=True, text=True, encoding="utf-8",
    )
    assert result.returncode == 0, result.stdout
