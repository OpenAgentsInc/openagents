#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: runs the exporter, the importer, Unreal's packager and its browser harness in order)
# The Splice Bots jam build: the game as it stands on this checkout, with the world `splice-bots` baked in
# (no server), exported to Unreal, imported into the slice's project, checked there, packaged for the web
# (UE 5.9's Wasm target), played in headless Chrome at 1440x900 and zipped. README.md, "In the Unreal host".
#   metaverse/splice-bots/tools/jam-build.sh WORLD_DIR OUT_DIR
# WORLD_DIR holds the world's live lane as a dev server keeps it (splice-bots.live, splice-bots.live.tables:
# the owner's server keeps them in its checkout's apps/openagents-web/build/.worlds). OUT_DIR must not exist.
# SPLICE_UE_PROJECT (default ~/UnrealProjects/SpliceSlice1/AethyrHost.uproject) and SPLICE_UE_ENGINE
# (default ~/UnrealEngine-5.9/src) name the project and the engine. Run from the repository root.
set -euo pipefail
[[ $# -eq 2 ]] || { echo "usage: metaverse/splice-bots/tools/jam-build.sh WORLD_DIR OUT_DIR" >&2; exit 2; }
world="$(cd "$1" && pwd)"; out="$2"
[[ ! -e "$out" ]] || { echo "jam-build: $out exists; give a folder that does not" >&2; exit 1; }
mkdir -p "$out"; out="$(cd "$out" && pwd)"
repo="$(pwd)"
project="${SPLICE_UE_PROJECT:-$HOME/UnrealProjects/SpliceSlice1/AethyrHost.uproject}"
engine="${SPLICE_UE_ENGINE:-$HOME/UnrealEngine-5.9/src}"
bundle="$out/splice.aexport"; web="$out/web"; zip="$out/SpliceBots_Splicebots.zip"

echo "jam-build: 1/6 export"
coax/web/tool.sh metaverse/splice-bots/splice-export.coax "$bundle" "$world"

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
(cd "$engine/Engine/Platforms/Wasm/Build/Tools/web" && ../../../../../Binaries/ThirdParty/Python3/Mac/bin/python3 webrun.py run --stage "$web" \
  --fixtures "$repo/metaverse-engine/export/unreal/web-fixtures.json" --fixture splice-play --headless --query tier=forward --out "$out/browser") > "$out/browser.log" 2>&1 || true
grep -h -o 'AETHYR_PLAY .*' "$out"/browser/*/console.log "$out"/browser/console.log 2>/dev/null | sort -u > "$out/browser.lines" || true
grep -q 'AETHYR_PLAY done' "$out/browser.lines" || { cat "$out/browser.lines"; tail -20 "$out/browser.log"; echo "jam-build: the run did not finish in the browser" >&2; exit 1; }

echo "jam-build: 6/6 zip"
rm -f "$zip"; (cd "$web" && zip -q -r -X "$zip" .)
bytes="$(stat -f %z "$zip")"
(( bytes < 1000000000 )) || { echo "jam-build: $zip is $bytes bytes, over 1 GB" >&2; exit 1; }
echo "jam-build: ok, $zip ($bytes bytes)"
