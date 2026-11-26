param([string]$Repo, [string]$Shots)
# The README's banner, the social preview and the avatar: the feather and the name on black;
# and the launcher screenshots copied into docs/media.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$tile = [Drawing.Image]::FromFile((Join-Path $Repo 'crates/wisp-theme/icons/wisp-256.png'))
$master = [Drawing.Image]::FromFile((Join-Path $Repo 'docs/media/brand/wisp-logo.png'))

function New-Card([int]$w, [int]$h, [string]$path, [string]$format) {
    $bmp = [Drawing.Bitmap]::new($w, $h)
    $g = [Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'; $g.InterpolationMode = 'HighQualityBicubic'; $g.TextRenderingHint = 'AntiAliasGridFit'
    $g.Clear([Drawing.Color]::Black)
    # A soft grey glow on the right, where the big feather sits.
    $glow = [Drawing.Drawing2D.GraphicsPath]::new()
    $r = $h * 0.95
    $cx = $w * 0.76; $cy = $h * 0.5
    $glow.AddEllipse($cx - $r, $cy - $r, 2 * $r, 2 * $r)
    $brush = [Drawing.Drawing2D.PathGradientBrush]::new($glow)
    $brush.CenterColor = [Drawing.Color]::FromArgb(255, 40, 40, 42)
    $brush.SurroundColors = [Drawing.Color[]]@([Drawing.Color]::Black)
    # The master brings its own glow round the feather.
    # The big feather from the master (its glow is black at the edges, so it blends in).
    $side = [int]($h * 1.0)
    $g.DrawImage($master, [Drawing.Rectangle]::new([int]($cx - $side / 2), [int]($cy - $side / 2), $side, $side),
        [Drawing.Rectangle]::new(130, 95, 990, 990), [Drawing.GraphicsUnit]::Pixel)
    $left = [int]($w * 0.09)
    $icon = [int]($h * 0.13)
    $g.DrawImage($tile, $left, [int]($h * 0.2), $icon, $icon)
    $small = [Drawing.Font]::new('Segoe UI Semibold', [single]($h * 0.04), [Drawing.GraphicsUnit]::Pixel)
    $g.DrawString(('MINECRAFT: JAVA EDITION 26.3  ' + [char]0x00B7 + '  RUST'), $small, [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(160, 160, 165)), $left + $icon + $h * 0.03, $h * 0.2 + $icon / 2 - $h * 0.028)
    $big = [Drawing.Font]::new('Segoe UI', [single]($h * 0.17), [Drawing.FontStyle]::Bold, [Drawing.GraphicsUnit]::Pixel)
    $g.DrawString('Wisp Client', $big, [Drawing.Brushes]::White, $left - $h * 0.015, $h * 0.38)
    $sub = [Drawing.Font]::new('Segoe UI', [single]($h * 0.055), [Drawing.GraphicsUnit]::Pixel)
    $g.DrawString('The game you know, rebuilt in Rust.', $sub, [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(200, 200, 205)), $left, $h * 0.62)
    $g.Dispose()
    if ($format -eq 'jpg') {
        $bmp.Save($path, [Drawing.Imaging.ImageFormat]::Jpeg)
    } else {
        $bmp.Save($path, [Drawing.Imaging.ImageFormat]::Png)
    }
    $bmp.Dispose()
}
New-Card 1600 600 (Join-Path $Repo 'docs/media/banner.png') 'png'
New-Card 1280 640 (Join-Path $Repo 'docs/media/social-preview.jpg') 'jpg'
New-Card 1280 640 (Join-Path $Repo 'docs/media/brand/social/github-social-1280x640.png') 'png'

# The avatar: the feather tile at 500 px.
$avatar = [Drawing.Bitmap]::new(500, 500)
$g = [Drawing.Graphics]::FromImage($avatar)
$g.InterpolationMode = 'HighQualityBicubic'; $g.Clear([Drawing.Color]::Black)
$g.DrawImage($master, [Drawing.Rectangle]::new(0, 0, 500, 500), [Drawing.Rectangle]::new(130, 95, 990, 990), [Drawing.GraphicsUnit]::Pixel)
$g.Dispose()
$avatar.Save((Join-Path $Repo 'docs/media/brand/social/github-avatar-500.png'), [Drawing.Imaging.ImageFormat]::Png)
$avatar.Dispose()

# The launcher's pages, as the screenshots took them (an empty data folder, not signed in).
foreach ($page in @('home', 'settings', 'shop')) {
    Copy-Item (Join-Path $Shots "$page/f00000.png") (Join-Path $Repo "docs/media/launcher-$page.png") -Force
}
$tile.Dispose(); $master.Dispose()
Write-Output 'brand art written'

