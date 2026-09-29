[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$PublishDir,
    [Parameter(Mandatory = $true)][string]$InnoCompiler
)

# Instala duas versões de TESTE sob .build-codex, com registro de desinstalação
# exclusivo. Não executa o Caixa, não usa o banco operacional e não cria atalhos.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$testId = 'CaixaModernizado.Test.' + [Guid]::NewGuid().ToString('N')
$testRoot = Join-Path $root ('.build-codex\installer-tests\' + $testId)
$installDir = Join-Path $testRoot 'app'
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
$checks = 0

function Check([bool]$Condition, [string]$Description) {
    if (-not $Condition) { throw "FAIL: $Description" }
    $script:checks++
    Write-Host "PASS: $Description"
}

function FileHash([string]$Path) {
    $sha = [Security.Cryptography.SHA256]::Create()
    $stream = [IO.File]::OpenRead($Path)
    try { return [BitConverter]::ToString($sha.ComputeHash($stream)) }
    finally { $stream.Dispose(); $sha.Dispose() }
}

function RunSetup([string]$Path, [string]$Label) {
    $log = Join-Path $testRoot ($Label + '.log')
    $arguments = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /NOICONS /TASKS="" /DIR="' + $installDir + '" /LOG="' + $log + '"'
    $process = Start-Process -FilePath $Path -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    return $process.ExitCode
}

# Usa o mesmo script do produto; somente o AppId e o número de versão mudam.
foreach ($version in @('1.6.0', '1.6.1')) {
    & $InnoCompiler '/Q' "/DAppVersion=$version" "/DPublishDir=$PublishDir" "/DInstallerOutputDir=$testRoot" "/DInstallerAppId=$testId" (Join-Path $PSScriptRoot 'CaixaModernizado.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Não foi possível compilar o instalador de teste.' }
}

$older = Join-Path $testRoot 'CaixaModernizado-Setup-1.6.0-win-x64.exe'
$current = Join-Path $testRoot 'CaixaModernizado-Setup-1.6.1-win-x64.exe'
$installedExe = Join-Path $installDir 'Caixa.WinForms.exe'
$uninstaller = Join-Path $installDir 'unins000.exe'
$registry = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser, [Microsoft.Win32.RegistryView]::Registry64)
$registryPath = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\' + $testId + '_is1'

function InstalledVersion {
    $key = $registry.OpenSubKey($registryPath)
    if (-not $key) { return $null }
    try { return $key.GetValue('DisplayVersion') }
    finally { $key.Dispose() }
}

# Sentinela com nome de banco dentro do destino, que o instalador não deve tocar.
$sentinel = Join-Path $installDir 'caixa.db'
[IO.File]::WriteAllText($sentinel, 'DADOS DESCARTAVEIS DO TESTE - NAO E UM BANCO OPERACIONAL')
$sentinelHash = FileHash $sentinel
try {
    Check ((RunSetup $older 'install') -eq 0) 'instalação inicial'
    Check ((InstalledVersion) -eq '1.6.0') 'versão inicial registrada'
    Check ((FileHash $installedExe) -eq (FileHash (Join-Path $PublishDir 'Caixa.WinForms.exe'))) 'executável instalado completo'
    Check ((FileHash $sentinel) -eq $sentinelHash) 'instalação preserva arquivo de dados'

    Check ((RunSetup $current 'upgrade') -eq 0) 'atualização sobre instalação existente'
    Check ((InstalledVersion) -eq '1.6.1') 'versão atualizada no mesmo registro'
    Check ((FileHash $sentinel) -eq $sentinelHash) 'atualização preserva dados'
    Check ((RunSetup $older 'downgrade') -ne 0) 'rejeição de versão anterior'
    Check ((InstalledVersion) -eq '1.6.1') 'versão preservada após downgrade rejeitado'

    $created = $false
    $mutex = New-Object System.Threading.Mutex($false, 'Local\CaixaModernizado.SingleInstance', [ref]$created)
    try {
        Check ((RunSetup $current 'running-app') -ne 0) 'instalação bloqueada com programa em execução (mutex)'
    } finally { $mutex.Dispose() }

    Check ((RunSetup $current 'reinstall') -eq 0) 'reinstalação da mesma versão'
    $uninstallLog = Join-Path $testRoot 'uninstall.log'
    $process = Start-Process -FilePath $uninstaller -ArgumentList ('/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /LOG="' + $uninstallLog + '"') -WindowStyle Hidden -Wait -PassThru
    Check ($process.ExitCode -eq 0) 'desinstalação'
    Check (-not (Test-Path -LiteralPath $installedExe)) 'executável removido'
    Check ((FileHash $sentinel) -eq $sentinelHash) 'desinstalação preserva dados'
    Check ($null -eq (InstalledVersion)) 'registro de teste removido'
    Write-Host "PASS: $checks verificações de instalação. Logs: $testRoot"
} finally {
    # Em caso de falha, remove somente a instalação de teste identificada acima.
    if ((Test-Path -LiteralPath $uninstaller) -and ($null -ne (InstalledVersion))) {
        Start-Process -FilePath $uninstaller -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -WindowStyle Hidden -Wait | Out-Null
    }
    $registry.Dispose()
}
