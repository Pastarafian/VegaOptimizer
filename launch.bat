@echo off
:: ═══════════════════════════════════════════════════
:: VegaOptimizer Launcher — Requests Admin Privileges
:: ═══════════════════════════════════════════════════

:: Check for admin rights
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo Requesting Administrator privileges...
    powershell -Command "Start-Process '%~f0' -Verb RunAs"
    exit /b
)

title VegaOptimizer v3.0.0
echo.
echo  ◈ VegaOptimizer v3.0.0
echo  ========================
echo.
echo  Running as Administrator ✓
echo  Starting development server...
echo  (Press Ctrl+C to stop)
echo.
cd /d "%~dp0"
npx tauri dev
pause
