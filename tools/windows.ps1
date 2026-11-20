param([string]$Mode = "dump")
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text; using System.Collections.Generic;
public class D {
  public delegate bool Proc(IntPtr h, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L,T,R,B; }
  [DllImport("user32.dll")] public static extern bool EnumWindows(Proc p, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  public static string Dump(uint target) {
    string res = "";
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid != target) return true;
      var sb = new StringBuilder(512); GetWindowTextW(h, sb, 512);
      RECT w; GetWindowRect(h, out w);
      res += "title='" + sb + "' rect=" + w.L + "," + w.T + " " + (w.R-w.L) + "x" + (w.B-w.T) + "\n";
      if (sb.ToString() == "Night Viewing") SetForegroundWindow(h);
      return true;
    }, IntPtr.Zero);
    return res;
  }
}
"@
$p = (Get-Process night_viewing).Id
[D]::Dump([uint32]$p)
