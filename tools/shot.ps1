param([string]$Out = "$env:TEMP\nv_shot.png")
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Scr {
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
"@
[Scr]::SetProcessDPIAware() | Out-Null
$w = [Scr]::GetSystemMetrics(0); $h = [Scr]::GetSystemMetrics(1)
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen(0, 0, 0, 0, $bmp.Size)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output ("{0} {1}x{2}" -f $Out, $w, $h)
