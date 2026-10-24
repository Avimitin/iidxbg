param([Parameter(Mandatory=$true)][string]$Fxc)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$source = Join-Path $root 'src/runtime/artwork.hlsl'
$binary = Join-Path $root 'target/artwork.ps'
& $Fxc /nologo /T ps_3_0 /E main /Fo $binary $source
if ($LASTEXITCODE -ne 0) { throw 'Shader compilation failed' }
$bytes = [IO.File]::ReadAllBytes($binary)
$lines = [Collections.Generic.List[string]]::new()
$lines.Add('// Generated from artwork.hlsl with fxc /T ps_3_0 /E main.')
$lines.Add('// Rebuild with scripts/build-artwork-shader.ps1.')
$lines.Add('pub const CODE: &[u32] = &[')
for ($offset = 0; $offset -lt $bytes.Length; $offset += 4) {
    $lines.Add(('    0x{0:x8},' -f [BitConverter]::ToUInt32($bytes, $offset)))
}
$lines.Add('];')
[IO.File]::WriteAllLines((Join-Path $root 'src/runtime/artwork_shader.rs'), $lines)
& rustfmt (Join-Path $root 'src/runtime/artwork_shader.rs')
if ($LASTEXITCODE -ne 0) { throw 'Shader source formatting failed' }
