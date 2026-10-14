param([string]$PostgresBin='C:\Program Files\PostgreSQL\15\bin')
$ErrorActionPreference='Stop'
. "$PSScriptRoot\Use-Rust.ps1"
. "$PSScriptRoot\Start-Postgres.ps1"
Push-Location $saasRoot
$testRoot=Join-Path $saasRoot ('.dev\cycle2-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $testRoot | Out-Null
$data=Join-Path $testRoot 'postgres'
$started=$false
try {
    $listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0)
    $listener.Start();$port=$listener.LocalEndpoint.Port;$listener.Stop()
    & "$PostgresBin\initdb.exe" -D $data -U contas --encoding=UTF8 --locale=C --auth=trust | Out-Null
    if($LASTEXITCODE -ne 0){throw 'initdb falhou'}
    Start-ProjectPostgres -PostgresBin $PostgresBin -Data $data -Log "$testRoot\postgres.log" -Port $port
    $started=$true
    $env:TEST_DISPOSABLE='1'
    $env:TEST_ADMIN_URL="postgres://contas@127.0.0.1:$port/postgres"
    $env:TEST_APP_URL="postgres://contas_app@127.0.0.1:$port/postgres"
    cargo test --locked -p contas-api --test isolation -- --ignored --nocapture
    if($LASTEXITCODE -ne 0){throw 'Teste de isolamento falhou'}
} finally {
    if($started){& "$PostgresBin\pg_ctl.exe" stop -D $data -m fast -w | Out-Null}
    $env:TEST_DISPOSABLE=$null;$env:TEST_ADMIN_URL=$null;$env:TEST_APP_URL=$null
    Pop-Location
}
