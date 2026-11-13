#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, metal and swiftc; holds no logic)
# Builds Everglade.app for this Mac (everglade/docs/desktop-app-spec.md): the worlds (the list,
# the Workshop, and the web's realms run natively: Everglade itself, everglade2, Experimental, fivelab, and the Archery Range, archery-range, with their sound) with the Next issue panel (desktop.coax) and your model (model.coax) compiled to C
# behind face.h, Next issue (next.coax, in the scene's unit: the Coax Claude Agent SDK), the
# context program it asks the host to run (next-context.coax, as everglade-next), their shaders,
# Everglade II's pack, on the shared Mac host (apps/shared/host/apple/ui-host.m, the ui-host.h
# contract OpenAgents uses) with the app's Metal layer and callbacks in Swift (Sources/Workshop.swift).
#   apps/everglade-desktop/build.sh [--open [APP ARGS...]]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
work="$here/build"
app="$work/Everglade.app"
mkdir -p "$work/c" "$app/Contents/MacOS" "$app/Contents/Resources"
"$coax" c "$here/desktop.coax" --library --header "$work/c/workshop.h" > "$work/c/workshop.c"
cflags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-variable -Wno-unused-but-set-variable -I"$work/c" -I"$here")
/usr/bin/clang "${cflags[@]}" -c "$work/c/workshop.c" -o "$work/c/workshop.o"
/usr/bin/clang "${cflags[@]}" -c "$here/face.c" -o "$work/c/face.o"
"$coax" c "$here/model.coax" --library --header "$work/c/model.h" > "$work/c/model.c"
/usr/bin/clang "${cflags[@]}" -w -c "$work/c/model.c" -o "$work/c/model.o"
/usr/bin/clang "${cflags[@]}" -c "$here/model-face.c" -o "$work/c/model-face.o"
/usr/bin/clang "${cflags[@]}" -c "$here/next-face.c" -o "$work/c/next-face.o"
/usr/bin/clang -std=gnu11 -O2 -w -I"$repo/coax/platform" -c "$repo/coax/platform/process-native.c" -o "$work/c/process.o"
xcrun -sdk macosx metal "$here/Workshop.metal" "$here/Model.metal" -o "$app/Contents/Resources/default.metallib"
# The place: the temple's parts (Aethyr's architecture kit) compiled to C, and Aethyr's shaders for
# it emitted from Coax for Metal, a library a pair (each emission carries its own helpers).
"$coax" c "$repo/everglade/workshop/temple.coax" --library --header "$work/c/temple.h" > "$work/c/temple.c"
/usr/bin/clang "${cflags[@]}" -w -c "$work/c/temple.c" -o "$work/c/temple.o"
/usr/bin/clang "${cflags[@]}" -c "$here/place-face.c" -o "$work/c/place-face.o"
for pair in sky scene light; do
  "$coax" metal "$repo/everglade/workshop/gpu/temple.coax" --vertex "temple_${pair}_vs" --fragment "temple_${pair}_fs" -o "$work/c/temple-$pair.metal" 2> "$work/c/temple-$pair.bind"
  xcrun -sdk macosx metal -w "$work/c/temple-$pair.metal" -o "$app/Contents/Resources/temple-$pair.metallib"
done
# The realms: the Everglade row is the web's everglade2 realm (Character Create, then the glade),
# and the Experimental row is the web's fivelab realm on its Experimental page, both natively,
# drawn by Aethyr's page host for Metal (aethyr/host/apple/). A realm's Coax as C and its face are
# built as verse/native/build.sh builds any realm, into a library the host loads; its shaders are
# the web's own Coax (apps/openagents-web/gpu/), emitted for Metal a pair at a time, each with the
# list of what it reads (.bind). The C takes over a minute to emit, so it is kept until its sources
# change.
key="$( (shasum -a 256 "$coax" | cut -d ' ' -f 1; git -C "$repo" rev-parse HEAD:apps/openagents-web HEAD:aethyr HEAD:verse HEAD:everglade HEAD:coax/lib; git -C "$repo" status --porcelain -- apps/openagents-web aethyr verse everglade coax/lib ':(exclude)aethyr/host/apple'; git -C "$repo" diff HEAD -- apps/openagents-web aethyr verse everglade coax/lib ':(exclude)aethyr/host/apple') | shasum -a 256 | cut -d ' ' -f 1)"
build_realm() {   # NAME: build/realm/NAME/libNAME.dylib, and a copy in the app's NAME folder
  local name="$1" realm="$work/realm/$1" pages="$app/Contents/Resources/$1"
  mkdir -p "$realm" "$pages"
  if [[ "$(cat "$realm/key" 2>/dev/null)" != "$key" || ! -f "$realm/lib$name.dylib" ]]; then
    rm -f "$realm/key"
    COAX="$coax" "$repo/verse/native/build.sh" "$name" "$realm" >/dev/null
    local realm_sources realm_objects=() realm_pids=() s pid
    read -r -a realm_sources < "$realm/sources"
    for s in "${realm_sources[@]}"; do
      /usr/bin/clang -std=gnu11 -O2 -ffp-contract=off -fno-fast-math -DCOAXIAL_NATIVE_ADDRESS -w -I"$realm" -c "$realm/$s" -o "$realm/${s%.c}.o" & realm_pids+=($!); realm_objects+=("$realm/${s%.c}.o")
    done
    for pid in "${realm_pids[@]}"; do wait "$pid"; done
    # The face's imports (the socket, HTTP, the clock) are the host's, found when the library loads.
    /usr/bin/clang -dynamiclib -Wl,-undefined,dynamic_lookup "${realm_objects[@]}" -lm -o "$realm/lib$name.dylib"
    echo "$key" > "$realm/key"
  fi
  cp "$realm/lib$name.dylib" "$pages/lib$name.dylib"
}
# Metal's relaxed math mode (it honours NaN and infinity, and keeps the math functions precise):
# the scene has millions of vertices a frame, and its vertex stage takes about 45% longer in the
# safe mode on this Mac (19.4 ms against 13.5 ms a frame at 990 by 660).
emit_pair() {   # REALM ROLE SOURCE VERTEX FRAGMENT
  "$coax" metal "$("$repo/verse/native/build.sh" --gpu "$3")" --vertex "$4" --fragment "$5" -o "$work/c/$1-$2.metal" 2> "$app/Contents/Resources/$1/$2.bind"
  xcrun -sdk macosx metal -w -fmetal-math-mode=relaxed -fmetal-math-fp32-functions=precise "$work/c/$1-$2.metal" -o "$app/Contents/Resources/$1/$2.metallib"
}
# A realm's eight pairs: the page's six from SOURCE (its entries begin PREFIX_), the particles' two
# from FX (they begin FXPREFIX_), and the page's bindings.
emit_page() {   # REALM SOURCE PREFIX FX FXPREFIX
  local pids=() pid
  emit_pair "$1" bg "$2" "$3_bg_vs" "$3_bg_fs" & pids+=($!)
  emit_pair "$1" shadow "$2" "$3_shadow_vs" "$3_shadow_fs" & pids+=($!)
  emit_pair "$1" scene "$2" "$3_scene_vs" "$3_scene_fs" & pids+=($!)
  emit_pair "$1" bloom "$2" "$3_bg_vs" "$3_bloom_fs" & pids+=($!)
  for pid in "${pids[@]}"; do wait "$pid"; done; pids=()
  emit_pair "$1" post "$2" "$3_bg_vs" "$3_post_fs" & pids+=($!)
  emit_pair "$1" fx "$2" "$3_fx_vs" "$3_fx_fs" & pids+=($!)
  emit_pair "$1" particle "$4" "$5_particle_vs" "$5_particle_fs" & pids+=($!)
  emit_pair "$1" particle_over "$4" "$5_particle_over_vs" "$5_particle_over_fs" & pids+=($!)
  for pid in "${pids[@]}"; do wait "$pid"; done
  local passes; passes="$("$repo/verse/native/build.sh" --gpu "$2")"
  cp "${passes%.coax}.bindings" "$app/Contents/Resources/$1/page.bindings"
  cp "$work/c/page-host.metallib" "$app/Contents/Resources/$1/page-host.metallib"
}
xcrun -sdk macosx metal -w "$repo/aethyr/host/apple/PageHost.metal" -o "$work/c/page-host.metallib"
build_realm everglade2
emit_page everglade2 everglade2 everglade2 everglade2-fx e2fx
# Experimental: the fivelab module on its page draws with Character Create's passes
# (the site's data-wgsl="/charactercreator.wgsl"), and its pack is the site's own file, gzipped as
# the site serves it; the page host inflates it as a browser does.
build_realm fivelab
emit_page fivelab charactercreator charactercreator fivelab-fx fxg
cp "$repo/apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz" "$app/Contents/Resources/fivelab/glade.pack.gz"
# The Archery Range: a module of its own (apps/openagents-web/archery-range.coax, an Everglade zone
# with the range on it), drawn with the same passes. Its pack is Experimental's, so the app keeps one
# copy: the realm table's row for it (Sources/Realms.swift) names module archery-range and pack
# fivelab/glade.pack.gz.
build_realm archery-range
emit_page archery-range charactercreator charactercreator fivelab-fx fxg
# The Bitcoin Lab: its own module (apps/openagents-web/bitcoin-lab-walk.coax), the same passes with its
# Grid and stations in the particle pass (bitcoin-lab-fx), Experimental's pack.
build_realm bitcoin-lab-walk
emit_page bitcoin-lab-walk charactercreator charactercreator bitcoin-lab-fx qfx
# The one pack reader (aethyr/host/pack-table.coax's pack_table, #13304) as C: PageHost.m and Engine.swift find a pack's entries through it.
"$coax" c "$repo/aethyr/host/pack-table-face.coax" --library --header "$work/c/aethyr-pack-table.h" > "$work/c/aethyr-pack-table.c"
/usr/bin/clang "${cflags[@]}" -w -c "$work/c/aethyr-pack-table.c" -o "$work/c/aethyr-pack-table.o"
/usr/bin/clang -fobjc-arc -fmodules -O2 -Wno-shadow-ivar -I"$work/c" -c "$repo/aethyr/host/apple/PageHost.m" -o "$work/c/page-host.o"
# The realms' sound: Aethyr's Apple audio host (aethyr/host/apple/audio-host.h, PageAudio.swift and
# Aethyrforge's MetalAudio.swift) over the shared Coax audio authorities, native-audio.coax as C,
# and the site's sound files a page names (/realms/everglade/sfx/ and their PCM companions).
"$coax" c "$repo/aethyr/host/native-audio.coax" --library --header "$work/c/native-audio.h" > "$work/c/native-audio.c"
/usr/bin/clang "${cflags[@]}" -w -c "$work/c/native-audio.c" -o "$work/c/native-audio.o"
printf '#include "native-audio.h"\n#include "%s"\n#include "%s"\n' "$repo/aethyr/host/apple/audio-host.h" "$repo/aethyr/host/apple/audio-host.c" > "$work/c/audio-host-unit.c"
/usr/bin/clang "${cflags[@]}" -w -c "$work/c/audio-host-unit.c" -o "$work/c/audio-host.o"
# The pictures the realm names (the classes' and spells' icons), where the site has them.
mkdir -p "$app/Contents/Resources/web/realms/everglade" "$app/Contents/Resources/web/realms/5e-mmo"
rsync -a --delete "$repo/apps/openagents-web/realms/everglade/hotbar" "$app/Contents/Resources/web/realms/everglade/"
rsync -a --delete "$repo/apps/openagents-web/realms/5e-mmo/hotbar" "$app/Contents/Resources/web/realms/5e-mmo/"
rsync -a --delete "$repo/apps/openagents-web/realms/everglade/sfx" "$repo/apps/openagents-web/realms/everglade/pcm-v1" "$app/Contents/Resources/web/realms/everglade/"
# Everglade's faces and their licences (SIL OFL 1.1), as the web realm ships them.
rm -rf "$app/Contents/Resources/fonts"
cp -R "$repo/apps/openagents-web/realms/everglade2/assets/fonts" "$app/Contents/Resources/fonts"
cp "$repo/apps/openagents-web/realms/everglade2/assets/everglade2.pack" "$app/Contents/Resources/everglade2.pack"
# The web's character code passes its Creator and Bodies by value, megabytes a call, so the main
# thread, which runs every Coax call, gets a 512 MB stack (reserved, not touched until used).
# The shared Mac host, and the component shader it draws the shared components with (components/backend/gpu-shader.coax as Metal).
/usr/bin/clang -fobjc-arc -O2 -w -I"$repo/apps/shared/host/linux" -I"$repo/apps/shared/host/apple" -c "$repo/apps/shared/host/apple/ui-host.m" -o "$work/c/ui-host.o"
"$coax" metal "$repo/components/backend/gpu-shader.coax" --vertex chat_fx_vs --fragment chat_fx_fs -o "$work/c/gpu-shader.metal" 2> "$work/c/gpu-shader.bind"
xcrun -sdk macosx metal "$work/c/gpu-shader.metal" -o "$app/Contents/Resources/zeron.metallib"
# The OpenAgents chat (apps/openagents-desktop/linux/chat.h), the Workshop's panel: the app's chat unit as C
# (zeron/linux.coax and its NIP-98 signer, as that app builds them), the component, CoreText's atlas,
# and no dictation (dictation-none.c). Its fonts and icons go in Resources/chat, apart from Everglade's faces.
oa="$repo/apps/openagents-desktop"
chatflags=(-std=gnu11 -O2 -ffp-contract=off -fno-fast-math -w -I"$work/c/chat" -I"$oa" -I"$oa/linux" -I"$repo/apps/shared/host/linux" -I"$repo/apps/shared/host/apple" -I"$repo/coax/platform" -I"$repo/psionic/platform")
mkdir -p "$work/c/chat"
"$coax" c "$oa/zeron/linux.coax" --library --header "$work/c/chat/zeron.h" > "$work/c/chat/zeron.c"
/usr/bin/clang "${chatflags[@]}" -DS_hash=eg_zeron_S_hash -DS_hex=eg_zeron_S_hex -c "$work/c/chat/zeron.c" -o "$work/c/chat/zeron.o"
"$coax" c "$oa/lab-sign.coax" --run-steps 250000000 --library --header "$work/c/chat/lab-sign.h" > "$work/c/chat/lab-sign.c"
/usr/bin/clang "${chatflags[@]}" -DS_hash=eg_lab_S_hash -DS_hex=eg_lab_S_hex -c "$work/c/chat/lab-sign.c" -o "$work/c/chat/lab-sign-all.o"
ld -r "$work/c/chat/lab-sign-all.o" -exported_symbol _coaxial_lab_nip98 -o "$work/c/chat/lab-sign.o"
for c in "$oa/linux/chat.c" "$oa/linux/dictation-none.c" "$oa/zeron-face.c" "$repo/coax/platform/store-file-native.c"; do
  /usr/bin/clang "${chatflags[@]}" -c "$c" -o "$work/c/chat/$(basename "$c" .c).o"
done
/usr/bin/clang -fobjc-arc -O2 -w -I"$work/c/chat" -I"$oa" -I"$repo/apps/shared/host/linux" -I"$repo/apps/shared/host/apple" -c "$repo/apps/shared/host/apple/ct-atlas.m" -o "$work/c/chat/ct-atlas.o"
rm -rf "$app/Contents/Resources/chat"; mkdir -p "$app/Contents/Resources/chat"
cp -R "$repo/components/zeron/fonts" "$repo/components/zeron/icons" "$repo/components/zeron/NOTICE" "$app/Contents/Resources/chat/"
printf '#include "%s"\n#import "%s"\n#include "%s"\n#include "%s"\n#include "%s"\n#include "%s"\n' "$here/face.h" "$repo/aethyr/host/apple/page-host.h" "$repo/aethyr/host/apple/audio-host.h" "$repo/apps/shared/host/apple/ui-host-apple.h" "$oa/linux/chat.h" "$repo/aethyr/host/pack-table.h" > "$work/c/bridge.h"
xcrun -sdk macosx swiftc -O -Xcc -I"$work/c" -import-objc-header "$work/c/bridge.h" "$here"/Sources/*.swift "$repo/aethyr/host/apple/PageAudio.swift" "$repo/apps/aethyrforge-desktop/Sources/MetalAudio.swift" "$work/c/ui-host.o" "$work/c/page-host.o" "$work/c/aethyr-pack-table.o" "$work/c/audio-host.o" "$work/c/native-audio.o" "$work/c/workshop.o" "$work/c/face.o" "$work/c/model.o" "$work/c/model-face.o" "$work/c/next-face.o" "$work/c/temple.o" "$work/c/place-face.o" "$work/c/process.o" "$work/c/chat"/{zeron,lab-sign,chat,dictation-none,zeron-face,store-file-native,ct-atlas}.o \
  -framework AppKit -framework Metal -framework MetalKit -framework QuartzCore -framework ImageIO -framework CoreGraphics -framework CoreText -framework AVFoundation -framework UniformTypeIdentifiers -lm -lz \
  -Xlinker -export_dynamic -Xlinker -stack_size -Xlinker 0x20000000 -o "$app/Contents/MacOS/Everglade"
# The context program: built as any Coax tool is (tool.sh keeps the executable by the compiler's
# hash; an unknown verb builds it and does nothing else), then copied in beside the app's own.
"$repo/coax/web/tool.sh" "$here/next-context.coax" build-only >/dev/null 2>&1 || true
tool="$repo/coax/web/build/tools/next-context-$(shasum -a 256 "$coax" | cut -d ' ' -f 1)"
[[ -x "$tool" ]] || { echo "build.sh: next-context did not build; run coax/web/tool.sh apps/everglade-desktop/next-context.coax status" >&2; exit 1; }
cp "$tool" "$app/Contents/MacOS/everglade-next"
# The Golems board's reader (golems.coax), the same way.
"$repo/coax/web/tool.sh" "$here/golems.coax" build-only >/dev/null 2>&1 || true
tool="$repo/coax/web/build/tools/golems-$(shasum -a 256 "$coax" | cut -d ' ' -f 1)"
[[ -x "$tool" ]] || { echo "build.sh: golems did not build; run coax/web/tool.sh apps/everglade-desktop/golems.coax snapshot --now 1" >&2; exit 1; }
cp "$tool" "$app/Contents/MacOS/everglade-golems"
# The checkout a worktree for an issue branches off: the main one, whichever worktree builds this.
dirname "$(git -C "$repo" rev-parse --path-format=absolute --git-common-dir)" > "$app/Contents/Resources/repo.txt"
cp "$here/Info.plist" "$app/Contents/Info.plist"
cp "$here/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"   # apps/everglade-mobile/icon/README.md: the iOS icon on a macOS rounded square
codesign --force --sign - "$app" >/dev/null
echo "built $app"
# --open runs a copy outside the checkout, so the app outlives a worktree. What follows --open
# goes to the app: `build.sh --open --scene 1` opens it in Everglade.
if [[ "${1:-}" == --open ]]; then
  shift
  home="$HOME/.openagents/everglade"
  mkdir -p "$home"
  rm -rf "$home/Everglade.app"
  ditto "$app" "$home/Everglade.app"
  if [[ $# -gt 0 ]]; then open "$home/Everglade.app" --args "$@"; else open "$home/Everglade.app"; fi
fi
exit 0
