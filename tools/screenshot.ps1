# Starts OpenGarageMusic on a given view, saves a picture of its window, and closes it.
#   powershell -ExecutionPolicy Bypass -File tools\screenshot.ps1 notes target\notes.png
# With a title, the picture is of the program's window of that title (the MIDI Pilot's).
param([string]$View = "notes", [string]$Out = "target\shot.png", [string]$Build = "release", [int]$Wait = 4, [string]$Title = "")

$root = Split-Path $PSScriptRoot -Parent
Add-Type -ReferencedAssemblies System.Drawing @'
using System; using System.Runtime.InteropServices; using System.Drawing;
public static class OgShot {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
  public static string Grab(IntPtr h, string path) {
    RECT r; if (!GetWindowRect(h, out r)) return "no window";
    int w = r.R - r.L, hh = r.B - r.T; if (w <= 0 || hh <= 0) return "empty";
    using (var bmp = new Bitmap(w, hh)) {
      using (var g = Graphics.FromImage(bmp)) { IntPtr dc = g.GetHdc(); PrintWindow(h, dc, 2); g.ReleaseHdc(dc); }
      bmp.Save(path, System.Drawing.Imaging.ImageFormat.Png);
    }
    return w + "x" + hh;
  }
}
'@
$env:OPENGARAGE_VIEW = $View
# As on a first start: the user's settings, session and plug-ins are left alone.
$env:OPENGARAGE_DEMO = '1'
$p = Start-Process (Join-Path $root "target\$Build\opengaragemusic.exe") -PassThru
Start-Sleep -Seconds $Wait
$p.Refresh()
if ($p.HasExited) { "exited with code $($p.ExitCode)"; exit 1 }
$target = if ([IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path $root $Out }
$window = if ($Title) { [OgShot]::FindWindowW([NullString]::Value, $Title) } else { $p.MainWindowHandle }
[OgShot]::Grab($window, $target)
Stop-Process -Id $p.Id -Force -Confirm:$false
