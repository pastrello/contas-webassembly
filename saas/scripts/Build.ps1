param([switch]$Release)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Use-Rust.ps1"
Push-Location $saasRoot
try {
    $profile = 'debug'
    $buildArgs = @('build', '--locked')
    if ($Release) { $buildArgs += '--release'; $profile = 'release' }
    & cargo @buildArgs -p contas-api
    if ($LASTEXITCODE -ne 0) { throw 'Falha no build da API' }
    & cargo @buildArgs -p contas-web --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'Falha no build WebAssembly' }
    & wasm-bindgen --target web --out-dir web/pkg "target/wasm32-unknown-unknown/$profile/contas_web.wasm"
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao gerar o carregador WebAssembly' }
} finally { Pop-Location }
