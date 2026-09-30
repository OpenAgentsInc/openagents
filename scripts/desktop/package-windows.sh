#!/usr/bin/env bash
# Packages OpenAgents for Windows from a Mac or Linux computer: the same
# per-user MSI and .zip as scripts/desktop/package-windows.ps1, cross-built
# for x86_64-pc-windows-gnu and put together with wixl (msitools), so a
# release needs no Windows computer. Usage:
#
#   scripts/desktop/package-windows.sh [--version V] [--out DIR]
#       [--bin-dir DIR] [--pfx FILE --pfx-password-file FILE]
#       [--require-signing] [--no-msi]
#
#   --version V    Defaults to MARKETING_VERSION in
#                  bins/openagents-ios/host/project.yml (the lockstep
#                  version); the desktop crate's version must equal it.
#   --out DIR      Where the packages go (default target/desktop/windows).
#   --bin-dir DIR  Package prebuilt openagents-desktop.exe, coder.exe,
#                  microcoder.exe, and coder-boundary.exe from DIR instead
#                  of running Cargo.
#   --pfx FILE     An Authenticode code-signing certificate and key
#                  (PKCS#12). Every .exe and the MSI are signed with
#                  SHA-256 and an RFC 3161 timestamp by osslsigncode.
#                  --pfx-password-file names a file holding its password;
#                  the password never appears on a command line or in a log.
#                  Without --pfx the packages are UNSIGNED, not for release.
#   --require-signing  Refuse to build unsigned packages (use for a release).
#   --no-msi       Only the .zip.
#
# Needs: Rust with the x86_64-pc-windows-gnu target and a MinGW-w64
# linker (x86_64-w64-mingw32-gcc; Homebrew's mingw-w64, or on NixOS
# pkgsCross.mingwW64.stdenv.cc) and its strip, `wixl` from msitools for
# the MSI, `zip`, and `osslsigncode` to sign. On NixOS:
#
#   nix shell nixpkgs#msitools nixpkgs#zip nixpkgs#osslsigncode
#
# The MSI is the .ps1's: per user (no administrator prompt) in
# %LOCALAPPDATA%\Programs\OpenAgents, the same UpgradeCode, the Run entry
# "OpenAgents" (OpenAgents.exe --start-host) owned by the MSI so an
# uninstall removes it, a Start menu shortcut, a major upgrade that
# replaces any earlier version, and the app opened when an interactive or
# progress-bar install finishes (the updater installs with /passive).
# Writes OpenAgents-VERSION-x64.msi, OpenAgents-VERSION-windows-x64.zip,
# SHA256SUMS, and BUILDINFO (OPENAGENTS_BUILD_COMMIT names the commit when
# the script runs outside the checkout the binaries came from, and
# OPENAGENTS_BUILD_RUSTC the compiler that built --bin-dir). scripts/desktop/sign-manifest-windows.sh then
# signs the update manifest and publishes them.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="x86_64-pc-windows-gnu"
# Stable for every release: it is how Windows Installer finds the version
# an upgrade replaces. The .ps1 uses the same one. Never change it.
upgrade_code="5B1759D8-E627-4987-8844-1923D7B2D16F"

version=""
out=""
bin_dir=""
pfx=""
pfx_password_file=""
require_signing=0
msi=1

die() {
  echo "package-windows: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) version="${2:-}"; shift 2 ;;
    --out) out="${2:-}"; shift 2 ;;
    --bin-dir) bin_dir="${2:-}"; shift 2 ;;
    --pfx) pfx="${2:-}"; shift 2 ;;
    --pfx-password-file) pfx_password_file="${2:-}"; shift 2 ;;
    --require-signing) require_signing=1; shift ;;
    --no-msi) msi=0; shift ;;
    -h | --help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument \`$1\` (see --help)" ;;
  esac
done

marketing="$(sed -n 's/^[[:space:]]*MARKETING_VERSION:[[:space:]]*\([0-9][0-9.]*\)[[:space:]]*$/\1/p' \
  "$root/bins/openagents-ios/host/project.yml" | head -n 1)"
crate="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/crates/openagents-desktop/Cargo.toml" | head -n 1)"
[[ -n "$version" ]] || version="$marketing"
[[ "$version" == "$marketing" && "$version" == "$crate" ]] ||
  die "version $version must equal MARKETING_VERSION ($marketing) and the desktop crate's ($crate)"
# Windows Installer versions are numeric fields, each small.
[[ "$version" =~ ^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,5}$ ]] ||
  die "version '$version' must be MAJOR.MINOR.PATCH (numbers only) for an MSI"

if [[ -n "$pfx" ]]; then
  [[ -r "$pfx" ]] || die "cannot read the certificate $pfx"
  [[ -n "$pfx_password_file" && -r "$pfx_password_file" ]] || die "--pfx needs --pfx-password-file"
  command -v osslsigncode >/dev/null || die "osslsigncode is not on PATH"
elif [[ "$require_signing" == 1 ]]; then
  die "--require-signing needs --pfx and --pfx-password-file"
else
  echo "package-windows: no certificate; building UNSIGNED packages (not for release)" >&2
fi
command -v zip >/dev/null || die "zip is not on PATH"
[[ "$msi" == 0 ]] || command -v wixl >/dev/null || die "wixl (msitools) is not on PATH, or pass --no-msi"

target_dir="${CARGO_TARGET_DIR:-$root/target}"
[[ -n "$out" ]] || out="$target_dir/desktop/windows"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
stage="$work/OpenAgents"
mkdir -p "$stage"

# Built name -> installed name. The window first.
built=(openagents-desktop.exe coder.exe microcoder.exe coder-boundary.exe)
installed=(OpenAgents.exe coder.exe microcoder.exe coder-boundary.exe)

rustc_used="$(rustc --version 2>/dev/null || echo unknown)"
if [[ -n "$bin_dir" ]]; then
  rustc_used="${OPENAGENTS_BUILD_RUSTC:-prebuilt}"
fi
if [[ -z "$bin_dir" ]]; then
  (cd "$root" && cargo build --release --locked --target "$target" \
    -p openagents-desktop -p coder -p microcoder -p coder-boundary \
    --bin openagents-desktop --bin coder --bin microcoder --bin coder-boundary)
  bin_dir="$target_dir/$target/release"
fi
for i in "${!built[@]}"; do
  src="$bin_dir/${built[$i]}"
  [[ -f "$src" ]] || die "missing $src"
  # A Windows executable starts with "MZ".
  [[ "$(head -c 2 "$src")" == MZ ]] || die "$src is not a Windows executable"
  cp "$src" "$stage/${installed[$i]}"
done

# The GNU toolchain leaves symbols in a release build (about 150 MB for
# coder.exe); strip them before signing, as package-linux.sh strips.
strip_tool="$(command -v x86_64-w64-mingw32-strip || command -v llvm-strip || true)"
if [[ -n "$strip_tool" ]]; then
  for name in "${installed[@]}"; do "$strip_tool" "$stage/$name"; done
else
  echo "package-windows: no x86_64-w64-mingw32-strip or llvm-strip; packaging unstripped binaries" >&2
fi

sign() {
  # $1: the file to sign in place; $2: its description
  [[ -n "$pfx" ]] || return 0
  osslsigncode sign -pkcs12 "$pfx" -readpass "$pfx_password_file" -h sha256 \
    -ts http://timestamp.digicert.com -n "$2" -i https://openagents.com \
    -in "$1" -out "$1.signed" >/dev/null
  mv "$1.signed" "$1"
  osslsigncode verify -in "$1" >/dev/null 2>&1 || die "the signature on $(basename "$1") does not verify"
}
for name in "${installed[@]}"; do
  sign "$stage/$name" OpenAgents
done

artifacts=()
zip_file="$out/OpenAgents-$version-windows-x64.zip"
rm -f "$zip_file"
(cd "$work" && zip -q -r -X "$zip_file" OpenAgents)
artifacts+=("$zip_file")

if [[ "$msi" == 1 ]]; then
  files=""
  for name in "${installed[@]}"; do
    id="File_${name//[^A-Za-z0-9]/_}"
    files+="          <File Id=\"$id\" Name=\"$name\" Source=\"$stage/$name\" />"$'\n'
  done
  # WiX v3 syntax, which wixl reads. A per-user component under the
  # profile has an HKCU key path and removes its folder on uninstall.
  cat >"$work/OpenAgents.wxs" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="OpenAgents" Manufacturer="OpenAgents" Version="$version"
           UpgradeCode="$upgrade_code" Language="1033">
    <Package InstallerVersion="500" Compressed="yes" InstallScope="perUser"
             Description="OpenAgents: connect your phone to this computer" />
    <MajorUpgrade DowngradeErrorMessage="A newer version of OpenAgents is already installed." />
    <Media Id="1" Cabinet="OpenAgents.cab" EmbedCab="yes" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="LocalAppDataFolder">
        <Directory Id="ProgramsDir" Name="Programs">
          <Directory Id="INSTALLFOLDER" Name="OpenAgents" />
        </Directory>
      </Directory>
      <Directory Id="ProgramMenuFolder" />
    </Directory>
    <DirectoryRef Id="INSTALLFOLDER">
      <Component Id="Binaries" Guid="73B1723C-2E97-486B-975C-97E1944428F9" Win64="yes">
$files          <RegistryValue Root="HKCU" Key="Software\\OpenAgents\\Desktop" Name="Installed"
                         Type="integer" Value="1" KeyPath="yes" />
          <RemoveFolder Id="RemoveInstallFolder" On="uninstall" />
      </Component>
      <Component Id="StartAtSignIn" Guid="7D3C1C81-BFEF-4F73-9B4F-2F7AF5F7429C" Win64="yes">
        <RegistryValue Root="HKCU" Key="Software\\Microsoft\\Windows\\CurrentVersion\\Run"
                       Name="OpenAgents" Type="string"
                       Value="&quot;[INSTALLFOLDER]OpenAgents.exe&quot; --start-host" KeyPath="yes" />
      </Component>
    </DirectoryRef>
    <DirectoryRef Id="ProgramMenuFolder">
      <Component Id="StartMenu" Guid="3A3CB2BF-7C63-4AF3-A571-3CAB42DDBEC3" Win64="yes">
        <Shortcut Id="StartMenuShortcut" Name="OpenAgents" Target="[INSTALLFOLDER]OpenAgents.exe"
                  WorkingDirectory="INSTALLFOLDER" />
        <RegistryValue Root="HKCU" Key="Software\\OpenAgents\\Desktop" Name="StartMenuShortcut"
                       Type="integer" Value="1" KeyPath="yes" />
      </Component>
    </DirectoryRef>
    <Feature Id="Main" Level="1">
      <ComponentRef Id="Binaries" />
      <ComponentRef Id="StartAtSignIn" />
      <ComponentRef Id="StartMenu" />
    </Feature>
    <!-- Open the app after an interactive or progress-bar install (the
         updater's /passive), so the code shows without another step. -->
    <CustomAction Id="LaunchApp" FileKey="File_OpenAgents_exe" ExeCommand="" Return="asyncNoWait" />
    <InstallExecuteSequence>
      <Custom Action="LaunchApp" After="InstallFinalize">NOT Installed AND NOT REMOVE AND UILevel &gt; 2</Custom>
    </InstallExecuteSequence>
  </Product>
</Wix>
EOF
  msi_file="$out/OpenAgents-$version-x64.msi"
  rm -f "$msi_file"
  wixl -a x64 -o "$msi_file" "$work/OpenAgents.wxs"
  sign "$msi_file" "OpenAgents"
  artifacts+=("$msi_file")
fi

sha256() { openssl dgst -sha256 -r "$1" | cut -d ' ' -f 1; }
: >"$out/SHA256SUMS"
for file in "${artifacts[@]}"; do
  printf '%s  %s\n' "$(sha256 "$file")" "$(basename "$file")" >>"$out/SHA256SUMS"
done
{
  echo "version $version"
  echo "commit ${OPENAGENTS_BUILD_COMMIT:-$(git -C "$root" rev-parse HEAD 2>/dev/null || echo unknown)}"
  echo "target $target"
  echo "rustc $rustc_used"
  echo "signed $([[ -n "$pfx" ]] && echo yes || echo no)"
} >"$out/BUILDINFO"
for file in "${artifacts[@]}"; do echo "package-windows: wrote $file"; done
echo "package-windows: wrote $out/SHA256SUMS and $out/BUILDINFO"
