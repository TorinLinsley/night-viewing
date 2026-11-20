param([string]$Out = "$env:TEMP\nv_win.png")
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;
public class W {
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 public struct RECT { public int l,t,r,b; }
 public static IntPtr Find(string title) { IntPtr res = IntPtr.Zero;
   EnumWindows((h,l) => { var sb = new StringBuilder(256); GetWindowTextW(h, sb, 256); var cb = new StringBuilder(256); GetClassNameW(h, cb, 256); if (cb.ToString() == "NightViewing::Main") { res = h; return false; } return true; }, IntPtr.Zero);
   return res; }
}
'@
[W]::SetProcessDPIAware() | Out-Null
$h = [W]::Find("Night Viewing")
if ($h -eq [IntPtr]::Zero) { "no window"; exit }
$r = New-Object W+RECT; [W]::GetWindowRect($h, [ref]$r) | Out-Null
"rect=$($r.l),$($r.t) $($r.r - $r.l)x$($r.b - $r.t)"
$w = $r.r - $r.l; $hh = $r.b - $r.t
$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.l, $r.t, 0, 0, $bmp.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose(); $g.Dispose()