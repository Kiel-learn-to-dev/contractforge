"""Run ContractForge from source with live reload in its desktop window."""
from __future__ import annotations

import argparse
import os
import socket
import signal
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
APP_DIR = ROOT / "ContractForge"
LOOPBACK = "127.0.0.1"


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind((LOOPBACK, 0))
        return sock.getsockname()[1]


def _wait_for_health(url: str, process: subprocess.Popen, timeout: float = 45) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            return False
        try:
            with urllib.request.urlopen(f"{url}/health", timeout=1) as response:
                if response.status == 200:
                    return True
        except (urllib.error.URLError, OSError):
            time.sleep(0.25)
    return False


def _stop_server(process: subprocess.Popen) -> None:
    if process.poll() is not None:
        return
    if os.name == "nt":
        # The owned server uses its own process group. CTRL_BREAK requests a
        # graceful Uvicorn shutdown; taskkill below is scoped to this PID tree.
        try:
            process.send_signal(signal.CTRL_BREAK_EVENT)
            process.wait(timeout=8)
            return
        except (OSError, subprocess.TimeoutExpired):
            pass
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    else:
        process.terminate()
    try:
        process.wait(timeout=8)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def _serve(port: int) -> int:
    # CTRL_BREAK is the launcher's private shutdown signal. Forward it to
    # Uvicorn's normal SIGINT shutdown path so its server closes cleanly.
    if os.name == "nt" and hasattr(signal, "SIGBREAK"):
        signal.signal(signal.SIGBREAK, lambda _sig, _frame: signal.raise_signal(signal.SIGINT))

    import uvicorn

    uvicorn.run(
        "app.live_app:create_app",
        app_dir=str(APP_DIR),
        host=LOOPBACK,
        port=port,
        factory=True,
        reload=False,
        ws="none",
        log_level="info",
    )
    return 0


def _reloadable_file(_change, path: str) -> bool:
    return Path(path).suffix.lower() in {".py", ".html", ".css", ".js"}


def _spawn_server(port: int, data_root: Path) -> subprocess.Popen:
    command = [sys.executable, str(Path(__file__).resolve()), "--_serve", "--port", str(port)]
    creationflags = getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0) if os.name == "nt" else 0
    env = os.environ.copy()
    env["CONTRACTFORGE_DATA_ROOT"] = str(data_root)
    return subprocess.Popen(command, cwd=str(APP_DIR), env=env, creationflags=creationflags)


def _watch_and_restart(port: int, data_root: Path, server: list[subprocess.Popen],
                       server_lock: threading.Lock, stop_event: threading.Event) -> None:
    from watchfiles import watch

    for changes in watch(APP_DIR, watch_filter=_reloadable_file, stop_event=stop_event,
                         raise_interrupt=False, ignore_permission_denied=True):
        if stop_event.is_set():
            return
        changed_paths = sorted({path for _change, path in changes})
        print("Thay đổi mã nguồn:", ", ".join(changed_paths), flush=True)
        with server_lock:
            if stop_event.is_set():
                return
            _stop_server(server[0])
            if stop_event.is_set():
                return
            server[0] = _spawn_server(port, data_root)
            # Do not wait here: a syntax error should not stop watching. Saving
            # the correction will trigger another restart and recover normally.


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=0, help="Port; default selects a free local port")
    parser.add_argument("--_serve", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--no-window", "--no-browser", dest="no_window", action="store_true",
                        help="Run server only (automation)")
    args = parser.parse_args(argv)
    if not 0 <= args.port <= 65535:
        parser.error("--port must be between 0 and 65535")
    if args._serve:
        return _serve(args.port)

    if os.name == "nt" and hasattr(signal, "SIGBREAK"):
        # CTRL+BREAK in the launcher console must unwind the window/wait loop
        # so its finally block can stop the owned Uvicorn process tree.
        def _handle_launcher_break(_sig, _frame):
            raise KeyboardInterrupt

        signal.signal(signal.SIGBREAK, _handle_launcher_break)

    # Resolve the normal app data root once and pass it to the reloader worker.
    # This keeps source mode on the same database as the existing installation.
    sys.path.insert(0, str(APP_DIR))
    from app.paths import DATA_DIR

    data_root = Path(os.environ.get("CONTRACTFORGE_DATA_ROOT", DATA_DIR)).resolve()
    os.environ["CONTRACTFORGE_DATA_ROOT"] = str(data_root)
    port = args.port or _free_port()
    url = f"http://{LOOPBACK}:{port}"
    print("ContractForge — chạy trực tiếp từ mã nguồn", flush=True)
    print(f"Địa chỉ: {url}", flush=True)
    print(f"Dữ liệu: {data_root}", flush=True)
    print("Sửa Python/HTML/CSS/JS sẽ tự khởi động lại server.", flush=True)

    process = _spawn_server(port, data_root)
    server = [process]
    server_lock = threading.Lock()
    stop_event = threading.Event()
    watcher = None

    try:
        if not _wait_for_health(url, process):
            print("Không khởi động được server ContractForge.", file=sys.stderr)
            return process.returncode or 1

        watcher = threading.Thread(
            target=_watch_and_restart,
            args=(port, data_root, server, server_lock, stop_event),
            name="contractforge-live-reload",
            daemon=True,
        )
        watcher.start()

        if args.no_window:
            print("Server sẵn sàng. Nhấn Ctrl+C để dừng.", flush=True)
            try:
                while not stop_event.wait(0.5):
                    pass
                return 0
            except KeyboardInterrupt:
                return 0

        try:
            import webview
        except ImportError:
            print("Không tìm thấy pywebview trong .venv; không mở trình duyệt tự động.", file=sys.stderr)
            return 1

        # Reuse the project's WebView2 settings, including file downloads.
        from app.desktop import open_window
        open_window(url)
        return 0
    except KeyboardInterrupt:
        return 0
    finally:
        stop_event.set()
        if watcher is not None:
            watcher.join(timeout=3)
        with server_lock:
            _stop_server(server[0])


if __name__ == "__main__":
    raise SystemExit(main())
