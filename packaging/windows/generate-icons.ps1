$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$repository = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
# The master: the owner's logo (docs/media/brand/wisp-logo.png), a square picture of the mark in
# its glowing ring on a dark violet ground. Every icon is cut from the square around the ring.
$master = [Drawing.Image]::FromFile((Join-Path $repository 'docs/media/brand/wisp-logo.png'))
$scale = $master.Width / 1254.0
$side = [int](990 * $scale)
$crop = [Drawing.Rectangle]::new([int](130 * $scale), [int](95 * $scale), $side, $side)

# The app tile at `size` pixels: the crop, with 22 % rounded corners, anti-aliased.
function New-Tile([int]$size) {
    $tile = [Drawing.Bitmap]::new($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $scaled = [Drawing.Bitmap]::new($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [Drawing.Graphics]::FromImage($scaled)
    $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($master, [Drawing.Rectangle]::new(0, 0, $size, $size), $crop, [Drawing.GraphicsUnit]::Pixel)
    $g.Dispose()
    $canvas = [Drawing.Graphics]::FromImage($tile)
    $canvas.Clear([Drawing.Color]::Transparent)
    $canvas.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $r = [single]($size * 0.22)
    $d = $r * 2
    $path = [Drawing.Drawing2D.GraphicsPath]::new()
    $path.AddArc(0, 0, $d, $d, 180, 90)
    $path.AddArc($size - $d, 0, $d, $d, 270, 90)
    $path.AddArc($size - $d, $size - $d, $d, $d, 0, 90)
    $path.AddArc(0, $size - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    $brush = [Drawing.TextureBrush]::new($scaled)
    $canvas.FillPath($brush, $path)
    $brush.Dispose(); $path.Dispose(); $canvas.Dispose(); $scaled.Dispose()
    return $tile
}

function Get-PngBytes($bitmap) {
    $stream = [IO.MemoryStream]::new()
    $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
    $bytes = $stream.ToArray()
    $stream.Dispose()
    return ,$bytes
}

# The launcher's sidebar logo (drawn in a circle) and the window icon the game and launcher set.
$tile = New-Tile 256
$tile.Save((Join-Path $repository 'crates/wisp-theme/icons/wisp-256.png'), [Drawing.Imaging.ImageFormat]::Png)
$tile.Dispose()
$pngs = @{}
foreach ($size in @(16,24,32,48,64,128,256,512)) {
    $image = New-Tile $size
    $folder = Join-Path $repository "packaging/icons/hicolor/${size}x${size}/apps"
    New-Item -ItemType Directory -Force -Path $folder | Out-Null
    $image.Save((Join-Path $folder 'wisp.png'), [Drawing.Imaging.ImageFormat]::Png)
    $pngs[$size] = Get-PngBytes $image
    if ($size -eq 64) {
        $image.Save((Join-Path $repository 'crates/wisp-theme/icons/wisp-64.png'), [Drawing.Imaging.ImageFormat]::Png)
    }
    $image.Dispose()
}
# Linux's scalable icon and the launcher's mark: the 512 px tile inside an SVG.
$embedded = [Convert]::ToBase64String($pngs[512])
$svg = '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 512 512"><image width="512" height="512" xlink:href="data:image/png;base64,' + $embedded + '"/></svg>'
[IO.File]::WriteAllText((Join-Path $repository 'launcher/assets/icons/wisp.svg'), $svg, [Text.UTF8Encoding]::new($false))
Copy-Item -LiteralPath (Join-Path $repository 'launcher/assets/icons/wisp.svg') -Destination (Join-Path $repository 'packaging/icons/hicolor/scalable/apps/wisp.svg') -Force

# The Windows .ico: PNG images at every size Windows asks for (Vista and later read them).
$sizes = @(16,24,32,48,64,128,256)
$ico = [IO.MemoryStream]::new()
$writer = [IO.BinaryWriter]::new($ico)
$writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
foreach ($size in $sizes) {
    $edge = if ($size -ge 256) { 0 } else { $size }
    $writer.Write([byte]$edge); $writer.Write([byte]$edge); $writer.Write([byte]0); $writer.Write([byte]0)
    $writer.Write([uint16]1); $writer.Write([uint16]32)
    $writer.Write([uint32]$pngs[$size].Length); $writer.Write([uint32]$offset)
    $offset += $pngs[$size].Length
}
foreach ($size in $sizes) { $writer.Write($pngs[$size]) }
$writer.Flush()
[IO.File]::WriteAllBytes((Join-Path $repository 'packaging/icons/wisp.ico'), $ico.ToArray())
$writer.Dispose()

# Build the macOS icon from the same tiles, keeping every platform consistent.
$icns = [IO.MemoryStream]::new()
function Write-BigEndian($stream, [int]$value) {
    $bytes = [BitConverter]::GetBytes($value)
    [Array]::Reverse($bytes)
    $stream.Write($bytes,0,4)
}
$icns.Write([Text.Encoding]::ASCII.GetBytes('icns'),0,4)
$length = 8
$chunks = @(@('icp4',16),@('icp5',32),@('icp6',64),@('ic07',128),@('ic08',256),@('ic09',512))
foreach ($chunk in $chunks) { $length += 8 + $pngs[$chunk[1]].Length }
Write-BigEndian $icns $length
foreach ($chunk in $chunks) {
    $bytes = $pngs[$chunk[1]]
    $icns.Write([Text.Encoding]::ASCII.GetBytes($chunk[0]),0,4)
    Write-BigEndian $icns ($bytes.Length+8)
    $icns.Write($bytes,0,$bytes.Length)
}
[IO.File]::WriteAllBytes((Join-Path $repository 'packaging/icons/wisp.icns'),$icns.ToArray())
$icns.Dispose()
foreach ($factor in @(1,2)) {
    $background = [Drawing.Bitmap]::new(660*$factor,420*$factor)
    $canvas = [Drawing.Graphics]::FromImage($background)
    $canvas.ScaleTransform($factor,$factor)
    $canvas.Clear([Drawing.ColorTranslator]::FromHtml('#101018'))
    $canvas.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $logo = [Drawing.Image]::FromFile((Join-Path $repository 'crates/wisp-theme/icons/wisp-256.png'))
    $canvas.DrawImage($logo,22,20,32,32)
    $titleFont = [Drawing.Font]::new('Segoe UI',14,[Drawing.FontStyle]::Bold)
    $smallFont = [Drawing.Font]::new('Segoe UI',10)
    $canvas.DrawString('Wisp',$titleFont,[Drawing.Brushes]::White,62,24)
    $accent = [Drawing.Pen]::new([Drawing.ColorTranslator]::FromHtml('#b984f5'),2)
    $canvas.DrawLine($accent,262,200,401,200)
    $canvas.DrawLine($accent,393,192,401,200)
    $canvas.DrawLine($accent,393,208,401,200)
    $canvas.DrawString('DRAG WISP TO APPLICATIONS',$smallFont,[Drawing.Brushes]::LightGray,217,348)
    $name = if ($factor -eq 1) {'background.png'} else {'background@2x.png'}
    $background.Save((Join-Path $repository "packaging/macos/$name"),[Drawing.Imaging.ImageFormat]::Png)
    $accent.Dispose(); $titleFont.Dispose(); $smallFont.Dispose(); $logo.Dispose(); $canvas.Dispose(); $background.Dispose()
}
$master.Dispose()
Write-Output 'Icons written from docs/media/brand/wisp-logo.png'
