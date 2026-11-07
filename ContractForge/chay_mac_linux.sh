#!/bin/bash
echo ""
echo "========================================"
echo "  CONTRACTFORGE - QUAN LY HOP DONG"
echo "========================================"
echo ""

# Chay duoc tu bat ky thu muc nao: moi duong dan ben duoi tinh tu thu muc chua script nay.
cd "$(dirname "$0")" || exit 1

# === Kiem tra Python ===
if ! command -v python3 &>/dev/null; then
    echo "[LOI] Khong tim thay Python 3!"
    echo "Mac: cai qua https://www.python.org/downloads/"
    echo "Ubuntu/Debian: sudo apt install python3 python3-pip python3-venv"
    exit 1
fi
if ! python3 -c "import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)"; then
    echo "[LOI] Can Python 3.11 tro len, may dang co $(python3 --version)."
    echo "Tai ban moi tai https://www.python.org/downloads/"
    exit 1
fi
echo "[OK] Da tim thay $(python3 --version)"

# === Virtual environment ===
if [ ! -x "venv/bin/python" ]; then
    echo "[...] Dang tao moi truong ao (lan dau)..."
    if ! python3 -m venv venv; then
        echo "[LOI] Khong tao duoc venv."
        echo "Ubuntu/Debian: sudo apt install python3-venv"
        rm -rf venv
        exit 1
    fi
    echo "[OK] Da tao."
fi

source venv/bin/activate

# === Cai thu vien ===
# Luon chay pip: neu da du thu vien thi pip xong ngay va KHONG can mang; neu requirements.txt
# vua doi (sau git pull) thi tu cap nhat, khong bi loi "thieu thu vien" kho hieu.
echo "[...] Kiem tra thu vien (lan dau cai mat 2-5 phut, can internet)..."
if ! python3 -m pip install -r requirements.txt -q --disable-pip-version-check; then
    echo "[LOI] Cai dat thu vien that bai - xem thong bao pip o tren."
    echo "Neu pip bao thieu trinh bien dich (gcc / Visual C++), Python cua ban co the qua moi:"
    echo "thu Python 3.12 hoac 3.13, xoa thu muc venv roi chay lai."
    exit 1
fi
echo "[OK] Thu vien san sang."

# === Mo trinh duyet ===
(sleep 2 && python3 -c "
import subprocess, sys
url = 'http://localhost:8888'
try:
    subprocess.run(['open', url])         # macOS
except:
    try:
        subprocess.run(['xdg-open', url]) # Linux
    except:
        pass
") &

echo ""
echo "[OK] Tat ca san sang!"
echo ""
echo " Server dang khoi dong tai: http://localhost:8888"
echo " De DUNG: Nhan Ctrl+C"
echo "========================================"
echo ""

# Chi lang nghe tren 127.0.0.1. App khong co dang nhap nen khong duoc mo ra LAN.
python3 -m uvicorn main:app --host 127.0.0.1 --port 8888
