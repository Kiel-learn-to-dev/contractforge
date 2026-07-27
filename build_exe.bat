@echo off
chcp 65001 >nul
echo ============================================
echo   ContractForge - Build EXE Launcher
echo ============================================
echo.

REM Kiem tra Python
python --version >nul 2>&1
if errorlevel 1 (
    echo [LOI] Python chua duoc cai dat!
    echo Tai Python tai: https://python.org/downloads
    echo Khi cai nho tick: "Add Python to PATH"
    pause
    exit /b 1
)

echo Python:
python --version
echo.

REM Cai PyInstaller va deps bang python -m pip (tranh loi PATH)
echo [1/3] Cai dat PyInstaller va dependencies...
python -m pip install pyinstaller pystray pillow --quiet --no-warn-script-location
if errorlevel 1 (
    echo [LOI] Khong cai duoc PyInstaller
    pause
    exit /b 1
)
echo     Done.
echo.

REM Build bang python -m PyInstaller (tranh loi pyinstaller.exe khong trong PATH)
echo [2/3] Build ContractForge.exe ...
python -m PyInstaller --clean --noconfirm ^
    --onefile ^
    --windowed ^
    --name ContractForge ^
    --icon cf_icon.ico ^
    --hidden-import pystray ^
    --hidden-import pystray._win32 ^
    --hidden-import PIL ^
    --hidden-import PIL.Image ^
    --hidden-import PIL.ImageDraw ^
    --hidden-import PIL.ImageFont ^
    --hidden-import tkinter ^
    ContractForge.pyw

if errorlevel 1 (
    echo [LOI] Build that bai! Xem loi o tren.
    pause
    exit /b 1
)

echo.
echo [3/3] Sao chep ContractForge.exe...
if exist "dist\ContractForge.exe" (
    copy /Y "dist\ContractForge.exe" "ContractForge.exe" >nul
    echo.
    echo ============================================
    echo  BUILD THANH CONG!
    echo  ContractForge.exe da san sang.
    echo  Double-click ContractForge.exe de chay.
    echo ============================================
    echo.
    REM Don dep
    rmdir /s /q build 2>nul
    rmdir /s /q dist 2>nul
    del /q *.spec 2>nul
) else (
    echo [LOI] Khong tim thay dist\ContractForge.exe
    echo Xem log o tren de biet nguyen nhan.
)

pause
