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
$oldMigration = $env:MIGRATION_DATABASE_URL
$oldOrigin = $env:PUBLIC_ORIGIN
$oldAppEnv = $env:APP_ENV
$oldRegistration = $env:ALLOW_REGISTRATION
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
    $env:MIGRATION_DATABASE_URL = "postgres://contas:${password}@127.0.0.1:55432/contas_saas"
    # Preserve all existing data before applying pending schema changes.
    $backupDir=Join-Path $saasRoot '.dev\backups'
    New-Item -ItemType Directory -Force $backupDir | Out-Null
    $backupFile=Join-Path $backupDir ('before-migrate-'+(Get-Date -Format 'yyyyMMdd-HHmmss')+'-'+[Guid]::NewGuid().ToString('N')+'.dump')
    & "$PostgresBin\pg_dump.exe" -h 127.0.0.1 -p 55432 -U contas -d contas_saas -Fc -f $backupFile
    if ($LASTEXITCODE -ne 0) {throw 'Backup falhou; migracoes nao serao aplicadas'}
    $hasUsers=& "$PostgresBin\psql.exe" -h 127.0.0.1 -p 55432 -U contas -d contas_saas -tAc "SELECT to_regclass('public.users') IS NOT NULL"
    if ($LASTEXITCODE -ne 0) {throw 'Verificacao pre-migracao falhou'}
    $identitySnapshot="SELECT md5(coalesce((SELECT string_agg(row_to_json(u)::text,'' ORDER BY id) FROM users u),'') || coalesce((SELECT string_agg(row_to_json(t)::text,'' ORDER BY id) FROM tenants t),'') || coalesce((SELECT string_agg(row_to_json(m)::text,'' ORDER BY tenant_id,user_id) FROM memberships m),''))"
    if ($hasUsers -eq 't') {
        $identityBefore=& "$PostgresBin\psql.exe" -h 127.0.0.1 -p 55432 -U contas -d contas_saas -tAc $identitySnapshot
        if ($LASTEXITCODE -ne 0) {throw 'Snapshot pre-migracao falhou'}
    }
    & .\target\debug\contas-api.exe --migrate
    if ($LASTEXITCODE -ne 0) { throw 'Migracoes falharam' }
    if ($hasUsers -eq 't') {
        $identityAfter=& "$PostgresBin\psql.exe" -h 127.0.0.1 -p 55432 -U contas -d contas_saas -tAc $identitySnapshot
        if ($LASTEXITCODE -ne 0 -or $identityBefore -ne $identityAfter) {throw 'Identidades divergiram apos migracao; confira o backup antes de continuar'}
        Write-Host 'Usuarios, clientes e vinculos existentes preservados.'
    }
    Write-Host "Backup pre-migracao: $backupFile"
    $appPasswordFile = Join-Path $saasRoot '.dev\app-password.txt'
    if (-not (Test-Path $appPasswordFile)) { [IO.File]::WriteAllText($appPasswordFile,[Guid]::NewGuid().ToString('N')) }
    $appPassword = [IO.File]::ReadAllText($appPasswordFile).Trim()
    if ($appPassword -notmatch '^[a-f0-9]{32}$') { throw 'Senha local inesperada' }
    "ALTER ROLE contas_app LOGIN PASSWORD '$appPassword';" | & "$PostgresBin\psql.exe" -h 127.0.0.1 -p 55432 -U contas -d contas_saas -v ON_ERROR_STOP=1
    if ($LASTEXITCODE -ne 0) { throw 'Configuracao da conta restrita falhou' }
    $env:PGPASSWORD = $null
    $env:MIGRATION_DATABASE_URL = $null
    $env:DATABASE_URL = "postgres://contas_app:${appPassword}@127.0.0.1:55432/contas_saas"
    $env:PUBLIC_ORIGIN = 'http://127.0.0.1:8080'
    $env:APP_ENV = 'development'
    $env:ALLOW_REGISTRATION = 'true'
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
    $env:MIGRATION_DATABASE_URL = $oldMigration
    $env:PUBLIC_ORIGIN = $oldOrigin
    $env:APP_ENV = $oldAppEnv
    $env:ALLOW_REGISTRATION = $oldRegistration
    Pop-Location
}
