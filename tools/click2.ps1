param([int]$X, [int]$Y, [string]$Out = "$env:TEMP\nv_click.png")
Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;
public class C {
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 public struct RECT { public int l,t,r,b; }
 public struct POINT { public int x,y; }
 public static IntPtr Find(string title) { IntPtr res = IntPtr.Zero;
   EnumWindows((h,l) => { var sb = new StringBuilder(256); GetWindowTextW(h, sb, 256); var cb = new StringBuilder(256); GetClassNameW(h, cb, 256); if (cb.ToString() == "NightViewing::Main") { res = h; return false; } return true; }, IntPtr.Zero);
   return res; }
}
'@
[C]::SetProcessDPIAware() | Out-Null
$h = [C]::Find("Night Viewing")
if ($h -eq [IntPtr]::Zero) { "no window"; exit }
$wr = New-Object C+RECT; [C]::GetWindowRect($h, [ref]$wr) | Out-Null
$p = New-Object C+POINT; $p.x = 0; $p.y = 0; [C]::ClientToScreen($h, [ref]$p) | Out-Null
$cx = $X; $cy = $Y
$lp = [IntPtr](($cy -shl 16) -bor ($cx -band 0xFFFF))
[C]::PostMessage($h, 0x0200, [IntPtr]::Zero, $lp) | Out-Null
[C]::PostMessage($h, 0x0201, [IntPtr]1, $lp) | Out-Null
Start-Sleep -Milliseconds 60
[C]::PostMessage($h, 0x0202, [IntPtr]::Zero, $lp) | Out-Null
"click client=$cx,$cy"
Start-Sleep -Milliseconds 250
Add-Type -AssemblyName System.Drawing
$w = $wr.r - $wr.l; $hh = $wr.b - $wr.t
$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($wr.l, $wr.t, 0, 0, $bmp.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose(); $g.Dispose()
