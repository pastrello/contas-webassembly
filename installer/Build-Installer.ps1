[CmdletBinding()]
param(
    [string]$InnoCompiler,
    [switch]$Offline
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$cacheRoot = Join-Path $projectRoot '.build-codex'
$outputDir = Join-Path $projectRoot 'dist'

if (-not $InnoCompiler) {
    $candidates = @(
        (Join-Path $cacheRoot 'tools\inno-7.1.0\ISCC.exe'),
        "$env:ProgramFiles\Inno Setup 7\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 7\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 7\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    )
    $InnoCompiler = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $InnoCompiler) {
        $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
        if ($command) { $InnoCompiler = $command.Source }
    }
}
if (-not $InnoCompiler -or -not (Test-Path -LiteralPath $InnoCompiler)) {
    throw 'Instale Inno Setup 6.3 ou superior (https://jrsoftware.org/isdl.php) ou informe -InnoCompiler com o caminho do ISCC.exe.'
}

[xml]$props = Get-Content -LiteralPath (Join-Path $projectRoot 'Directory.Build.props')
$version = [string]$props.Project.PropertyGroup.Version
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Version deve usar o formato numero.numero.numero.' }

# Publicação em uma pasta nova: não apaga publicações anteriores nem mistura arquivos antigos.
$publishDir = Join-Path $cacheRoot ('installer-publish\' + $version + '-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $publishDir, $outputDir | Out-Null
$environment = @{
    DOTNET_CLI_HOME = (Join-Path $cacheRoot 'dotnet')
    APPDATA = (Join-Path $cacheRoot 'appdata')
    NUGET_PACKAGES = (Join-Path $cacheRoot 'packages')
    NUGET_HTTP_CACHE_PATH = (Join-Path $cacheRoot 'http-cache')
    DOTNET_CLI_TELEMETRY_OPTOUT = '1'
    DOTNET_ADD_GLOBAL_TOOLS_TO_PATH = 'false'
}
$previousEnvironment = @{}
try {
    foreach ($name in $environment.Keys) {
        $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
        [Environment]::SetEnvironmentVariable($name, $environment[$name], 'Process')
    }
    New-Item -ItemType Directory -Force -Path $environment.APPDATA | Out-Null
    $publishArgs = @(
        'publish', (Join-Path $projectRoot 'src\Caixa.WinForms\Caixa.WinForms.csproj'),
        '-c', 'Release', '-r', 'win-x64', '--self-contained', 'true',
        '-p:PublishSingleFile=true', '-p:IncludeNativeLibrariesForSelfExtract=true',
        '-p:PublishTrimmed=false', '-p:DebugType=None', '-p:DebugSymbols=false',
        '-p:TargetPlatformDisplayName=Windows', '-o', $publishDir, '--nologo'
    )
    if ($Offline) {
        $publishArgs += @('--source', (Join-Path $cacheRoot 'feed'), '-p:NuGetAudit=false')
    } else {
        $publishArgs += @('--source', 'https://api.nuget.org/v3/index.json')
    }
    & dotnet @publishArgs
    if ($LASTEXITCODE -ne 0) { throw 'Falha na publicação .NET. O instalador não foi gerado.' }

    $exePath = Join-Path $publishDir 'Caixa.WinForms.exe'
    if (-not (Test-Path -LiteralPath $exePath)) { throw 'Publicação não gerou o executável.' }
    $files = @(Get-ChildItem -LiteralPath $publishDir -File -Recurse)
    if ($files.Count -ne 1 -or $files[0].Name -ne 'Caixa.WinForms.exe') {
        throw 'A publicação contém arquivos adicionais. Revise a lista [Files] do instalador antes de continuar.'
    }
    $fileVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($exePath)
    if ($fileVersion.ProductVersion -ne $version) { throw 'A versão do executável difere de Directory.Build.props.' }

    & $InnoCompiler "/DAppVersion=$version" "/DPublishDir=$publishDir" "/DInstallerOutputDir=$outputDir" (Join-Path $PSScriptRoot 'CaixaModernizado.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Falha na compilação do instalador.' }
    $installer = Join-Path $outputDir "CaixaModernizado-Setup-$version-win-x64.exe"
    $sha = [Security.Cryptography.SHA256]::Create()
    $stream = [IO.File]::OpenRead($installer)
    try { $hash = [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '') }
    finally { $stream.Dispose(); $sha.Dispose() }
    "$hash  $([IO.Path]::GetFileName($installer))" | Set-Content -LiteralPath ($installer + '.sha256') -Encoding ASCII
    Write-Host "Instalador gerado: $installer"
    Write-Host "SHA256: $hash"
} finally {
    foreach ($name in $previousEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name], 'Process')
    }
}
