param([int]$Delta = -240, [int]$Times = 3)
Add-Type @'
using System;using System.Text;using System.Runtime.InteropServices;
public class Ms2 {
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int d, IntPtr e);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 public struct RECT { public int l,t,r,b; }
 public static IntPtr Find(string title) { IntPtr res = IntPtr.Zero;
   EnumWindows((h,l) => { var sb = new StringBuilder(256); GetWindowTextW(h, sb, 256); if (sb.ToString() == title) { res = h; return false; } return true; }, IntPtr.Zero);
   return res; }
}
'@
$h = [Ms2]::Find("Night Viewing")
if ($h -eq [IntPtr]::Zero) { "no window"; exit }
$r = New-Object Ms2+RECT; [Ms2]::GetWindowRect($h, [ref]$r) | Out-Null
"at=$($r.l),$($r.t) $($r.r-$r.l)x$($r.b-$r.t)"
[Ms2]::SetForegroundWindow($h) | Out-Null
[Ms2]::SetCursorPos([int](($r.l + $r.r)/2), [int](($r.t + $r.b)/2)) | Out-Null
Start-Sleep -Milliseconds 250
for ($i=0; $i -lt $Times; $i++) { [Ms2]::mouse_event(0x0800, 0, 0, $Delta, [IntPtr]::Zero); Start-Sleep -Milliseconds 140 }
"wheeled"