$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$out = Join-Path $root 'dist/iidxbg'
$dll = Join-Path $root 'target/release/iidxbg.dll'
$tool = Join-Path $root 'target/release/iidxbg-tool.exe'
if (-not (Test-Path $dll) -or -not (Test-Path $tool)) { throw 'Build release first.' }
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
Compress-Archive -Path $packageFiles -DestinationPath "$root/dist/iidxbg-0.1.0-windows-x64.zip" -Force
Write-Output "Packaged: $root/dist/iidxbg-0.1.0-windows-x64.zip"
