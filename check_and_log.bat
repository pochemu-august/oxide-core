@echo off
REM ==========================================================================
REM  oxide-core :: quality checks + log collection + summary
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

set "STATUS_FMT=SKIPPED"
set "STATUS_CHECK=SKIPPED"
set "STATUS_CLIPPY=SKIPPED"
set "STATUS_RUN=NOT REQUESTED"

echo.
echo ==========================================================
echo   oxide-core :: Quality Checks (%DATE% %TIME%)
echo ==========================================================
echo.

REM --- 1. Formatting ---------------------------------------------------------
echo | set /p "=[1/%TOTAL%] Running cargo fmt check... "
echo === Cargo Fmt Check (%DATE% %TIME%) === > logs\fmt_check.log
cargo fmt --verbose -- --check >> logs\fmt_check.log 2>&1
if errorlevel 1 (
    echo Status: FAILED (Run 'cargo fmt' to fix) >> logs\fmt_check.log
    echo FAILED
    set "STATUS_FMT=FAILED (needs 'cargo fmt')"
    set "FAILED=1"
) else (
    echo Status: OK (All files formatted properly) >> logs\fmt_check.log
    echo Done.
    set "STATUS_FMT=OK (Clean)"
)

REM --- 2. Compilation --------------------------------------------------------
echo | set /p "=[2/%TOTAL%] Running cargo check... "
echo === Cargo Check (%DATE% %TIME%) === > logs\cargo_check.log
cargo check --verbose --color never >> logs\cargo_check.log 2>&1
if errorlevel 1 (
    echo Status: FAILED >> logs\cargo_check.log
    echo FAILED
    set "STATUS_CHECK=FAILED (Compilation errors)"
    set "FAILED=1"
) else (
    echo Status: OK (No compilation errors) >> logs\cargo_check.log
    echo Done.
    set "STATUS_CHECK=OK (Compiled)"
)

REM --- 3. Clippy (pedantic) --------------------------------------------------
echo | set /p "=[3/%TOTAL%] Running cargo clippy (pedantic)... "
echo === Cargo Clippy Pedantic (%DATE% %TIME%) === > logs\clippy_analysis.log
cargo clippy --verbose --color never -- -W clippy::pedantic >> logs\clippy_analysis.log 2>&1
if errorlevel 1 (
    echo Status: FAILED >> logs\clippy_analysis.log
    echo FAILED
    set "STATUS_CLIPPY=FAILED (Errors found)"
    set "FAILED=1"
) else (
    findstr /R /C:"^warning:" logs\clippy_analysis.log > nul
    if errorlevel 1 (
        echo Status: OK (0 warnings) >> logs\clippy_analysis.log
        echo Done.
        set "STATUS_CLIPPY=OK (0 warnings)"
    ) else (
        echo Status: WARNINGS FOUND >> logs\clippy_analysis.log
        echo Warnings found!
        set "STATUS_CLIPPY=WARNING (See clippy_analysis.log)"
    )
)

REM --- 4. Optional: run the game --------------------------------------------
if /I "%~1"=="run" (
    echo [4/%TOTAL%] Running the game ^(close the game window to finish^)...
    echo === Runtime Log (%DATE% %TIME%) === > logs\runtime.log
    set "RUST_LOG=oxide_core=debug,bevy=info,wgpu=error"
    set "RUST_BACKTRACE=1"
    cargo run --color never >> logs\runtime.log 2>&1
    if errorlevel 1 (
        echo        Game exited with an error.
        set "STATUS_RUN=CRASHED / ERROR (See runtime.log)"
        set "FAILED=1"
    ) else (
        echo        Done.
        set "STATUS_RUN=OK (Exited cleanly)"
    )
)

REM --- Final Summary Table --------------------------------------------------
echo.
echo ==========================================================
echo                      SUMMARY REPORT
echo ==========================================================
echo   1. Code Style (fmt)    : %STATUS_FMT%
echo   2. Compilation (check) : %STATUS_CHECK%
echo   3. Linter (clippy)     : %STATUS_CLIPPY%
echo   4. Game Runtime        : %STATUS_RUN%
echo ----------------------------------------------------------
echo   Logs directory         : %CD%\logs\
if "%FAILED%"=="1" (
    echo   Overall Status         : SOME CHECKS FAILED [!]
) else (
    echo   Overall Status         : ALL CHECKS PASSED [OK]
)
echo ==========================================================
echo.

REM Keep the window open until the user presses a key
pause
endlocal & exit /b %FAILED%