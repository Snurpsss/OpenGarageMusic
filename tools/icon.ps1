# Makes the program's icon from a picture of a guitar on a plain or chequered
# grey background: the background is taken out, the guitar is put on a dark
# tile with rounded corners, and the tile is written at each size the program
# and Windows use, into apps\opengarage\icon.
#   powershell -ExecutionPolicy Bypass -File tools\icon.ps1 path\to\picture.webp
# The picture is read with the codecs of Windows (PNG, JPEG, WebP...).
param([Parameter(Mandatory = $true)][string]$Picture)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root 'apps\opengarage\icon'
New-Item -ItemType Directory -Force $out | Out-Null
Add-Type -AssemblyName PresentationCore, WindowsBase, System.Drawing
Add-Type -ReferencedAssemblies System.Drawing @'
using System; using System.Collections.Generic; using System.Drawing; using System.Drawing.Drawing2D; using System.Drawing.Imaging; using System.Runtime.InteropServices;
public static class OgIcon {
  // A pixel of the background: a grey, neither dark (the outline of the drawing) nor white.
  static bool Grey(byte[] p, int o) {
    int b = p[o], g = p[o + 1], r = p[o + 2];
    int max = Math.Max(r, Math.Max(g, b)), min = Math.Min(r, Math.Min(g, b));
    return max - min <= 16 && max >= 100 && max <= 228;
  }
  // Makes see-through every grey pixel that can be reached from the edges of
  // the picture; the part of the top edge between skipFrom and skipTo (the
  // neck, which leaves the picture there) is not a way in.
  public static int Cut(byte[] p, int w, int h, int skipFrom, int skipTo) {
    var seen = new bool[w * h];
    var todo = new Stack<int>();
    Action<int, int> push = (x, y) => {
      if (x < 0 || y < 0 || x >= w || y >= h) return;
      int i = y * w + x;
      if (seen[i] || !Grey(p, i * 4)) return;
      seen[i] = true; todo.Push(i);
    };
    for (int x = 0; x < w; x++) { if (x < skipFrom || x > skipTo) push(x, 0); push(x, h - 1); }
    for (int y = 0; y < h; y++) { push(0, y); push(w - 1, y); }
    int cut = 0;
    while (todo.Count > 0) {
      int i = todo.Pop(); int x = i % w, y = i / w;
      cut++;
      push(x + 1, y); push(x - 1, y); push(x, y + 1); push(x, y - 1);
    }
    // The pixels next to what was cut are half grey: they go too, twice over.
    for (int pass = 0; pass < 2; pass++) {
      var edge = new List<int>();
      for (int y = 1; y < h - 1; y++) for (int x = 1; x < w - 1; x++) {
        int i = y * w + x;
        if (seen[i]) continue;
        if (!(seen[i - 1] || seen[i + 1] || seen[i - w] || seen[i + w])) continue;
        int o = i * 4; int max = Math.Max(p[o], Math.Max(p[o + 1], p[o + 2])), min = Math.Min(p[o], Math.Min(p[o + 1], p[o + 2]));
        if (max - min <= 40 && max >= 70) edge.Add(i);
      }
      foreach (int i in edge) seen[i] = true;
    }
    for (int i = 0; i < w * h; i++) if (seen[i]) { p[i * 4] = 0; p[i * 4 + 1] = 0; p[i * 4 + 2] = 0; p[i * 4 + 3] = 0; }
    return cut;
  }
  public static Bitmap FromBgra(byte[] p, int w, int h) {
    var bmp = new Bitmap(w, h, PixelFormat.Format32bppArgb);
    var data = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
    Marshal.Copy(p, 0, data.Scan0, p.Length);
    bmp.UnlockBits(data);
    return bmp;
  }
  // The tile: a dark rounded square, lighter at the top, with the guitar on it.
  public static Bitmap Tile(Bitmap guitar, int size) {
    var tile = new Bitmap(size, size, PixelFormat.Format32bppArgb);
    using (var g = Graphics.FromImage(tile)) {
      g.SmoothingMode = SmoothingMode.AntiAlias; g.InterpolationMode = InterpolationMode.HighQualityBicubic; g.PixelOffsetMode = PixelOffsetMode.HighQuality;
      g.Clear(Color.Transparent);
      float m = size * 0.015f, d = size * 0.40f, s = size - 2 * m;
      using (var path = new GraphicsPath()) {
        path.AddArc(m, m, d, d, 180, 90); path.AddArc(m + s - d, m, d, d, 270, 90);
        path.AddArc(m + s - d, m + s - d, d, d, 0, 90); path.AddArc(m, m + s - d, d, d, 90, 90); path.CloseFigure();
        using (var brush = new LinearGradientBrush(new PointF(0, 0), new PointF(0, size), Color.FromArgb(255, 74, 76, 84), Color.FromArgb(255, 24, 24, 29))) g.FillPath(brush, path);
        g.SetClip(path);
        float inset = size * 0.035f;
        g.DrawImage(guitar, new RectangleF(inset, inset * 0.2f, size - 2 * inset, size - 2 * inset), new RectangleF(0, 0, guitar.Width, guitar.Height), GraphicsUnit.Pixel);
      }
    }
    return tile;
  }
  // Red, green, blue, opacity, row by row: what the program reads.
  public static byte[] Rgba(Bitmap bmp) {
    int w = bmp.Width, h = bmp.Height; var raw = new byte[w * h * 4];
    var data = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
    Marshal.Copy(data.Scan0, raw, 0, raw.Length); bmp.UnlockBits(data);
    for (int i = 0; i < raw.Length; i += 4) { byte b = raw[i]; raw[i] = raw[i + 2]; raw[i + 2] = b; }
    return raw;
  }
}
'@
$stream = [IO.File]::OpenRead((Resolve-Path $Picture))
$frame = [Windows.Media.Imaging.BitmapDecoder]::Create($stream, 'PreservePixelFormat', 'OnLoad').Frames[0]
$bgra = New-Object Windows.Media.Imaging.FormatConvertedBitmap $frame, ([Windows.Media.PixelFormats]::Bgra32), $null, 0
$w = $frame.PixelWidth; $h = $frame.PixelHeight
$pixels = New-Object byte[] (4 * $w * $h)
$bgra.CopyPixels($pixels, 4 * $w, 0)
$stream.Close()
# The neck leaves the picture through the middle of its top edge.
$cut = [OgIcon]::Cut($pixels, $w, $h, [int]($w * 0.40), [int]($w * 0.60))
"background: $cut of $($w * $h) pixels taken out"
$guitar = [OgIcon]::FromBgra($pixels, $w, $h)
$big = [OgIcon]::Tile($guitar, 1024)
$big.Save((Join-Path $out 'icon.png'), [Drawing.Imaging.ImageFormat]::Png)
foreach ($size in 16, 24, 32, 48, 64, 256) {
    $small = New-Object Drawing.Bitmap $size, $size, ([Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [Drawing.Graphics]::FromImage($small)
    $g.InterpolationMode = 'HighQualityBicubic'; $g.PixelOffsetMode = 'HighQuality'; $g.CompositingQuality = 'HighQuality'
    $g.DrawImage($big, 0, 0, $size, $size)
    $g.Dispose()
    [IO.File]::WriteAllBytes((Join-Path $out "$size.rgba"), [OgIcon]::Rgba($small))
    "$size.rgba"
}
