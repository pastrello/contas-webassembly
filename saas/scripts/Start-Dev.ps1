param([string]$PostgresBin = 'C:\Program Files\PostgreSQL\15\bin')
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Use-Rust.ps1"
. "$PSScriptRoot\Start-Postgres.ps1"
Push-Location $saasRoot
$startedDatabase = $false
$oldDatabase = $env:DATABASE_URL
$oldBind = $env:APP_BIND
$oldWeb = $env:WEB_DIR
$oldPassword = $env:PGPASSWORD
try {
    if (-not (Test-Path "$PostgresBin\pg_ctl.exe")) { throw 'Informe -PostgresBin ou use Docker Compose conforme o README.' }
    if (-not (Test-Path 'target\debug\contas-api.exe') -or -not (Test-Path 'web\pkg\contas_web_bg.wasm')) {
        & "$PSScriptRoot\Build.ps1"
    }
    $data = Join-Path $saasRoot '.dev\postgres'
    New-Item -ItemType Directory -Force .dev | Out-Null
    $passwordFile = Join-Path $saasRoot '.dev\postgres-password.txt'
    if (-not (Test-Path "$data\PG_VERSION")) {
        $password = [Guid]::NewGuid().ToString('N')
        [IO.File]::WriteAllText($passwordFile, $password)
        & "$PostgresBin\initdb.exe" -D $data -U contas --encoding=UTF8 --locale=C --auth=scram-sha-256 --pwfile=$passwordFile
        if ($LASTEXITCODE -ne 0) { throw 'Falha ao criar banco local' }
    }
    if (-not (Test-Path $passwordFile)) { throw 'Senha do banco de estudo ausente. Nao altere o banco; confira .dev/postgres-password.txt.' }
    $password = [IO.File]::ReadAllText($passwordFile).Trim()
    & "$PostgresBin\pg_ctl.exe" status -D $data *> $null
    if ($LASTEXITCODE -ne 0) {
        Start-ProjectPostgres -PostgresBin $PostgresBin -Data $data -Log "$saasRoot\.dev\postgres.log" -Port 55432
        $startedDatabase = $true
    }
    $env:PGPASSWORD = $password
    $exists = & "$PostgresBin\psql.exe" -h 127.0.0.1 -p 55432 -U contas -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname = 'contas_saas'"
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao verificar banco de estudo' }
    if ($exists -ne '1') {
        & "$PostgresBin\createdb.exe" -h 127.0.0.1 -p 55432 -U contas contas_saas
        if ($LASTEXITCODE -ne 0) { throw 'Falha ao criar contas_saas' }
    }
    $env:PGPASSWORD = $oldPassword
    $env:DATABASE_URL = "postgres://contas:${password}@127.0.0.1:55432/contas_saas"
    $env:APP_BIND = '127.0.0.1:8080'
    $env:WEB_DIR = Join-Path $saasRoot 'web'
    Write-Host 'Abra http://127.0.0.1:8080 - Ctrl+C encerra o ambiente iniciado por este script.'
    & .\target\debug\contas-api.exe
    if ($LASTEXITCODE -ne 0) { throw 'A API encerrou com erro.' }
} finally {
    if ($startedDatabase) { & "$PostgresBin\pg_ctl.exe" stop -D $data -m fast -w }
    $env:DATABASE_URL = $oldDatabase
    $env:APP_BIND = $oldBind
    $env:WEB_DIR = $oldWeb
    $env:PGPASSWORD = $oldPassword
    Pop-Location
}
