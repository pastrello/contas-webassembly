@echo off
setlocal
cd /d "%~dp0"
rem Keep build caches and NuGet settings inside the writable project directory.
set "DOTNET_CLI_HOME=%CD%\.build-codex\dotnet"
set "APPDATA=%CD%\.build-codex\appdata"
set "NUGET_PACKAGES=%CD%\.build-codex\packages"
set "NUGET_HTTP_CACHE_PATH=%CD%\.build-codex\http-cache"
set "DOTNET_CLI_TELEMETRY_OPTOUT=1"
set "DOTNET_GENERATE_ASPNET_CERTIFICATE=false"
set "DOTNET_ADD_GLOBAL_TOOLS_TO_PATH=false"
if not exist "%APPDATA%" mkdir "%APPDATA%"
if errorlevel 1 exit /b 1
rem Offline source from packages already restored on this computer.
set "LOCAL_FEED=%CD%\.build-codex\feed"
if not exist "%LOCAL_FEED%" mkdir "%LOCAL_FEED%"
if errorlevel 1 exit /b 1
if exist "%USERPROFILE%\.nuget\packages" (
  for /r "%USERPROFILE%\.nuget\packages" %%F in (*.nupkg) do (
    if not exist "%LOCAL_FEED%\%%~nxF" copy /b "%%F" "%LOCAL_FEED%\%%~nxF" >nul
    if errorlevel 1 exit /b 1
  )
)
rem This display label avoids SDK name discovery; it does not change the target framework.
rem Vulnerability audit requires internet and must be run separately when online.
dotnet restore CaixaModernizado.sln --configfile NuGet.Codex.Config --source "%LOCAL_FEED%" -p:NuGetAudit=false -p:TargetPlatformDisplayName=Windows --nologo
if errorlevel 1 exit /b 1
dotnet build CaixaModernizado.sln -c Release --no-restore --nologo -p:TargetPlatformDisplayName=Windows %*
exit /b %errorlevel%
