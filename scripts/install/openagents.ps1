# Install OpenAgents Terminal on Windows: openagents.exe, and the
# microcoder.exe engine it runs Coder with.
#
#   irm https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.ps1 | iex
#
# Environment:
#   OPENAGENTS_VERSION         A version to install, such as 1.0.0-rc.1.
#   OPENAGENTS_CHANNEL         The channel to follow: stable or rc. Unset, it
#                              follows stable, and rc while no stable release
#                              exists yet.
#   OPENAGENTS_BIN_DIR         Default %USERPROFILE%\.openagents\bin.
#   OPENAGENTS_BASE_URL        Where releases are read from.
#   OPENAGENTS_NO_PATH_UPDATE  1 leaves the user PATH alone.
#   OPENAGENTS_NO_LAUNCH       1 does not start OpenAgents Terminal afterwards.
#
# It reads the same files scripts/install/openagents.sh reads and installs
# nothing whose SHA-256 does not match SHA256SUMS-openagents-<version>.
# Windows PowerShell 5.1 works. It throws on failure rather than calling exit,
# which would close a window that runs it through iex.
#
# On Windows, `openagents connect`, `labor`, `service`, `ssh`, `wallet`, and
# `x402` answer that they need macOS or Linux; the terminal and chat work.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$BaseUrl = if ($env:OPENAGENTS_BASE_URL) { $env:OPENAGENTS_BASE_URL.TrimEnd('/') } else { 'https://storage.googleapis.com/openagentsgemini-cli-releases/openagents' }
$HomeDir = if ($env:USERPROFILE) { $env:USERPROFILE } else { $HOME }
$BinDir = if ($env:OPENAGENTS_BIN_DIR) { $env:OPENAGENTS_BIN_DIR } else { Join-Path $HomeDir '.openagents\bin' }
$Product = 'openagents'
$Engine = 'microcoder'
# Windows on ARM runs the x86_64 build.
$Platform = 'windows-x86_64'

function Test-OpenAgentsVersion([string] $Value) {
    return $Value -cmatch '\A[0-9]+\.[0-9]+\.[0-9]+(?:-rc\.(?:0|[1-9][0-9]*))?\z'
}

function Get-Pointer([string] $Name) {
    try {
        return (Invoke-RestMethod -Uri "$BaseUrl/$Product.$Name" -TimeoutSec 30).ToString().Trim()
    } catch {
        return ''
    }
}

if ($env:OPENAGENTS_VERSION) {
    $Version = $env:OPENAGENTS_VERSION
    if (-not (Test-OpenAgentsVersion $Version)) { throw "Not a version: $Version" }
} else {
    $Channel = $env:OPENAGENTS_CHANNEL
    if (-not $Channel) {
        $Channel = 'stable'
        $Version = Get-Pointer 'stable'
        if (-not $Version) {
            $Channel = 'rc'
            $Version = Get-Pointer 'rc'
            if ($Version) { Write-Host 'No stable release yet; following the rc channel.' }
        }
    } else {
        if ($Channel -cnotmatch '\A[A-Za-z0-9_-]+\z') { throw "Not a channel name: $Channel" }
        $Version = Get-Pointer $Channel
    }
    if (-not $Version) { throw "Could not read the '$Channel' channel from $BaseUrl/$Product.$Channel. Check your connection, or set OPENAGENTS_VERSION." }
    if (-not (Test-OpenAgentsVersion $Version)) { throw "The '$Channel' channel names something that is not a version: $Version" }
}

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
$Work = Join-Path $BinDir (".install." + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $Work | Out-Null

try {
    Write-Host "Installing OpenAgents Terminal $Version for $Platform..."
    $SumsPath = Join-Path $Work 'SHA256SUMS'
    try {
        Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS-$Product-$Version" -OutFile $SumsPath -UseBasicParsing -TimeoutSec 60
    } catch {
        throw "Version $Version is not published (no SHA256SUMS-$Product-$Version), or $BaseUrl is unreachable."
    }
    $Sums = @{}
    foreach ($Line in Get-Content -LiteralPath $SumsPath) {
        $Entry = [regex]::Match($Line, '\A([A-Fa-f0-9]{64})[ \t]+\*?(\S+)\z')
        if ($Entry.Success) { $Sums[$Entry.Groups[2].Value] = $Entry.Groups[1].Value.ToLowerInvariant() }
    }
    $Installs = @(
        @{ Name = $Product; Command = 'openagents.exe' },
        @{ Name = $Engine; Command = 'microcoder.exe' }
    )
    foreach ($Install in $Installs) {
        $Artifact = "$($Install.Name)-$Version-$Platform"
        # The sums file names a Windows artifact with .exe; its URL has none.
        $Expected = $Sums["$Artifact.exe"]
        if (-not $Expected) { throw "Version $Version has no $Platform build ($Artifact.exe is not in its checksum file)." }
        $Path = Join-Path $Work $Install.Command
        try {
            Invoke-WebRequest -Uri "$BaseUrl/$Artifact" -OutFile $Path -UseBasicParsing -TimeoutSec 600
        } catch {
            throw "Could not download $BaseUrl/$Artifact, which version $Version lists. Check your connection and run this again."
        }
        $Actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
        if ($Actual -cne $Expected) { throw "Checksum mismatch for ${Artifact}: expected $Expected, got $Actual. Nothing was installed." }
        Write-Host "  Verified $Artifact (sha256 $Actual)."
    }
    # Both are verified before either replaces what is there.
    foreach ($Install in $Installs) {
        Move-Item -Force -LiteralPath (Join-Path $Work $Install.Command) -Destination (Join-Path $BinDir $Install.Command)
    }
} finally {
    Remove-Item -Recurse -Force -LiteralPath $Work -ErrorAction SilentlyContinue
}

$Command = Join-Path $BinDir 'openagents.exe'
Write-Host "  Installed $Command and $(Join-Path $BinDir 'microcoder.exe')"
if ($env:OPENAGENTS_NO_PATH_UPDATE -ne '1') {
    $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not $UserPath) { $UserPath = '' }
    if (($UserPath -split ';') -notcontains $BinDir) {
        [Environment]::SetEnvironmentVariable('Path', ($BinDir + ';' + $UserPath).TrimEnd(';'), 'User')
        $env:Path = "$BinDir;$env:Path"
        Write-Host "  Added $BinDir to your PATH. Open a new terminal to pick it up."
    }
}
Write-Host 'Run it with: openagents'
if ($env:OPENAGENTS_NO_LAUNCH -ne '1') {
    Write-Host 'Starting OpenAgents Terminal...'
    try { & $Command terminal } catch { Write-Warning "OpenAgents is installed, but could not start: $($_.Exception.Message)" }
}
