"""Vòng đời vỏ desktop — phần kiểm được mà không cần mở cửa sổ thật.

Cửa sổ WebView2 phải kiểm bằng tay trên Windows (xem
tests/smoke/windows_release_checklist.md). Những gì ở đây là phần đã gây lỗi
trong thực tế: chọn cổng, chờ server sẵn sàng, và quyết định link nào mở trong
ứng dụng.
"""

from __future__ import annotations

import socket
from pathlib import Path

import pytest


# ─── Chọn cổng ───────────────────────────────────────────────────────────────

def test_find_free_port_returns_a_usable_port(db_engine):
    from app.desktop import find_free_port, is_port_free

    port = find_free_port()
    assert 1024 < port < 65536
    assert is_port_free(port)


def test_find_free_port_avoids_a_port_in_use(db_engine):
    """Cổng cố định là nguồn lỗi 'không mở được app' kinh điển."""
    from app.desktop import find_free_port

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as taken:
        taken.bind(("127.0.0.1", 0))
        taken.listen(1)
        busy = taken.getsockname()[1]

        assert find_free_port() != busy


def test_is_port_free_detects_a_bound_port(db_engine):
    from app.desktop import is_port_free

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as taken:
        taken.bind(("127.0.0.1", 0))
        taken.listen(1)
        assert is_port_free(taken.getsockname()[1]) is False


def test_server_thread_builds_a_loopback_url(db_engine):
    from app.desktop import ServerThread

    server = ServerThread()
    assert server.base_url.startswith("http://127.0.0.1:")
    assert server.host == "127.0.0.1", "vỏ desktop không được mở ra LAN"


# ─── Chờ server sẵn sàng ─────────────────────────────────────────────────────

def test_wait_for_health_times_out_on_a_dead_server(db_engine):
    """Server chết thì phải bỏ cuộc, đừng treo mãi."""
    from app.desktop import find_free_port, wait_for_health

    dead = f"http://127.0.0.1:{find_free_port()}"
    slept: list[float] = []

    assert wait_for_health(dead, timeout=0.5, interval=0.1,
                           sleep=slept.append) is False
    assert slept, "phải có nghỉ giữa các lần thử, không quay vòng cháy CPU"


def test_server_thread_serves_health_and_stops_cleanly():
    """Chạy thật ServerThread — đúng mô hình mà bản .exe sẽ dùng.

    Cố ý KHÔNG dùng fixture ``db_engine``: fixture đó vá SessionLocal cho các
    test dùng TestClient, còn ở đây ta muốn chính xác thứ mà bản đóng gói chạy —
    uvicorn thật, cổng thật, HTTP thật. Data root đã là thư mục tạm do
    conftest đặt env, nên dữ liệu thật vẫn không bị đụng.
    """
    import threading
    import traceback

    from app.desktop import ServerThread, wait_for_health

    crash: list[str] = []
    server = ServerThread()

    original = server.start

    def guarded_start():
        try:
            original()
        except BaseException:
            crash.append(traceback.format_exc())
            raise

    guarded_start()
    try:
        ready = wait_for_health(server.base_url, timeout=25)
        assert ready is True, (
            "server không phản hồi /health"
            + (f"\nthread lỗi:\n{crash[0]}" if crash else "")
        )
    finally:
        server.stop(timeout=10)

    assert not server.is_alive(), "server phải dừng sạch khi đóng cửa sổ"
    assert threading.active_count() >= 1


# ─── Chính sách điều hướng ───────────────────────────────────────────────────

BASE = "http://127.0.0.1:8123"


@pytest.mark.parametrize("url,internal", [
    ("http://127.0.0.1:8123/contracts", True),
    ("http://127.0.0.1:8123/", True),
    ("/contracts/12", True),
    ("contracts/12", True),
    ("http://127.0.0.1:9999/contracts", False),   # cổng khác
    ("https://example.com", False),
    ("http://127.0.0.1.evil.example/x", False),
    ("", False),
])
def test_internal_url_policy(db_engine, url, internal):
    from app.desktop import is_internal_url

    assert is_internal_url(url, BASE) is internal


# ─── Cấu hình chịu được bản đóng gói ─────────────────────────────────────────

def test_server_config_survives_a_windowed_build(db_engine, monkeypatch):
    """Bản .exe console=False có sys.stdout = None.

    Bộ định dạng log mặc định của uvicorn gọi sys.stdout.isatty() ngay trong
    __init__, nên uvicorn.Config() ném lỗi và server không khởi động nổi — bản
    đóng gói chết hoàn toàn dù mọi test đều xanh. Chỉ lộ ra khi chạy chính file
    .exe. log_config=None là thứ giữ cho nó sống.
    """
    import uvicorn

    from app.desktop import ServerThread

    captured = {}

    class _Spy(uvicorn.Config):
        def __init__(self, *args, **kwargs):
            captured.update(kwargs)
            super().__init__(*args, **kwargs)

    monkeypatch.setattr(uvicorn, "Config", _Spy)
    monkeypatch.setattr(uvicorn, "Server", lambda config: type(
        "S", (), {"run": lambda self: None, "should_exit": False})())

    ServerThread().start()

    assert captured.get("log_config") is None, (
        "log_config phải là None, nếu không bản đóng gói dạng cửa sổ sẽ chết"
    )
    assert captured.get("ws") == "none", "app không có WebSocket — đừng nạp lớp đó"


def test_windowed_build_never_writes_to_a_missing_stdout():
    """Launcher phải vá sys.stdout/stderr khi chúng là None."""
    source = (Path(__file__).resolve().parents[2] / "ContractForge.pyw").read_text(
        encoding="utf-8")

    assert "sys.stdout is None" in source, (
        "launcher phải xử lý stdout=None của bản đóng gói dạng cửa sổ"
    )


def test_webview_availability_is_reported_not_assumed(db_engine):
    """pywebview là phụ thuộc tuỳ chọn — thiếu nó không được làm sập app."""
    from app.desktop import webview_available

    assert isinstance(webview_available(), bool)
