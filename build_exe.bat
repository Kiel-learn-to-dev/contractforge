@echo off
chcp 65001 >nul
echo ============================================
echo   ContractForge - Build EXE
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

echo [1/3] Cai dependencies (runtime + desktop + PyInstaller)...
python -m pip install --quiet --no-warn-script-location ^
    -r ContractForge\requirements.txt ^
    pywebview pystray pillow pyinstaller
if errorlevel 1 (
    echo [LOI] Khong cai duoc dependencies
    pause
    exit /b 1
)
echo     Done.
echo.

REM Build theo spec file. Spec giu toan bo cau hinh dong goi o mot noi
REM (packaging/pyinstaller/contractforge.spec) thay vi mot chuoi co dai
REM tham so trong file .bat nay - de doc, de sua, va CI dung chung duoc.
echo [2/3] Build ContractForge.exe theo packaging\pyinstaller\contractforge.spec ...
python -m PyInstaller --clean --noconfirm packaging\pyinstaller\contractforge.spec
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
    echo.
    echo  Luu y: du lieu nguoi dung nam o
    echo  %%LOCALAPPDATA%%\ContractForge — KHONG nam trong .exe.
    echo  Go cai dat khong lam mat du lieu.
    echo ============================================
    echo.
    rmdir /s /q build 2>nul
    rmdir /s /q dist 2>nul
) else (
    echo [LOI] Khong tim thay dist\ContractForge.exe
    echo Xem log o tren de biet nguyen nhan.
)

pause
