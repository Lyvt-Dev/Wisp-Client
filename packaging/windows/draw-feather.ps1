param([string]$Repository, [string]$Preview)
# Wisp's mark: a white feather on black. Writes the master picture the icon pipeline cuts
# (docs/media/brand/wisp-logo.png, 1254 px, the mark centred in the square it crops) and an SVG.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

# The feather upright, its middle at the origin, y down, tip at -400: the right vane narrower
# than the left, both meeting the shaft at the bottom of the vane (y 235).
$vane = @(
    @(0,-430), @(58,-380), @(96,-250), @(98,-90),
    @(100,40), @(72,170), @(18,232),
    @(4,236), @(-6,236), @(-12,232),
    @(-82,200), @(-128,80), @(-125,-60),
    @(-121,-230), @(-62,-372), @(0,-430)
)
# Splits in the vane, running in to the shaft along the barbs (cut only inside the vane).
$notches = @(
    @(@(140,-215), @(12,-128), @(140,-178)),
    @(@(140,0), @(9,82), @(140,30)),
    @(@(-150,-150), @(-10,-52), @(-150,-112)),
    @(@(-150,70), @(-8,160), @(-150,102))
)
$shaft = @(@(0,-410), @(10,-150), @(5,120), @(1,234))
$quill = @(@(1,234), @(0,300), @(-6,360), @(-14,415))
# Down at the foot of the vane, the wisps the client is named for.
$down = @(
    @(@(0,222), @(-40,216), @(-72,236), @(-88,268)),
    @(@(0,246), @(-30,246), @(-52,266), @(-60,294)),
    @(@(2,228), @(36,228), @(58,248), @(68,274)),
    @(@(1,252), @(26,254), @(40,272), @(46,294))
)
$turn = 40.0

function New-Path {
    $p = [Drawing.Drawing2D.GraphicsPath]::new()
    $pts = [Drawing.PointF[]]($vane | ForEach-Object { [Drawing.PointF]::new($_[0], $_[1]) })
    $p.AddBeziers($pts)
    $p.CloseFigure()
    return $p
}

function Draw-Mark($g, [float]$cx, [float]$cy, [float]$scale) {
    $state = $g.Save()
    $g.TranslateTransform($cx, $cy)
    $g.RotateTransform($turn)
    $g.ScaleTransform($scale, $scale)
    $white = [Drawing.SolidBrush]::new([Drawing.Color]::White)
    $black = [Drawing.SolidBrush]::new([Drawing.Color]::Black)
    $path = New-Path
    $g.FillPath($white, $path)
    $g.SetClip($path)
    foreach ($n in $notches) {
        $tri = [Drawing.PointF[]]($n | ForEach-Object { [Drawing.PointF]::new($_[0], $_[1]) })
        $g.FillPolygon($black, $tri)
    }
    $g.ResetClip()
    $downPen = [Drawing.Pen]::new([Drawing.Color]::White, 7)
    $downPen.StartCap = 'Round'; $downPen.EndCap = 'Round'
    foreach ($d in $down) {
        $g.DrawBeziers($downPen, [Drawing.PointF[]]($d | ForEach-Object { [Drawing.PointF]::new($_[0], $_[1]) }))
    }
    $pen = [Drawing.Pen]::new([Drawing.Color]::Black, 11)
    $pen.StartCap = 'Round'; $pen.EndCap = 'Round'
    $g.DrawBeziers($pen, [Drawing.PointF[]]($shaft | ForEach-Object { [Drawing.PointF]::new($_[0], $_[1]) }))
    $quillPen = [Drawing.Pen]::new([Drawing.Color]::White, 16)
    $quillPen.StartCap = 'Flat'; $quillPen.EndCap = 'Round'
    $g.DrawBeziers($quillPen, [Drawing.PointF[]]($quill | ForEach-Object { [Drawing.PointF]::new($_[0], $_[1]) }))
    $g.Restore($state)
}

$size = 1254
$bmp = [Drawing.Bitmap]::new($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.PixelOffsetMode = 'HighQuality'
$g.Clear([Drawing.Color]::Black)
# The square the pipeline crops: 990 px from (130, 95). A faint grey glow behind the mark.
$cx = 130 + 495; $cy = 95 + 495
$glow = [Drawing.Drawing2D.GraphicsPath]::new()
$glow.AddEllipse($cx - 430, $cy - 430, 860, 860)
$brush = [Drawing.Drawing2D.PathGradientBrush]::new($glow)
$brush.CenterColor = [Drawing.Color]::FromArgb(255, 46, 46, 46)
$brush.SurroundColors = [Drawing.Color[]]@([Drawing.Color]::FromArgb(255, 0, 0, 0))
$g.FillPath($brush, $glow)
Draw-Mark $g $cx $cy 1.02
$g.Dispose()
$bmp.Save((Join-Path $Repository 'docs/media/brand/wisp-logo.png'), [Drawing.Imaging.ImageFormat]::Png)
if ($Preview) { $bmp.Save($Preview, [Drawing.Imaging.ImageFormat]::Png) }
$bmp.Dispose()

# The same mark as an SVG (1024 square, black rounded tile).
function Svg-Path($list) {
    $s = "M$($list[0][0]) $($list[0][1])"
    for ($i = 1; $i -lt $list.Count; $i += 3) {
        $s += " C$($list[$i][0]) $($list[$i][1]) $($list[$i+1][0]) $($list[$i+1][1]) $($list[$i+2][0]) $($list[$i+2][1])"
    }
    return $s
}
$notchSvg = '<g clip-path="url(#v)">' + (($notches | ForEach-Object { "<path d=`"M$($_[0][0]) $($_[0][1]) L$($_[1][0]) $($_[1][1]) L$($_[2][0]) $($_[2][1])Z`" fill=`"#000`"/>" }) -join '') + '</g>'
$notchSvg += ($down | ForEach-Object { "<path d=`"$(Svg-Path $_)`" fill=`"none`" stroke=`"#fff`" stroke-width=`"7`" stroke-linecap=`"round`"/>" }) -join ''
$svg = @"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024"><defs><radialGradient id="g"><stop offset="0" stop-color="#2e2e2e"/><stop offset="1" stop-color="#000"/></radialGradient><clipPath id="v"><path d="$(Svg-Path $vane)Z"/></clipPath></defs><rect width="1024" height="1024" rx="225" fill="#000"/><circle cx="512" cy="512" r="445" fill="url(#g)"/><g transform="translate(512 512) rotate($turn) scale(1.055)"><path d="$(Svg-Path $vane)Z" fill="#fff"/>$notchSvg<path d="$(Svg-Path $shaft)" fill="none" stroke="#000" stroke-width="11" stroke-linecap="round"/><path d="$(Svg-Path $quill)" fill="none" stroke="#fff" stroke-width="16" stroke-linecap="round"/></g></svg>
"@
[IO.File]::WriteAllText((Join-Path $Repository 'docs/media/brand/icon.svg'), $svg.Trim(), [Text.UTF8Encoding]::new($false))

# The feather alone, its splits and shaft cut out (a mask), in `$ink` on no background: for light
# backgrounds (mark.svg) and dark ones (mark-dark.svg).
$cuts = (($notches | ForEach-Object { "<path d=`"M$($_[0][0]) $($_[0][1]) L$($_[1][0]) $($_[1][1]) L$($_[2][0]) $($_[2][1])Z`" fill=`"#000`"/>" }) -join '')
$downs = (($down | ForEach-Object { "<path d=`"$(Svg-Path $_)`" fill=`"none`" stroke=`"#fff`" stroke-width=`"7`" stroke-linecap=`"round`"/>" }) -join '')
function Mark-Svg([string]$ink) {
    return "<svg xmlns=`"http://www.w3.org/2000/svg`" viewBox=`"0 0 1024 1024`"><defs><clipPath id=`"v`"><path d=`"$(Svg-Path $vane)Z`"/></clipPath><mask id=`"m`" maskUnits=`"userSpaceOnUse`" x=`"-600`" y=`"-600`" width=`"1200`" height=`"1200`"><path d=`"$(Svg-Path $vane)Z`" fill=`"#fff`"/><g clip-path=`"url(#v)`">$cuts</g><path d=`"$(Svg-Path $shaft)`" fill=`"none`" stroke=`"#000`" stroke-width=`"11`" stroke-linecap=`"round`"/><path d=`"$(Svg-Path $quill)`" fill=`"none`" stroke=`"#fff`" stroke-width=`"16`" stroke-linecap=`"round`"/>$downs</mask></defs><g transform=`"translate(512 512) rotate($turn) scale(1.15)`"><rect x=`"-600`" y=`"-600`" width=`"1200`" height=`"1200`" fill=`"$ink`" mask=`"url(#m)`"/></g></svg>"
}
[IO.File]::WriteAllText((Join-Path $Repository 'docs/media/brand/mark.svg'), (Mark-Svg '#111111'), [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $Repository 'docs/media/brand/mark-dark.svg'), (Mark-Svg '#ffffff'), [Text.UTF8Encoding]::new($false))
# Full-bleed for services that round or crop the picture: the icon without its corners.
[IO.File]::WriteAllText((Join-Path $Repository 'docs/media/brand/avatar.svg'), $svg.Trim().Replace(' rx="225"', ''), [Text.UTF8Encoding]::new($false))
Write-Output 'feather written'
