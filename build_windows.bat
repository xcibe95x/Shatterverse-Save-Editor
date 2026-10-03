@echo off
setlocal
cd /d "%~dp0"
where npm >nul 2>nul
if errorlevel 1 (
  echo Node.js and npm are required to build the app.
  exit /b 1
)
where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust and Cargo are required to build the app.
  exit /b 1
)
if not exist "node_modules\@tauri-apps\cli" (
  call npm install
  if errorlevel 1 goto failed
)
call npm run tauri -- build
if errorlevel 1 goto failed
if not exist "release" mkdir "release"
copy /y "src-tauri\target\release\shatterverse-save-editor.exe" "release\Shatterverse Save Editor.exe" >nul
if errorlevel 1 goto failed
echo.
echo Portable build complete: release\Shatterverse Save Editor.exe
exit /b 0
:failed
echo Build failed. See the output above for missing Windows build prerequisites.
exit /b 1
