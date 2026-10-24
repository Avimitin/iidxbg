$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$out = Join-Path $root 'dist/iidxbg'
$dll = Join-Path $root 'target/release/iidxbg.dll'
$configGui = Join-Path $root 'ui/target/release/iidxbg-config.exe'
if (-not (Test-Path $dll) -or -not (Test-Path $configGui)) {
    throw 'Build the release DLL and configuration GUI first.'
}
$metadataJson = & cargo metadata --manifest-path "$root/Cargo.toml" --no-deps --locked --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Cannot read package version' }
$metadata = $metadataJson | ConvertFrom-Json
$version = ($metadata.packages | Where-Object name -eq 'iidxbg').version
if (-not $version) { throw 'iidxbg package version missing' }
$archive = Join-Path $root "dist/iidxbg-$version-windows-x64.zip"
if (Test-Path $out) { Remove-Item -LiteralPath $out -Recurse -Force }
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item -LiteralPath $dll -Destination "$out/iidxbg.dll" -Force
Copy-Item -LiteralPath "$root/iidxbg.toml" -Destination "$out/iidxbg.toml" -Force
Copy-Item -LiteralPath $configGui -Destination "$out/iidxbg-config.exe" -Force
Copy-Item -LiteralPath "$root/THIRD-PARTY-NOTICES.txt" -Destination "$out/THIRD-PARTY-NOTICES.txt" -Force
Compress-Archive -Path @(
    "$out/iidxbg.dll",
    "$out/iidxbg.toml",
    "$out/iidxbg-config.exe",
    "$out/THIRD-PARTY-NOTICES.txt"
) -DestinationPath $archive -Force
if ($env:GITHUB_OUTPUT) {
    "archive=$archive" | Out-File -LiteralPath $env:GITHUB_OUTPUT -Encoding utf8 -Append
}
Write-Output "Packaged: $archive"
