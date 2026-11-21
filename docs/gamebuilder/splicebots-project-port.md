# Build SPLICEBOTS from the Unreal project

```sh
git -C "$HOME/games/SPLICEBOTS" pull --ff-only origin main
scripts/port-splicebots-project.sh "$HOME/games/SPLICEBOTS" \
  --out "$HOME/.openagents/splicebots-from-project"
"$HOME/.openagents/splicebots-from-project/play.sh"
```

Run the build from GameBuilder. Open the loopback address printed by
`play.sh` in a WebGPU browser. Pass `--engine /path/to/UnrealEngine` if the
built Unreal 5.9 source tree is somewhere other than
`$HOME/UnrealEngine-5.9/src`. The build also needs GameBuilder’s pinned Coax
compiler, its browser Wasm toolchain, Blender for the rig export, and Python
3 for the player’s local file server. `OPENAGENTS_BLENDER` can override the
default Blender executable. The first Unreal export may compile the editor
target and shaders.

The command snapshots the project’s committed `HEAD` in an isolated local
checkout. Pull first to build the latest `main`. The working copy’s untracked
and uncommitted files are outside the snapshot. The Unreal project supplies
the game content, with zero model calls.

## What the project adapter builds

- Unreal Editor exports the ten selected maps, including the solar farm,
  Greek quarter, Sniper’s cabin, the plain, the ring, and the lab. The engine
  importer converts placement, collision, and the supported materials.
  Unreferenced library meshes, materials, and textures appear in the skip report.
- The rig importer rebuilds the Director and six raiders from `Assets/Source`.
  The Director uses her source clips. The raiders use the Coax clip library.
  The map export records the Unreal animation sequences and unplaced skeletal
  meshes as skips. This also avoids a glTF export crash in a baked raider clip.
- The data importer converts `Data/` and `Config/` and verifies the result
  against the checkout. The browser mounts the resulting gameplay tables and
  the pickup SoundWave. The versioned project profile also tells Unreal
  Editor to render the 14 MetaSound graphs offline as mono 24 kHz PCM. Coax
  selects their four music beds, gates the drill, heat, and speed effects,
  and plays the seven Sniper chirps as their story cues arrive.
- The pinned Coax campaign rules drive the browser game. The build gates
  publication of the output directory on the Coax replay through the ending.
- At Sniper’s edit bay, click **Build Microduck** or press **B**. The recipe
  spends one battery and three scrap from the main inventory and spawns an
  imported Microduck that follows in the Coax pet formation. The edit bay
  supports up to 16 active Microducks. The local output includes the
  Unreal project’s Microduck model notice.

The browser plays fixed renders made with the graphs’ authored defaults. The
music renders cover 32 beats per track, the effects cover eight seconds apiece,
and the browser fades between beds. Heat haze uses the Coax heat state. The
farm’s Unreal ambient emitter still appears in the map skip report. The
Microduck uses the project’s whole mesh with a walking bob. The browser saves
the pet roster, the Director’s inventory, the selected hotbar slot, Sniper’s
chest, and Digger’s collected and installed parts and cards in one checkpoint.
Wait for **Microduck saved** in the edit bay before reloading. **Continue**
brings the pets back in formation and restores Digger’s progression. Use the
same loopback address so the browser can find the save. If an older save
omitted an unlock, collect it once and save again with the updated build.

`port.json` records the source commit, OpenAgents pin, and hashes of the
game packs and checks. `editor-skipped.json` and `data-skipped.json` name
unsupported or unstated source entries. The two import reports describe
the converters’ substitutions, `project-data-check.txt` contains the data
comparison, and `coax-replay.txt` contains the campaign and movement checks.
Keep the output local. It contains the project’s private assets.

## Browser verification

With the generated `play.sh` server running, use the address it prints.

```sh
vendor/openagents/coax/web/tool.sh tools/splicebots-smoke.coax \
  http://127.0.0.1:8828/ --project-features
```

The muted Chrome check decodes all 14 bakes, observes live music and a story
chirp, clicks the edit-bay craft button, checks the exact debit and an
insufficient-material refusal, then walks the Director and verifies the
Microduck follows. The crafting check starts with a fixture containing one
recipe’s ingredients.

```sh
vendor/openagents/coax/web/tool.sh tools/splicebots-smoke.coax \
  http://127.0.0.1:8828/ --project-save
```

The save check crafts two Microducks, waits for IndexedDB to confirm the
checkpoint, reloads, chooses **Continue**, and checks the pets and the
remaining inventory.

A fresh project build also passed the campaign checkpoint check in Chrome.
The Director collected and installed Digger’s parts and cards, crafted a
Microduck from farm pickups, and reloaded after the save. Continue restored
the Drill Arm, Treads, Move and Drill unlocks and their installation, the
Microduck, one scrap and zero batteries. She entered Sniper and drove to the
garage at dusk using the keyboard, with no repeat pickups after Continue.

The [input-only campaign verification](splicebots-project-campaign.md#input-only-campaign-verification)
records a fresh-build browser playthrough from the title through the teaser,
including crafting with farm pickups, Reload → Continue, the boss, and the
night exit. The same report records differences from the Unreal project.

## Record a browser run

With the generated player running, use the browser recorder from the
GameBuilder repository root. It uses the Coax media library and FFmpeg.

```sh
vendor/openagents/coax/web/tool.sh tools/browser-run-record.coax \
  http://127.0.0.1:8839/ "$DRIVER" "$OUTPUT_MP4" --reload
```

The driver defines `browserRun.ready()` to wait for the page and
`browserRun.first()` to play the first stretch. With `--reload`, the
recorder reloads the same page and profile and calls
`browserRun.resume(checkpoint)` with the JSON value returned by
`first()`. The driver advances the game using `coaxialClipStep`.
The recorder captures the page throughout those steps and joins both
documents into one 20 fps MP4 with the game's offline stereo audio.
Use `--silent` for a video-only recording. The capture includes both
sides of a reload. Loading time is outside the fixed game clock.
