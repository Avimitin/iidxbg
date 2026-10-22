param(
    [string]$Config = '',
    [string]$SdkRoot = ''
)
$ErrorActionPreference = 'Stop'
$arguments = @('build', '--release', '--locked')
if ($SdkRoot) {
    & (Join-Path $PSScriptRoot 'build.ps1') -CargoArgs $arguments -SdkRoot $SdkRoot
} else {
    & (Join-Path $PSScriptRoot 'build.ps1') -CargoArgs $arguments
}
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Join-Path (Split-Path $PSScriptRoot -Parent) 'target\release\iidxbg-config.exe'
if (-not (Test-Path $exe)) { throw "Missing built executable: $exe" }
if ($Config) {
    & $exe --config $Config
} else {
    & $exe
}
