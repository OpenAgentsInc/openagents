#!/usr/bin/env bash
# Firm reason: 5 (thin build wrapper compiling Coax music/studio.coax -> C11 + Swift macOS bundle).
# Usage:
#   apps/music-studio/build.sh [--open]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
open_app=0
for arg in "$@"; do
  case "$arg" in
    --open) open_app=1 ;;
    *) echo "build.sh: unknown argument: $arg (usage: build.sh [--open])" >&2; exit 2 ;;
  esac
done

work="${MUSIC_STUDIO_BUILD:-$here/build}"
app="$work/OpenAgents Music Studio.app"
mkdir -p "$work/c" "$app/Contents/MacOS" "$app/Contents/Resources"

# 1. Compile Coax core (music/studio.coax) -> self-contained C11 + layout-checked header.
"$coax" c "$repo/music/studio.coax" --library --header "$work/c/music_studio.h" > "$work/c/music_studio.c"

cflags=(-mmacosx-version-min=14.0 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c" -I"$here")
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$work/c/music_studio.c" -o "$work/c/music_studio.o"
/usr/bin/clang -std=gnu11 "${cflags[@]}" -c "$here/music-bridge.c" -o "$work/c/music-bridge.o"

# 2. Compile thin macOS Swift shell over the Coax C core.
xcrun -sdk macosx swiftc -O -swift-version 5 -parse-as-library -target arm64-apple-macos14.0 \
  -import-objc-header "$here/music-bridge.h" \
  "$here/Sources/MusicModel.swift" \
  "$here/Sources/AudioEngine.swift" \
  "$here/Sources/PianoRollView.swift" \
  "$here/Sources/VisualizerViews.swift" \
  "$here/Sources/StudioApp.swift" \
  "$work/c/music_studio.o" \
  "$work/c/music-bridge.o" \
  -framework AppKit -framework AVFoundation -framework AudioToolbox -framework CoreAudio \
  -framework CryptoKit -framework UniformTypeIdentifiers -framework QuartzCore \
  -o "$app/Contents/MacOS/OpenAgentsMusicStudio.new"

mv "$app/Contents/MacOS/OpenAgentsMusicStudio.new" "$app/Contents/MacOS/OpenAgentsMusicStudio"
cp "$here/Info.plist" "$app/Contents/Info.plist"
codesign --force --deep --sign - "$app" >/dev/null 2>&1 || true

echo "built $app"
if [[ "$open_app" == "1" ]]; then
  open "$app"
fi
