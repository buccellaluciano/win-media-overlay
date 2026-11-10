# Renders the WMO monogram (option A: white W on graphite, blue island pill) to PNG sizes
# and packs them into a multi-size .ico (PNG-compressed entries, Vista+ format).
param([string]$OutDir)
Add-Type -AssemblyName System.Drawing

function New-RoundRect([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = 2 * $r
  $p.AddArc($x, $y, $d, $d, 180, 90)
  $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
  $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
  $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
  $p.CloseFigure()
  return $p
}

function Render([int]$size) {
  $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'; $g.PixelOffsetMode = 'HighQuality'; $g.CompositingQuality = 'HighQuality'
  $g.Clear([System.Drawing.Color]::Transparent)
  $s = $size / 64.0
  $small = $size -le 24
  # Graphite squircle
  $bg = New-RoundRect 0 0 $size $size (14 * $s)
  $g.FillPath((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 0x1C, 0x1C, 0x1E))), $bg)
  # Island pill (thicker at tray sizes so it survives 16 px)
  if ($small) { $px = 22; $py = 9; $pw = 20; $ph = 7 } else { $px = 24; $py = 11; $pw = 16; $ph = 5 }
  $pill = New-RoundRect ($px * $s) ($py * $s) ($pw * $s) ($ph * $s) ($ph * $s / 2)
  $g.FillPath((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 0x0A, 0x84, 0xFF))), $pill)
  # W with round caps and joins
  if ($small) { $pts = @(14,25, 23,48, 32,32, 41,48, 50,25); $sw = 8 } else { $pts = @(15,25, 23,47, 32,31, 41,47, 49,25); $sw = 5.5 }
  $points = for ($i = 0; $i -lt $pts.Count; $i += 2) { New-Object System.Drawing.PointF ([single]($pts[$i] * $s)), ([single]($pts[$i + 1] * $s)) }
  $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::White), ([single]($sw * $s))
  $pen.StartCap = 'Round'; $pen.EndCap = 'Round'; $pen.LineJoin = 'Round'
  $g.DrawLines($pen, [System.Drawing.PointF[]]$points)
  $g.Dispose()
  return $bmp
}

function PngBytes($bmp) {
  $ms = New-Object System.IO.MemoryStream
  $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
  return $ms.ToArray()
}

# Classic ICO entry: BITMAPINFOHEADER + bottom-up BGRA pixels + 1-bpp AND mask (all zero:
# the alpha channel carries the transparency). GDI+ and older readers only accept this below 256.
function DibBytes($bmp) {
  $n = $bmp.Width
  $rect = New-Object System.Drawing.Rectangle 0, 0, $n, $n
  $data = $bmp.LockBits($rect, 'ReadOnly', ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb))
  $stride = $data.Stride
  $raw = New-Object byte[] ($stride * $n)
  [System.Runtime.InteropServices.Marshal]::Copy($data.Scan0, $raw, 0, $raw.Length)
  $bmp.UnlockBits($data)
  $ms = New-Object System.IO.MemoryStream
  $w = New-Object System.IO.BinaryWriter $ms
  $w.Write([uint32]40); $w.Write([int32]$n); $w.Write([int32]($n * 2))
  $w.Write([uint16]1); $w.Write([uint16]32); $w.Write([uint32]0)
  $w.Write([uint32]($n * $n * 4)); $w.Write([int32]0); $w.Write([int32]0); $w.Write([uint32]0); $w.Write([uint32]0)
  for ($y = $n - 1; $y -ge 0; $y--) { $w.Write($raw, $y * $stride, $n * 4) }
  $maskRow = [int]([math]::Ceiling($n / 32.0) * 4)
  $w.Write((New-Object byte[] ($maskRow * $n)))
  $w.Flush()
  return $ms.ToArray()
}

New-Item -ItemType Directory -Force $OutDir | Out-Null
$sizes = 16, 20, 24, 32, 40, 48, 64, 128, 256
$entries = @()
foreach ($sz in $sizes) {
  $b = Render $sz
  $bytes = if ($sz -ge 256) { PngBytes $b } else { DibBytes $b }
  $entries += , @($sz, $bytes); $b.Dispose()
}

# ICO: 6-byte header, 16-byte directory entry per image, then the PNG payloads
$ms = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $ms
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$entries.Count)
$offset = 6 + 16 * $entries.Count
foreach ($e in $entries) {
  $sz = $e[0]; $data = $e[1]
  $dim = if ($sz -ge 256) { 0 } else { $sz }
  $w.Write([byte]$dim); $w.Write([byte]$dim); $w.Write([byte]0); $w.Write([byte]0)
  $w.Write([uint16]1); $w.Write([uint16]32)
  $w.Write([uint32]$data.Length); $w.Write([uint32]$offset)
  $offset += $data.Length
}
foreach ($e in $entries) { $w.Write([byte[]]$e[1]) }
$w.Flush()
[IO.File]::WriteAllBytes((Join-Path $OutDir "icon.ico"), $ms.ToArray())

# PNGs for the bundle / README
foreach ($pair in @(@(32, "32x32.png"), @(128, "128x128.png"), @(512, "icon.png"))) {
  $b = Render $pair[0]; $b.Save((Join-Path $OutDir $pair[1]), [System.Drawing.Imaging.ImageFormat]::Png); $b.Dispose()
}
Get-ChildItem $OutDir | Select-Object Name, Length
