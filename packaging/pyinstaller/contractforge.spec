# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec — bản Windows một file, không cần cài Python.

Chạy từ thư mục gốc dự án:

    pyinstaller packaging/pyinstaller/contractforge.spec --noconfirm

Nguyên tắc đóng gói (OPEN_SOURCE_DESKTOP_PLAN.md Task 13):

* Gói kèm MÃ NGUỒN: templates/, static/, assets/ — thiếu là app trắng trang.
* KHÔNG gói DỮ LIỆU: không cơ sở dữ liệu, không hồ sơ tổ chức, không mẫu riêng.
  Bản .exe chạy lần đầu tự dựng data root sạch trong %LOCALAPPDATA%\\ContractForge.
* Không mở cửa sổ console: đây là ứng dụng có giao diện.
"""

from pathlib import Path

from PyInstaller.utils.hooks import collect_submodules

SPEC_DIR = Path(SPECPATH).resolve()
ROOT = SPEC_DIR.parent.parent
APP = ROOT / "ContractForge"

# Mã nguồn và tài nguyên đi kèm. assets/default_templates chỉ chứa mẫu hư cấu
# đã commit — mẫu riêng của từng cơ quan nằm trong data root, không ở đây.
datas = [
    (str(APP / "templates"), "ContractForge/templates"),
    (str(APP / "static"), "ContractForge/static"),
    (str(APP / "assets"), "ContractForge/assets"),
    (str(APP / "main.py"), "ContractForge"),
    (str(APP / "app"), "ContractForge/app"),
]

# Router và service được import động ở vài chỗ; PyInstaller không lần ra hết
# bằng phân tích tĩnh nên khai báo tường minh.
hiddenimports = (
    collect_submodules("app")
    + [
        "main",
        "uvicorn.logging",
        "uvicorn.loops.auto",
        "uvicorn.protocols.http.auto",
        "uvicorn.lifespan.on",
        # WebSocket cố ý KHÔNG có: app không dùng, và app/desktop.py đặt
        # ws="none" nên uvicorn không nạp lớp đó.
    ]
)

a = Analysis(
    [str(ROOT / "ContractForge.pyw")],
    pathex=[str(ROOT), str(APP)],
    binaries=[],
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    runtime_hooks=[],
    excludes=[
        # Kéo theo hàng chục MB mà không dùng tới.
        "tkinter", "matplotlib", "numpy", "pandas", "pytest",
        "websockets",
    ],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="ContractForge",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    runtime_tmpdir=None,
    console=False,          # ứng dụng GUI — không hiện cửa sổ đen
    disable_windowed_traceback=False,
    icon=str(ROOT / "cf_icon.ico") if (ROOT / "cf_icon.ico").exists() else None,
)
