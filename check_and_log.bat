@echo off
REM ==========================================================================
REM  oxide-core :: quality checks + log collection
REM
REM  Usage:
REM    check_and_log.bat        - run fmt / check / clippy, save reports to logs\
REM    check_and_log.bat run    - same, then launch the game and capture runtime log
REM ==========================================================================
setlocal EnableExtensions

REM Always operate from the project root (folder of this script),
REM so the script works no matter where it is launched from.
cd /d "%~dp0"

if not exist logs mkdir logs

set "FAILED=0"
set "TOTAL=3"
if /I "%~1"=="run" set "TOTAL=4"

echo.
echo === oxide-core checks (%DATE% %TIME%) ===
echo.

REM --- 1. Formatting ---------------------------------------------------------
echo | set /p "=[1/%TOTAL%] Running cargo fmt check... "
cargo fmt -- --check > logs\fmt_check.log 2>&1
if errorlevel 1 (
    echo FAILED  ^(see logs\fmt_check.log, fix with: cargo fmt^)
    set "FAILED=1"
) else (
    echo Done.
)

REM --- 2. Compilation --------------------------------------------------------
echo | set /p "=[2/%TOTAL%] Running cargo check... "
cargo check --color never > logs\cargo_check.log 2>&1
if errorlevel 1 (
    echo FAILED  ^(see logs\cargo_check.log^)
    set "FAILED=1"
) else (
    echo Done.
)

REM --- 3. Clippy (pedantic) --------------------------------------------------
echo | set /p "=[3/%TOTAL%] Running cargo clippy (pedantic)... "
cargo clippy --color never -- -W clippy::pedantic > logs\clippy_analysis.log 2>&1
if errorlevel 1 (
    echo FAILED  ^(see logs\clippy_analysis.log^)
    set "FAILED=1"
) else (
    findstr /R /C:"^warning" logs\clippy_analysis.log > nul
    if errorlevel 1 (
        echo Done. No warnings.
    ) else (
        echo Done, but warnings found ^(see logs\clippy_analysis.log^)
    )
)

REM --- 4. Optional: run the game --------------------------------------------
if /I "%~1"=="run" (
    echo [4/%TOTAL%] Running the game ^(close the window to finish^)...
    set "RUST_LOG=oxide_core=debug,bevy=warn,wgpu=error"
    set "RUST_BACKTRACE=1"
    cargo run --color never > logs\runtime.log 2>&1
    if errorlevel 1 (
        echo        Game exited with an error ^(see logs\runtime.log^)
        set "FAILED=1"
    ) else (
        echo        Done.
    )
)

echo.
echo All reports saved to the logs/ folder.
if "%FAILED%"=="1" (
    echo Status: SOME CHECKS FAILED
    endlocal & exit /b 1
)
echo Status: OK
endlocal & exit /b 0
