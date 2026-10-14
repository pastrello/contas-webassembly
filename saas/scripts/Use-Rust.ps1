# Dot-source from the build/development scripts. Prefer a project-local toolchain if present.
$saasRoot = Split-Path $PSScriptRoot -Parent
if (Test-Path (Join-Path $saasRoot '.dev\cargo\bin\cargo.exe')) {
    $env:CARGO_HOME = Join-Path $saasRoot '.dev\cargo'
    $env:RUSTUP_HOME = Join-Path $saasRoot '.dev\rustup'
    $env:Path = "$env:CARGO_HOME\bin;$env:Path"
}
if (Test-Path (Join-Path $saasRoot '.dev\wasm-bindgen\wasm-bindgen.exe')) {
    $env:Path = "$(Join-Path $saasRoot '.dev\wasm-bindgen');$env:Path"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Instale Rust: https://rust-lang.org/tools/install/' }

# Optional portable SDK, for machines with MSVC but no complete Windows SDK.
$portableSdk = Join-Path $saasRoot '.dev\microsoft.windows.sdk.cpp'
$portableLib = Join-Path $saasRoot '.dev\microsoft.windows.sdk.cpp.x64\c'
if (Test-Path "$portableLib\um\x64\kernel32.lib") {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) { throw 'MSVC C++ Build Tools nao encontrado' }
    $msvc = Get-ChildItem "$vs\VC\Tools\MSVC" -Directory | Sort-Object Name -Descending | Select-Object -First 1
    $env:Path = "$($msvc.FullName)\bin\Hostx64\x64;$env:Path"
    $env:LIB = "$portableLib\um\x64;$portableLib\ucrt\x64;$($msvc.FullName)\lib\x64"
    $headers = Get-ChildItem $portableSdk -Filter windows.h -Recurse | Select-Object -First 1
    if (-not $headers) { throw 'Headers do Windows SDK local ausentes' }
    $includes = Split-Path $headers.DirectoryName -Parent
    $env:INCLUDE = "$includes\ucrt;$includes\um;$includes\shared;$($msvc.FullName)\include"
}
