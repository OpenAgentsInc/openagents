#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: runs the exporter, the importer, Unreal's packager and its browser harness in order)
# The Splice Bots jam build: the game as it stands on this checkout (its shipped world, or a saved world
# `splice-bots` baked in; no server either way), exported to Unreal, imported into the slice's project with
# its sounds, checked there, packaged for the web (UE 5.9's Wasm target), played in headless Chrome at
# 1440x900 (the scripted run, then real keys) and zipped. README.md, "In the Unreal host".
#   metaverse/splice-bots/tools/jam-build.sh OUT_DIR [WORLD_DIR]
# With no WORLD_DIR the export is the shipped world (content.coax: a new world, the farm opening). WORLD_DIR
# holds a saved world's live lane as a dev server keeps it (splice-bots.live, splice-bots.live.tables, in its
# checkout's apps/openagents-web/build/.worlds) to bake that world instead. OUT_DIR must not exist.
# SPLICE_UE_PROJECT (default ~/UnrealProjects/SpliceSlice1/AethyrHost.uproject) and SPLICE_UE_ENGINE
# (default ~/UnrealEngine-5.9/src) name the project and the engine. Run from the repository root.
set -euo pipefail
[[ $# -eq 1 || $# -eq 2 ]] || { echo "usage: metaverse/splice-bots/tools/jam-build.sh OUT_DIR [WORLD_DIR]" >&2; exit 2; }
out="$1"; world=""; [[ $# -eq 2 ]] && world="$(cd "$2" && pwd)"
[[ ! -e "$out" ]] || { echo "jam-build: $out exists; give a folder that does not" >&2; exit 1; }
mkdir -p "$out"; out="$(cd "$out" && pwd)"
repo="$(pwd)"
project="${SPLICE_UE_PROJECT:-$HOME/UnrealProjects/SpliceSlice1/AethyrHost.uproject}"
engine="${SPLICE_UE_ENGINE:-$HOME/UnrealEngine-5.9/src}"
bundle="$out/splice.aexport"; web="$out/web"; zip="$out/SpliceBots_Splicebots.zip"
stage="$web/Wasm"   # the archive's playable folder (Wasm-Debug beside it holds the symbols, not shipped)

echo "jam-build: 1/6 export${world:+ with the world in $world}"
coax/web/tool.sh metaverse/splice-bots/splice-export.coax "$bundle" ${world:+"$world"}

echo "jam-build: 2/6 import into $project"
# The importer's fidelity checks: the far mesas' few welded vertices and the boss-chamber reference shot drift
# (known since the garage moved the world out to 2.9 km); any other failure stops the build. The level is saved
# before the checks are read.
if ! coax/web/tool.sh metaverse-engine/tools/unreal-import.coax "$bundle" "$project" "$engine" > "$out/import.log" 2>&1; then
  report="$(dirname "$project")/Saved/aethyr-import-report.json"
  others="$(python3 -I -c 'import json,re,sys; r=json.load(open(sys.argv[1])); print("\n".join(f for f in r["failures"] if not re.match(r"(\d+ vertices differ from world\.glb|shot boss-chamber: mean colour difference)", f)))' "$report")"
  if [[ -n "$others" ]]; then cat "$out/import.log"; echo "jam-build: the import failed: $others" >&2; exit 1; fi
  echo "jam-build: the import's known drift only (see $out/import.log)"
fi

echo "jam-build: 2b the sounds (realms/splice-bots/assets/sounds) into the project and its overrides rows"
SPLICE_SOUNDS="$repo/apps/openagents-web/realms/splice-bots/assets/sounds" "$engine/Engine/Binaries/Mac/UnrealEditor-Cmd" "$project" "-ExecutePythonScript=$repo/metaverse/splice-bots/tools/ue-sounds.py" -unattended -nosplash -RenderOffscreen \
  "-abslog=$out/sounds.log" > /dev/null 2>&1 < /dev/null || true
grep -q 'UE_SOUNDS' "$out/sounds.log" || { tail -20 "$out/sounds.log"; echo "jam-build: the sounds did not go in" >&2; exit 1; }
grep -o 'UE_SOUNDS.*' "$out/sounds.log"

echo "jam-build: 3/6 the scripted run inside Unreal"
"$engine/Engine/Binaries/Mac/UnrealEditor-Cmd" "$project" /Game/Aethyr/Splice/Generated/L_Splice -game -RenderOffscreen -unattended -nosplash \
  -AethyrScript -AethyrExit "-abslog=$out/ue-script.log" > /dev/null 2>&1 || true
grep -o 'AETHYR_SCRIPT .*' "$out/ue-script.log" | grep -v 'AETHYR_SCRIPT result' > "$out/ue-script.lines" || true
if ! diff <(cat "$bundle/rules/run.expected"; echo) "$out/ue-script.lines" > /dev/null; then
  diff <(cat "$bundle/rules/run.expected"; echo) "$out/ue-script.lines" || true
  echo "jam-build: the run inside Unreal differs from the bundle's" >&2; exit 1
fi

echo "jam-build: 4/6 package for the web"
(cd "$engine" && WASM_EMSDK_ROOT="$HOME/emsdk-ue" ./Engine/Build/BatchFiles/RunUAT.sh BuildCookRun -project="$project" -platform=Wasm \
  -clientconfig=Development -build -cook -stage -pak -iostore -package -archive -archivedirectory="$web" -nocompileeditor -unattended -utf8output) > "$out/package.log" 2>&1 \
  || { tail -40 "$out/package.log"; echo "jam-build: packaging failed" >&2; exit 1; }

echo "jam-build: 5/6 the run in headless Chrome at 1440x900"
(cd "$engine/Engine/Platforms/Wasm/Build/Tools/web" && ../../../../../Binaries/ThirdParty/Python3/Mac/bin/python3 webrun.py run --stage "$stage" \
  --fixtures "$repo/metaverse-engine/export/unreal/web-fixtures.json" --fixture splice-play --headless --query tier=forward --out "$out/browser") > "$out/browser.log" 2>&1 || true
grep -h -o 'AETHYR_PLAY .*' "$out"/browser/*/console.log "$out"/browser/console.log 2>/dev/null | sort -u > "$out/browser.lines" || true
grep -q 'AETHYR_PLAY done' "$out/browser.lines" || { cat "$out/browser.lines"; tail -20 "$out/browser.log"; echo "jam-build: the run did not finish in the browser" >&2; exit 1; }

echo "jam-build: 5b real keys in headless Chrome: the title, a new run, the farm on foot, then Digger turned to (shots in $out/keys, $out/digger)"
(cd "$engine/Engine/Platforms/Wasm/Build/Tools/web" && ../../../../../Binaries/ThirdParty/Python3/Mac/bin/python3 webrun.py run --stage "$stage" \
  --fixtures "$repo/metaverse-engine/export/unreal/web-fixtures.json" --fixture splice-keys --headless --query tier=forward --out "$out/keys") > "$out/keys.log" 2>&1 || true
grep -q 'AETHYR_STATE mode 1 ' "$out/keys/console.log" || { tail -20 "$out/keys.log"; echo "jam-build: real keys did not start a run in the browser" >&2; exit 1; }
(cd "$engine/Engine/Platforms/Wasm/Build/Tools/web" && ../../../../../Binaries/ThirdParty/Python3/Mac/bin/python3 webrun.py run --stage "$stage" \
  --fixtures "$repo/metaverse-engine/export/unreal/web-fixtures.json" --fixture splice-digger --headless --query tier=forward --out "$out/digger") > "$out/digger.log" 2>&1 || true
[[ -f "$out/digger/turn12.png" ]] || echo "jam-build: no Digger shot (see $out/digger.log)"
echo "jam-build: 5c the player camera's three views, the board at speed and a room's walls (shots in $out/camera)"
(cd "$engine/Engine/Platforms/Wasm/Build/Tools/web" && ../../../../../Binaries/ThirdParty/Python3/Mac/bin/python3 webrun.py run --stage "$stage" \
  --fixtures "$repo/metaverse-engine/export/unreal/web-fixtures.json" --fixture splice-camera --headless --query tier=forward --out "$out/camera") > "$out/camera.log" 2>&1 || true
grep -h -o 'AETHYR_CAMERA [^"]*' "$out/camera/console.log" 2>/dev/null > "$out/camera.lines" || true
grep -q 'AETHYR_CAMERA view first owned 1' "$out/camera.lines" || echo "jam-build: the camera run never showed first person on foot (see $out/camera.log)"
grep -h -o 'AETHYR_SOUND [^"]*' "$out"/browser/console.log "$out"/keys/console.log 2>/dev/null | sort | uniq -c | sort -rn > "$out/sounds-played.txt" || true
echo "jam-build: sounds the browser runs played: $(wc -l < "$out/sounds-played.txt" | tr -d ' ') kinds (see $out/sounds-played.txt)"

echo "jam-build: 6/6 zip"
rm -f "$zip"; (cd "$stage" && zip -q -r -X "$zip" .)
bytes="$(stat -f %z "$zip")"
(( bytes < 1000000000 )) || { echo "jam-build: $zip is $bytes bytes, over 1 GB" >&2; exit 1; }
echo "jam-build: ok, $zip ($bytes bytes)"
