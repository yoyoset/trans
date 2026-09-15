# Generates the 1024x1024 source icon used by `npx tauri icon`.
# Usage: powershell -NoProfile -File make-icon.ps1   (writes icon.png next to this script)
# NOTE: keep this file ASCII-only. Windows PowerShell 5.1 reads BOM-less files as the ANSI code page.
Add-Type -AssemblyName System.Drawing
$size = 1024
$bmp = New-Object System.Drawing.Bitmap $size, $size
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.TextRenderingHint = 'AntiAlias'
$g.Clear([System.Drawing.Color]::Transparent)

$rect = New-Object System.Drawing.Rectangle 64, 64, 896, 896
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$r = 200
$path.AddArc($rect.X, $rect.Y, $r, $r, 180, 90)
$path.AddArc($rect.Right - $r, $rect.Y, $r, $r, 270, 90)
$path.AddArc($rect.Right - $r, $rect.Bottom - $r, $r, $r, 0, 90)
$path.AddArc($rect.X, $rect.Bottom - $r, $r, $r, 90, 90)
$path.CloseFigure()
$g.FillPath((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 5, 150, 105))), $path)

$font = New-Object System.Drawing.Font 'Segoe UI', 300, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$format = New-Object System.Drawing.StringFormat
$format.Alignment = 'Center'
$format.LineAlignment = 'Center'
$g.DrawString('A|' + [char]0x6587, $font, [System.Drawing.Brushes]::White, (New-Object System.Drawing.RectangleF 64, 64, 896, 896), $format)

$bmp.Save((Join-Path $PSScriptRoot 'icon.png'), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
