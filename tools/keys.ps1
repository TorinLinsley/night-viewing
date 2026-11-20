param([string]$Spec = "ctrl+alt+up:3")
# 用 SendInput 以真实按键方式触发全局热键
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Kb {
  [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Explicit, Size=40)] public struct INPUT { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KEYBDINPUT ki; }
  [DllImport("user32.dll", SetLastError=true)] public static extern uint SendInput(uint n, INPUT[] p, int cb);
}
"@
$map = @{ "ctrl"=0x11; "alt"=0x12; "shift"=0x10; "up"=0x26; "down"=0x28; "0"=0x30; "n"=0x4E; "1"=0x31; "2"=0x32; "3"=0x33; "4"=0x34; "5"=0x35; "6"=0x36; "7"=0x37; "8"=0x38; "9"=0x39; "tab"=0x09; "left"=0x25; "right"=0x27; "enter"=0x0D; "esc"=0x1B; "back"=0x08 }
$KEYUP = 0x0002
foreach ($chunk in $Spec.Split(",")) {
  $parts = $chunk.Split(":")
  $seq = $parts[0].Split("+")
  $times = if ($parts.Length -gt 1) { [int]$parts[1] } else { 1 }
  for ($i = 0; $i -lt $times; $i++) {
    $inputs = New-Object 'Kb+INPUT[]' ($seq.Count * 2)
    $k = 0
    foreach ($s in $seq) { $inputs[$k].type = 1; $inputs[$k].ki.wVk = [uint16]$map[$s.ToLower()]; $k++ }
    foreach ($s in ($seq | Select-Object -Last 0)) { }
    for ($j = $seq.Count - 1; $j -ge 0; $j--) { $inputs[$k].type = 1; $inputs[$k].ki.wVk = [uint16]$map[$seq[$j].ToLower()]; $inputs[$k].ki.dwFlags = $KEYUP; $k++ }
    [void][Kb]::SendInput([uint32]$inputs.Count, $inputs, [System.Runtime.InteropServices.Marshal]::SizeOf([type][Kb+INPUT]) * $inputs.Count)
    Start-Sleep -Milliseconds 90
  }
  Start-Sleep -Milliseconds 120
}
Write-Output "keys-done"
