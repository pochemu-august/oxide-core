@echo off
REM ==========================================================================
REM  oxide-core :: quality checks + log collection
REM
REM  Usage:
REM    check_and_log.bat        - run fmt / check / clippy, save reports to logs\
REM    check_and_log.bat run    - same, then launch the game and capture runtime log
REM ==========================================================================
setlocal EnableExtensions

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
echo === Cargo Fmt Check (%DATE% %TIME%) === > logs\fmt_check.log
cargo fmt --verbose -- --check >> logs\fmt_check.log 2>&1
if errorlevel 1 (
    echo Status: FAILED (Run 'cargo fmt' to fix) >> logs\fmt_check.log
    echo FAILED  ^(see logs\fmt_check.log, fix with: cargo fmt^)
    set "FAILED=1"
) else (
    echo Status: OK (All files formatted properly) >> logs\fmt_check.log
    echo Done.
)

REM --- 2. Compilation --------------------------------------------------------
echo | set /p "=[2/%TOTAL%] Running cargo check... "
echo === Cargo Check (%DATE% %TIME%) === > logs\cargo_check.log
cargo check --verbose --color never >> logs\cargo_check.log 2>&1
if errorlevel 1 (
    echo Status: FAILED >> logs\cargo_check.log
    echo FAILED  ^(see logs\cargo_check.log^)
    set "FAILED=1"
) else (
    echo Status: OK (No compilation errors) >> logs\cargo_check.log
    echo Done.
)

REM --- 3. Clippy (pedantic) --------------------------------------------------
echo | set /p "=[3/%TOTAL%] Running cargo clippy (pedantic)... "
echo === Cargo Clippy Pedantic (%DATE% %TIME%) === > logs\clippy_analysis.log
cargo clippy --verbose --color never -- -W clippy::pedantic >> logs\clippy_analysis.log 2>&1
if errorlevel 1 (
    echo Status: FAILED >> logs\clippy_analysis.log
    echo FAILED  ^(see logs\clippy_analysis.log^)
    set "FAILED=1"
) else (
    findstr /R /C:"^warning:" logs\clippy_analysis.log > nul
    if errorlevel 1 (
        echo Status: OK (0 warnings) >> logs\clippy_analysis.log
        echo Done. No warnings.
    ) else (
        echo Status: WARNINGS FOUND >> logs\clippy_analysis.log
        echo Done, but warnings found ^(see logs\clippy_analysis.log^)
    )
)

REM --- 4. Optional: run the game --------------------------------------------
if /I "%~1"=="run" (
    echo [4/%TOTAL%] Running the game ^(close the window to finish^)...
    echo === Runtime Log (%DATE% %TIME%) === > logs\runtime.log
    set "RUST_LOG=oxide_core=debug,bevy=info,wgpu=error"
    set "RUST_BACKTRACE=1"
    cargo run --color never >> logs\runtime.log 2>&1
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