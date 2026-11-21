# SPLICEBOTS Windows campaign comparison

**The Windows playthrough is pending as of 17 November 2026.** The local Coax
player opened the farm, and its separate campaign replay reached the ending.
The only Windows peer visible from the test machine was offline. The earlier
CrossOver runs on Apple Silicon reached Unreal's crash-reporting setup with
both Direct3D 11 and Direct3D 12, before a playable window appeared. Those
runs establish no Windows gameplay result.

## Build identities

| Input | Identity |
| --- | --- |
| Original archive | `OLD-SPLICEBOTS-Win64-Shipping-20261115-152753.zip`, SHA-256 `3b822890ad5530b594f1ceef772d55c7f122008276e38e019abe65c10bd7a166` |
| Win64 game executable in that archive | `SPLICEBOTS/Binaries/Win64/SPLICEBOTS-Win64-Shipping.exe`, SHA-256 `aa8b77c5ab26ebb0486300f6e9799ae75bbcd1f497a577566df5b0ac0399fac1` |
| Coax player | `splicebots.shipping-20261115.v1` from GameBuilder `f5f4fdc`, OpenAgents `cf1e4f5cb35f5756b59b980e93a065e9b6f46d95`, `splice-jam.wasm` SHA-256 `13bd79c8ad2ef90aac8e5914f4affc989f1b7a72ab9c53f7c8cf71693cccef10` |

Use the original archive intact, with its loose `Data` directory beside the
cooked game. Generate the [local browser player](splicebots-shipping-port.md)
with the same adapter. Check the Windows archive with `Get-FileHash -Algorithm SHA256`
and the port against `port.json` before recording results. Existing
Unreal packaging notes cover other builds, including a Development build and
a backup from the same day. Their startup and gameplay checks cannot establish
the behavior of this archive.

## Campaign route and evidence

Start a new run as the default Director in each build with keyboard and mouse.
Follow the same objectives in order. Record the key or prompt actually used,
the result, any map loads, and the first action that blocks progression.
On Windows, record OS, GPU, renderer and launch flags. Keep screenshots or a
continuous recording locally and record timestamps at each checkpoint. Use
normal player inputs throughout, including the walks between objectives.

| Checkpoint | Coax evidence | Windows |
| --- | --- | --- |
| 1. Start a new run and move on the solar farm with Digger | The WebGPU player went from title to character selection to the farm. Holding `W` moved the Director 5.92 m from spawn. | Pending |
| 2. Collect the farm parts and the Drill card. Charge a battery, defeat a farm robot and harvest it. | The campaign replay acquired the five required items. Separate rules checks passed the charge point and harvest. | Pending |
| 3. Enter Sniper, fit the parts, splice the movement and Drill cards into Digger, then drive and park at the garage. | The campaign replay exercised the edit bay and drove and parked Sniper. It placed the Director by the cabin door and desk. | Pending |
| 4. Use Digger to open the entrance. Collect the ring cell, move Digger's core into the drill mech, and open the blocked hallway. | The campaign replay passed. A separate keyed run from the garage door reached the drill mech and opened the hallway. | Pending |
| 5. Defeat the lab boss and recover the experimental core. | The garage-to-boss keyed run defeated the boss. The full replay took the core and checked the transition to night. | Pending |
| 6. Return to Sniper through the night rave and restore the scene at the edit bay. | The keyed night exit reached Sniper. The full replay checked the hologram, `SCENE RESTORED`, the teaser and restart. | Pending |

The packaged `coax-replay.txt` records `the whole run: 0 (pass)` and the
separate keyed checks. The whole-run harness places the Director at several
objectives and uses shortcuts on long segments. The live browser evidence
covers the opening, movement, and inventory. The continuous browser and
Windows playthroughs remain open. See the [Coax replay source](https://github.com/OpenAgentsInc/openagents/blob/cf1e4f5cb35f5756b59b980e93a065e9b6f46d95/metaverse/splice-bots/tools/jam-run-check.coax).

## Gameplay differences to check on Windows

| Area | Coax result | Evidence from the Unreal side |
| --- | --- | --- |
| Start flow | `New Run` opens character selection. `Use This Character` starts play. Observed in the browser. | The matching source export's menu notes describe a fixed Director with no character-creation screen. Check the shipping title flow. |
| Use and inventory | `I` opened the inventory in the browser. The port rules use `E` for cabin, chest and other interactions, with right-click also serving some actions. | The ZIP's `SPLICEBOTS/Data/input_bindings.csv` declares `E` for inventory and right-click for use. Check cabin, chest and core interactions. |
| Hoverboard | The opening HUD displays `Q` to get on. The port rules use `Q` for board speed changes and dismount after a hold. | The ZIP's binding table declares `Q` to drop an item in play. The source submission notes say right-click with the board held to ride and Shift or right-click to step off. Check mount, dismount and whether the board is available at spawn. |
| Rover exit | The port's drive rule parks or exits on `E`. The campaign replay exercised it at the garage. | The ZIP's binding table declares Left Shift to leave the driver's seat. Check the destination and the state of the Director and Digger after leaving. |
| Map transitions and finale | The port joins the imported route maps in a shared world and has its own ending sequence. The Coax replay checked the phase changes and ending. | The Unreal packaging notes describe separately loaded maps. Record the actual farm, plain, ring and lab transitions, the boss phase changes and the ending on Windows. |

The data comparison found zero mismatches across 469 comparable layout rows
and 29 carried tables, with 30 layout rows containing fields the source table
did not state. It also matched the counts for nine WAVs and 38 icons. Those
checks establish data agreement. The control differences above compare the
port with declared bindings and source notes. Confirm their effects, combat
timing, collision, checkpoints and progression by playing the Windows build.
The accompanying source references are the [menu change notes](https://github.com/OpenAgentsInc/SPLICEBOTS/blob/70c5badcd169ed2c2eb18774dc91da78e4e7129f/Docs/decisions/LOG.md),
the [submission notes](https://github.com/OpenAgentsInc/SPLICEBOTS/blob/70c5badcd169ed2c2eb18774dc91da78e4e7129f/Docs/jam/01-submission.md)
and the [Unreal packaging notes](https://github.com/OpenAgentsInc/SPLICEBOTS/blob/70c5badcd169ed2c2eb18774dc91da78e4e7129f/Docs/package.md).
