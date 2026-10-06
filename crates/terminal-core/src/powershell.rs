//! PowerShell and PSReadLine hooks. Explicit `# ` requests keep the native editor.
//! Shell directory marks are advisory; the owner reads the process directory separately.

/// The startup script runs after PowerShell's normal profiles, without changing them.
/// PSReadLine is required; otherwise the shell keeps its ordinary prompt and editor.
pub const HOOK: &str = r#"
$openagentsHistory = $null
if ($env:OPENAGENTS_POWERSHELL_PROFILE) {
    $openagentsProfile = $env:OPENAGENTS_POWERSHELL_PROFILE
    $openagentsHistory = Join-Path (Split-Path -Parent $openagentsProfile) 'PSReadLineHistory.txt'
    Remove-Item Env:OPENAGENTS_POWERSHELL_PROFILE
    if (Test-Path -LiteralPath $openagentsProfile -PathType Leaf) { . $openagentsProfile }
    Remove-Variable openagentsProfile
}
$openagentsIntegrated = Get-Variable -Name _openagents_integrated -Scope Global -ErrorAction SilentlyContinue
if (-not ($openagentsIntegrated -and $openagentsIntegrated.Value) -and $Host.Name -eq 'ConsoleHost') {
    try { Import-Module PSReadLine -ErrorAction Stop } catch { return }
    if ($openagentsHistory) { Set-PSReadLineOption -HistorySavePath $openagentsHistory }
    $global:_openagents_integrated = $true
    $global:_openagents_running = $false
    $global:_openagents_native = $false
    $global:_openagents_prompt = (Get-Command prompt -CommandType Function).ScriptBlock
    $global:_openagents_escape = [char]27
    $global:_openagents_bell = [char]7
    function global:_openagents_hex([string]$value) {
        $bytes = [Text.Encoding]::UTF8.GetBytes($value)
        if ($bytes.Length -gt 8192) { return $null }
        return [BitConverter]::ToString($bytes).Replace('-', '').ToLowerInvariant()
    }
    function global:_openagents_mark([string]$name, [string]$value) {
        $hex = _openagents_hex $value
        if ($null -ne $hex) {
            [Console]::Write("$global:_openagents_escape]777;openagents;$name;$hex$global:_openagents_bell")
        }
    }
    function global:_openagents_directory {
        if ($PWD.Provider.Name -ne 'FileSystem') { return $false }
        try {
            $path = $PWD.ProviderPath
            if ([Text.Encoding]::UTF8.GetByteCount($path) -gt 8192) { return $false }
            $uri = [Uri]::new($path).AbsoluteUri
            if ([Text.Encoding]::UTF8.GetByteCount($uri) -gt 8192) { return $false }
            [Environment]::CurrentDirectory = $path
            [Console]::Write("$global:_openagents_escape]7;$uri$global:_openagents_bell")
            return $true
        } catch { return $false }
    }
    function global:prompt {
        $succeeded = $?
        $exitCode = if ($succeeded) { 0 } else { 1 }
        $nativeExit = Get-Variable -Name LASTEXITCODE -Scope Global -ErrorAction SilentlyContinue
        if ($global:_openagents_native -and $nativeExit -and $null -ne $nativeExit.Value) { $exitCode = $nativeExit.Value }
        if ($global:_openagents_running) {
            [Console]::Write("$global:_openagents_escape]133;D;$exitCode$global:_openagents_bell")
        }
        $global:_openagents_running = $false
        $global:_openagents_native = $false
        $rendered = (& $global:_openagents_prompt | Out-String).TrimEnd([char[]]"`r`n")
        # A custom prompt can change providers or locations; synchronize after it returns.
        $ready = _openagents_directory
        if (-not $ready) {
            # Clear prior prompt eligibility; provider locations are not filesystem authority.
            return "$global:_openagents_escape]133;C$global:_openagents_bell$rendered"
        }
        return "$global:_openagents_escape]133;A$global:_openagents_bell$rendered$global:_openagents_escape]133;B$global:_openagents_bell"
    }
    Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
        param($key, $argument)
        $line = ''; $cursor = 0
        [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
        $ready = _openagents_directory
        if ($ready -and $line.StartsWith('# ') -and -not [string]::IsNullOrWhiteSpace($line.Substring(2)) -and $line.Substring(2) -notmatch '[\x00-\x1f\x7f]' -and $null -ne (_openagents_hex ($line.Substring(2)))) {
            _openagents_mark 'request' ($line.Substring(2))
            [void][Microsoft.PowerShell.PSConsoleReadLine]::AddToHistory($line)
            [Microsoft.PowerShell.PSConsoleReadLine]::Replace(0, $line.Length, '')
            [Microsoft.PowerShell.PSConsoleReadLine]::InvokePrompt()
            return
        }
        if ($ready -and -not [string]::IsNullOrWhiteSpace($line)) {
            _openagents_mark 'command' $line
            $tokens = $null; $errors = $null
            $ast = [System.Management.Automation.Language.Parser]::ParseInput($line, [ref]$tokens, [ref]$errors)
            $command = $ast.Find({ param($node) $node -is [System.Management.Automation.Language.CommandAst] }, $true)
            $name = if ($null -ne $command) { $command.GetCommandName() }
            $resolved = if (-not [string]::IsNullOrEmpty($name)) { Get-Command -Name $name -ErrorAction SilentlyContinue | Select-Object -First 1 }
            $global:_openagents_native = $null -ne $resolved -and $resolved.CommandType -eq 'Application'
            [Console]::Write("$global:_openagents_escape]133;C$global:_openagents_bell")
            $global:_openagents_running = $true
        }
        [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
    }
}
"#;
