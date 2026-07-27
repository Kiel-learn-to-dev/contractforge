#!/bin/bash
echo ""
echo "========================================"
echo "  CONTRACTFORGE - QUAN LY HOP DONG"
echo "========================================"
echo ""

# === Kiem tra Python ===
if ! command -v python3 &>/dev/null; then
    echo "[LOI] Khong tim thay Python 3!"
    echo "Mac: cai qua https://www.python.org/downloads/"
    echo "Ubuntu/Debian: sudo apt install python3 python3-pip python3-venv"
    exit 1
fi
echo "[OK] Da tim thay $(python3 --version)"

# === Virtual environment ===
if [ ! -d "venv" ]; then
    echo "[...] Dang tao moi truong ao (lan dau)..."
    python3 -m venv venv
    echo "[OK] Da tao."
fi

source venv/bin/activate

# === Cai thu vien ===
python3 -c "import fastapi, uvicorn, docx, openpyxl" 2>/dev/null
if [ $? -ne 0 ]; then
    echo "[...] Dang cai dat thu vien (lan dau, mat 2-5 phut)..."
    pip install -r requirements.txt -q
    echo "[OK] Cai dat xong."
fi

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

python3 -m uvicorn main:app --host 0.0.0.0 --port 8888
