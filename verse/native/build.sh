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
#               prints them
# The face gets its struct defaults from the compiler, as in the browser: each unit's header
# declares coax_default_T, and the face calls it for its globals and a new state (coaxial_state_init).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../.." && pwd -P)"
web="$root/apps/openagents-web"
# The libraries the site's build links REALM's module with: the paths after its wasm.sh line's
# output, continued over backslashed lines, with $m (verse/) and $here (apps/openagents-web) put in.
site_libraries() {
  awk -v want="\"\$here/$1.coax\"" '
    !on && index($0, "coax/web/wasm.sh\" " want " ") { on = 1; line = $0 }
    on && line != $0 { line = line " " $0 }
    on { if ($0 !~ /\\[ \t]*$/) { print line; exit } }' "$web/build.sh" |
    tr ' \t\\&' '\n\n\n\n' | tr -d '"' | grep '\.coax$' | sed 1d |
    sed -e "s#^\$m/#$root/verse/#" -e "s#^\$here/#$web/#"
}
if [[ "${1:-}" == --libraries ]]; then site_libraries "${2:?usage: build.sh --libraries REALM}"; exit; fi
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
    libraries=("$root/verse/player.coax" "$root/verse/grid.coax" "$root/verse/everglade/river.coax" "$root/verse/everglade/guard-guide-policy.coax") ;;
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
  *) [[ -f "$web/$realm.coax" && -f "$web/$realm-face.c" ]] || [[ -f "$out/sources" ]] || { echo "build.sh: no realm $realm" >&2; exit 2; }
    app="$web/$realm.coax"; face="$web/$realm-face.c"; libraries=()
    if [[ -f "$web/build.sh" ]]; then while IFS= read -r l; do libraries+=("$(cd "$(dirname "$l")" && pwd -P)/$(basename "$l")"); done < <(site_libraries "$realm"); fi ;;
esac
mkdir -p "$out"
out="$(cd "$out" && pwd -P)"
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
    source_app="$root/aethyr/render/carriers/$realm.coax"
    [[ -f "$source_app" ]] || { echo "build.sh: no renderer source carrier for $realm" >&2; exit 2; }
  fi
  generated=()
  (cd "$root" && "$coax" c "$(relative "$source_app")" --library --header "$out/$realm.h" ${emit_args[@]+"${emit_args[@]}"}) > "$out/$realm.c" & generated+=($!)
  for library in ${libraries[@]+"${libraries[@]}"}; do
    base="$(basename "$library" .coax)"; base="$(printf '%s' "${base:0:1}" | tr a-z A-Z)${base:1}"
    (cd "$root" && "$coax" c "$(relative "$library")" --library --header "$out/$base.h" ${emit_args[@]+"${emit_args[@]}"}) > "$out/$base.c" & generated+=($!)
  done
  for unit in ${units_too[@]+"${units_too[@]}"}; do
    base="$(basename "$unit" .coax)"
    source_unit="$unit"
    if [[ -n "$render_profile" && "$realm" == newland && "$base" == landing-browser ]]; then
      source_unit="$root/aethyr/render/carriers/landing-browser.coax"
    fi
    (cd "$root" && "$coax" c "$(relative "$source_unit")" --library --header "$out/$base.h" ${emit_args[@]+"${emit_args[@]}"}) > "$out/$base.c" & generated+=($!)
  done
  for pid in "${generated[@]}"; do wait "$pid"; done
  if [[ -n "$render_profile" ]]; then
    owner_header="$out/$realm.h"
    [[ "$realm" == newland ]] && owner_header="$out/landing-browser.h"
    cp "$owner_header" "$out/RenderHost.h"
    mkdir -p "$out/render-native"
    (cd "$root" && "$coax" c aethyr/render/native-texture.coax --library --header "$out/render-native/RenderTexture.h") > "$out/render-native/RenderTexture.c"
    cp "$here/render-host.c" "$here/render-host.h" "$here/render-realm.c" "$here/render-realm.h" "$here/render-textures.c" "$here/render-textures.h" "$here/render-media.c" "$here/render-media.h" "$root/aethyr/render/tests/frame-input.h" "$out/render-native/"
    (cd "$root" && COAX="$coax" coax/web/tool.sh aethyr/render/build.coax "$render_profile" "$out/render" --coax "$coax")
    python3 "$here/render_package.py" --package "$out/render/render.json" --app "$realm" --profile "$render_profile" --output "$out/RenderIdentity.h"
    (cd "$root" && COAX="$coax" coax/web/tool.sh aethyr/render/build.coax --asset-table "$render_profile") > "$out/render-assets.pending.json"
    python3 "$here/render_assets.py" --table "$out/render-assets.pending.json" --package "$out/render/render.json" --web-root "$web" --output "$out/render-assets"
    rm "$out/render-assets.pending.json"
  fi
  cp "$face" "$web"/*-face.h "$web"/*-bridge.h ${faces_too[@]+"${faces_too[@]}"} "$out/"
  cp "$here"/face.h "$here"/realm-host.h "$here"/realm-host.c "$here"/realm-run.c "$out/"
  units="$(cd "$out" && ls ./*.c | grep -v -e face -e realm-host -e realm-run | xargs -n1 basename | tr '\n' ' ')"
  # C the face includes itself (the haunted house's realm-flight.c ...): beside it, not a unit
  for inc in $(grep -o '^#include "[^"]*\.c"' "$face" | cut -d'"' -f2); do cp "$web/$inc" "$out/"; done
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
[[ "$(uname)" == Linux ]] || exit 0
read -r -a sources < "$out/sources"
# The browser's flags: no fast math and no contraction, so it computes what the checks computed.
flags=(-std=gnu11 -O2 -no-pie -ffp-contract=off -fno-fast-math -Wno-attributes -Wno-unused-function -Wno-unused-variable
       -Wno-unused-but-set-variable -w -I "$out")
link_flags=()
if [[ -n "$render_profile" ]]; then
  flags+=(-ffunction-sections -fdata-sections)
  link_flags+=(-Wl,--gc-sections)
fi
objects=()
pids=()
for s in "${sources[@]}" realm-host.c; do
  gcc "${flags[@]}" -c "$out/$s" -o "$out/${s%.c}.o" & objects+=("$out/${s%.c}.o"); pids+=($!)
done
for pid in "${pids[@]}"; do wait "$pid"; done
# the wasm's math (musl's, verse/native/musl-math.sh) ahead of glibc's
read -r -a math <<< "$("$here/musl-math.sh" "$out")"
gcc "${flags[@]}" ${link_flags[@]+"${link_flags[@]}"} "$out/realm-run.c" "${objects[@]}" ${math[@]+"${math[@]}"} -lm -o "$out/realm-run"
echo "$out/realm-run"
