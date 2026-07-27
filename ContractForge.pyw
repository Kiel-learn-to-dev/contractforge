"""ContractForge launcher — lớp vỏ mỏng gọi app.desktop.

Bản trước tự lo mọi thứ: đi tìm một Python trên máy, chạy `pip install`, spawn
uvicorn thành tiến trình con, rồi mở trình duyệt. Bản đóng gói thành .exe không
có Python nào để tìm, và tiến trình con hay sống sót sau khi đóng app — giữ
khoá luôn file cơ sở dữ liệu.

Nay server chạy ngay trong tiến trình này (xem ContractForge/app/desktop.py),
trên một cổng trống do hệ điều hành cấp, và hiện trong cửa sổ WebView2 riêng.
Đóng cửa sổ là server dừng theo.

Chạy:
    pythonw ContractForge.pyw          # từ mã nguồn
    ContractForge.exe                  # bản đóng gói

Phụ thuộc cho cửa sổ riêng (không có thì tự mở trình duyệt hệ thống):
    pip install "contractforge[desktop]"
"""

from __future__ import annotations

import sys
import threading
import time
import traceback
from pathlib import Path

# ── Định vị mã nguồn ───────────────────────────────────────────────────────
if getattr(sys, "frozen", False):
    # PyInstaller: mã nguồn nằm trong thư mục tạm _MEIPASS
    LAUNCHER_DIR = Path(sys.executable).parent
    APP_DIR = Path(getattr(sys, "_MEIPASS", LAUNCHER_DIR)) / "ContractForge"
else:
    LAUNCHER_DIR = Path(__file__).resolve().parent
    APP_DIR = LAUNCHER_DIR / "ContractForge"

if str(APP_DIR) not in sys.path:
    sys.path.insert(0, str(APP_DIR))


# ── Luồng chuẩn ────────────────────────────────────────────────────────────
# Bản đóng gói dạng cửa sổ (console=False) có sys.stdout/stderr = None. Bất kỳ
# print() hay thư viện nào ghi ra đó sẽ ném AttributeError ở chỗ không ai ngờ.
# Trỏ chúng vào hố đen để mã dùng chung không cần biết mình đang chạy ở đâu.
if sys.stdout is None or sys.stderr is None:
    import io

    _null = io.StringIO()
    if sys.stdout is None:
        sys.stdout = _null
    if sys.stderr is None:
        sys.stderr = _null


# ── Ghi log ────────────────────────────────────────────────────────────────
# Log nằm dưới data root (không bao giờ ghi vào thư mục cài đặt, vốn có thể
# chỉ đọc). Nếu chưa dựng được data root thì tạm ghi cạnh launcher để không
# mất chẩn đoán lúc khởi động.
def _resolve_log_file() -> Path:
    try:
        import app.paths as paths
        paths.ensure_runtime_dirs()
        return paths.LOGS_DIR / "contractforge.log"
    except Exception:
        return LAUNCHER_DIR / "contractforge.log"


LOG_FILE = _resolve_log_file()


def log(msg: str) -> None:
    try:
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write(f"[{time.strftime('%Y-%m-%d %H:%M:%S')}] {msg}\n")
    except Exception:
        pass


# ── Khay hệ thống ──────────────────────────────────────────────────────────
def _icon_image():
    """Icon 64x64 cho khay hệ thống. None nếu Pillow không có."""
    try:
        from PIL import Image, ImageDraw, ImageFont
    except Exception:
        return None
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw.ellipse([2, 2, 62, 62], fill=(27, 46, 90))
    try:
        font = ImageFont.truetype("arial.ttf", 24)
    except Exception:
        font = ImageFont.load_default()
    draw.text((10, 17), "CF", fill=(255, 255, 255), font=font)
    return img


#: Đặt khi người dùng chọn "Thoát" ở khay, hoặc khi cửa sổ đóng.
_shutdown = threading.Event()


def _run_tray(base_url: str, on_quit) -> None:
    """Chạy icon khay ở thread nền. Im lặng bỏ qua nếu pystray không có."""
    try:
        import pystray
    except Exception:
        log("pystray không có — chạy không khay hệ thống")
        return
    image = _icon_image()
    if image is None:
        log("Pillow không có — chạy không khay hệ thống")
        return

    import webbrowser

    def action_open(icon, item):
        webbrowser.open(base_url)

    def action_quit(icon, item):
        icon.stop()
        _shutdown.set()
        on_quit()

    icon = pystray.Icon(
        "contractforge", image, "ContractForge — Quản lý hợp đồng",
        pystray.Menu(
            pystray.MenuItem("Mở trong trình duyệt", action_open, default=True),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem("Thoát", action_quit),
        ),
    )
    threading.Thread(target=icon.run, name="contractforge-tray", daemon=True).start()
    log("Tray icon running")


# ── Main ───────────────────────────────────────────────────────────────────
def main() -> int:
    log("=" * 50)
    log("ContractForge starting...")
    log(f"APP_DIR:  {APP_DIR}")

    try:
        from app.desktop import ServerThread, open_window, wait_for_health
        from app.paths import active_data_root, ensure_runtime_dirs
        from app.version import __version__
    except Exception:
        log("Không import được ứng dụng:\n" + traceback.format_exc())
        return 1

    log(f"Version:  {__version__}")
    ensure_runtime_dirs()
    log(f"DATA_DIR: {active_data_root()}")

    server = ServerThread()
    try:
        server.start()
    except Exception:
        log("Không khởi động được server:\n" + traceback.format_exc())
        return 1
    log(f"Server:   {server.base_url}")

    if not wait_for_health(server.base_url):
        log("Server không phản hồi /health — dừng lại.")
        server.stop()
        return 1
    log("Server ready")

    _run_tray(server.base_url, on_quit=_shutdown.set)

    try:
        mode = open_window(server.base_url)
        if mode == "browser":
            # Không có pywebview → giao diện nằm trong trình duyệt hệ thống và
            # open_window() trả về ngay. Phải giữ tiến trình sống, nếu không
            # server tắt ngay khi tab vừa mở. Thoát bằng menu khay hoặc Ctrl+C.
            log("Chạy chế độ trình duyệt — thoát bằng khay hệ thống hoặc Ctrl+C.")
            try:
                while not _shutdown.wait(timeout=1.0):
                    pass
            except KeyboardInterrupt:
                log("Nhận Ctrl+C")
        log(f"UI closed (mode={mode})")
    except Exception:
        log("Lỗi khi mở giao diện:\n" + traceback.format_exc())
        return 1
    finally:
        server.stop()
        log("Server stopped, exiting")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
