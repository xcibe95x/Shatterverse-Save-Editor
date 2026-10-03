@echo off
setlocal
cd /d "%~dp0"
where npm >nul 2>nul
if errorlevel 1 (
  echo Node.js and npm are required. Install the current Node.js LTS release, then retry.
  pause
  exit /b 1
)
if not exist "node_modules\@tauri-apps\cli" (
  echo Installing frontend dependencies...
  call npm install
  if errorlevel 1 goto failed
)
call npm run tauri -- dev
if errorlevel 1 goto failed
exit /b 0
:failed
echo The development app could not start. Review the output above and README.md.
pause
exit /b 1
