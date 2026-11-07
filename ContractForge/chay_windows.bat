@echo off
chcp 65001 >nul 2>&1
set "PYTHONUTF8=1"
set "PYTHONIOENCODING=utf-8"
title CONTRACTFORGE

cd /d "%~dp0"
set "APPDIR=%~dp0"
set "LOGFILE=%~dp0startup.log"
echo [%date% %time%] KHOI DONG >"%LOGFILE%"
echo [%date% %time%] Thu muc: %CD% >>"%LOGFILE%"

echo.
echo ============================================
echo   CONTRACTFORGE
echo   Log: startup.log
echo ============================================
echo.

echo [1/5] Kiem tra thu muc...
echo [%date% %time%] Buoc 1: main.py >>"%LOGFILE%"
if not exist "%~dp0main.py" (
    echo [LOI] Khong tim thay main.py!
    echo [LOI] main.py khong co tai %CD% >>"%LOGFILE%"
    echo.
    echo Huong dan:
    echo   1. Mo thu muc ContractForge ben trong thu muc du an
    echo      - thu muc co main.py va requirements.txt
    echo   2. ROI double-click chay_windows.bat
    echo.
    pause
    exit /b 1
)
echo      [OK] Thu muc: %CD%
echo [%date% %time%] Buoc 1: OK >>"%LOGFILE%"

echo [2/5] Kiem tra Python...
echo [%date% %time%] Buoc 2: Python >>"%LOGFILE%"
python --version >>"%LOGFILE%" 2>&1
if errorlevel 1 (
    echo [LOI] Khong tim thay Python!
    echo [LOI] Python khong tim thay >>"%LOGFILE%"
    echo.
    echo Tai: https://www.python.org/downloads/
    echo Nho tick Add Python to PATH khi cai dat!
    echo.
    pause
    exit /b 1
)
for /f "tokens=*" %%V in ('python --version 2^>^&1') do (
    echo      [OK] %%V
    echo [%date% %time%] Python: %%V >>"%LOGFILE%"
)
python -c "import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)" >>"%LOGFILE%" 2>&1
if errorlevel 1 (
    echo [LOI] Can Python 3.11 tro len!
    echo [LOI] Python qua cu - can 3.11+ >>"%LOGFILE%"
    echo.
    echo Tai ban moi tai: https://www.python.org/downloads/
    echo.
    pause
    exit /b 1
)

echo [3/5] Chuan bi moi truong Python...
echo [%date% %time%] Buoc 3: venv >>"%LOGFILE%"
if not exist "%~dp0venv\Scripts\python.exe" (
    echo      Tao venv - lan dau, vui long cho ~30 giay...
    python -m venv "%~dp0venv" >>"%LOGFILE%" 2>&1
    if errorlevel 1 (
        echo [LOI] Khong tao duoc venv!
        echo [LOI] Tao venv that bai >>"%LOGFILE%"
        echo Thu: click phai file nay - Run as Administrator
        pause
        exit /b 1
    )
    echo      [OK] Tao venv thanh cong.
    echo [%date% %time%] Buoc 3: venv OK >>"%LOGFILE%"
) else (
    echo      [OK] Venv da co san.
    echo [%date% %time%] Buoc 3: venv da co >>"%LOGFILE%"
)

call "%~dp0venv\Scripts\activate.bat"
echo [%date% %time%] Kich hoat venv OK >>"%LOGFILE%"

echo [4/5] Kiem tra thu vien...
echo [%date% %time%] Buoc 4: thu vien >>"%LOGFILE%"
REM Luon chay pip: du thu vien thi pip xong ngay va KHONG can mang; requirements.txt vua
REM doi (sau git pull) thi tu cap nhat thay vi bao loi "thieu thu vien" kho hieu.
echo      Lan dau can internet, ~2-5 phut. Cac lan sau chi vai giay...
"%~dp0venv\Scripts\python.exe" -m pip install -r "%~dp0requirements.txt" --progress-bar off --disable-pip-version-check >>"%LOGFILE%" 2>&1
if errorlevel 1 (
    echo [LOI] Cai dat thu vien that bai!
    echo [LOI] pip install that bai >>"%LOGFILE%"
    echo.
    echo Kiem tra ket noi internet roi chay lai.
    echo Neu startup.log bao "Microsoft Visual C++ 14.0 or greater is required":
    echo   Python cua ban qua moi, mot thu vien chua co ban dung san.
    echo   Cai Python 3.12 hoac 3.13 tu python.org, xoa thu muc venv, roi chay lai.
    echo Xem startup.log de biet loi cu the.
    echo.
    pause
    exit /b 1
)
echo      [OK] Thu vien san sang.
echo [%date% %time%] Buoc 4: OK >>"%LOGFILE%"

echo [5/5] Kiem tra ung dung...
echo [%date% %time%] Buoc 5: import app >>"%LOGFILE%"
echo import main >"%~dp0_chk.py"
"%~dp0venv\Scripts\python.exe" "%~dp0_chk.py" >>"%LOGFILE%" 2>&1
if errorlevel 1 (
    del "%~dp0_chk.py" >nul 2>&1
    echo [LOI] Ung dung bi loi khi khoi dong!
    echo [LOI] Import app that bai >>"%LOGFILE%"
    echo Xem startup.log de biet chi tiet.
    pause
    exit /b 1
)
del "%~dp0_chk.py" >nul 2>&1
echo      [OK] Ung dung san sang.
echo [%date% %time%] Buoc 5: OK >>"%LOGFILE%"

echo.
echo ============================================
echo   [OK] TAT CA SAN SANG!
echo.
echo   Dia chi : http://localhost:8888
echo   De tat  : Nhan Ctrl+C trong cua so nay
echo   Log     : startup.log
echo ============================================
echo.
echo [%date% %time%] Khoi dong uvicorn >>"%LOGFILE%"

start "" cmd /c "timeout /t 3 /nobreak >nul && start http://localhost:8888"

REM Chi lang nghe tren 127.0.0.1. App khong co dang nhap nen khong duoc mo ra LAN.
"%~dp0venv\Scripts\python.exe" -m uvicorn main:app --host 127.0.0.1 --port 8888 >>"%LOGFILE%" 2>&1

echo [%date% %time%] Uvicorn exit code: %errorlevel% >>"%LOGFILE%"
echo [%date% %time%] Server da tat >>"%LOGFILE%"
echo.
echo Server da tat. Nhan phim bat ky de dong...
pause
