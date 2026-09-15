# Generates synthetic OCR fixtures: web-bubbles.png (web chat), game-chat.png (game chat box).
# Usage: powershell -NoProfile -File gen.ps1   (writes next to this script; fixed RNG seed = reproducible)
# NOTE: keep this file ASCII-only. Windows PowerShell 5.1 reads BOM-less files as the ANSI code page.
Add-Type -AssemblyName System.Drawing
$dir = $PSScriptRoot

$chat = @(
    "Hi there! Is the item still available?",
    "Yeah, it's still here. Are you interested?",
    "Could you do `$40 including shipping to Austin, TX?",
    "lol that's a bit low, how about 55?"
)
$bmp = New-Object System.Drawing.Bitmap 520, 260
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.TextRenderingHint = 'ClearTypeGridFit'
$g.Clear([System.Drawing.Color]::White)
$font = New-Object System.Drawing.Font 'Segoe UI', 10.5
$y = 12; $i = 0
foreach ($m in $chat) {
    $w = [int]$g.MeasureString($m, $font).Width + 20
    if ($i % 2 -eq 0) { $x = 12; $bg = [System.Drawing.Color]::FromArgb(233, 233, 235); $fg = [System.Drawing.Color]::Black }
    else { $x = 520 - 12 - $w; $bg = [System.Drawing.Color]::FromArgb(0, 132, 255); $fg = [System.Drawing.Color]::White }
    $g.FillRectangle((New-Object System.Drawing.SolidBrush $bg), $x, $y, $w, 36)
    $g.DrawString($m, $font, (New-Object System.Drawing.SolidBrush $fg), $x + 10, $y + 8)
    $y += 60; $i++
}
$bmp.Save("$dir\web-bubbles.png"); $g.Dispose(); $bmp.Dispose()

$game = @(
    @("[Trade] Stormrage: WTS Enigma 2.5b, pm me", 'Gold'),
    @("[Party] Zhan: gg, need 1 more for baal run", 'LightSkyBlue'),
    @("Zhan: can you drop the key at act 5?", 'White'),
    @("[Guild] Nyx: brb 5 min", 'LimeGreen')
)
$bmp = New-Object System.Drawing.Bitmap 420, 110
$g = [System.Drawing.Graphics]::FromImage($bmp)
$rnd = New-Object System.Random 42
for ($px = 0; $px -lt 420; $px += 6) {
    for ($py = 0; $py -lt 110; $py += 6) {
        $c = [System.Drawing.Color]::FromArgb($rnd.Next(40, 160), $rnd.Next(30, 120), $rnd.Next(20, 90))
        $g.FillRectangle((New-Object System.Drawing.SolidBrush $c), $px, $py, 6, 6)
    }
}
$g.FillRectangle((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(110, 0, 0, 0))), 0, 0, 420, 110)
$g.TextRenderingHint = 'AntiAlias'
$font = New-Object System.Drawing.Font 'Arial', 9
$y = 6
foreach ($l in $game) {
    $g.DrawString($l[0], $font, (New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromName($l[1]))), 6, $y)
    $y += 25
}
$bmp.Save("$dir\game-chat.png"); $g.Dispose(); $bmp.Dispose()
