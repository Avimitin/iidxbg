$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$out = Join-Path $root 'dist/iidxbg'
$dll = Join-Path $root 'target/release/iidxbg.dll'
$tool = Join-Path $root 'target/release/iidxbg-tool.exe'
if (-not (Test-Path $dll) -or -not (Test-Path $tool)) { throw 'Build release first.' }
$metadataJson = & cargo metadata --manifest-path "$root/Cargo.toml" --no-deps --locked --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Cannot read package version' }
$metadata = $metadataJson | ConvertFrom-Json
$version = ($metadata.packages | Where-Object name -eq 'iidxbg').version
if (-not $version) { throw 'iidxbg package version missing' }
$archive = Join-Path $root "dist/iidxbg-$version-windows-x64.zip"
New-Item -ItemType Directory -Force -Path "$out/images" | Out-Null
Copy-Item -LiteralPath $dll -Destination "$out/iidxbg.dll"
if (-not (Test-Path "$out/iidxbg.toml")) {
    Copy-Item -LiteralPath "$root/iidxbg.toml" -Destination "$out/iidxbg.toml"
}
Copy-Item -LiteralPath "$root/LICENSE" -Destination "$out/LICENSE"
if (Test-Path "$root/THIRD-PARTY-NOTICES.txt") {
    Copy-Item -LiteralPath "$root/THIRD-PARTY-NOTICES.txt" -Destination "$out/THIRD-PARTY-NOTICES.txt"
}
if (-not (Test-Path "$out/images/test-card.png")) {
    & $tool test-image "$out/images/test-card.png"
    if ($LASTEXITCODE -ne 0) { throw 'Test image creation failed' }
}
$packageFiles = @("$out/iidxbg.dll", "$out/iidxbg.toml", "$out/images", "$out/LICENSE")
if (Test-Path "$out/THIRD-PARTY-NOTICES.txt") {
    $packageFiles += "$out/THIRD-PARTY-NOTICES.txt"
}
Compress-Archive -Path $packageFiles -DestinationPath $archive -Force
if ($env:GITHUB_OUTPUT) {
    "archive=$archive" | Out-File -LiteralPath $env:GITHUB_OUTPUT -Encoding utf8 -Append
}
Write-Output "Packaged: $archive"
