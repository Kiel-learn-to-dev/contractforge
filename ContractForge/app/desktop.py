"""desktop.py — vỏ ứng dụng desktop cho ContractForge.

Chạy server FastAPI trong một thread nền rồi mở nó trong cửa sổ WebView2 riêng,
để người dùng thấy một ứng dụng desktop chứ không phải một tab trình duyệt
(OPEN_SOURCE_DESKTOP_PLAN.md Task 12).

Vì sao chạy thread thay vì tiến trình con
------------------------------------------
Bản launcher cũ đi tìm một Python trên máy rồi ``subprocess`` gọi uvicorn. Bản
đóng gói thành .exe không có Python nào để tìm cả. Chạy uvicorn ngay trong tiến
trình này bỏ được cả bước dò tìm lẫn bước cài phụ thuộc, và tắt sạch được server
khi đóng cửa sổ — không còn tiến trình mồ côi giữ khoá file cơ sở dữ liệu.

Vì sao cổng động
----------------
Cổng cố định 8000/8888 đụng nhau khi máy đã có thứ khác chiếm, hoặc khi người
dùng mở hai bản. Ta xin hệ điều hành một cổng trống rồi mới khởi động.

Suy biến êm
-----------
``pywebview`` là phụ thuộc tuỳ chọn (extra ``desktop``). Không có nó thì mở bằng
trình duyệt hệ thống — vẫn dùng được đầy đủ, chỉ là không có cửa sổ riêng.
"""

from __future__ import annotations

import logging
import socket
import threading
import time
import urllib.error
import urllib.request
import webbrowser
from typing import Callable, Optional

LOOPBACK = "127.0.0.1"
WINDOW_TITLE = "ContractForge — Quản lý Hợp đồng"
WINDOW_MIN_SIZE = (1024, 700)
HEALTH_TIMEOUT_SECONDS = 30.0

log = logging.getLogger("contractforge.desktop")


# ─── Cổng ────────────────────────────────────────────────────────────────────

def find_free_port(host: str = LOOPBACK) -> int:
    """Xin hệ điều hành một cổng TCP đang trống.

    Có khoảng trống nhỏ giữa lúc đóng socket này và lúc uvicorn bind — không
    tránh được, nhưng trên một máy cá nhân thì gần như không bao giờ chạm phải,
    và đây vẫn tốt hơn nhiều so với cổng cố định chắc chắn có ngày đụng.
    """
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind((host, 0))
        return sock.getsockname()[1]


def is_port_free(port: int, host: str = LOOPBACK) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        try:
            sock.bind((host, port))
            return True
        except OSError:
            return False


# ─── Vòng đời server ─────────────────────────────────────────────────────────

class ServerThread:
    """Chạy uvicorn trong thread nền, dừng được một cách sạch sẽ."""

    def __init__(self, app_import_string: str = "main:app",
                 host: str = LOOPBACK, port: Optional[int] = None):
        self.host = host
        self.port = port or find_free_port(host)
        self.app_import_string = app_import_string
        self._server = None
        self._thread: Optional[threading.Thread] = None

    @property
    def base_url(self) -> str:
        return f"http://{self.host}:{self.port}"

    def start(self) -> None:
        import uvicorn

        config = uvicorn.Config(
            self.app_import_string,
            host=self.host,
            port=self.port,
            log_level="warning",
            access_log=False,
            # ContractForge không có endpoint WebSocket nào. Mặc định uvicorn
            # vẫn nạp lớp WebSocket, kéo theo `websockets.legacy` — thư viện đã
            # bị khai tử và phát cảnh báo ngay lúc import. Tắt đi: khởi động
            # nhanh hơn, và không phụ thuộc vào một module đang chờ bị gỡ bỏ.
            ws="none",
            # log_config=None là BẮT BUỘC cho bản đóng gói dạng cửa sổ.
            # PyInstaller build với console=False đặt sys.stdout = None; bộ định
            # dạng log mặc định của uvicorn gọi sys.stdout.isatty() ngay trong
            # __init__ → AttributeError → server không khởi động nổi. Vỏ desktop
            # tự ghi log ra file, nên cấu hình log console của uvicorn vừa thừa
            # vừa nguy hiểm.
            log_config=None,
        )
        self._server = uvicorn.Server(config)
        # daemon=True: nếu có gì đó chặn đường tắt sạch, thread này không giữ
        # cho tiến trình sống mãi.
        self._thread = threading.Thread(target=self._server.run,
                                        name="contractforge-server", daemon=True)
        self._thread.start()
        log.info("Server thread started on %s", self.base_url)

    def stop(self, timeout: float = 5.0) -> None:
        if self._server is not None:
            self._server.should_exit = True
        if self._thread is not None:
            self._thread.join(timeout=timeout)
        log.info("Server stopped")

    def is_alive(self) -> bool:
        return self._thread is not None and self._thread.is_alive()


def wait_for_health(base_url: str, timeout: float = HEALTH_TIMEOUT_SECONDS,
                    interval: float = 0.25,
                    sleep: Callable[[float], None] = time.sleep) -> bool:
    """Chờ tới khi /health trả 200. False nếu quá hạn.

    Cửa sổ chỉ được mở sau khi hàm này trả True — mở sớm hơn thì người dùng
    thấy một trang lỗi kết nối rồi phải tự bấm tải lại.
    """
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(f"{base_url}/health", timeout=2) as resp:
                if resp.status == 200:
                    return True
        except (urllib.error.URLError, OSError):
            pass
        sleep(interval)
    return False


# ─── Chính sách điều hướng ───────────────────────────────────────────────────

def is_internal_url(url: str, base_url: str) -> bool:
    """True nếu URL thuộc chính ứng dụng.

    Link ra ngoài phải mở bằng trình duyệt hệ thống, không mở trong cửa sổ ứng
    dụng: cửa sổ này không có thanh địa chỉ, không nút quay lại, và người dùng
    không có cách nào nhận ra mình đã rời khỏi ứng dụng.
    """
    if not url:
        return False
    from urllib.parse import urlsplit

    try:
        target = urlsplit(url)
        base = urlsplit(base_url)
    except ValueError:
        return False
    if not target.scheme and not target.netloc:
        return True                      # đường dẫn tương đối
    return (target.hostname or "") == (base.hostname or "") and \
           (target.port or 80) == (base.port or 80)


# ─── Cửa sổ ──────────────────────────────────────────────────────────────────

def webview_available() -> bool:
    try:
        import webview  # noqa: F401
        return True
    except ImportError:
        return False


def open_window(base_url: str, on_closed: Optional[Callable[[], None]] = None) -> str:
    """Mở giao diện. Trả về ``"webview"`` hoặc ``"browser"`` — cách đã dùng.

    Hàm này CHẶN cho tới khi cửa sổ đóng (khi dùng webview).
    """
    if not webview_available():
        log.warning("pywebview chưa được cài — mở bằng trình duyệt hệ thống. "
                    "Cài thêm: pip install 'contractforge[desktop]'")
        webbrowser.open(base_url)
        return "browser"

    import webview

    window = webview.create_window(
        WINDOW_TITLE,
        base_url,
        width=WINDOW_MIN_SIZE[0],
        height=WINDOW_MIN_SIZE[1],
        min_size=WINDOW_MIN_SIZE,
        confirm_close=False,
    )
    if on_closed is not None:
        window.events.closed += on_closed

    # gui='edgechromium' → WebView2 trên Windows (AD-2). Trên nền tảng khác để
    # pywebview tự chọn engine sẵn có.
    import sys as _sys
    gui = "edgechromium" if _sys.platform == "win32" else None
    webview.start(gui=gui)
    return "webview"


# ─── Điểm vào ────────────────────────────────────────────────────────────────

def run() -> int:
    """Khởi động server rồi mở cửa sổ. Trả về mã thoát của tiến trình."""
    from app.paths import ensure_runtime_dirs

    ensure_runtime_dirs()

    server = ServerThread()
    server.start()

    if not wait_for_health(server.base_url):
        log.error("Server không sẵn sàng sau %.0fs — dừng lại.",
                  HEALTH_TIMEOUT_SECONDS)
        server.stop()
        return 1

    try:
        open_window(server.base_url)
    finally:
        server.stop()
    return 0
