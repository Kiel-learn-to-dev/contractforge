"""
ContractForge Launcher
======================
Double-click để khởi động ContractForge.
- Lần đầu: tự cài dependencies, tạo thư mục data/
- Lần sau: mở browser trực tiếp (nếu server đang chạy)
- System tray: icon góc phải taskbar, menu Mở / Dừng / Thoát
- Không có cửa sổ console (chạy ngầm hoàn toàn)

Đặt file này ở thư mục ContractForge/ (cùng cấp với ContractForge/ code)
"""

import os
import sys
import time
import json
import subprocess
import threading
import webbrowser
import urllib.request
import urllib.error
from pathlib import Path

# ── Paths ──────────────────────────────────────────────────────────────────
# Runtime data location is resolved by the app itself (app.paths), which by
# default writes to %LOCALAPPDATA%\ContractForge so the installation/source
# directory stays read-only. See OPEN_SOURCE_DESKTOP_PLAN.md AD-4 & Task 3.
# We import it lazily once APP_DIR is known (below) so the resolution runs at
# launcher start, not at module import.
if getattr(sys, 'frozen', False):
    # Đang chạy từ .exe (PyInstaller)
    LAUNCHER_DIR = Path(sys.executable).parent
else:
    # Đang chạy từ .pyw
    LAUNCHER_DIR = Path(__file__).resolve().parent

APP_DIR  = LAUNCHER_DIR / "ContractForge"   # thư mục code
MAIN_PY  = APP_DIR / "main.py"
REQ_FILE = APP_DIR / "requirements.txt"

# Resolve the runtime data root via the app's own logic (defaults to
# LocalAppData for a fresh install, or the existing legacy data/ if present).
# We fall back to a sibling data/ dir only if the app module cannot be imported.
def _resolve_app_data_dir():
    try:
        sys.path.insert(0, str(APP_DIR))
        import app.paths as _p
        _p.ensure_runtime_dirs()
        return _p.active_data_root()
    except Exception:
        return LAUNCHER_DIR / "data"

DATA_DIR = _resolve_app_data_dir()

HOST = "127.0.0.1"
PORT = 8888
URL  = f"http://{HOST}:{PORT}"

# ── Globals ────────────────────────────────────────────────────────────────
_server_proc = None
_tray_icon   = None


# ── Logging (file, không console) ─────────────────────────────────────────
# Logs live under the resolved data root's logs/ dir (never in the read-only
# installation directory). Falls back to the launcher dir only if the data root
# cannot be created (e.g. permission error) so we never lose startup diagnostics.
def _resolve_log_file():
    log_dir = DATA_DIR / "logs"
    try:
        log_dir.mkdir(parents=True, exist_ok=True)
        return log_dir / "contractforge.log"
    except Exception:
        return LAUNCHER_DIR / "contractforge.log"

LOG_FILE = _resolve_log_file()

def log(msg):
    try:
        ts = time.strftime("%H:%M:%S")
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write(f"[{ts}] {msg}\n")
    except Exception:
        pass


# ── Environment check & setup ─────────────────────────────────────────────
def find_python():
    """Tìm Python phù hợp — ưu tiên Python trong cùng thư mục (portable)."""
    candidates = [
        LAUNCHER_DIR / "python" / "python.exe",     # portable python kèm theo
        LAUNCHER_DIR / "python" / "pythonw.exe",
    ]
    # Fallback: python trong PATH
    for exe in ["python", "python3", "py"]:
        candidates.append(exe)
    for c in candidates:
        try:
            r = subprocess.run(
                [str(c), "--version"],
                capture_output=True, text=True,
                creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
            )
            if r.returncode == 0 and "Python 3" in r.stdout + r.stderr:
                log(f"Python found: {c}")
                return str(c)
        except Exception:
            continue
    return None


def ensure_dependencies(python_exe):
    """Cài requirements.txt nếu chưa có."""
    if not REQ_FILE.exists():
        log("requirements.txt not found, skip")
        return True
    log("Checking/installing dependencies...")
    try:
        flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
        r = subprocess.run(
            [python_exe, "-m", "pip", "install", "-r", str(REQ_FILE), "--quiet"],
            capture_output=True, text=True, creationflags=flags,
            cwd=str(APP_DIR)
        )
        if r.returncode != 0:
            log(f"pip error: {r.stderr[:300]}")
            return False
        log("Dependencies OK")
        return True
    except Exception as e:
        log(f"pip exception: {e}")
        return False


def ensure_data_dirs():
    """Tạo toàn bộ thư mục runtime data (delegate cho app.paths).

    Đã được gọi một lần trong _resolve_app_data_dir() khi import app.paths, nên
    hàm này chủ yếu để đảm bảo idempotent và ghi log. Runtime data mặc định nằm
    ở %LOCALAPPDATA%\\ContractForge (xem app.paths), không còn ở thư mục cài đặt.
    """
    try:
        sys.path.insert(0, str(APP_DIR))
        import app.paths as _p
        _p.ensure_runtime_dirs()
    except Exception:
        # Fallback: tạo tối thiểu các thư mục con thiết yếu.
        for sub in ["outputs/contracts", "outputs/batch", "uploads/templates"]:
            (DATA_DIR / sub).mkdir(parents=True, exist_ok=True)
    log(f"Data dir ready: {DATA_DIR}")


# ── Server management ──────────────────────────────────────────────────────
def is_server_running():
    try:
        r = urllib.request.urlopen(f"{URL}/health", timeout=2)
        return r.status == 200
    except Exception:
        return False


def start_server(python_exe):
    global _server_proc
    if is_server_running():
        log("Server already running")
        return True

    log(f"Starting server: {python_exe} -m uvicorn main:app ...")
    flags = subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0
    try:
        _server_proc = subprocess.Popen(
            [python_exe, "-m", "uvicorn", "main:app",
             "--host", HOST, "--port", str(PORT)],
            cwd=str(APP_DIR),
            creationflags=flags,
            stdout=open(LAUNCHER_DIR / "server.log", "w", encoding="utf-8"),
            stderr=subprocess.STDOUT,
        )
    except Exception as e:
        log(f"Failed to start server: {e}")
        return False

    # Chờ server sẵn sàng (tối đa 30s)
    for i in range(30):
        time.sleep(1)
        if is_server_running():
            log(f"Server ready after {i+1}s")
            return True
        if _server_proc.poll() is not None:
            log("Server process died early")
            return False
    log("Server start timeout")
    return False


def stop_server():
    global _server_proc
    if _server_proc and _server_proc.poll() is None:
        log("Stopping server...")
        _server_proc.terminate()
        try:
            _server_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            _server_proc.kill()
        _server_proc = None
        log("Server stopped")


def open_browser():
    time.sleep(0.5)
    webbrowser.open(URL)


# ── System tray icon ───────────────────────────────────────────────────────
def _make_icon_image():
    """Tạo icon PNG 64x64 — logo CF màu xanh."""
    try:
        from PIL import Image, ImageDraw, ImageFont
        img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
        draw = ImageDraw.Draw(img)
        # Circle background
        draw.ellipse([2, 2, 62, 62], fill=(27, 46, 90))
        # "CF" text
        try:
            font = ImageFont.truetype("arial.ttf", 24)
        except Exception:
            font = ImageFont.load_default()
        draw.text((10, 17), "CF", fill=(255, 255, 255), font=font)
        return img
    except Exception:
        # Minimal fallback
        try:
            from PIL import Image
            img = Image.new("RGB", (16, 16), (27, 46, 90))
            return img
        except Exception:
            return None


def run_tray(python_exe):
    """Chạy system tray icon với menu."""
    try:
        import pystray
        from PIL import Image
    except Exception:
        log("pystray/PIL not available, running without tray")
        # Chỉ mở browser, không có tray
        open_browser()
        return

    icon_img = _make_icon_image()
    if icon_img is None:
        log("Cannot create icon")
        return

    def action_open(icon, item):
        if is_server_running():
            webbrowser.open(URL)
        else:
            threading.Thread(target=lambda: (start_server(python_exe), open_browser()), daemon=True).start()

    def action_stop(icon, item):
        stop_server()

    def action_restart(icon, item):
        stop_server()
        time.sleep(1)
        threading.Thread(target=lambda: (start_server(python_exe), open_browser()), daemon=True).start()

    def action_quit(icon, item):
        stop_server()
        icon.stop()

    def server_status(item):
        return "✅ Server đang chạy" if is_server_running() else "⛔ Server đã dừng"

    menu = pystray.Menu(
        pystray.MenuItem("ContractForge v43", None, enabled=False),
        pystray.Menu.SEPARATOR,
        pystray.MenuItem("🌐 Mở trình duyệt", action_open, default=True),
        pystray.MenuItem(server_status, None, enabled=False),
        pystray.Menu.SEPARATOR,
        pystray.MenuItem("🔄 Khởi động lại server", action_restart),
        pystray.MenuItem("⏹ Dừng server",           action_stop),
        pystray.Menu.SEPARATOR,
        pystray.MenuItem("✖ Thoát",                 action_quit),
    )

    global _tray_icon
    _tray_icon = pystray.Icon(
        "contractforge",
        icon_img,
        "ContractForge — Quản lý hợp đồng",
        menu,
    )
    log("Tray icon running")
    _tray_icon.run()


# ── Main ───────────────────────────────────────────────────────────────────
def main():
    log("=" * 50)
    log("ContractForge Launcher starting...")
    log(f"LAUNCHER_DIR: {LAUNCHER_DIR}")
    log(f"APP_DIR:      {APP_DIR}")
    log(f"DATA_DIR:     {DATA_DIR}")

    # Kiểm tra thư mục app
    if not MAIN_PY.exists():
        _show_error(
            "Không tìm thấy ContractForge/main.py\n\n"
            "Hãy đảm bảo cấu trúc thư mục:\n"
            "  ContractForge/\n"
            "    ContractForge.exe  ← file này\n"
            "    ContractForge/     ← thư mục code\n"
            "      main.py\n"
            "    data/              ← thư mục data\n"
        )
        return

    # Tìm Python
    python_exe = find_python()
    if not python_exe:
        _show_error(
            "Không tìm thấy Python 3!\n\n"
            "Cài Python tại: https://python.org/downloads\n"
            "Nhớ tick ✅ 'Add Python to PATH'"
        )
        return

    # Tạo thư mục data
    ensure_data_dirs()

    # Cài dependencies (chạy ngầm)
    deps_ok = ensure_dependencies(python_exe)
    if not deps_ok:
        log("WARNING: dependencies may be incomplete")

    # Nếu server đang chạy rồi → chỉ mở browser
    if is_server_running():
        log("Server already up, opening browser")
        open_browser()
    else:
        # Khởi động server trong thread riêng
        def boot():
            ok = start_server(python_exe)
            if ok:
                open_browser()
            else:
                _show_error(
                    "Không khởi động được server!\n\n"
                    "Xem file contractforge.log để biết chi tiết."
                )
        threading.Thread(target=boot, daemon=True).start()

    # Chạy system tray (blocking)
    run_tray(python_exe)


def _show_error(msg):
    """Hiển thị lỗi mà không cần console."""
    log(f"ERROR: {msg}")
    try:
        import tkinter as tk
        from tkinter import messagebox
        root = tk.Tk()
        root.withdraw()
        messagebox.showerror("ContractForge — Lỗi khởi động", msg)
        root.destroy()
    except Exception:
        # tkinter không khả dụng — ghi log là đủ
        pass


if __name__ == "__main__":
    main()
