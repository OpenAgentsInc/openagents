#!/usr/bin/env bash
# Packages OpenAgents for Mac as a signed, notarized, stapled .dmg.
#
#   scripts/desktop/package-macos.sh [options]
#
# Steps (docs/desktop/release.md has the full runbook):
#   1. Build universal (arm64 + x86_64) release binaries of the app
#      (`openagents-desktop`), `coder`, `microcoder`, and `openagents`, and glue each pair
#      with `lipo`.
#   2. Assemble OpenAgents.app: Contents/MacOS/{OpenAgents,coder,microcoder,openagents},
#      Info.plist, icon, and the host's launchd plist in
#      Contents/Library/LaunchAgents/.
#   3. Sign every executable, inner ones first, with the Developer ID
#      Application identity, the hardened runtime, a secure timestamp, and
#      entitlements (the app's and the embedded host's).
#   4. Notarize the app with `xcrun notarytool submit --wait` and staple it.
#   5. Build the .dmg (the app plus an Applications symlink to drag it onto),
#      sign it, notarize it, staple it.
#   6. Check: `codesign --verify --strict`, `spctl --assess` on the app and
#      the .dmg, and `stapler validate` on both.
#
# Options:
#   --app PATH          Package an already assembled .app instead of building
#                       one (skips steps 1-2). Any macOS .app works, e.g. the
#                       deck from scripts/bundle-openagents-deck.sh.
#   --out DIR           Where the .app and .dmg go
#                       (default: $CARGO_TARGET_DIR/desktop-release, or
#                       target/desktop-release).
#   --identity ID       Signing identity: a name or SHA-1 from
#                       `security find-identity -v -p codesigning`, or `-` for
#                       ad hoc (local testing; implies --no-notarize).
#                       Default: $OA_DEVELOPER_ID_APPLICATION, else the first
#                       "Developer ID Application" identity in the keychain,
#                       else ad hoc.
#   --adhoc             Same as --identity -.
#   --no-notarize       Sign and build the .dmg, but do not notarize or staple.
#   --bin-dir DIR       Take the four binaries from DIR (named like their
#                       Cargo binaries: openagents-desktop, coder,
#                       microcoder, openagents) instead of building them; each may be
#                       universal or single-architecture.
#   --native            Build only for this Mac's architecture (faster; not
#                       for release).
#   --notary-env FILE   Source notarization credentials from FILE (see below).
#   --volname NAME      .dmg volume name (default: the app's name).
#   -h, --help          This text.
#
# Notarization credentials, first match wins:
#   NOTARY_KEYCHAIN_PROFILE   a profile saved with
#                             `xcrun notarytool store-credentials`.
#   ASC_API_KEY_ID, ASC_API_ISSUER_ID, ASC_API_PRIVATE_KEY_PATH
#                             an App Store Connect API key (.p8); the same key
#                             that uploads TestFlight builds.
#
# Build inputs from bins/openagents-desktop-macos/ are used when present:
#   Info.plist, OpenAgents.entitlements, host.entitlements,
#   com.openagents.desktop.host.plist, AppIcon.icns. Anything missing falls
#   back to the defaults written by this script.
#
# Overrides: DESKTOP_PACKAGE (openagents-desktop), DESKTOP_BIN (the package's
# binary, default = DESKTOP_PACKAGE), MACOSX_DEPLOYMENT_TARGET (13.0).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"
macos_dir="$root/bins/openagents-desktop-macos"

app_in=""
out=""
identity="${OA_DEVELOPER_ID_APPLICATION:-}"
identity_set=0
notarize=1
native=0
bin_dir=""
volname=""

usage() { sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d' | sed 's/^# \{0,1\}//'; }
die() { echo "package-macos: $*" >&2; exit 1; }
step() { echo "==> $*" >&2; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app) app_in="${2:?}"; shift 2 ;;
    --out) out="${2:?}"; shift 2 ;;
    --identity) identity="${2:?}"; identity_set=1; shift 2 ;;
    --adhoc) identity="-"; identity_set=1; shift ;;
    --no-notarize) notarize=0; shift ;;
    --native) native=1; shift ;;
    --bin-dir) bin_dir="$(cd "${2:?}" && pwd)"; shift 2 ;;
    --notary-env)
      [[ -f "${2:?}" ]] || die "no such file: $2"
      set -a; # shellcheck disable=SC1090
      source "$2"; set +a; shift 2 ;;
    --volname) volname="${2:?}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown option: $1 (see --help)" ;;
  esac
done

[[ "$(uname -s)" == "Darwin" ]] || die "macOS only"
out="${out:-$target/desktop-release}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/oa-package-macos.XXXXXX")"
trap 'rm -rf "$work"' EXIT

# ---------------------------------------------------------------- identity
if [[ -z "$identity" && $identity_set -eq 0 ]]; then
  identity="$(security find-identity -v -p codesigning |
    sed -n 's/.*[0-9A-F]\{40\} "\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)"
  if [[ -z "$identity" ]]; then
    echo "package-macos: no Developer ID Application identity in the keychain; signing ad hoc (local testing only)" >&2
    identity="-"
  fi
fi
if [[ "$identity" == "-" ]]; then
  notarize=0
  timestamp=(--timestamp=none)
else
  security find-identity -v -p codesigning | grep -qF "$identity" ||
    die "signing identity not found in the keychain: $identity"
  timestamp=(--timestamp)
fi

notary_auth=()
if [[ $notarize -eq 1 ]]; then
  if [[ -n "${NOTARY_KEYCHAIN_PROFILE:-}" ]]; then
    notary_auth=(--keychain-profile "$NOTARY_KEYCHAIN_PROFILE")
  elif [[ -n "${ASC_API_KEY_ID:-}" && -n "${ASC_API_ISSUER_ID:-}" && -n "${ASC_API_PRIVATE_KEY_PATH:-}" ]]; then
    [[ -f "$ASC_API_PRIVATE_KEY_PATH" ]] || die "ASC_API_PRIVATE_KEY_PATH does not exist"
    notary_auth=(--key "$ASC_API_PRIVATE_KEY_PATH" --key-id "$ASC_API_KEY_ID" --issuer "$ASC_API_ISSUER_ID")
  else
    die "notarization needs NOTARY_KEYCHAIN_PROFILE or ASC_API_KEY_ID/ASC_API_ISSUER_ID/ASC_API_PRIVATE_KEY_PATH (or --notary-env FILE, or --no-notarize)"
  fi
fi

# ------------------------------------------------------------ entitlements
# Outside the App Sandbox the network and the login keychain need no
# entitlement; the network keys below are declared so the app keeps working
# if it is ever sandboxed. No `keychain-access-groups`: that key needs a
# provisioning profile, and without one a Developer ID app will not launch.
# Keychain items are bound to the Developer ID designated requirement, so a
# rebuilt, re-signed app (and its host) keeps reading them.
default_entitlements() {
  cat <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.network.client</key>
  <true/>
  <key>com.apple.security.network.server</key>
  <true/>
</dict>
</plist>
PLIST
}
entitlements_for() { # $1 = file name in bins/openagents-desktop-macos
  if [[ -f "$macos_dir/$1" ]]; then
    echo "$macos_dir/$1"
  else
    default_entitlements >"$work/$1"
    echo "$work/$1"
  fi
}
app_entitlements="$(entitlements_for OpenAgents.entitlements)"
host_entitlements="$(entitlements_for host.entitlements)"
plutil -lint -s "$app_entitlements" "$host_entitlements"

# ------------------------------------------------------------ build + bundle
plist_get() { /usr/libexec/PlistBuddy -c "Print :$2" "$1" 2>/dev/null || true; }

built="" # set by build_universal: path to the (fat) binary
build_universal() { # $1 package, $2 bin
  local package="$1" bin="$2" triples=() t
  if [[ -n "$bin_dir" ]]; then
    [[ -x "$bin_dir/$bin" ]] || die "no executable $bin_dir/$bin"
    built="$bin_dir/$bin"
    return
  fi
  if [[ $native -eq 1 ]]; then
    triples=("$(uname -m | sed 's/arm64/aarch64/')-apple-darwin")
  else
    triples=(aarch64-apple-darwin x86_64-apple-darwin)
  fi
  for t in "${triples[@]}"; do
    rustup target list --installed 2>/dev/null | grep -qx "$t" ||
      die "missing Rust target $t; run: rustup target add $t"
    MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-13.0}" \
      cargo build --release --locked --manifest-path "$root/Cargo.toml" \
      --target "$t" -p "$package" --bin "$bin" >&2
  done
  if [[ ${#triples[@]} -eq 1 ]]; then
    built="$target/${triples[0]}/release/$bin"
  else
    lipo -create "$target/aarch64-apple-darwin/release/$bin" \
      "$target/x86_64-apple-darwin/release/$bin" -output "$work/$bin"
    built="$work/$bin"
  fi
}

assemble_app() {
  local package="${DESKTOP_PACKAGE:-openagents-desktop}"
  local bin="${DESKTOP_BIN:-$package}"
  local version exe app_bin coder_bin micro_bin cli_bin
  version="$(cargo metadata --no-deps --format-version 1 --manifest-path "$root/Cargo.toml" |
    sed -n "s/.*\"name\":\"$package\",\"version\":\"\([^\"]*\)\".*/\1/p")"
  version="${version:-0.1.0}"

  if [[ -n "$bin_dir" ]]; then
    step "taking $bin, coder, microcoder, openagents from $bin_dir"
  else
    step "building $package, coder, microcoder, openagents ($([[ $native -eq 1 ]] && echo native || echo universal))"
  fi
  build_universal "$package" "$bin"; app_bin="$built"
  build_universal coder coder; coder_bin="$built"
  build_universal microcoder microcoder; micro_bin="$built"
  # The command a phone's read-only command card runs on this Mac; the
  # host puts Contents/MacOS first on its terminals' PATH.
  build_universal openagents-cli openagents; cli_bin="$built"

  app="$out/OpenAgents.app"
  step "assembling $app"
  rm -rf "$app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Library/LaunchAgents"

  if [[ -f "$macos_dir/Info.plist" ]]; then
    cp "$macos_dir/Info.plist" "$app/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app/Contents/Info.plist" 2>/dev/null ||
      /usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string $version" "$app/Contents/Info.plist"
    # The build number, as bins/openagents-desktop-macos/bundle.sh sets it.
    local build
    build="$(git -C "$root" rev-list --count HEAD 2>/dev/null || echo 1)"
    /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $build" "$app/Contents/Info.plist" 2>/dev/null ||
      /usr/libexec/PlistBuddy -c "Add :CFBundleVersion string $build" "$app/Contents/Info.plist"
  else
    cat >"$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>OpenAgents</string>
  <key>CFBundleDisplayName</key><string>OpenAgents</string>
  <key>CFBundleIdentifier</key><string>com.openagents.desktop</string>
  <key>CFBundleExecutable</key><string>OpenAgents</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>${MACOSX_DEPLOYMENT_TARGET:-13.0}</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSLocalNetworkUsageDescription</key><string>OpenAgents lets your phone reach this Mac.</string>
</dict>
</plist>
PLIST
  fi
  exe="$(plist_get "$app/Contents/Info.plist" CFBundleExecutable)"
  exe="${exe:-OpenAgents}"
  cp "$app_bin" "$app/Contents/MacOS/$exe"
  cp "$coder_bin" "$app/Contents/MacOS/coder"
  cp "$micro_bin" "$app/Contents/MacOS/microcoder"
  cp "$cli_bin" "$app/Contents/MacOS/openagents"

  if [[ -f "$macos_dir/com.openagents.desktop.host.plist" ]]; then
    cp "$macos_dir/com.openagents.desktop.host.plist" "$app/Contents/Library/LaunchAgents/"
  else
    cat >"$app/Contents/Library/LaunchAgents/com.openagents.desktop.host.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.openagents.desktop.host</string>
  <key>BundleProgram</key><string>Contents/MacOS/coder</string>
  <key>ProgramArguments</key>
  <array><string>coder</string><string>host</string><string>serve</string><string>--keychain</string><string>--iroh</string><string>--control</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
PLIST
  fi
  plutil -lint -s "$app/Contents/Info.plist" "$app/Contents/Library/LaunchAgents/"*.plist

  local icon_name
  icon_name="$(plist_get "$app/Contents/Info.plist" CFBundleIconFile)"
  icon_name="${icon_name:-AppIcon}"
  icon_name="${icon_name%.icns}"
  if [[ -f "$macos_dir/AppIcon.icns" ]]; then
    cp "$macos_dir/AppIcon.icns" "$app/Contents/Resources/$icon_name.icns"
  else
    local src="$root/bins/openagents-ios/host/App/Assets.xcassets/AppIcon.appiconset/icon-1024.png"
    local iconset="$work/AppIcon.iconset" size
    mkdir -p "$iconset"
    for size in 16 32 128 256 512; do
      sips -z "$size" "$size" "$src" --out "$iconset/icon_${size}x${size}.png" >/dev/null
      sips -z $((size * 2)) $((size * 2)) "$src" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
    done
    iconutil -c icns "$iconset" -o "$app/Contents/Resources/$icon_name.icns"
  fi
}

if [[ -n "$app_in" ]]; then
  [[ -d "$app_in" && -f "$app_in/Contents/Info.plist" ]] || die "not an app bundle: $app_in"
  app="$out/$(basename "$app_in")"
  if [[ "$(cd "$(dirname "$app_in")" && pwd)/$(basename "$app_in")" != "$app" ]]; then
    rm -rf "$app"
    ditto "$app_in" "$app"
  fi
else
  assemble_app
fi

app_name="$(basename "$app" .app)"
main_exe="$(plist_get "$app/Contents/Info.plist" CFBundleExecutable)"
version="$(plist_get "$app/Contents/Info.plist" CFBundleShortVersionString)"
version="${version:-0.0.0}"
dmg_base="$(echo "$app_name" | tr ' ' '-')-$version"
[[ $native -eq 1 ]] && dmg_base="$dmg_base-$(uname -m)"
dmg="$out/$dmg_base.dmg"

# -------------------------------------------------------------------- sign
bundle_id="$(plist_get "$app/Contents/Info.plist" CFBundleIdentifier)"
sign() { # $1 path, $2 entitlements (optional), $3 identifier (optional)
  local args=(--force --options runtime "${timestamp[@]}" --sign "$identity")
  [[ -n "${2:-}" ]] && args+=(--entitlements "$2")
  [[ -n "${3:-}" ]] && args+=(--identifier "$3")
  codesign "${args[@]}" "$1"
}

step "signing with ${identity/#-/ad hoc identity}"
xattr -cr "$app"
# Inner code first: every Mach-O in the bundle except the main executable
# (helpers such as coder and microcoder, and any dylibs or frameworks),
# deepest paths first, then the bundle itself.
while IFS= read -r f; do
  [[ "$f" == "$app/Contents/MacOS/$main_exe" ]] && continue
  if file -b "$f" | grep -q "Mach-O"; then
    case "$f" in
      *.dylib) sign "$f" ;;
      # A helper's identifier is <bundle id>.<name> (com.openagents.desktop.coder),
      # as bundle.sh signs it: the host's keychain items are bound to it.
      *) sign "$f" "$host_entitlements" "${bundle_id:+$bundle_id.$(basename "$f")}" ;;
    esac
  fi
done < <(find "$app/Contents" -type f -perm -u+x -o -type f -name '*.dylib' | awk '{ print length, $0 }' | sort -rn | cut -d' ' -f2-)
while IFS= read -r nested; do sign "$nested"; done < <(
  find "$app/Contents" -depth \( -name '*.framework' -o -name '*.app' -o -name '*.appex' -o -name '*.xpc' \) -type d)
sign "$app" "$app_entitlements"
codesign --verify --strict --deep --verbose=2 "$app"

# ---------------------------------------------------------------- notarize
notarize() { # $1 file to submit
  local log id status
  log="$work/notary-$(basename "$1").json"
  step "notarizing $(basename "$1") (this takes a few minutes)"
  xcrun notarytool submit "$1" "${notary_auth[@]}" --wait --output-format json >"$log" || true
  id="$(plutil -extract id raw -o - "$log" 2>/dev/null || true)"
  status="$(plutil -extract status raw -o - "$log" 2>/dev/null || true)"
  echo "    submission ${id:-?}: ${status:-no status}"
  if [[ "$status" != "Accepted" ]]; then
    if [[ -n "$id" ]]; then
      xcrun notarytool log "$id" "${notary_auth[@]}" >&2 || true
    else
      cat "$log" >&2
    fi
    die "notarization of $(basename "$1") was not accepted"
  fi
}

if [[ $notarize -eq 1 ]]; then
  ditto -c -k --keepParent "$app" "$work/$app_name.zip"
  notarize "$work/$app_name.zip"
  xcrun stapler staple -q "$app"
  xcrun stapler validate -q "$app"
fi

# --------------------------------------------------------------------- dmg
step "building $dmg"
stage="$work/dmg"
mkdir -p "$stage"
ditto "$app" "$stage/$(basename "$app")"
ln -s /Applications "$stage/Applications"
rm -f "$dmg"
hdiutil create -quiet -volname "${volname:-$app_name}" -srcfolder "$stage" \
  -fs HFS+ -format UDZO -imagekey zlib-level=9 -ov "$dmg"
codesign --force "${timestamp[@]}" --sign "$identity" "$dmg"
codesign --verify --strict "$dmg"

if [[ $notarize -eq 1 ]]; then
  notarize "$dmg"
  xcrun stapler staple -q "$dmg"
fi

# ------------------------------------------------------------------ checks
step "checking"
if [[ $notarize -eq 1 ]]; then
  spctl --assess --type execute --verbose=2 "$app"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"
  xcrun stapler validate "$app"
  xcrun stapler validate "$dmg"
else
  echo "    not notarized: Gatekeeper will refuse this .dmg on another Mac (local testing only)"
fi
codesign --display --verbose=2 "$app" 2>&1 | grep -E '^(Identifier|Authority|TeamIdentifier|Timestamp|flags|Runtime)' | sed 's/^/    /' || true
lipo -archs "$app/Contents/MacOS/$main_exe" | sed 's/^/    architectures: /'
shasum -a 256 "$dmg" | sed 's/^/    sha256: /'
echo "built $dmg"
