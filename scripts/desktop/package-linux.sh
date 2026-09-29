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
#   --version V            package version (default: the workspace version)
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
# Signing: Linux packages are not code-signed here. SHA256SUMS lists every
# artifact; publish it beside them.
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
  version="$(awk '/^\[workspace.package\]/{p=1;next} /^\[/{p=0} p && /^version *=/{gsub(/"/,"",$3); print $3; exit}' "$root/Cargo.toml")"
fi
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.+~-][0-9A-Za-z.+~-]+)?$ ]] \
  || { echo "package-linux: bad version '$version'" >&2; exit 1; }

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
  cargo build --release --locked --manifest-path "$root/Cargo.toml" \
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
X-GNOME-UsesNotifications=false
EOF
}

artifacts=()

# --- .tar.gz ------------------------------------------------------------------
tar_root="$work/tar/openagents-$version"
mkdir -p "$tar_root"
cp -a "$lib/." "$tar_root/"
tarball="$out/openagents-$version-linux-$machine.tar.gz"
tar -C "$work/tar" --owner=0 --group=0 --numeric-owner -czf "$tarball" "openagents-$version"
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
  if command -v dpkg-deb >/dev/null; then
    dpkg-deb --root-owner-group -Zgzip --build "$deb_root" "$deb" >/dev/null
  else
    # The .deb format by hand: an ar archive of debian-binary,
    # control.tar.gz, and data.tar.gz, in that order.
    ( cd "$deb_root/DEBIAN" && tar --owner=0 --group=0 --numeric-owner -czf "$work/control.tar.gz" ./control )
    ( cd "$deb_root" && tar --owner=0 --group=0 --numeric-owner --exclude=./DEBIAN -czf "$work/data.tar.gz" . )
    printf '2.0\n' > "$work/debian-binary"
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
  appimage="$out/OpenAgents-$version-$machine.AppImage"
  rm -f "$appimage"
  if command -v appimagetool >/dev/null; then
    ARCH="$machine" appimagetool --no-appstream "$appdir" "$appimage" >/dev/null
  elif [[ -n "$runtime" ]]; then
    command -v mksquashfs >/dev/null \
      || { echo "package-linux: --appimage-runtime needs mksquashfs (nix shell nixpkgs#squashfsTools)" >&2; exit 1; }
    [[ -f "$runtime" ]] || { echo "package-linux: no runtime at $runtime" >&2; exit 1; }
    mksquashfs "$appdir" "$work/app.squashfs" -root-owned -noappend -comp zstd -quiet >/dev/null
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
