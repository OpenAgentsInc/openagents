#!/usr/bin/env bash
# A realm natively: its Coax to C (as coax/web/wasm.sh does for wasm32),
# its browser face, and this directory's host, in OUT.
#   verse/native/build.sh REALM OUT [--render-profile SLUG]
# The optional profile binds this producer to a complete Coax-built renderer
# package, its shared source carrier/header, and native renderer effect sources.
# On Linux it then builds OUT/realm-run with gcc, without PIE, so every
# pointer the face hands back fits its 32 bits (realm-host.c). The realms:
#   everglade   apps/openagents-web/everglade.coax and everglade-face.c, as
#               apps/openagents-web/build.sh links them for the browser
#   stair       Hold the Stair: apps/openagents-web/stair.coax and stair-face.c (no pack)
#   any other   apps/openagents-web/REALM.coax and REALM-face.c with the libraries the site's
#               build (apps/openagents-web/build.sh) links it with: `build.sh --libraries REALM`
#               prints them. A realm whose Coax lives elsewhere (everglade2: everglade/client/client.coax)
#               is found by the site's own line for build/REALM.wasm: `build.sh --resolve REALM`
#               prints its module, its face and its libraries, a line each (#13389)
#   verse/native/build.sh --gpu NAME   prints the one gpu/NAME.coax the site's passes are written in
#               (apps/openagents-web/gpu/, or a product's own gpu/ folder under everglade/)
# The face gets its struct defaults from the compiler, as in the browser: each unit's header
# declares coax_default_T, and the face calls it for its globals and a new state (coaxial_state_init).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
web="$root/apps/openagents-web"
# The libraries the site's build links REALM's module with: the paths after its wasm.sh line's
# output, continued over backslashed lines, with $m (verse/), $a (aethyr/) and $here (apps/openagents-web) put in.
site_libraries() {
  awk -v want="\"\$here/$1.coax\"" '
    !on && index($0, "coax/web/wasm.sh\" " want " ") { on = 1; line = $0 }
    on && line != $0 { line = line " " $0 }
    on { if ($0 !~ /\\[ \t]*$/) { print line; exit } }' "$web/build.sh" |
    tr ' \t\\&' '\n\n\n\n' | tr -d '"' | grep '\.coax$' | sed 1d |
    sed -e "s#^\$m/#$root/verse/#" -e "s#^\$a/#$root/aethyr/#" -e "s#^\$here/#$web/#"
}
if [[ "${1:-}" == --libraries ]]; then site_libraries "${2:?usage: build.sh --libraries REALM}"; exit; fi
# The site's line that builds build/REALM.wasm, whatever its module is called and wherever it lives:
# every .coax and .c path on it in order (the module, the face, then the libraries), made absolute.
site_line() {
  awk -v want="\"\$here/build/$1.wasm\"" '
    !on && index($0, "coax/web/wasm.sh\" ") && index($0, " " want) { on = 1; line = $0 }
    on && line != $0 { line = line " " $0 }
    on { if ($0 !~ /\\[ \t]*$/) { print line; exit } }' "$web/build.sh" |
    tr ' \t\\&' '\n\n\n\n' | tr -d '"' | grep -e '\.coax$' -e '-face\.c$' |
    sed -e "s#^\$m/#$root/verse/#" -e "s#^\$a/#$root/aethyr/#" -e "s#^\$here/#$web/#" |
    while IFS= read -r l; do [[ -f "$l" ]] || { echo "build.sh: the site's line for $1 names $l, which is not a file" >&2; exit 2; }
      echo "$(cd "$(dirname "$l")" && pwd -P)/$(basename "$l")"; done
}
if [[ "${1:-}" == --resolve ]]; then
  lines="$(site_line "${2:?usage: build.sh --resolve REALM}")" || true
  [[ "$(sed -n 1p <<< "$lines")" == *.coax && "$(sed -n 2p <<< "$lines")" == *-face.c ]] || { echo "build.sh: no realm $2 (apps/openagents-web/build.sh has no wasm.sh line for build/$2.wasm)" >&2; exit 2; }
  echo "$lines"; exit
fi
if [[ "${1:-}" == --gpu ]]; then
  name="${2:?usage: build.sh --gpu NAME}"
  found="$(find "$web/gpu" "$root/everglade" -type f -path "*/gpu/$name.coax" -not -path '*/build/*' | sort)"
  [[ -n "$found" && "$(wc -l <<< "$found" | tr -d ' ')" == 1 ]] || { echo "build.sh: expected one gpu/$name.coax under apps/openagents-web/gpu or everglade/, found: ${found:-none}" >&2; exit 2; }
  echo "$found"; exit
fi
realm="${1:?usage: build.sh REALM OUT}"
out="${2:?usage: build.sh REALM OUT}"
shift 2
render_profile=""
if [[ $# -gt 0 ]]; then
  [[ $# == 2 && "$1" == --render-profile ]] || { echo "build.sh: expected --render-profile SLUG" >&2; exit 2; }
  render_profile="$2"
fi
units_too=(); faces_too=()   # Coax units kept by their own names, and C faces of their own
case "$realm" in
  everglade) app="$web/everglade.coax"; face="$web/everglade-face.c"
    libraries=("$root/aethyr/move/player.coax" "$root/verse/grid.coax" "$root/verse/everglade/river.coax" "$root/verse/everglade/guard-guide-policy.coax") ;;
  stair) app="$web/stair.coax"; face="$web/stair-face.c"; libraries=() ;;
  # a flat realm (belt, waterlab, orbitleague): its module and face, its libraries #loaded
  # the Grid (/realms/the-grid, the homepage's module): its session (verse/gridwalk.coax, which
  # loads the player and the thumbs) and the homepage's own units (coax/web/project/build-home.sh's:
  # the reader, the fold, the cache)
  newland) app="$web/newland.coax"; face="$web/newland-face.c"; libraries=("$root/verse/gridwalk.coax")
    units_too=("$root/coax/web/project/landing-browser.coax" "$root/coax/web/project/landing-fold.coax")
    faces_too=("$root/coax/web/project/landing-reader-face.c" "$root/coax/web/project/landing-fold-face.c" "$root/coax/web/project/landing-cache-face.c") ;;
  # the galaxy (the verse app's page 1, which /realms opens natively), where travel starts
  verse) app="$web/verse.coax"; face="$web/verse-face.c"; libraries=() ;;
  belt|waterlab|orbitleague) app="$web/$realm.coax"; face="$web/$realm-face.c"; libraries=() ;;
  *) app="$web/$realm.coax"; face="$web/$realm-face.c"; libraries=()
    if [[ -f "$app" && -f "$face" ]]; then
      if [[ -f "$web/build.sh" ]]; then while IFS= read -r l; do libraries+=("$(cd "$(dirname "$l")" && pwd -P)/$(basename "$l")"); done < <(site_libraries "$realm"); fi
    elif [[ -f "$web/build.sh" ]]; then
      # its module is not apps/openagents-web/REALM.coax: the site's line for build/REALM.wasm names it
      resolved=(); while IFS= read -r l; do resolved+=("$l"); done < <("$0" --resolve "$realm")
      [[ ${#resolved[@]} -ge 2 ]] || { [[ -f "$out/sources" ]] || exit 2; resolved=("$app" "$face"); }
      app="${resolved[0]}"; face="${resolved[1]}"; libraries=(${resolved[@]:2})
    else
      [[ -f "$out/sources" ]] || { echo "build.sh: no realm $realm" >&2; exit 2; }
    fi ;;
esac
# The module's unit is named as its file is (client.coax gives client.h, which its face includes).
unit="$(basename "$app" .coax)"
mkdir -p "$out"
out="$(cd "$out" && pwd -P)"
# fivelab's and the range's checks run natively, as coax/web/wasm.sh runs them (native_check_units):
# interpreted, fivelab's film-stage check runs past the compiler's traversal budget and nothing is emitted.
case "$realm" in fivelab|archery-range|bitcoin-lab-walk) export COAX_NATIVE_CHECKS=1 ;; esac
coax="${COAX:-$root/coax/bin/coax}"
if [[ -x "$coax" ]]; then
  relative() { echo "${1#"$root"/}"; }
  emit_args=()
  if [[ -n "$render_profile" ]]; then
    # All seventeen composed source carriers were measured with this compiler;
    # Everglade's 102,161 aggregate initialization steps set the largest case.
    emit_args=(--run-steps "${COAXIAL_RUN_STEPS:-128000}")
  elif [[ -n "${COAXIAL_RUN_STEPS:-}" ]]; then
    emit_args=(--run-steps "$COAXIAL_RUN_STEPS")
  fi
  source_app="$app"
  if [[ -n "$render_profile" && "$realm" != newland ]]; then
    source_app="$root/apps/openagents-web/carriers/$realm.coax"
    [[ -f "$source_app" ]] || { echo "build.sh: no renderer source carrier for $realm" >&2; exit 2; }
  fi
  generated=()
  (cd "$root" && "$coax" c "$(relative "$source_app")" --library --header "$out/$unit.h" ${emit_args[@]+"${emit_args[@]}"}) > "$out/$unit.c" & generated+=($!)
  for library in ${libraries[@]+"${libraries[@]}"}; do
    base="$(basename "$library" .coax)"; base="$(printf '%s' "${base:0:1}" | tr a-z A-Z)${base:1}"
    (cd "$root" && "$coax" c "$(relative "$library")" --library --header "$out/$base.h" ${emit_args[@]+"${emit_args[@]}"}) > "$out/$base.c" & generated+=($!)
  done
  for unit in ${units_too[@]+"${units_too[@]}"}; do
    base="$(basename "$unit" .coax)"
    source_unit="$unit"
    if [[ -n "$render_profile" && "$realm" == newland && "$base" == landing-browser ]]; then
      source_unit="$root/apps/openagents-web/carriers/landing-browser.coax"
    fi
    (cd "$root" && "$coax" c "$(relative "$source_unit")" --library --header "$out/$base.h" ${emit_args[@]+"${emit_args[@]}"}) > "$out/$base.c" & generated+=($!)
  done
  for pid in "${generated[@]}"; do wait "$pid"; done
  if [[ -n "$render_profile" ]]; then
    owner_header="$out/$unit.h"
    [[ "$realm" == newland ]] && owner_header="$out/landing-browser.h"
    cp "$owner_header" "$out/RenderHost.h"
    (cd "$root" && COAX="$coax" coax/web/tool.sh aethyr/host/prepare-native-network.coax "$out" "$coax")
    mkdir -p "$out/render-native"
    (cd "$root" && "$coax" c aethyr/render/native-images.coax --library --header "$out/render-native/RenderTexture.h") > "$out/render-native/RenderTexture.c"
    cp "$here/render-host.c" "$here/render-host.h" "$here/render-realm.c" "$here/render-realm.h" "$here/render-textures.c" "$here/render-textures.h" "$here/render-media.c" "$here/render-media.h" "$here/render-images.c" "$here/render-images.h" "$here/render-live-media.c" "$here/render-live-media.h" "$root/aethyr/render/tests/frame-input.h" "$out/render-native/"
    (cd "$root" && COAX="$coax" coax/web/tool.sh aethyr/render/build.coax "$render_profile" "$out/render" --coax "$coax")
    (cd "$root" && COAX="$coax" coax/web/tool.sh verse/native/render-package.coax --package "$out/render/render.json" --app "$realm" --profile "$render_profile" --output "$out/RenderIdentity.h")
    (cd "$root" && COAX="$coax" coax/web/tool.sh aethyr/render/build.coax --asset-table "$render_profile") > "$out/render-assets.pending.json"
    (cd "$root" && COAX="$coax" coax/web/tool.sh verse/native/render-assets.coax --table "$out/render-assets.pending.json" --package "$out/render/render.json" --web-root "$web" --output "$out/render-assets")
    rm "$out/render-assets.pending.json"
  fi
  cp "$face" "$web"/*-face.h "$web"/*-bridge.h "$web/face-address.h" ${faces_too[@]+"${faces_too[@]}"} "$out/"
  cp "$here"/face.h "$here"/face-defaults.h "$here"/realm-host.h "$here"/realm-host.c "$here"/pack-table-gen.h "$here"/audio-host.h "$here"/realm-run.c "$out/"
  units="$(cd "$out" && ls ./*.c | grep -v -e face -e realm-host -e realm-run | xargs -n1 basename | tr '\n' ' ')"
  # C the face includes itself (the haunted house's realm-flight.c ...), and a header of the
  # site's it shares with another face (experimental-sound-bank.h): beside it, not a unit
  # (and what those include in turn: lab-walk-face.h names the sound bank for the Bitcoin Lab's face)
  seen=" "; todo=("$face")
  while [[ ${#todo[@]} -gt 0 ]]; do
    from="${todo[0]}"; todo=("${todo[@]:1}")
    for inc in $(grep -o '^#include "[^"]*\.[ch]"' "$from" | cut -d'"' -f2); do
      [[ "$seen" == *" $inc "* || ! -f "$web/$inc" ]] && continue
      seen="$seen$inc "; cp "$web/$inc" "$out/"; todo+=("$web/$inc")
    done
  done
  # The face's unit, so the export aliases below go with it rather than into the face.
  printf '#include "%s"\n' "$(basename "$face")" > "$out/$realm.face.c"
  # An export named apart from its C function (the homepage's EXPORT("coaxial_page") void page): wasm
  # renames it; natively the name is an alias of the function, so the host's call reaches it.
  grep -o 'EXPORT("[a-z_0-9]*")[^(;{]*(' "$face" | sed -E 's/^EXPORT\("([a-z_0-9]+)"\).*[^a-z_0-9]([a-z_0-9]+)\($/\1 \2/' |
    while read -r name fn; do
      [[ -n "$fn" && "$name" != "$fn" ]] || continue
      (cd "$out" && grep -qw -- "$name" $units) && continue   # a unit's own procedure already (#export_c)
      echo "extern __typeof__($fn) $name __attribute__((alias(\"$fn\")));"
    done >> "$out/$realm.face.c" || true
  extra=""; for f in ${faces_too[@]+"${faces_too[@]}"}; do extra="$extra $(basename "$f")"; done
  echo "$realm.face.c $units$extra" > "$out/sources"
fi
# COAXIAL_REALM_EMIT_ONLY=1 stops here on Linux too: the C and its sources list, as an app's build takes them (verse/native/app-realms.coax).
[[ "$(uname)" == Linux && -z "${COAXIAL_REALM_EMIT_ONLY:-}" ]] || exit 0
read -r -a sources < "$out/sources"
# The browser's flags: no fast math and no contraction, so it computes what the checks computed.
flags=(-std=gnu11 -O2 -no-pie -ffp-contract=off -fno-fast-math -Wno-attributes -Wno-unused-function -Wno-unused-variable
       -Wno-unused-but-set-variable -w -I "$out")
link_flags=(); network_libs=()
if [[ -n "$render_profile" ]]; then
  flags+=(-ffunction-sections -fdata-sections)
  link_flags+=(-Wl,--gc-sections)
  read -r -a network_cflags < "$out/network.cflags" || true
  read -r -a network_libs < "$out/network.libs" || true
  flags+=("${network_cflags[@]}")
fi
objects=()
pids=()
for s in "${sources[@]}" realm-host.c; do
  gcc "${flags[@]}" -c "$out/$s" -o "$out/${s%.c}.o" & objects+=("$out/${s%.c}.o"); pids+=($!)
done
for pid in "${pids[@]}"; do wait "$pid"; done
# the wasm's math (musl's, verse/native/musl-math.sh) ahead of glibc's
read -r -a math <<< "$("$here/musl-math.sh" "$out")"
gcc "${flags[@]}" ${link_flags[@]+"${link_flags[@]}"} "$out/realm-run.c" "${objects[@]}" ${math[@]+"${math[@]}"} ${network_libs[@]+"${network_libs[@]}"} -lpthread -lm -o "$out/realm-run"
echo "$out/realm-run"
