$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $env:CARGO_HOME = Join-Path $PSScriptRoot '.cargo-home'
    $env:RUSTUP_HOME = Join-Path $PSScriptRoot 'runtime\rustup'
    $env:Path = (Join-Path $env:CARGO_HOME 'bin') + ';' + $env:Path
}
$previousFlags = $env:CARGO_ENCODED_RUSTFLAGS
try {
    $flags = @()
    if ($previousFlags) { $flags += $previousFlags.Split([char]31) }
    elseif ($env:RUSTFLAGS) { $flags += $env:RUSTFLAGS.Trim() -split '\s+' }
    foreach ($root in @($env:USERPROFILE, $PSScriptRoot, $env:CARGO_HOME, $env:RUSTUP_HOME, $env:CARGO_TARGET_DIR)) {
        if ($root) { $flags += "--remap-path-prefix=$root=/build" }
    }
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]31
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw 'La compilation a échoué.' }
} finally {
    $env:CARGO_ENCODED_RUSTFLAGS = $previousFlags
}
$metadataJson = cargo metadata --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) {
    throw 'Impossible de localiser la compilation. Aucun paquet ne sera créé.'
}
$buildDirectory = ($metadataJson | ConvertFrom-Json).target_directory
$binary = Join-Path $buildDirectory 'release\rustyshelf.exe'
$stage = Join-Path $PSScriptRoot 'dist\RustyShelf-windows-x86_64'
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Copy-Item -LiteralPath $binary -Destination $stage -Force
Copy-Item 'README.md' $stage -Force
Copy-Item 'CUSTOM_STYLE.md' $stage -Force
Copy-Item 'examples\rustyshelf.css' $stage -Force
$archive = Join-Path $PSScriptRoot 'dist\RustyShelf-windows-x86_64.zip'
$files = @('rustyshelf.exe', 'README.md', 'CUSTOM_STYLE.md', 'rustyshelf.css') | ForEach-Object { Join-Path $stage $_ }
Compress-Archive -LiteralPath $files -DestinationPath $archive -Force
Write-Host "Package créé : $archive"
