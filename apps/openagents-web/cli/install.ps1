# Install Coder. Reimplemented from the public Grok installer workflow and the
# OpenAgents release contract. Rerun this script to update Coder.
#
# Each version publishes coder-VERSION-windows-x86_64.zip, holding coder.exe,
# openagents.exe, microcoder.exe (the engine Coder runs with), and
# coder-boundary.exe, and SHA256SUMS-coder-VERSION. Versions up to 1.0.0-rc.5
# published each executable separately; a sums file without the archive
# selects that layout.
#
#   irm https://openagents.com/cli/install.ps1 | iex
#   $env:CODER_VERSION = '1.0.0'; irm https://openagents.com/cli/install.ps1 | iex
#   $env:CODER_CHANNEL = 'rc'; irm https://openagents.com/cli/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://openagents.com/cli/install.ps1))) -Version 1.0.0
#
# CODER_CHANNEL is stable or rc. Without one, the installer follows stable, and
# rc until a stable release is published.
# CODER_BIN_DIR defaults to %USERPROFILE%\.openagents\bin. CODER_BASE_URL
# overrides the public /coder release prefix. CODER_NO_PATH_UPDATE=1 leaves
# the user PATH unchanged. Windows PowerShell 5.1 and PowerShell 7 are supported.

param(
    [Parameter(Position = 0)]
    [string] $Version,
    [string] $Channel
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

if ($PSVersionTable.Platform -and $PSVersionTable.Platform -ne 'Win32NT') {
    throw 'On macOS and Linux, use: curl -fsSL https://openagents.com/cli/install.sh | bash'
}

$CoderBaseUrl = if ($env:CODER_BASE_URL) { $env:CODER_BASE_URL.TrimEnd('/') } else { 'https://storage.googleapis.com/openagentsgemini-cli-releases/coder' }
$CoderHomeDir = if ($env:USERPROFILE) { $env:USERPROFILE } else { $HOME }
$CoderRoot = if ($env:OPENAGENTS_HOME) { $env:OPENAGENTS_HOME } else { Join-Path $CoderHomeDir '.openagents' }
$CoderBinDir = if ($env:CODER_BIN_DIR) { $env:CODER_BIN_DIR } else { Join-Path $CoderRoot 'bin' }
if (-not $Version) { $Version = $env:CODER_VERSION }
if (-not $Channel) { $Channel = $env:CODER_CHANNEL }
if ($Version -ceq 'stable' -or $Version -ceq 'rc') { $Channel = $Version; $Version = '' }
if ($Channel -and $Channel -cnotmatch '\A(?:stable|rc)\z') { throw "Unknown channel '$Channel'; choose stable or rc." }

function Test-CoderVersion([string] $Value) {
    return $Value -cmatch '\A[0-9]+\.[0-9]+\.[0-9]+(?:-rc\.(?:0|[1-9][0-9]*))?\z'
}

if ($Version -and -not (Test-CoderVersion $Version)) { throw "Not a version: $Version (use X.Y.Z or X.Y.Z-rc.N)." }

# PROCESSOR_ARCHITEW6432 reports the native architecture from 32-bit PowerShell.
$CoderArchitecture = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
switch ($CoderArchitecture) {
    'AMD64' { $CoderPlatform = 'windows-x86_64' }
    'x86' { $CoderPlatform = 'windows-x86_64' }
    'ARM64' {
        $CoderPlatform = 'windows-x86_64'
        Write-Host 'Windows ARM detected; installing the x86_64 compatibility build.'
    }
    default { throw "Unsupported Windows architecture: $CoderArchitecture." }
}

function Get-CoderText([string] $Url) {
    $Response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 60
    if ($Response.Content -is [byte[]]) { return [Text.Encoding]::UTF8.GetString($Response.Content) }
    return $Response.Content.ToString()
}

if (-not $Version -and -not $Channel) {
    # No channel named: stable, or the newest release candidate while no
    # stable release is published.
    $Channel = 'stable'
    try { $Version = (Get-CoderText "$CoderBaseUrl/coder.stable").Trim() }
    catch {
        try {
            $Version = (Get-CoderText "$CoderBaseUrl/coder.rc").Trim()
            $Channel = 'rc'
            Write-Host 'No stable release is published yet; installing the release candidate.'
        }
        catch { $Version = '' }
    }
    if (-not $Version) { throw "Could not read coder.stable from $CoderBaseUrl. Check your connection or set CODER_VERSION." }
}
if (-not $Version) {
    try { $Version = (Get-CoderText "$CoderBaseUrl/coder.$Channel").Trim() }
    catch { throw "Could not read coder.$Channel from $CoderBaseUrl. Check your connection or set CODER_VERSION." }
}
if (-not (Test-CoderVersion $Version)) { throw "The channel does not name a valid version: $Version." }

New-Item -ItemType Directory -Force -Path $CoderBinDir | Out-Null
$CoderWork = Join-Path $CoderBinDir ('.coder-install.' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $CoderWork | Out-Null
$CoderInstalls = @(
    @{ Name = 'coder'; VersionCheck = $true },
    @{ Name = 'openagents'; VersionCheck = $true },
    @{ Name = 'microcoder'; VersionCheck = $true },
    @{ Name = 'coder-boundary'; VersionCheck = $false }
)
$CoderChanged = New-Object System.Collections.ArrayList
$CoderKeepWork = $false

try {
    Write-Host "Installing Coder $Version for $CoderPlatform..."
    $CoderSumsPath = Join-Path $CoderWork 'SHA256SUMS'
    try { Invoke-WebRequest -Uri "$CoderBaseUrl/SHA256SUMS-coder-$Version" -OutFile $CoderSumsPath -UseBasicParsing -TimeoutSec 60 }
    catch { throw "Version $Version is not published, or $CoderBaseUrl is unreachable." }
    $CoderSums = @{}
    foreach ($CoderLine in Get-Content -LiteralPath $CoderSumsPath) {
        $CoderEntry = [regex]::Match($CoderLine, '\A([A-Fa-f0-9]{64})[ \t]+\*?(\S+)\z')
        if ($CoderEntry.Success) {
            $CoderName = $CoderEntry.Groups[2].Value
            if ($CoderSums.ContainsKey($CoderName)) { throw "Duplicate checksum entry: $CoderName." }
            $CoderSums[$CoderName] = $CoderEntry.Groups[1].Value.ToLowerInvariant()
        }
    }
    $CoderArchive = "coder-$Version-$CoderPlatform.zip"
    if ($CoderSums.ContainsKey($CoderArchive)) {
        $CoderArchivePath = Join-Path $CoderWork 'archive.zip'
        try { Invoke-WebRequest -Uri "$CoderBaseUrl/$CoderArchive" -OutFile $CoderArchivePath -UseBasicParsing -TimeoutSec 600 }
        catch { throw "Could not download $CoderArchive. Rerun the installer to retry." }
        $CoderActual = (Get-FileHash -Algorithm SHA256 -LiteralPath $CoderArchivePath).Hash.ToLowerInvariant()
        if ($CoderActual -cne $CoderSums[$CoderArchive]) { throw "Checksum mismatch for $CoderArchive. The existing installation is unchanged." }
        Write-Host "  Verified $CoderArchive."
        $CoderUnpacked = Join-Path $CoderWork 'unpacked'
        Expand-Archive -LiteralPath $CoderArchivePath -DestinationPath $CoderUnpacked
        foreach ($CoderInstall in $CoderInstalls) {
            $CoderFile = Join-Path $CoderUnpacked "$($CoderInstall.Name).exe"
            if (-not (Test-Path -LiteralPath $CoderFile -PathType Leaf)) { throw "$CoderArchive has no $($CoderInstall.Name).exe. The existing installation is unchanged." }
            Move-Item -LiteralPath $CoderFile -Destination (Join-Path $CoderWork "$($CoderInstall.Name).exe")
        }
    } else {
        foreach ($CoderInstall in $CoderInstalls) {
            $CoderArtifact = "$($CoderInstall.Name)-$Version-$CoderPlatform.exe"
            $CoderExpected = $CoderSums[$CoderArtifact]
            if (-not $CoderExpected) { throw "Version $Version has no verified $CoderPlatform build." }
            $CoderPath = Join-Path $CoderWork "$($CoderInstall.Name).exe"
            try { Invoke-WebRequest -Uri "$CoderBaseUrl/$CoderArtifact" -OutFile $CoderPath -UseBasicParsing -TimeoutSec 600 }
            catch { throw "Could not download $CoderArtifact. Rerun the installer to retry." }
            $CoderActual = (Get-FileHash -Algorithm SHA256 -LiteralPath $CoderPath).Hash.ToLowerInvariant()
            if ($CoderActual -cne $CoderExpected) { throw "Checksum mismatch for $CoderArtifact. The existing installation is unchanged." }
            Write-Host "  Verified $CoderArtifact."
        }
    }
    foreach ($CoderInstall in $CoderInstalls) {
        if ($CoderInstall.VersionCheck) {
            $CoderPath = Join-Path $CoderWork "$($CoderInstall.Name).exe"
            $CoderVersionOutput = & $CoderPath --version
            if ($LASTEXITCODE -ne 0) { throw "Downloaded $($CoderInstall.Name) does not run. The existing installation is unchanged." }
            $CoderVersionWords = @(($CoderVersionOutput -join "`n").Trim() -split '\s+', 3)
            if ($CoderVersionWords.Count -lt 2 -or $CoderVersionWords[0] -cne $CoderInstall.Name -or $CoderVersionWords[1] -cne $Version) {
                throw "Downloaded $($CoderInstall.Name) does not report version $Version. The existing installation is unchanged."
            }
            if ($CoderInstall.Name -ceq 'coder') { Write-Host "  $CoderVersionOutput" }
        }
    }
    # File.Replace preserves the old destination until the atomic replacement
    # succeeds. A locked executable is refused; close it and rerun this script.
    foreach ($CoderInstall in $CoderInstalls) {
        $CoderCommand = "$($CoderInstall.Name).exe"
        $CoderSource = Join-Path $CoderWork $CoderCommand
        $CoderDestination = Join-Path $CoderBinDir $CoderCommand
        $CoderBackup = Join-Path $CoderWork "$CoderCommand.previous"
        if (Test-Path -LiteralPath $CoderDestination -PathType Container) { throw "$CoderDestination is a directory." }
        $CoderExisted = Test-Path -LiteralPath $CoderDestination
        try {
            if ($CoderExisted) { [IO.File]::Replace($CoderSource, $CoderDestination, $CoderBackup, $true) }
            else { [IO.File]::Move($CoderSource, $CoderDestination) }
        } catch { throw "Could not replace $CoderCommand. Close running copies of Coder and its CLI, then rerun the installer." }
        [void] $CoderChanged.Add(@{ Destination = $CoderDestination; Backup = $CoderBackup; Existed = $CoderExisted })
    }
} catch {
    $CoderFailure = $_
    for ($CoderIndex = $CoderChanged.Count - 1; $CoderIndex -ge 0; $CoderIndex--) {
        $CoderPrevious = $CoderChanged[$CoderIndex]
        try {
            if ($CoderPrevious.Existed) { [IO.File]::Replace($CoderPrevious.Backup, $CoderPrevious.Destination, [NullString]::Value, $true) }
            else { [IO.File]::Delete($CoderPrevious.Destination) }
        } catch {
            $CoderKeepWork = $true
            Write-Warning "Could not restore $($CoderPrevious.Destination); its previous copy remains in $CoderWork."
        }
    }
    throw $CoderFailure
} finally {
    if (-not $CoderKeepWork) { Remove-Item -Recurse -Force -LiteralPath $CoderWork -ErrorAction SilentlyContinue }
}

Write-Host "Installed Coder (coder.exe and openagents.exe) in $CoderBinDir."
if ($env:CODER_NO_PATH_UPDATE -ne '1') {
    $CoderUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $CoderPathEntries = @($CoderUserPath -split ';' | Where-Object { $_ })
    if ($CoderPathEntries -notcontains $CoderBinDir) {
        [Environment]::SetEnvironmentVariable('Path', ((@($CoderBinDir) + $CoderPathEntries) -join ';'), 'User')
        Write-Host 'Added the install directory to your user PATH. Open a new terminal to use it.'
    }
    if (($env:Path -split ';') -notcontains $CoderBinDir) { $env:Path = "$CoderBinDir;$env:Path" }
}
Write-Host 'Run coder from your project directory. Rerun this installer to update Coder.'
