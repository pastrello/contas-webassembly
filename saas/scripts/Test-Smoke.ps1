param([string]$PostgresBin = 'C:\Program Files\PostgreSQL\15\bin')
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Start-Postgres.ps1"
$saasRoot = Split-Path $PSScriptRoot -Parent
$testRoot = Join-Path $saasRoot ('.dev\smoke-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $testRoot | Out-Null
$data = Join-Path $testRoot 'postgres'
$databaseStarted = $false
$apiProcess = $null
$oldDatabase = $env:DATABASE_URL
$oldBind = $env:APP_BIND
$oldWeb = $env:WEB_DIR
$oldMigration = $env:MIGRATION_DATABASE_URL
$oldOrigin = $env:PUBLIC_ORIGIN
$oldAppEnv = $env:APP_ENV
function Get-FreePort {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()
    return $port
}
function Assert-Status([string]$Url, [int]$Expected) {
    try { $status = [int](Invoke-WebRequest $Url -UseBasicParsing -TimeoutSec 8).StatusCode }
    catch {
        if (-not $_.Exception.Response) { throw }
        $status = [int]$_.Exception.Response.StatusCode
    }
    if ($status -ne $Expected) { throw "$Url retornou $status; esperado $Expected" }
    Write-Host "PASS $Expected $Url"
}
try {
    $dbPort = Get-FreePort
    $apiPort = Get-FreePort
    # Trust is restricted to this disposable, loopback-only test cluster.
    & "$PostgresBin\initdb.exe" -D $data -U contas --encoding=UTF8 --locale=C --auth=trust | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'initdb falhou' }
    Start-ProjectPostgres -PostgresBin $PostgresBin -Data $data -Log "$testRoot\postgres.log" -Port $dbPort
    $databaseStarted = $true
    $env:DATABASE_URL = "postgres://contas@127.0.0.1:$dbPort/postgres"
    $env:MIGRATION_DATABASE_URL = $env:DATABASE_URL
    Push-Location $saasRoot
    try { & .\target\debug\contas-api.exe --migrate; if($LASTEXITCODE -ne 0){throw 'Migracoes falharam'} } finally {Pop-Location}
    "ALTER ROLE contas_app LOGIN;" | & "$PostgresBin\psql.exe" -h 127.0.0.1 -p $dbPort -U contas -d postgres -v ON_ERROR_STOP=1 | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Role de teste falhou'}
    $env:MIGRATION_DATABASE_URL = $null
    $env:DATABASE_URL = "postgres://contas_app@127.0.0.1:$dbPort/postgres"
    $env:APP_ENV = 'development'
    $env:PUBLIC_ORIGIN = "http://127.0.0.1:$apiPort"
    $env:APP_BIND = "127.0.0.1:$apiPort"
    $env:WEB_DIR = Join-Path $saasRoot 'web'
    $apiProcess = Start-Process "$saasRoot\target\debug\contas-api.exe" -WindowStyle Hidden -PassThru -RedirectStandardOutput "$testRoot\api.log" -RedirectStandardError "$testRoot\api-error.log"
    $base = "http://127.0.0.1:$apiPort"
    $available = $false
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        try { Invoke-WebRequest "$base/health/live" -UseBasicParsing -TimeoutSec 1 | Out-Null; $available = $true; break } catch { Start-Sleep -Milliseconds 200 }
    }
    if (-not $available) { throw 'API nao iniciou' }
    Assert-Status "$base/health/live" 204
    Assert-Status "$base/health/ready" 204
    $status = Invoke-RestMethod "$base/api/status"
    if ($status.database -ne 'connected') { throw 'PostgreSQL nao conectado' }
    Write-Host 'PASS resposta real PostgreSQL conectado'
    Assert-Status "$base/" 200
    Assert-Status "$base/pkg/contas_web.js" 200
    $wasm = Invoke-WebRequest "$base/pkg/contas_web_bg.wasm" -UseBasicParsing
    if ($wasm.Headers['Content-Type'] -notmatch 'application/wasm') { throw 'MIME WebAssembly incorreto' }
    Write-Host 'PASS modulo WASM servido como application/wasm'
    Assert-Status "$base/api/unknown" 404
    & "$PostgresBin\pg_ctl.exe" stop -D $data -m fast -w | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao parar banco de teste' }
    $databaseStarted = $false
    Assert-Status "$base/health/live" 204
    Assert-Status "$base/health/ready" 503
    Assert-Status "$base/api/status" 503
    Write-Host "Smoke test concluido. Logs em $testRoot"
} finally {
    if ($apiProcess -and -not $apiProcess.HasExited) { Stop-Process -Id $apiProcess.Id }
    if ($databaseStarted) { & "$PostgresBin\pg_ctl.exe" stop -D $data -m fast -w | Out-Null }
    $env:DATABASE_URL = $oldDatabase
    $env:APP_BIND = $oldBind
    $env:WEB_DIR = $oldWeb
    $env:MIGRATION_DATABASE_URL = $oldMigration
    $env:PUBLIC_ORIGIN = $oldOrigin
    $env:APP_ENV = $oldAppEnv
}
