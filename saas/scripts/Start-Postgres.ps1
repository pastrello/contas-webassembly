function Start-ProjectPostgres {
    param([string]$PostgresBin, [string]$Data, [string]$Log, [int]$Port)
    # Do not pipe pg_ctl into PowerShell: the daemon may inherit a pipe handle.
    # Do not use Start-Process -Wait either: Windows then waits for descendants.
    $arguments = @('start', '-D', ('"' + $Data + '"'), '-l', ('"' + $Log + '"'), '-o', ('"-h 127.0.0.1 -p ' + $Port + '"'), '-w', '-t', '30')
    $process = Start-Process "$PostgresBin\pg_ctl.exe" -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput ($Log + '.startup') -RedirectStandardError ($Log + '.startup-error')
    if (-not $process.WaitForExit(40000)) { throw "Tempo excedido ao iniciar PostgreSQL; confira $Log" }
    $process.WaitForExit()
    & "$PostgresBin\pg_ctl.exe" status -D $Data *> $null
    if ($LASTEXITCODE -ne 0) { throw "PostgreSQL nao iniciou; confira $Log" }
}
