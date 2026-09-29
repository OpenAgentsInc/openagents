<#
.SYNOPSIS
Package the OpenAgents desktop app for Windows: a signed per-user MSI and a
.zip, from one release build.

.DESCRIPTION
The app is two processes (docs/coder/design/2026-09-29-auto-pairing.md):
the window (OpenAgents.exe, built from openagents-desktop) and the host
(coder.exe host serve). Both, plus the binaries a task needs, install
together in %LOCALAPPDATA%\Programs\OpenAgents (a per-user MSI; no
administrator prompt).

The host starts at sign-in from the per-user Run key, value "OpenAgents",
whose command runs OpenAgents.exe --start-host: that GUI process starts
coder.exe with no console window and exits. The MSI writes the same value
the app writes on first launch, so uninstalling removes it; the app's own
"Stop Coder on this computer" removes it too.

Signing: pass -CertificateThumbprint (or set OPENAGENTS_WINDOWS_CERT_SHA1)
for a code-signing certificate in the current user's store; every .exe
and the MSI are signed with SHA-256 and an RFC 3161 timestamp by signtool.
Without a certificate the packages are built unsigned and the script says
so; -RequireSigning turns that into an error (use it for a release).

-SkipCoder packages the window alone (as SKIP_CODER=1 does for the Mac
bundle): `coder` does not build for Windows yet, because `supervise` owns
a task's processes through Unix process groups. Without coder.exe the app
registers no sign-in entry and shows the host as offline.

Needs: Rust with the x86_64-pc-windows-msvc target, the WiX Toolset v4 or
later (`dotnet tool install --global wix`) for the MSI, and signtool (the
Windows SDK) for signing.

.EXAMPLE
scripts\desktop\package-windows.ps1 -CertificateThumbprint 0123...ABCD -RequireSigning
#>
[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$OutDir = "",
    [switch]$SkipBuild,
    [switch]$NoMsi,
    [string]$CertificateThumbprint = $env:OPENAGENTS_WINDOWS_CERT_SHA1,
    [string]$TimestampUrl = "http://timestamp.digicert.com",
    [switch]$RequireSigning,
    [switch]$SkipCoder,
    [string]$Target = "x86_64-pc-windows-msvc"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
# Stable for every release: it is how Windows Installer finds the version
# an upgrade replaces. Never change it.
$UpgradeCode = "5B1759D8-E627-4987-8844-1923D7B2D16F"
# Built binary name -> installed name. The window first.
$Binaries = [ordered]@{
    "openagents-desktop.exe" = "OpenAgents.exe"
    "coder.exe"              = "coder.exe"
    "microcoder.exe"         = "microcoder.exe"
}

if ($SkipCoder) {
    $Binaries.Remove("coder.exe")
    $Binaries.Remove("microcoder.exe")
}

function Fail([string]$Message) {
    Write-Error "package-windows: $Message"
    exit 1
}

if (-not $Version) {
    $inPackage = $false
    foreach ($line in Get-Content (Join-Path $Root "Cargo.toml")) {
        if ($line -match '^\[workspace\.package\]') { $inPackage = $true; continue }
        if ($line -match '^\[') { $inPackage = $false }
        if ($inPackage -and $line -match '^version\s*=\s*"([^"]+)"') { $Version = $Matches[1]; break }
    }
}
# Windows Installer versions are three or four numeric fields, each small.
if ($Version -notmatch '^(\d{1,3})\.(\d{1,3})\.(\d{1,5})$') {
    Fail "version '$Version' must be MAJOR.MINOR.PATCH (numbers only) for an MSI"
}

$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root "target" }
if (-not $OutDir) { $OutDir = Join-Path $TargetDir "desktop\windows" }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path
$Work = Join-Path ([IO.Path]::GetTempPath()) ("openagents-package-" + [Guid]::NewGuid().ToString("N"))
$Stage = Join-Path $Work "OpenAgents"
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

try {
    if (-not $SkipBuild) {
        $cargoArgs = @("build", "--release", "--locked", "--target", $Target,
            "--manifest-path", (Join-Path $Root "Cargo.toml"),
            "-p", "openagents-desktop")
        if (-not $SkipCoder) { $cargoArgs += @("-p", "coder", "-p", "microcoder") }
        foreach ($exe in $Binaries.Keys) { $cargoArgs += @("--bin", [IO.Path]::GetFileNameWithoutExtension($exe)) }
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) { Fail "cargo build failed" }
    }

    foreach ($entry in $Binaries.GetEnumerator()) {
        $src = Join-Path $TargetDir "$Target\release\$($entry.Key)"
        if (-not (Test-Path $src)) { Fail "missing $src (build it or drop -SkipBuild)" }
        Copy-Item $src (Join-Path $Stage $entry.Value)
    }

    # --- signing ---------------------------------------------------------
    $signtool = $null
    if ($CertificateThumbprint) {
        if ($CertificateThumbprint -notmatch '^[0-9A-Fa-f]{40}$') { Fail "the certificate thumbprint must be 40 hex digits" }
        $signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
        if (-not $signtool) {
            $kits = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
            $signtool = Get-ChildItem $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
                Where-Object { $_.FullName -match '\\x64\\' } | Sort-Object FullName -Descending |
                Select-Object -First 1 -ExpandProperty FullName
        }
        if (-not $signtool) { Fail "signtool.exe not found; install the Windows SDK" }
    } elseif ($RequireSigning) {
        Fail "-RequireSigning needs -CertificateThumbprint or OPENAGENTS_WINDOWS_CERT_SHA1"
    } else {
        Write-Warning "package-windows: no certificate; building UNSIGNED packages (not for release)"
    }

    function Sign([string[]]$Files) {
        if (-not $signtool) { return }
        & $signtool sign /sha1 $CertificateThumbprint /fd SHA256 /tr $TimestampUrl /td SHA256 /d "OpenAgents" @Files
        if ($LASTEXITCODE -ne 0) { Fail "signtool failed" }
        & $signtool verify /pa @Files
        if ($LASTEXITCODE -ne 0) { Fail "signature verification failed" }
    }

    Sign (Get-ChildItem $Stage -Filter *.exe | ForEach-Object FullName)

    $artifacts = @()

    # --- .zip ------------------------------------------------------------
    $zip = Join-Path $OutDir "OpenAgents-$Version-windows-x64.zip"
    if (Test-Path $zip) { Remove-Item $zip }
    Compress-Archive -Path $Stage -DestinationPath $zip
    $artifacts += $zip

    # --- MSI -------------------------------------------------------------
    if (-not $NoMsi) {
        $wix = (Get-Command wix -ErrorAction SilentlyContinue).Source
        if (-not $wix) { Fail "the WiX Toolset is not on PATH (dotnet tool install --global wix), or pass -NoMsi" }
        $files = ""
        foreach ($name in $Binaries.Values) {
            $id = "File_" + ($name -replace '[^A-Za-z0-9]', '_')
            $keyPath = if ($name -eq "OpenAgents.exe") { ' KeyPath="yes"' } else { "" }
            $files += "        <File Id=`"$id`" Source=`"$(Join-Path $Stage $name)`"$keyPath />`n"
        }
        # Start the host at sign-in: the same value the app writes, owned by
        # the MSI so an uninstall removes it. Only with the host installed.
        $runEntry = ""
        if (-not $SkipCoder) {
            $runEntry = @'
      <Component Id="StartAtSignIn">
        <RegistryValue Root="HKCU" Key="Software\Microsoft\Windows\CurrentVersion\Run"
                       Name="OpenAgents" Type="string"
                       Value="&quot;[INSTALLFOLDER]OpenAgents.exe&quot; --start-host" KeyPath="yes" />
      </Component>

'@
        }
        $wxs = @"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="OpenAgents" Manufacturer="OpenAgents" Version="$Version"
           UpgradeCode="$UpgradeCode" Scope="perUser" Compressed="yes" Language="1033">
    <SummaryInformation Description="OpenAgents: connect your phone to this computer" />
    <MajorUpgrade DowngradeErrorMessage="A newer version of OpenAgents is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <StandardDirectory Id="ProgramFiles6432Folder">
      <Directory Id="INSTALLFOLDER" Name="OpenAgents" />
    </StandardDirectory>
    <StandardDirectory Id="ProgramMenuFolder" />
    <ComponentGroup Id="App" Directory="INSTALLFOLDER">
      <Component Id="Binaries">
$files      </Component>
$runEntry      <Component Id="StartMenu" Directory="ProgramMenuFolder">
        <Shortcut Id="StartMenuShortcut" Name="OpenAgents" Target="[INSTALLFOLDER]OpenAgents.exe"
                  WorkingDirectory="INSTALLFOLDER" />
        <RegistryValue Root="HKCU" Key="Software\OpenAgents\Desktop" Name="StartMenuShortcut"
                       Type="integer" Value="1" KeyPath="yes" />
      </Component>
    </ComponentGroup>
    <Feature Id="Main">
      <ComponentGroupRef Id="App" />
    </Feature>
    <!-- Open the app after install, so the code shows without another step. -->
    <CustomAction Id="LaunchApp" Directory="INSTALLFOLDER"
                  ExeCommand="&quot;[INSTALLFOLDER]OpenAgents.exe&quot;" Return="asyncNoWait" />
    <InstallExecuteSequence>
      <Custom Action="LaunchApp" After="InstallFinalize" Condition="NOT Installed AND NOT REMOVE AND UILevel &gt;= 2" />
    </InstallExecuteSequence>
  </Package>
</Wix>
"@
        $wxsPath = Join-Path $Work "OpenAgents.wxs"
        Set-Content -Path $wxsPath -Value $wxs -Encoding UTF8
        $msi = Join-Path $OutDir "OpenAgents-$Version-x64.msi"
        & $wix build -arch x64 -o $msi $wxsPath
        if ($LASTEXITCODE -ne 0) { Fail "wix build failed" }
        Sign @($msi)
        $artifacts += $msi
    }

    $sums = foreach ($a in $artifacts) {
        "{0}  {1}" -f (Get-FileHash -Algorithm SHA256 $a).Hash.ToLowerInvariant(), (Split-Path $a -Leaf)
    }
    Set-Content -Path (Join-Path $OutDir "SHA256SUMS") -Value $sums -Encoding ASCII
    foreach ($a in $artifacts) { Write-Host "package-windows: wrote $a" }
    Write-Host "package-windows: wrote $(Join-Path $OutDir 'SHA256SUMS')"
}
finally {
    Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
}
