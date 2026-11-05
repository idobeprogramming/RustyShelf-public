$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $env:CARGO_HOME = Join-Path $PSScriptRoot '.cargo-home'
    $env:RUSTUP_HOME = Join-Path $PSScriptRoot 'runtime\rustup'
    $env:Path = (Join-Path $env:CARGO_HOME 'bin') + ';' + $env:Path
}
cargo build --release
if ($LASTEXITCODE -ne 0) {
    throw 'La compilation a échoué. Aucun paquet ne sera créé.'
}
$stage = Join-Path $PSScriptRoot 'dist\RustyShelf-windows-x86_64'
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Copy-Item 'target\release\rustyshelf.exe' $stage -Force
Copy-Item 'README.md' $stage -Force
Copy-Item 'CUSTOM_STYLE.md' $stage -Force
Copy-Item 'examples\rustyshelf.css' $stage -Force
$archive = Join-Path $PSScriptRoot 'dist\RustyShelf-windows-x86_64.zip'
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -Force
Write-Host "Package créé : $archive"
