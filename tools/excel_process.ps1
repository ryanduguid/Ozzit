# Copyright 2026 Ryan Duguid
# SPDX-License-Identifier: Apache-2.0

# The Excel process a script started, and a wait for it to exit after Quit(). Dot-sourced
# by the scripts that drive Excel over COM and refuse to start while any Excel runs.
#
# Quit() returns before Excel exits, and a COM reference PowerShell still holds keeps it
# running without a window. On 16 November 2026 one `ozzit verify_cache` run left such an
# Excel behind, and the native gates after it refused to start. The process is found from
# the Application's window handle, never by the name EXCEL, which would also match the
# user's own Excel.

Add-Type -Namespace OzzitExcel -Name Window -MemberDefinition @'
[DllImport("user32.dll")]
public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
'@

function Get-ExcelProcess($xl) {
    $id = [uint32]0
    [void][OzzitExcel.Window]::GetWindowThreadProcessId([IntPtr]$xl.Hwnd, [ref]$id)
    if ($id -eq 0) { throw 'Excel returned no window to find its process from.' }
    $process = Get-Process -Id $id
    # Open the handle now: the waits below then follow this process, not a later one given its id.
    [void]$process.Handle
    $process
}

# Call after Quit() and after releasing every COM reference to that Excel. A process
# still running after the wait is this script's own, with its workbook closed, so end it.
function Wait-ExcelExit($process, [int]$seconds = 30) {
    if ($null -eq $process -or $process.WaitForExit($seconds * 1000)) { return }
    # Kill() throws if Excel exited after the wait ran out; only a process still running is an error.
    try { $process.Kill() } catch { if (-not $process.HasExited) { throw } }
    $process.WaitForExit()
    Write-Output "Excel (process $($process.Id)) was still running $seconds s after Quit(), so it was ended."
}
