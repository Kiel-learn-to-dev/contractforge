@echo off
chcp 65001 >nul 2>&1
set "PYTHONUTF8=1"
set "PYTHONIOENCODING=utf-8"
title Kiem Tra Moi Truong

cd /d "%~dp0"
set "LOGFILE=%~dp0kiem_tra.log"
echo [%date% %time%] KIEM TRA MOI TRUONG > "%LOGFILE%"

echo.
echo ============================================
echo   KIEM TRA MOI TRUONG CAN THIET
echo   (Ket qua luu trong: kiem_tra.log)
echo ============================================
echo.

REM 1. Thu muc
echo [1/6] Thu muc hien tai:
echo       %CD%
echo [%date% %time%] Thu muc: %CD% >> "%LOGFILE%"
if exist "%~dp0main.py" (
    echo       [PASS] main.py tim thay - thu muc dung
    echo [%date% %time%] main.py: PASS >> "%LOGFILE%"
) else (
    echo       [FAIL] KHONG TIM THAY main.py!
    echo [%date% %time%] main.py: FAIL >> "%LOGFILE%"
    echo       HAY CHAY FILE NAY TU BEN TRONG THU MUC contract_manager
)

REM 2. Python
echo.
echo [2/6] Python:
python --version >> "%LOGFILE%" 2>&1
if errorlevel 1 (
    echo       [FAIL] PYTHON CHUA CAI DAT
    echo [%date% %time%] Python: FAIL >> "%LOGFILE%"
    echo       Tai: https://www.python.org/downloads/
    echo       Nho tick "Add Python to PATH"!
) else (
    for /f "tokens=*" %%V in ('python --version 2^>^&1') do (
        echo       [PASS] %%V
        echo [%date% %time%] Python: %%V >> "%LOGFILE%"
    )
)

REM 3. pip
echo.
echo [3/6] pip:
pip --version >> "%LOGFILE%" 2>&1
if errorlevel 1 (
    echo       [FAIL] pip khong tim thay
    echo [%date% %time%] pip: FAIL >> "%LOGFILE%"
) else (
    for /f "tokens=1,2" %%A in ('pip --version 2^>^&1') do (
        echo       [PASS] pip %%B
    )
    echo [%date% %time%] pip: PASS >> "%LOGFILE%"
)

REM 4. Internet
echo.
echo [4/6] Ket noi internet:
ping -n 1 pypi.org >nul 2>&1
if errorlevel 1 (
    echo       [WARN] Khong ping duoc pypi.org
    echo       (Can internet de cai thu vien lan dau)
    echo [%date% %time%] Internet: WARN >> "%LOGFILE%"
) else (
    echo       [PASS] Ket noi OK
    echo [%date% %time%] Internet: PASS >> "%LOGFILE%"
)

REM 5. venv va thu vien
echo.
echo [5/6] Moi truong ao (venv):
if exist "%~dp0venv\Scripts\activate.bat" (
    echo       [PASS] venv da co
    echo [%date% %time%] venv: PASS >> "%LOGFILE%"
    call "%~dp0venv\Scripts\activate.bat" >nul 2>&1
    python -c "import main" >nul 2>&1
    if errorlevel 1 (
        echo       [WARN] venv co nhung thieu thu vien - chay_windows.bat se tu cai
        echo [%date% %time%] Thu vien: thieu >> "%LOGFILE%"
    ) else (
        echo       [PASS] Tat ca thu vien da co
        echo [%date% %time%] Thu vien: PASS >> "%LOGFILE%"
    )
) else (
    echo       [INFO] venv chua co - se tu tao khi chay lan dau
    echo [%date% %time%] venv: chua co >> "%LOGFILE%"
)

REM 6. Port 8888
echo.
echo [6/6] Port 8888:
netstat -an 2>nul | find ":8888 " >nul 2>&1
if errorlevel 1 (
    echo       [PASS] Port 8888 trong - san sang
    echo [%date% %time%] Port 8888: PASS >> "%LOGFILE%"
) else (
    echo       [WARN] Port 8888 dang dung
    echo       Thu mo: http://localhost:8888
    echo [%date% %time%] Port 8888: WARN - dang dung >> "%LOGFILE%"
)

echo.
echo ============================================
echo   XONG. Xem kiem_tra.log de biet them.
echo ============================================
echo.
pause
