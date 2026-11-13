# Unreal exporter probe (step 0)

Proves the two risks in
[the exporter spec](../../../../docs/unreal/2026-11-12-aethyr-to-unreal-exporter-spec.md)
before any exporter work (GitHub #13399):

1. An OpenAgents Metaverse Engine world's C compiles and runs inside an Unreal module, on Mac and
   on Unreal's 64-bit Wasm target, and writes the same bytes as the web build.
2. Where Unreal's glTF import puts the Metaverse Engine's axes.

## Files

| File | What it is |
|---|---|
| `../aethyr_host.h` | The host interface with real pointers |
| `../../../../metaverse/splice/splice-ue-face.c` | Splice's face for that interface |
| `frame-dump.c` | Runs a fixed 34-frame script and prints one hash line per chunk a host reads |
| `AethyrProbe/` | A bare Unreal project whose one module compiles the world's C and runs the script at start |
| `import_axes.py` | Imports the axis fixture in the Unreal editor and prints where the cubes land |
| `../../glb-write.coax` | The first glTF writer for cel vertices |
| `../../../tools/unreal-axes-fixture.coax` | Writes the three marker cubes |

## Run it

All output goes to a scratch folder `$S`. Unreal is at `$UE`
(`~/UnrealEngine-5.9/src` on the owner's Mac). Copy `AethyrProbe/` out of the
repo first (to `$T`): Unreal writes build output beside the project.

```sh
# 1. The world's C, as the web build emits it
coax/bin/coax c metaverse/splice/splice.coax --library --build-profile release --header $S/splice.h > $S/splice.c
coax/bin/coax c apps/openagents-web/animelab-style.coax --library --build-profile release --header $S/Animelab-style.h > $S/animelab-style.c

# 2. Native reference lines
clang -std=c11 -O1 -ffp-contract=off -fno-fast-math -w -I $S -I metaverse-engine/export/unreal \
  $S/splice.c $S/animelab-style.c metaverse/splice/splice-ue-face.c \
  metaverse-engine/export/unreal/probe/frame-dump.c -o $S/dump && $S/dump > $S/native.txt

# 3. Inside Unreal, Mac
mkdir -p $T/Source/AethyrProbe/Private/World
cp $S/splice.[ch] $S/animelab-style.c $S/Animelab-style.h metaverse/splice/splice-ue-face.c \
  metaverse-engine/export/unreal/aethyr_host.h metaverse-engine/export/unreal/probe/frame-dump.c $T/Source/AethyrProbe/Private/World/
(cd $UE && ./Engine/Build/BatchFiles/RunUBT.sh AethyrProbeEditor Mac Development -project=$T/AethyrProbe.uproject -NoHotReload)
$UE/Engine/Binaries/Mac/UnrealEditor-Cmd $T/AethyrProbe.uproject -run=pythonscript -script="print(1)" -nullrhi -unattended -stdout | grep AETHYR_PROBE

# 4. Inside Unreal, 64-bit Wasm
(cd $UE && WASM_EMSDK_ROOT=~/emsdk-ue ./Engine/Build/BatchFiles/RunUAT.sh BuildCookRun -project=$T/AethyrProbe.uproject \
  -platform=Wasm -clientconfig=Development -build -cook -stage -pak -iostore -package -archive \
  -archivedirectory=$T/Saved/WasmOut -nocompileeditor -unattended -utf8output)
# then open the stage with Engine/Platforms/Wasm/Build/Tools/web/webrun.py and read console.log

# 5. Axes
coax/web/tool.sh metaverse-engine/tools/unreal-axes-fixture.coax $S/axes.glb
AETHYR_AXES_GLB=$S/axes.glb AETHYR_AXES_OUT=$S/axes.txt $UE/Engine/Binaries/Mac/UnrealEditor-Cmd \
  $T/AethyrProbe.uproject -run=pythonscript -script=$PWD/metaverse-engine/export/unreal/probe/import_axes.py -nullrhi -unattended
```

The lines from steps 2, 3 and 4 must be equal. Results are in the spec's
section 8.

## Things this found

- Unreal's build settings make `-Wunreachable-code-aggressive` an error. The
  Coax compiler's C returns after a call that never returns, so the module
  turns that warning off (`AethyrProbe.Build.cs`).
- Building an editor target while another editor from the same engine is
  open needs `-NoHotReload`. The build also re-signs the shared
  `UnrealEditor.app`.
- `unreal.log` lines do not reach `-stdout`. Write results to a file.
- Unreal's Python mesh description exposes positions and UVs, not normals or
  colours. Check those in a render.
