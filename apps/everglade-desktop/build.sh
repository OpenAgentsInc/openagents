#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax, clang, metal and swiftc; holds no logic)
# Builds Everglade.app for this Mac (everglade/docs/desktop-app-spec.md): the worlds (the list,
# the Workshop, and Everglade itself: the web's everglade2 realm run natively) with the Next issue panel (desktop.coax) and your model (model.coax) compiled to C
# behind face.h, Next issue (next.coax, in the scene's unit: the Coax Claude Agent SDK), the
# context program it asks the host to run (next-context.coax, as everglade-next), their shaders,
# Everglade II's pack, and the AppKit and Metal host in Swift.
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
# The Everglade row: the web's everglade2 realm (Character Create, then the glade), natively, drawn
# by Aethyr's page host for Metal (aethyr/host/apple/). Its Coax as C and its face are built as
# verse/native/build.sh builds any realm, into a library the host loads; its shaders are the web's
# own Coax (apps/openagents-web/gpu/), emitted for Metal a pair at a time, each with the list of
# what it reads (.bind). The C takes over a minute to emit, so it is kept until its sources change.
realm="$work/realm"; pages="$app/Contents/Resources/everglade2"
mkdir -p "$realm" "$pages"
key="$( (shasum -a 256 "$coax" | cut -d ' ' -f 1; git -C "$repo" rev-parse HEAD:apps/openagents-web HEAD:aethyr HEAD:verse HEAD:coax/lib; git -C "$repo" status --porcelain -- apps/openagents-web aethyr verse coax/lib ':(exclude)aethyr/host/apple'; git -C "$repo" diff HEAD -- apps/openagents-web aethyr verse coax/lib ':(exclude)aethyr/host/apple') | shasum -a 256 | cut -d ' ' -f 1)"
if [[ "$(cat "$realm/key" 2>/dev/null)" != "$key" || ! -f "$realm/libeverglade2.dylib" ]]; then
  rm -f "$realm/key"
  COAX="$coax" "$repo/verse/native/build.sh" everglade2 "$realm" >/dev/null
  read -r -a realm_sources < "$realm/sources"
  realm_objects=(); realm_pids=()
  for s in "${realm_sources[@]}"; do
    /usr/bin/clang -std=gnu11 -O2 -ffp-contract=off -fno-fast-math -DCOAXIAL_NATIVE_ADDRESS -w -I"$realm" -c "$realm/$s" -o "$realm/${s%.c}.o" & realm_pids+=($!); realm_objects+=("$realm/${s%.c}.o")
  done
  for pid in "${realm_pids[@]}"; do wait "$pid"; done
  # The face's imports (the socket, HTTP, the clock) are the host's, found when the library loads.
  /usr/bin/clang -dynamiclib -Wl,-undefined,dynamic_lookup "${realm_objects[@]}" -lm -o "$realm/libeverglade2.dylib"
  echo "$key" > "$realm/key"
fi
cp "$realm/libeverglade2.dylib" "$pages/libeverglade2.dylib"
# Metal's relaxed math mode (it honours NaN and infinity, and keeps the math functions precise):
# the scene has millions of vertices a frame, and its vertex stage takes about 45% longer in the
# safe mode on this Mac (19.4 ms against 13.5 ms a frame at 990 by 660).
emit_pair() {   # ROLE SOURCE VERTEX FRAGMENT
  "$coax" metal "$repo/apps/openagents-web/gpu/$2.coax" --vertex "$3" --fragment "$4" -o "$work/c/e2-$1.metal" 2> "$pages/$1.bind"
  xcrun -sdk macosx metal -w -fmetal-math-mode=relaxed -fmetal-math-fp32-functions=precise "$work/c/e2-$1.metal" -o "$pages/$1.metallib"
}
emit_pair bg everglade2 everglade2_bg_vs everglade2_bg_fs
emit_pair shadow everglade2 everglade2_shadow_vs everglade2_shadow_fs
emit_pair scene everglade2 everglade2_scene_vs everglade2_scene_fs
emit_pair bloom everglade2 everglade2_bg_vs everglade2_bloom_fs
emit_pair post everglade2 everglade2_bg_vs everglade2_post_fs
emit_pair fx everglade2 everglade2_fx_vs everglade2_fx_fs
emit_pair particle everglade2-fx e2fx_particle_vs e2fx_particle_fs
emit_pair particle_over everglade2-fx e2fx_particle_over_vs e2fx_particle_over_fs
cp "$repo/apps/openagents-web/gpu/everglade2.bindings" "$pages/page.bindings"
xcrun -sdk macosx metal -w "$repo/aethyr/host/apple/PageHost.metal" -o "$pages/page-host.metallib"
/usr/bin/clang -fobjc-arc -fmodules -O2 -Wno-shadow-ivar -c "$repo/aethyr/host/apple/PageHost.m" -o "$work/c/page-host.o"
# The pictures the realm names (the classes' and spells' icons), where the site has them.
mkdir -p "$app/Contents/Resources/web/realms/everglade" "$app/Contents/Resources/web/realms/5e-mmo"
rsync -a --delete "$repo/apps/openagents-web/realms/everglade/hotbar" "$app/Contents/Resources/web/realms/everglade/"
rsync -a --delete "$repo/apps/openagents-web/realms/5e-mmo/hotbar" "$app/Contents/Resources/web/realms/5e-mmo/"
# Everglade's faces and their licences (SIL OFL 1.1), as the web realm ships them.
rm -rf "$app/Contents/Resources/fonts"
cp -R "$repo/apps/openagents-web/realms/everglade2/assets/fonts" "$app/Contents/Resources/fonts"
cp "$repo/apps/openagents-web/realms/everglade2/assets/everglade2.pack" "$app/Contents/Resources/everglade2.pack"
# The web's character code passes its Creator and Bodies by value, megabytes a call, so the main
# thread, which runs every Coax call, gets a 512 MB stack (reserved, not touched until used).
printf '#include "%s"\n#import "%s"\n' "$here/face.h" "$repo/aethyr/host/apple/page-host.h" > "$work/c/bridge.h"
xcrun -sdk macosx swiftc -O -import-objc-header "$work/c/bridge.h" "$here"/Sources/*.swift "$work/c/page-host.o" "$work/c/workshop.o" "$work/c/face.o" "$work/c/model.o" "$work/c/model-face.o" "$work/c/next-face.o" "$work/c/temple.o" "$work/c/place-face.o" "$work/c/process.o" \
  -framework AppKit -framework Metal -framework MetalKit -framework QuartzCore -framework ImageIO -framework CoreGraphics -framework CoreText -lm \
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
