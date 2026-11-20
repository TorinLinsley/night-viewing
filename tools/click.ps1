param([string]$Title = "Night Viewing", [double]$Fx = 0.5, [double]$Fy = 0.5, [switch]$Drag, [double]$Fx2 = 0.9, [switch]$ActivateOnly)
$cs = @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class Ms {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  delegate bool EnumProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr p);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool AttachThreadInput(uint a, uint b, bool attach);
  [DllImport("user32.dll")] static extern IntPtr SetFocus(IntPtr h);
  [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
  public static IntPtr Find(string title) {
    IntPtr hit = IntPtr.Zero;
    SetProcessDPIAware();
    EnumWindows(delegate(IntPtr h, IntPtr p) {
      StringBuilder sb = new StringBuilder(512);
      GetWindowTextW(h, sb, 512);
      if (sb.ToString() == title && IsWindowVisible(h)) { hit = h; return false; }
      return true;
    }, IntPtr.Zero);
    return hit;
  }
  public static void Activate(IntPtr h) {
    uint p1; uint p2;
    uint fg = GetWindowThreadProcessId(GetForegroundWindow(), out p1);
    uint tid = GetWindowThreadProcessId(h, out p2);
    AttachThreadInput(fg, tid, true);
    BringWindowToTop(h); SetForegroundWindow(h); SetFocus(h);
    AttachThreadInput(fg, tid, false);
  }
  public static void Click(IntPtr h, double fx, double fy, bool drag, double fx2) {
    RECT r; GetWindowRect(h, out r);
    Activate(h);
    int w = r.R - r.L; int ht = r.B - r.T;
    int x = r.L + (int)(w * fx); int y = r.T + (int)(ht * fy);
    System.Threading.Thread.Sleep(300);
    SetCursorPos(x, y); System.Threading.Thread.Sleep(150);
    mouse_event(0x0002, 0, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(90);
    if (drag) { SetCursorPos(r.L + (int)(w * fx2), y); System.Threading.Thread.Sleep(150); }
    mouse_event(0x0004, 0, 0, 0, IntPtr.Zero);
    Console.WriteLine("clicked rect=" + r.L + "," + r.T + " size=" + w + "x" + ht + " at=" + x + "," + y);
  }
}
"@
Add-Type -TypeDefinition $cs
$h = [Ms]::Find($Title)
if ($h -eq [IntPtr]::Zero) { Write-Output "gui-window-not-found"; exit 1 }
if ($ActivateOnly) { [Ms]::Activate($h); Write-Output "activated" } else { [Ms]::Click($h, $Fx, $Fy, [bool]$Drag, $Fx2) }