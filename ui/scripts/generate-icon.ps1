# Original vector artwork, rendered at each Windows icon size. No external assets.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

function Rounded-Path([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
    $path = [Drawing.Drawing2D.GraphicsPath]::new()
    $d = 2 * $r
    $path.AddArc($x, $y, $d, $d, 180, 90)
    $path.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $path.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $path.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    return $path
}

$sizes = @(16, 20, 24, 32, 40, 48, 64, 128, 256)
$frames = @()
foreach ($size in $sizes) {
    $bitmap = [Drawing.Bitmap]::new($size * 4, $size * 4)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.ScaleTransform($size / 16.0, $size / 16.0)
    $dark = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#111817'))
    $green = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#5FE08E'))
    $white = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#E8EFEB'))
    $edge = [Drawing.Pen]::new([Drawing.ColorTranslator]::FromHtml('#36473D'), 1)
    $frame = [Drawing.Pen]::new($white, 2.5)
    $outer = Rounded-Path 2 2 60 60 13
    $screen = Rounded-Path 12 12 40 30 4
    $graphics.FillPath($dark, $outer)
    $graphics.DrawPath($edge, $outer)
    $graphics.DrawPath($frame, $screen)
    $points = [Drawing.PointF[]]@(
        [Drawing.PointF]::new(16, 37), [Drawing.PointF]::new(25, 26),
        [Drawing.PointF]::new(33, 34), [Drawing.PointF]::new(39, 29),
        [Drawing.PointF]::new(48, 37)
    )
    $graphics.FillPolygon($green, $points)
    $graphics.FillEllipse($white, 38, 18, 6, 6)
    for ($i = 0; $i -lt 7; $i++) {
        $brush = if ($i % 2 -eq 0) { $white } else { $green }
        $graphics.FillRectangle($brush, 12 + 6 * $i, 47, 4, 9)
    }
    $output = [Drawing.Bitmap]::new($size, $size)
    $scaled = [Drawing.Graphics]::FromImage($output)
    $scaled.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $scaled.DrawImage($bitmap, 0, 0, $size, $size)
    $stream = [IO.MemoryStream]::new()
    $output.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
    $frames += ,$stream.ToArray()
    foreach ($item in @($stream, $scaled, $output, $frame, $edge, $outer, $screen, $dark, $green, $white, $graphics, $bitmap)) {
        $item.Dispose()
    }
}
$destination = Join-Path (Split-Path $PSScriptRoot -Parent) 'assets\iidxbg.ico'
$file = [IO.File]::Create($destination)
$writer = [IO.BinaryWriter]::new($file)
try {
    $writer.Write([uint16]0)
    $writer.Write([uint16]1)
    $writer.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        $dimension = if ($sizes[$i] -eq 256) { 0 } else { $sizes[$i] }
        $writer.Write([byte]$dimension)
        $writer.Write([byte]$dimension)
        $writer.Write([uint16]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]32)
        $writer.Write([uint32]$frames[$i].Length)
        $writer.Write([uint32]$offset)
        $offset += $frames[$i].Length
    }
    foreach ($frame in $frames) { $writer.Write([byte[]]$frame) }
} finally {
    $writer.Dispose()
    $file.Dispose()
}
Write-Output "Created $destination ($($sizes -join ', ') px)"
