#!/usr/bin/env bash
# Package the OpenAgents desktop app for Linux: an AppImage, a .deb, and a
# plain .tar.gz, from one release build.
#
# The app is two processes (docs/coder/design/2026-09-29-auto-pairing.md):
# the window (`openagents-desktop`) and the host (`coder host serve`), which
# the app registers on first launch as the systemd user unit
# `com.openagents.desktop.host.service`. Both binaries, plus the ones a
# task needs, ship together in one directory, `/usr/lib/openagents/` in the
# .deb and `usr/lib/openagents/` inside the AppImage. The AppImage's AppRun
# runs `coder` when its first argument is `coder`, so the unit can start
# the host through the AppImage file, whose mount point changes each run.
#
# usage: scripts/desktop/package-linux.sh [--version V] [--out DIR]
#          [--skip-build] [--no-appimage] [--no-deb]
#          [--appimage-runtime FILE]
#
#   --version V            package version (default: the phone app's MARKETING_VERSION
#                          in bins/openagents-ios/host/project.yml)
#   --out DIR              output directory (default: target/desktop/linux)
#   --skip-build           package the binaries already in the target dir
#   --no-appimage          skip the AppImage
#   --no-deb               skip the .deb
#   --appimage-runtime F   an AppImage type-2 runtime to prepend to the
#                          squashfs (with `mksquashfs` on PATH), used when
#                          `appimagetool` is not on PATH
#
# Binaries built on NixOS carry a /nix/store ELF interpreter; this script
# resets it to the standard /lib64 loader with `patchelf` (on NixOS:
# `nix shell nixpkgs#patchelf nixpkgs#dpkg nixpkgs#squashfsTools`) so the
# packages run on other distributions. The newest glibc symbol version the
# binaries need is printed; build on an older base for wider reach.
#
# DEB_MAINTAINER overrides the .deb's Maintainer field.
#
# Reproducible: every file in the packages carries the time
# SOURCE_DATE_EPOCH (default: the checkout's last commit time), archives
# list files in name order with owner root, gzip leaves out its own time,
# and the squashfs is built with SOURCE_DATE_EPOCH, so the same binaries
# always make the same bytes. For the release, build the binaries the same
# way too: scripts/desktop/build-linux-release.sh runs this script inside a
# pinned container with fixed paths and a pinned AppImage runtime.
#
# Signing: Linux packages carry no code signature. SHA256SUMS lists every
# artifact; scripts/desktop/sign-manifest-linux.sh signs it and the update
# manifest with the Ed25519 update key and publishes them beside the
# packages.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
app_id="com.openagents.desktop"
package="openagents"
# The window binary, then the host and what a task runs.
binaries=(openagents-desktop coder microcoder)

version=""
out=""
build=1
want_appimage=1
want_deb=1
runtime=""
while (($#)); do
  case "$1" in
    --version) version="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --skip-build) build=0; shift ;;
    --no-appimage) want_appimage=0; shift ;;
    --no-deb) want_deb=0; shift ;;
    --appimage-runtime) runtime="$2"; shift 2 ;;
    -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "package-linux: unknown argument $1" >&2; exit 2 ;;
  esac
done

[[ "$(uname -s)" == Linux ]] || { echo "package-linux: run this on Linux" >&2; exit 1; }

if [[ -z "$version" ]]; then
  version="$(sed -n 's/^ *MARKETING_VERSION: *\([0-9][0-9.]*\) *$/\1/p' "$root/bins/openagents-ios/host/project.yml" | head -1)"
fi
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.+~-][0-9A-Za-z.+~-]+)?$ ]] \
  || { echo "package-linux: bad version '$version'" >&2; exit 1; }
# The desktop crate's version is the updater's running version; it must be
# the one on the packages (INVARIANTS.md, App versions).
crate_version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/crates/openagents-desktop/Cargo.toml" | head -1)"
[[ "$crate_version" == "$version" ]] \
  || { echo "package-linux: openagents-desktop is $crate_version but the package is $version; change crates/openagents-desktop/Cargo.toml to match" >&2; exit 1; }

if [[ -z "${SOURCE_DATE_EPOCH:-}" ]]; then
  SOURCE_DATE_EPOCH="$(git -C "$root" log -1 --format=%ct 2>/dev/null || echo 0)"
fi
[[ "$SOURCE_DATE_EPOCH" =~ ^[0-9]+$ ]] || { echo "package-linux: bad SOURCE_DATE_EPOCH" >&2; exit 1; }
export SOURCE_DATE_EPOCH
# Same bytes for the same files: fixed times, name order, root owner, and
# no gzip timestamp.
settle() { find "$1" -exec touch -h -d "@$SOURCE_DATE_EPOCH" {} +; }
tgz() {
  # $1: the archive, $2: the folder to run in, then what to archive
  local archive="$1" dir="$2"
  shift 2
  ( cd "$dir" && tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 \
      --numeric-owner --format=gnu -cf - "$@" ) | gzip -9n > "$archive"
}

machine="$(uname -m)"
case "$machine" in
  x86_64) deb_arch=amd64; loader=/lib64/ld-linux-x86-64.so.2 ;;
  aarch64) deb_arch=arm64; loader=/lib/ld-linux-aarch64.so.1 ;;
  *) echo "package-linux: unsupported machine $machine" >&2; exit 1 ;;
esac

target_dir="${CARGO_TARGET_DIR:-$root/target}"
out="${out:-$target_dir/desktop/linux}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/openagents-package.XXXXXX")"
trap 'rm -rf "$work"' EXIT

if ((build)); then
  args=()
  for bin in "${binaries[@]}"; do args+=(--bin "$bin"); done
  # OPENAGENTS_DESKTOP_RELEASE=1 marks the release build: only it uses the
  # release app's Secret Service items; any other build keeps its own
  # (#10096).
  OPENAGENTS_DESKTOP_RELEASE=1 cargo build --release --locked --manifest-path "$root/Cargo.toml" \
    -p openagents-desktop -p coder -p microcoder "${args[@]}"
fi

# --- stage the binaries -----------------------------------------------------
lib="$work/lib"
mkdir -p "$lib"
for bin in "${binaries[@]}"; do
  src="$target_dir/release/$bin"
  [[ -x "$src" ]] || { echo "package-linux: missing $src (build it or drop --skip-build)" >&2; exit 1; }
  install -m 0755 "$src" "$lib/$bin"
  interp="$(readelf -l "$lib/$bin" 2>/dev/null | sed -n 's/.*Requesting program interpreter: \(.*\)]/\1/p')"
  if [[ "$interp" == /nix/store/* ]]; then
    command -v patchelf >/dev/null \
      || { echo "package-linux: $bin uses a Nix loader; put patchelf on PATH (nix shell nixpkgs#patchelf)" >&2; exit 1; }
    patchelf --set-interpreter "$loader" --remove-rpath "$lib/$bin"
  fi
  strip --strip-debug "$lib/$bin" 2>/dev/null || true
done
glibc="$(for bin in "${binaries[@]}"; do objdump -T "$lib/$bin" 2>/dev/null; done \
  | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1 || true)"
echo "package-linux: needs ${glibc:-an unknown glibc} or newer"

icon_src="$root/bins/openagents-ios/host/App/Assets.xcassets/AppIcon.appiconset/icon-1024.png"
desktop_entry() {
  # $1: the Exec= command
  cat <<EOF
[Desktop Entry]
Type=Application
Name=OpenAgents
Comment=Connect your phone to this computer
Exec=$1
Icon=$app_id
Terminal=false
Categories=Development;Utility;
StartupWMClass=openagents-desktop
X-GNOME-UsesNotifications=true
EOF
}

artifacts=()

# --- .tar.gz ------------------------------------------------------------------
tar_root="$work/tar/openagents-$version"
mkdir -p "$tar_root"
cp -a "$lib/." "$tar_root/"
settle "$work/tar"
tarball="$out/openagents-$version-linux-$machine.tar.gz"
tgz "$tarball" "$work/tar" "openagents-$version"
artifacts+=("$tarball")

# --- .deb ---------------------------------------------------------------------
if ((want_deb)); then
  deb_root="$work/deb"
  install -d "$deb_root/DEBIAN" "$deb_root/usr/lib/$package" "$deb_root/usr/bin" \
    "$deb_root/usr/share/applications" "$deb_root/usr/share/pixmaps"
  cp -a "$lib/." "$deb_root/usr/lib/$package/"
  ln -s "../lib/$package/openagents-desktop" "$deb_root/usr/bin/openagents-desktop"
  desktop_entry "/usr/bin/openagents-desktop" > "$deb_root/usr/share/applications/$app_id.desktop"
  install -m 0644 "$icon_src" "$deb_root/usr/share/pixmaps/$app_id.png"
  size_kb="$(du -sk "$deb_root/usr" | cut -f1)"
  cat > "$deb_root/DEBIAN/control" <<EOF
Package: $package
Version: $version
Architecture: $deb_arch
Maintainer: ${DEB_MAINTAINER:-OpenAgents <https://openagents.com>}
Installed-Size: $size_kb
Depends: libc6
Recommends: gnome-keyring | kwalletmanager | keepassxc
Section: utils
Priority: optional
Homepage: https://openagents.com
Description: Connect your phone to this computer
 OpenAgents shows a code to scan with the OpenAgents app on your phone.
 Once connected, your phone can send work to Coder on this computer.
EOF
  # The host runs per user as a systemd user unit the app registers; a
  # package removal cannot reach every user's unit, so the app's own
  # "Stop Coder on this computer" removes it.
  deb="$out/${package}_${version}_${deb_arch}.deb"
  settle "$deb_root"
  if command -v dpkg-deb >/dev/null; then
    dpkg-deb --root-owner-group -Zgzip --build "$deb_root" "$deb" >/dev/null
  else
    # The .deb format by hand: an ar archive of debian-binary,
    # control.tar.gz, and data.tar.gz, in that order.
    tgz "$work/control.tar.gz" "$deb_root/DEBIAN" ./control
    tgz "$work/data.tar.gz" "$deb_root" --exclude=./DEBIAN .
    printf '2.0\n' > "$work/debian-binary"
    settle "$work/debian-binary"; settle "$work/control.tar.gz"; settle "$work/data.tar.gz"
    rm -f "$deb"
    ( cd "$work" && ar rcD "$deb" debian-binary control.tar.gz data.tar.gz )
  fi
  artifacts+=("$deb")
fi

# --- AppImage -----------------------------------------------------------------
if ((want_appimage)); then
  appdir="$work/OpenAgents.AppDir"
  install -d "$appdir/usr/lib/$package"
  cp -a "$lib/." "$appdir/usr/lib/$package/"
  cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
# `OpenAgents.AppImage coder host serve` runs the host (the systemd user
# unit does this); anything else opens the window.
here="$(dirname "$(readlink -f "$0")")"
if [ "${1:-}" = coder ]; then
  shift
  exec "$here/usr/lib/openagents/coder" "$@"
fi
exec "$here/usr/lib/openagents/openagents-desktop" "$@"
EOF
  chmod 0755 "$appdir/AppRun"
  desktop_entry "openagents-desktop" > "$appdir/$app_id.desktop"
  install -m 0644 "$icon_src" "$appdir/$app_id.png"
  ln -s "$app_id.png" "$appdir/.DirIcon"
  settle "$appdir"
  appimage="$out/OpenAgents-$version-$machine.AppImage"
  rm -f "$appimage"
  if command -v appimagetool >/dev/null; then
    ARCH="$machine" appimagetool --no-appstream "$appdir" "$appimage" >/dev/null
  elif [[ -n "$runtime" ]]; then
    command -v mksquashfs >/dev/null \
      || { echo "package-linux: --appimage-runtime needs mksquashfs (nix shell nixpkgs#squashfsTools)" >&2; exit 1; }
    [[ -f "$runtime" ]] || { echo "package-linux: no runtime at $runtime" >&2; exit 1; }
    # mksquashfs takes its times from SOURCE_DATE_EPOCH.
    mksquashfs "$appdir" "$work/app.squashfs" -root-owned -noappend -no-xattrs -comp zstd -quiet >/dev/null
    cat "$runtime" "$work/app.squashfs" > "$appimage"
    chmod 0755 "$appimage"
  else
    echo "package-linux: skipping the AppImage: put appimagetool on PATH or pass --appimage-runtime" >&2
    appimage=""
  fi
  [[ -n "$appimage" ]] && artifacts+=("$appimage")
fi

( cd "$out" && for a in "${artifacts[@]}"; do sha256sum "$(basename "$a")"; done ) > "$out/SHA256SUMS"
printf 'package-linux: wrote %s\n' "${artifacts[@]}" "$out/SHA256SUMS"
