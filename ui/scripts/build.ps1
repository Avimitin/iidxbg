param(
    [string[]]$CargoArgs = @('build', '--release', '--locked'),
    [string]$SdkRoot = ''
)
$ErrorActionPreference = 'Stop'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

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

# Optional standalone Windows SDK NuGet layout, for machines without the
# Windows 10/11 SDK installed through Visual Studio.
if (-not $SdkRoot -and $env:IIDXBG_SDK_ROOT) { $SdkRoot = $env:IIDXBG_SDK_ROOT }
if (-not $SdkRoot) {
    $candidate = Join-Path (Split-Path $PSScriptRoot -Parent) 'reference\sdk'
    if (Test-Path "$candidate\microsoft.windows.sdk.cpp\c\Include") { $SdkRoot = $candidate }
}
if ($SdkRoot -and (Test-Path "$SdkRoot\microsoft.windows.sdk.cpp\c\Include")) {
    $include = Get-ChildItem "$SdkRoot\microsoft.windows.sdk.cpp\c\Include" -Directory |
        Sort-Object Name -Descending | Select-Object -First 1
    $kernel = Get-ChildItem "$SdkRoot\microsoft.windows.sdk.cpp.x64" -Filter kernel32.lib -Recurse |
        Select-Object -First 1
    $ucrt = Get-ChildItem "$SdkRoot\microsoft.windows.sdk.cpp.x64\c\ucrt\x64" -Filter ucrt.lib |
        Select-Object -First 1
    $rc = Get-ChildItem "$SdkRoot\microsoft.windows.sdk.cpp\c\bin" -Filter rc.exe -Recurse |
        Where-Object { $_.DirectoryName -match '\\x64$' } | Select-Object -First 1
    if (-not $include -or -not $kernel -or -not $ucrt -or -not $rc) {
        throw 'Incomplete standalone Windows SDK layout'
    }
    $env:LIB = "$($kernel.DirectoryName);$($ucrt.DirectoryName);$env:LIB"
    $env:INCLUDE = "$($include.FullName)\ucrt;$($include.FullName)\shared;$($include.FullName)\um;$env:INCLUDE"
    $env:PATH = "$($rc.DirectoryName);$env:PATH"
    $env:RC = $rc.FullName
}

Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    & cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo failed: $LASTEXITCODE" }
} finally { Pop-Location }
