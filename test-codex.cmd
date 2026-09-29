@echo off
setlocal
cd /d "%~dp0"
call build-codex.cmd
if errorlevel 1 exit /b 1
set "DOTNET_CLI_HOME=%CD%\.build-codex\dotnet"
set "APPDATA=%CD%\.build-codex\appdata"
set "NUGET_PACKAGES=%CD%\.build-codex\packages"
set "NUGET_HTTP_CACHE_PATH=%CD%\.build-codex\http-cache"
set "DOTNET_CLI_TELEMETRY_OPTOUT=1"
set "DOTNET_ADD_GLOBAL_TOOLS_TO_PATH=false"
dotnet restore tests\Caixa.RegressionTests\Caixa.RegressionTests.csproj --configfile NuGet.Codex.Config --source "%CD%\.build-codex\feed" -p:NuGetAudit=false -p:TargetPlatformDisplayName=Windows --nologo
if errorlevel 1 exit /b 1
dotnet run --project tests\Caixa.RegressionTests -c Release --no-restore -p:TargetPlatformDisplayName=Windows -- "%CD%\.build-codex\regression-data"
exit /b %errorlevel%
