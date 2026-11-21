# SPLICEBOTS browser campaign parity

The original audit found that Continue dropped four Digger unlocks. The
input-only run verified the fix in the current [project build](splicebots-project-port.md).

## Input-only campaign verification

On November 17, 2026 a fresh one-command export of Unreal revision
`007b7c281fc6fe272caa479879e7f150a850cf01` finished the full browser
campaign. GameBuilder was at `d7dae0c` with OpenAgents pinned to
`f5a5677d3205449877bbaae67f415bd525ebe8d2`. The manifest recorded zero
model calls. All 19 file hashes verified, and the manifest matched the
previous build at the same source and engine revisions.

Run the command from GameBuilder, using a new output directory.

```sh
scripts/port-splicebots-project.sh "$HOME/games/SPLICEBOTS" \
  --out "$HOME/.openagents/splicebots-input-only"
"$HOME/.openagents/splicebots-input-only/play.sh" 8839
```

Muted headless Chrome started with an empty profile. The driver used
keyboard and mouse events on the browser player and advanced the clock in
fixed steps. The headless pointer-lock shim passed mouse deltas to the
player's view controls. Read-only probes checked results. The run used no
replay positioning, staged game state, or injected inventory.

| Beat | Verified result |
| --- | --- |
| Farm and craft | Started a new run, collected Digger's parts and cards on foot, took farm scrap and a battery from a charge point, entered Sniper, and installed Digger's configuration. Crafting one Microduck reduced four scrap and one battery to one scrap and zero batteries. IndexedDB acknowledged the save. |
| Reload → Continue | Reloaded the same browser origin. Continue restored the Drill Arm, Treads, Move and Drill unlocks, Digger's installed configuration, the exact remaining inventory, and the Microduck. It followed the Director. No unlocks had to be collected again. |
| Crossing and garage | Drove and parked at dusk. Deployed Digger through the edit bay, used regular jumps at the entrance, walked to the drill mech, and put Digger's core into it. The mech followed the Director to the hallway and drilled the barricade. |
| Lab | Used the battle cards and **F** at all three weak points. Sprinting out of the bite telegraph kept all three hearts. Picked up the banned core and returned Digger's core so he could follow out of the garage. |
| Night exit and ending | Bypassed the rave patrols on the east side and sprinted between the guards. The night exit recorded a win with no hits. Walked around the parked rover to its door, used **Tab** and **X** to splice the core, and watched the memory, **SCENE RESTORED**, and the closing teaser. |

The earlier attempts stalled on the straight-line approach to the garage
and took fatal hits crossing the night patrols. The completed run resolved
both with regular player controls. The test trace is
`$HOME/.openagents/splicebots-input-only-evidence/fresh-1.jsonl` on the
build machine.

The recorded repeat used another fresh export with the same manifest. It
reached the closing teaser with three hearts after crafting, Reload →
Continue, the boss, and the night exit. The video runs for 5 minutes
20 seconds at 1024 × 640 and 20 fps with 48 kHz stereo game audio. Continue
appears at 0:57. The local recording is
`$HOME/.openagents/splicebots-recording/SPLICEBOTS-full-run.mp4`.
[Recording instructions](splicebots-project-port.md#record-a-browser-run).

## Original parity audit build

November 17, 2026. The browser player runs on the OpenAgents Metaverse Engine. GameBuilder
exports the committed Unreal project in a headless editor and runs the
campaign in Coax and WebGPU. This audit compares the browser run with the
Unreal source, authored data, and functional-test code from the same project
revision. Native Unreal runtime parity remains unverified by this run.

| Input | Revision or value |
| --- | --- |
| SPLICEBOTS `main` | `007b7c281fc6fe272caa479879e7f150a850cf01` |
| GameBuilder checkout used for the build | `0cd90cbd00a9ac3f817a60d2ed74d91c0fe1c5c7` |
| OpenAgents pin | `7c2e19c3fc00827ec08972775f27c159abf3c534` |
| Adapter and model calls | `splicebots.unreal-project.v2`, zero model calls |
| Browser | Muted headless Chrome 157 on macOS, WebGPU, new browser profile |

From GameBuilder, build into a new output directory and serve that output
on loopback.

```sh
scripts/port-splicebots-project.sh "$HOME/games/SPLICEBOTS" \
  --out "$HOME/.openagents/splicebots-campaign-audit"
"$HOME/.openagents/splicebots-campaign-audit/play.sh" 8837
```

All 19 entries in `port.json` passed SHA-256 verification. The new manifest
also matched the previous build at the same source and engine revisions,
including the four packs, player Wasm, and campaign replay.

## Browser playthrough

One run used the browser's keyboard and pointer handlers, a fixed-step
clock, and the game's existing replay hooks for positioning. The driver
used the hooks to approach selected farm pickups, cover parts of the road
and garage, and stand at the boss's weak points. It used ordinary inputs
for crafting, driving and parking, opening both barriers, combat, core pickup,
and restoration. The positioning hooks were replay messages `611` (waypoint) and `643`
(boss weak point). The driver never loaded a staged campaign state or wrote
inventory or progress directly. This establishes campaign progression with
positioning assistance for the original audit. The input-only verification
above covers continuous navigation with player controls.

| Campaign beat | Browser result |
| --- | --- |
| New Run and solar farm | Started from an empty title screen, created the Director, walked the farm, and collected the Digger unlocks. Collected four scrap and one battery from farm pickups and a charge point. |
| Craft at Sniper | Entered the cabin, opened the edit bay, and clicked **Build Microduck**. The inventory went from four scrap and one battery to one scrap and zero batteries. One Microduck spawned and IndexedDB acknowledged the save. |
| Reload → Continue | Reloaded the same origin, chose **Continue**, and recovered the one scrap, zero batteries, and one Microduck. The pet moved 3.36 m while following the Director. The Arm, Treads, Move, and Drill unlocks were missing and had to be collected again in the same run. The separate Digger battery unlock was present after Continue. |
| Crossing and garage | Spliced Digger's parts and cards, used Sniper's throttle and steering, then used **E** to park and step out at dusk. Digger drilled the rock wall. **E** put his core in the drill mech, which followed the Director and opened the hallway barricade. The night flag was already set at that point. |
| Lab and boss | Entered the lab and used battle keys **1–4** and **F** at the three weak points while they were open. The boss went down with the Director at three hearts. **E** picked up the banned core. |
| Night exit and restoration | Returned through the rave and crossed the exit line. The exit recorded a win with six Imperial robots posted and no hits. At Sniper, **Tab** and **X** opened the bay and spliced the core. The memory advanced to **SCENE RESTORED** and then the teaser. |

The build's separate native Coax replay in `coax-replay.txt` also passed
the whole run, the farm-to-Sniper and garage-to-boss input checks, the night
exit under pursuit, and the campaign sound-cue checks. The project's
`--project-features` browser check independently decoded all 14 MetaSound
renders, observed playback from the four music beds and a Sniper chirp, and
verified the craft debit and insufficient-material refusal. Audio checks
ran muted and establish playback, with the mix and acoustic parity open.

## Differences from the full Unreal project

| Area | Unreal project and browser difference |
| --- | --- |
| Continue and campaign unlocks | The Unreal save structure records inventory, pickup and wall tags, current objective, and per-robot state. In the browser, the craft save uses live inventory and pet count alongside Digger unlocks from the separate rover checkpoint snapshot. The cabin save was acknowledged while that legacy snapshot lacked four Digger unlocks, even though all five were present in play before reload. Continue restored that snapshot and the live inventory together. This can make a player repeat farm progression before the drive. [Unreal save structure][ue-save] · [Browser run codec and restore][web-save] · [Browser Continue][web-continue] · [Rover checkpoint update][web-checkpoint] |
| World and checkpoint restoration | Unreal's save also records the named checkpoint, Director and Sniper transforms, phase, individual robot modes and cores, loose items, charge-point state, run timer, and learned recipes. Browser Continue places the Director at a configured spawn, derives phase from progress, and rebuilds the pets from their count in formation. A browser checkpoint cannot reproduce the same world snapshot as the Unreal save. [Unreal save structure][ue-save] · [Browser run codec and restore][web-save] |
| Travel and controls | Unreal transfers between the farm, crossing, ring, and lab and carries run state between those maps. The browser joins the imported route in one coordinate space and runs Coax's corresponding gates. Unreal binds **Left Shift** to leave the driver's seat and uses the edit bay's **RECIPES** screen. The browser uses **E** to park and step out and **Tab** to open the bay, with a direct **Build Microduck** action and **B** shortcut. The browser route exercised the single Microduck recipe with keyboard and pointer. [Unreal travel triggers][ue-travel] · [Unreal input bindings][ue-keys] · [Unreal crafting notes][ue-craft] · [Browser build guide](splicebots-project-port.md) |
| Boss and core pickup | Unreal's lab test powers down the dinosaur, right-clicks **CORE IN** with a small core to take it, and walks over the experimental core that drops. In the browser run, three timed **F** stuns brought the boss down and **E** picked up the core directly. Both paths reach the edit bay's **SPLICE CORE** action. [Unreal boss and core test][ue-boss] · [Browser fight and core][web-core] |
| Microduck behaviour | The recipe costs one battery and three scrap in both versions. Unreal uses articulated legs, RVO avoidance, stay/follow and battery feeding, and free-ground placement when leaving the cabin. The browser uses the imported whole mesh with a walking bob, its own collision and follow formation, and a 16-pet limit. Its crew and feed controls do not target Microducks. Continue recreates the pets in formation. [Unreal crafting notes][ue-craft] · [Browser pet runtime][web-pet] · [Browser crew and targeting][web-crew] |
| Cinematics, animation, and presentation | Unreal drives cameras, memory, and ending cues through its own systems and Level Sequences. The browser reconstructs the memory and teaser in Coax. The Director uses her exported source clips, and the six retargeted raider meshes use the Coax clip library. The export skips Sequencer and Niagara entries, substitutes five materials, refuses eight textures, and carries 34 of 55 light entries. Imported geometry and restored scene progression therefore coexist with differences in lighting, materials, animation, and effects. [Unreal campaign test][ue-campaign] · [Browser ending][web-ending] · [Browser build guide](splicebots-project-port.md) |
| MetaSound | The Unreal project runs its graphs at runtime. The browser selects 14 offline renders at authored defaults, using four music beds, gated effects, and seven Sniper chirps. The source farm's heat-haze ambient emitter is skipped as a scene emitter. Coax still drives the heat effect. [Unreal music][ue-music] · [Browser build guide](splicebots-project-port.md) |

The fresh editor skip report contains 1,345 entries, including 482 hidden
entries, 740 unsupported entries, 86 logic entries, 22 Niagara entries, 12
Sequencer entries, two widgets, and one audio graph emitter. The data skip
report lists 40 unsupported rows. These counts include unplaced assets and
other entries outside the playable route. The exact inventory is in
`editor-skipped.json`, `editor-import.txt`, `data-skipped.json`, and
`data-import.txt` alongside the manifest. The generated player and raw
browser trace remain local because the project includes private assets. The
browser driver and its result, `fresh-browser.jsonl`, are in
`$HOME/.openagents/splicebots-campaign-evidence/` on the build machine.

[ue-save]: https://github.com/OpenAgentsInc/SPLICEBOTS/blob/007b7c281fc6fe272caa479879e7f150a850cf01/Source/SpliceRules/Public/SpliceRunSave.h#L37-L81
[ue-travel]: https://github.com/OpenAgentsInc/SPLICEBOTS/blob/007b7c281fc6fe272caa479879e7f150a850cf01/Source/SpliceGame/Private/SpliceStageSubsystem.cpp#L585-L609
[ue-keys]: https://github.com/OpenAgentsInc/SPLICEBOTS/blob/007b7c281fc6fe272caa479879e7f150a850cf01/Data/input_bindings.csv#L65-L95
[ue-craft]: https://github.com/OpenAgentsInc/SPLICEBOTS/blob/007b7c281fc6fe272caa479879e7f150a850cf01/Docs/first-scene.md#L422-L493
[ue-boss]: https://github.com/OpenAgentsInc/SPLICEBOTS/blob/007b7c281fc6fe272caa479879e7f150a850cf01/Source/SpliceTests/Functional/SpliceFullRunTest.cpp#L1260-L1301
[ue-campaign]: https://github.com/OpenAgentsInc/SPLICEBOTS/blob/007b7c281fc6fe272caa479879e7f150a850cf01/Source/SpliceTests/Functional/SpliceFullRunTest.cpp#L1355-L1442
[ue-music]: https://github.com/OpenAgentsInc/SPLICEBOTS/blob/007b7c281fc6fe272caa479879e7f150a850cf01/Source/SpliceRules/Private/SpliceMusic.cpp
[web-save]: https://github.com/OpenAgentsInc/openagents/blob/7c2e19c3fc00827ec08972775f27c159abf3c534/metaverse/splice-bots/front.coax#L74-L98
[web-continue]: https://github.com/OpenAgentsInc/openagents/blob/7c2e19c3fc00827ec08972775f27c159abf3c534/metaverse/splice-bots/front.coax#L205-L234
[web-checkpoint]: https://github.com/OpenAgentsInc/openagents/blob/7c2e19c3fc00827ec08972775f27c159abf3c534/metaverse/splice-bots/splice-game.coax#L709-L736
[web-core]: https://github.com/OpenAgentsInc/openagents/blob/7c2e19c3fc00827ec08972775f27c159abf3c534/metaverse/splice-bots/splice-battle.coax#L462-L480
[web-pet]: https://github.com/OpenAgentsInc/openagents/blob/7c2e19c3fc00827ec08972775f27c159abf3c534/metaverse/splice-bots/jam/jam-pet.coax
[web-crew]: https://github.com/OpenAgentsInc/openagents/blob/7c2e19c3fc00827ec08972775f27c159abf3c534/metaverse/splice-bots/jam/jam-farm.coax#L186-L330
[web-ending]: https://github.com/OpenAgentsInc/openagents/blob/7c2e19c3fc00827ec08972775f27c159abf3c534/metaverse/splice-bots/splice-battle.coax#L1320-L1380
