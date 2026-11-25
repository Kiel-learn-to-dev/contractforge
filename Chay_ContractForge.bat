@echo off
chcp 65001 >nul
title ContractForge - Live Source
cd /d "%~dp0"
if not exist ".venv\Scripts\python.exe" (
  echo Khong tim thay .venv\Scripts\python.exe trong thu muc nay.
  pause
  exit /b 1
)
"%~dp0.venv\Scripts\python.exe" "%~dp0run_live.py"
if errorlevel 1 (
  echo.
  echo ContractForge dung do co loi. Xem thong bao phia tren.
  pause
)
