# Internal helper for test-windows-rendering.ps1. Run in a separate process:
# PrintWindow can block indefinitely when the target stops processing messages.
param(
    [Parameter(Mandatory = $true)][long]$WindowHandle,
    [Parameter(Mandatory = $true)][uint32]$ExpectedProcessId,
    [Parameter(Mandatory = $true)][string]$OutputFile,
    [Parameter(Mandatory = $true)][string]$MetadataFile
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OpenAgentsClientCapture {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")]
    public static extern bool GetClientRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr hwnd, IntPtr dc, uint flags);
    [DllImport("user32.dll")]
    public static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")]
    public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
'@
$hwnd = [IntPtr]::new($WindowHandle)
[uint32]$owner = 0
[void][OpenAgentsClientCapture]::GetWindowThreadProcessId($hwnd, [ref]$owner)
if ($owner -ne $ExpectedProcessId -or $owner -eq 0) { throw 'Capture HWND does not belong to the fixture' }
# Read physical client dimensions instead of DPI-virtualized coordinates.
$previousDpi = [OpenAgentsClientCapture]::SetThreadDpiAwarenessContext([IntPtr]::new(-4))
$bitmap = $null
try {
    $rect = [OpenAgentsClientCapture+Rect]::new()
    if (-not [OpenAgentsClientCapture]::GetClientRect($hwnd, [ref]$rect)) { throw 'GetClientRect failed' }
    $dpi = [OpenAgentsClientCapture]::GetDpiForWindow($hwnd)
    if ($dpi -eq 0 -or $rect.Right -le 0 -or $rect.Bottom -le 0) { throw 'Invalid window dimensions/DPI' }
    $bitmap = [Drawing.Bitmap]::new($rect.Right, $rect.Bottom)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $dc = $graphics.GetHdc()
    try {
        # PW_CLIENTONLY | PW_RENDERFULLCONTENT. Never capture the desktop.
        if (-not [OpenAgentsClientCapture]::PrintWindow($hwnd, $dc, 3)) { throw 'PrintWindow failed' }
    } finally { $graphics.ReleaseHdc($dc); $graphics.Dispose() }
    $bitmap.Save($OutputFile, [Drawing.Imaging.ImageFormat]::Png)
    @{ dpi = $dpi; width = $bitmap.Width; height = $bitmap.Height } |
        ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath $MetadataFile
} finally {
    if ($null -ne $bitmap) { $bitmap.Dispose() }
    if ($previousDpi -ne [IntPtr]::Zero) { [void][OpenAgentsClientCapture]::SetThreadDpiAwarenessContext($previousDpi) }
}
