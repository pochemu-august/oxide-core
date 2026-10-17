@echo off
REM ==========================================================================
REM  oxide-core :: quality checks + log collection + summary
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
echo   oxide-core :: Quality Checks [%DATE% %TIME%]
echo ==========================================================
echo.

REM --- 1. Formatting ---------------------------------------------------------
echo [1/%TOTAL%] Running cargo fmt check...
echo === Cargo Fmt Check [%DATE% %TIME%] === > logs\fmt_check.log
cargo fmt --verbose -- --check >> logs\fmt_check.log 2>&1
if errorlevel 1 goto FMT_FAIL
echo Status: OK - All files formatted properly >> logs\fmt_check.log
echo        Result: OK
set "STATUS_FMT=OK [Clean]"
goto STEP_2

:FMT_FAIL
echo Status: FAILED - Run 'cargo fmt' to fix >> logs\fmt_check.log
echo        Result: FAILED [run 'cargo fmt' to fix]
set "STATUS_FMT=FAILED [needs 'cargo fmt']"
set "FAILED=1"

REM --- 2. Compilation --------------------------------------------------------
:STEP_2
echo [2/%TOTAL%] Running cargo check...
echo === Cargo Check [%DATE% %TIME%] === > logs\cargo_check.log
cargo check --verbose --color never >> logs\cargo_check.log 2>&1
if errorlevel 1 goto CHECK_FAIL
echo Status: OK - No compilation errors >> logs\cargo_check.log
echo        Result: OK
set "STATUS_CHECK=OK [Compiled]"
goto STEP_3

:CHECK_FAIL
echo Status: FAILED >> logs\cargo_check.log
echo        Result: FAILED [see logs\cargo_check.log]
set "STATUS_CHECK=FAILED [Compilation errors]"
set "FAILED=1"

REM --- 3. Clippy (pedantic) --------------------------------------------------
:STEP_3
echo [3/%TOTAL%] Running cargo clippy [pedantic]...
echo === Cargo Clippy Pedantic [%DATE% %TIME%] === > logs\clippy_analysis.log
cargo clippy --verbose --color never -- -W clippy::pedantic >> logs\clippy_analysis.log 2>&1
if errorlevel 1 goto CLIPPY_FAIL

findstr /R /C:"^warning:" logs\clippy_analysis.log > nul
if errorlevel 1 goto CLIPPY_CLEAN
echo Status: WARNINGS FOUND >> logs\clippy_analysis.log
echo        Result: WARNINGS FOUND [see logs\clippy_analysis.log]
set "STATUS_CLIPPY=WARNING [See clippy_analysis.log]"
goto STEP_4

:CLIPPY_CLEAN
echo Status: OK - 0 warnings >> logs\clippy_analysis.log
echo        Result: OK [0 warnings]
set "STATUS_CLIPPY=OK [0 warnings]"
goto STEP_4

:CLIPPY_FAIL
echo Status: FAILED >> logs\clippy_analysis.log
echo        Result: FAILED [see logs\clippy_analysis.log]
set "STATUS_CLIPPY=FAILED [Errors found]"
set "FAILED=1"

REM --- 4. Optional: run the game --------------------------------------------
:STEP_4
if /I not "%~1"=="run" goto SUMMARY

echo [4/%TOTAL%] Running the game [close the game window to finish]...
echo === Runtime Log [%DATE% %TIME%] === > logs\runtime.ansi
set "RUST_LOG=oxide_core=debug,bevy=info,wgpu=error"
set "RUST_BACKTRACE=1"
cargo run --color never >> logs\runtime.ansi 2>&1
if errorlevel 1 goto RUN_FAIL
echo        Result: OK
set "STATUS_RUN=OK [Exited cleanly]"
goto SUMMARY

:RUN_FAIL
echo        Result: CRASHED / ERROR [see logs\runtime.log]
set "STATUS_RUN=CRASHED / ERROR [See runtime.log]"
set "FAILED=1"

REM --- Final Summary Table --------------------------------------------------
:SUMMARY
echo.
echo ==========================================================
echo                      SUMMARY REPORT
echo ==========================================================
echo   1. Code Style [fmt]    : %STATUS_FMT%
echo   2. Compilation [check] : %STATUS_CHECK%
echo   3. Linter [clippy]     : %STATUS_CLIPPY%
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

if not "%CI%"=="" goto FINISH
if "%OXIDE_NO_PAUSE%"=="1" goto FINISH
pause
:FINISH
endlocal & exit /b %FAILED%