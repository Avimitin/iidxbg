param(
    [string[]]$CargoArgs = @('build', '--release', '--locked'),
    [string]$SdkRoot = ''
)
$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'Install Visual Studio C++ build tools and Windows SDK.' }
$vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'
$environment = & $env:ComSpec /c "`"$vcvars`" >nul && set"
if ($LASTEXITCODE -ne 0) { throw 'vcvars64 failed' }
foreach ($line in $environment) {
    $pair = $line -split '=', 2
    if ($pair.Length -eq 2 -and $pair[0] -notmatch '^=') {
        [Environment]::SetEnvironmentVariable($pair[0], $pair[1], 'Process')
    }
}
# Optional standalone Microsoft Windows SDK NuGet layout.
if ($SdkRoot) {
    $include = Get-ChildItem "$SdkRoot\microsoft.windows.sdk.cpp\c\Include" -Directory | Sort-Object Name -Descending | Select-Object -First 1
    $kernel = Get-ChildItem "$SdkRoot\microsoft.windows.sdk.cpp.x64" -Filter kernel32.lib -Recurse | Select-Object -First 1
    $ucrt = Get-ChildItem "$SdkRoot\microsoft.windows.sdk.cpp.x64\c\ucrt\x64" -Filter ucrt.lib | Select-Object -First 1
    if (-not $include -or -not $kernel -or -not $ucrt) { throw 'Incomplete SDK layout' }
    $env:LIB = "$($kernel.DirectoryName);$($ucrt.DirectoryName);$env:LIB"
    $env:INCLUDE = "$($include.FullName)\ucrt;$($include.FullName)\shared;$($include.FullName)\um;$env:INCLUDE"
    $resourceCompiler = Join-Path $SdkRoot "microsoft.windows.sdk.cpp\c\bin\$($include.Name)\x64\rc.exe"
    if (Test-Path -LiteralPath $resourceCompiler) {
        $env:PATH = "$(Split-Path $resourceCompiler);$env:PATH"
    }
}
Push-Location (Split-Path $PSScriptRoot)
try {
    & cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo failed: $LASTEXITCODE" }
} finally { Pop-Location }
