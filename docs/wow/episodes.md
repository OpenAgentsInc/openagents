# WoW episodes

These episodes are private research fixtures for [Verse Engine](../verse/engine/architecture.md),
the game engine powering the Verse metaverse. The original-content milestone
replaces WoW assets in the product path; retained recordings document the
compatibility research.

Build `wow-bridge` with `scripts/build-wow-bridge.sh` and Voyager with
`cargo build -p voyager --bin voyager`. Keep the realm running as described in
[the realm runbook](realm.md). Run:

```sh
export VOYAGER_WOW_ACCOUNTS="$HOME/wow-gym/accounts.json"
export VOYAGER_WOW_BRIDGE="$HOME/work/openagents-target-agent1/release/wow-bridge"
cargo run -p voyager --bin voyager -- run --world northshire
```

`worlds/northshire.json` attaches to the private CoderOS realm. Change `wow.auth`
for another private realm; the manifest digest changes with it. The helper uses
the world endpoint returned by authentication. Realm credentials never enter
manifest files, Lua programs, or traced arguments.

The `wow` section declares ordinary accounts, the character name, numeric race
and class IDs, expected start map, position, radius, and level, and trusted setup
commands (at most 32). Exactly one of `wow` or `minecraft` is required. WoW worlds use the solo
curriculum and attach to an existing realm; Minecraft worlds retain their
supervised server. Each episode deletes and recreates its declared character,
then verifies the empty quest log before setup and the declared start afterward. Do not use a spectator's name.

Trusted commands substitute `{character}` and run on a separate `GYMSETUP`
helper. The setup helper moves to and selects the episode character before
applying the commands. They are recorded as setup observations. `WOW_BRIDGE_SETUP` is removed
from the agent helper's environment. Lua has no `gm` function; chat beginning
with `.` or `!` is refused. Give ordinary pool accounts no `account_access` grant.
Setup commands can seed items and quests after the fresh-character check.

Lua programs use `state()`, `say(text)`, `wait(seconds)`, and table arguments:
`move_to({x=..., y=..., z=..., seconds=60})`, `target({entry=6})`,
`attack({seconds=40})`, `cast({spell=78, target="current"})`,
`loot({all=true})`, `quest({entry=197, quest=7, action="reward"})`,
`use({bag=255, slot=23})`, and `vendor({entry=..., action="sell_junk"})`.
`move_to` also accepts up to 32 waypoint triples. Lua reserves the word `goto`,
so the Lua name is `move_to`; the wire operation remains `goto`. Quest actions
are `accept`, `complete`, and `reward`. Vendor sales use server item quality and
sell only quality-zero backpack items. No purchases or arbitrary vendor policy
are exposed. Movement follows straight segments; supply routes around walls.

Critics read server observations: `xp_gained`, `level_at_least`, `quest_status`
(`accepted`, `complete`, or `turned_in`), `item_count`, `killed`, and
`at_position`. `all` combines mechanical checks. Missing evidence fails.
Kill counts require a server kill-credit event for this player. Rewarded quests
require the server's quest-complete event. Calls and events use the existing ATIF
trace; `voyager:verify:<task>` records completed or failed checks for coderbench.

Northshire completes quest 783, then kills ten Kobold Vermin for quest 7 and
returns to Marshal McBride. The manifest grades quest rewards, attributed kills,
XP, level, and return position. Random drops and timing are not pass criteria.
`crates/coderbench/tasks/wow-northshire-first-quests/task.json` also checks the
bridge path and `ended` completion. `wow-metrics.json` reports quests, throughput,
deaths, XP per action, and model cost per quest. Declared scripts have zero model
cost; model-driven runs report unknown cost until metering is available. Host
and realm costs are excluded explicitly.

For a driver-verified benchmark, build `coderbench-world` with
`cargo build -p coderbench --bin coderbench-world`. Its arguments are the task
JSON, world JSON, Voyager binary, an episode workspace, and a separate runs
root. The driver snapshots the workspace before and after, runs Voyager under
the task timeout, and writes `grade.json` beside the run. Use an unchanged
workspace containing the world manifest, with run artifacts outside it. The
driver grades the observed exit status, ATIF ending, and deterministic checks;
`coderbench diff` alone has no independent workspace or process-exit evidence.

On October 3, 2026, the private CoderOS realm passed the full Northshire task:
two quests, ten credited kills, level 2, 726 earned XP, and zero deaths in 300.25
seconds of episode work. The driver also verified cleanup and an unchanged
workspace. A fresh reset repeat earned the expected 40 XP for quest 783. Trusted
setup and gray-item sale passed separately. The retained summary is
[`bench/wow/2026-10-03/northshire.json`](../../bench/wow/2026-10-03/northshire.json);
raw traces and game assets stay private.

## Staged ritual scene

[The Anthropic scene](../../scripts/wow/scenes/anthropic.json) places Claude and
twelve NPCs named `Cultist of Anthropic` in Scholomance's laboratory. The filming
script sends real NPC yells and emotes, starts with a cinematic camera, cuts
behind the adventurer, and fires a bow. Subtitles repeat the actual dialogue.
This is a directed set with idle, hostile NPCs, not a graded combat episode.

On the private Linux realm host, with MariaDB, Wine, Xvfb, xdotool, xclip, and an FFmpeg
build that supports X11 capture and subtitles on `PATH`, run:

```sh
export WOW_GYM_ROOT="$HOME/wow-gym"
python3 scripts/wow/stage-scene.py scripts/wow/scenes/anthropic.json
scripts/wow/realm.sh stop
scripts/wow/realm.sh start
python3 scripts/wow/film-scene.py scripts/wow/scenes/anthropic.json
```

Use a separate client copy under `~/wow-gym/video-client`, configured for a
1280×720 window, with its first-launch loader prompt already accepted. The
bundled `VanillaFixes.exe` and `WoW_tweaked.exe` provide the extended nameplate
range. Tool paths can be supplied through the scripts' flags. Credentials stay
in the private `accounts.json`; filming exclusively leases `GYMSETUP`, uses its
existing character, and returns it to Northshire at level 1. Original room spawn
settings are saved beside the private recording for restoration. The scene's
names, dialogue, and final video are retained in this repository; client assets,
credentials, and Wine state are not.

The [recorded sequence](../../bench/wow/2026-10-03/anthropic-ritual.mp4) runs for
72 seconds. Its [cue log](../../bench/wow/2026-10-03/anthropic-film-events.json)
records the actual dialogue, camera cut, and five bow shots.

The [cinematic version](../../bench/wow/2026-10-03/anthropic-ritual-cinematic.mp4)
adds deeper shadows, warmer highlights, a vignette, and soft bloom around bright
light sources. This is a video grade; it does not change the live game's
lighting. To reproduce it, run:

```sh
python3 scripts/wow/grade-scene.py \
  bench/wow/2026-10-03/anthropic-ritual.mp4 \
  bench/wow/2026-10-03/anthropic-ritual-cinematic.mp4
```

## Verse renderer

The owned renderer imports the private chamber pack through
[`wow-import`](../../wow-import/README.md). It draws indexed textured geometry,
GPU-skinned actors, point lighting, cube shadow maps for four local sources,
and distance fog. Static geometry is merged and uploaded once. The overlay
uses Verse's glyph pipeline after world rendering, independently of exposure.

```sh
CARGO_TARGET_DIR="$HOME/work/openagents-target-agent1" cargo run -p verse \
  --no-default-features --features capture --example wow_capture -- \
  "$HOME/wow-gym/verse-assets/pack.json" "$HOME/wow-gym/chamber.png"
```

The capture's `--no-shadows` mode keeps the same camera and lights while disabling
occlusion. The October 3 check changed 367,300 pixels by more than five channel
levels when shadows were disabled. This lighting is calculated in the engine;
there is no video grading step.

The [ritual scene](../../assets/verse/wow/anthropic.json) declares the cast,
yells, camera cut, and bow cues. `verse-wow::director` evaluates them from
simulation time; it does not send keyboard input or use a chat box. Hostile
actors have head-anchored red health bars. Directed arrow impacts reduce the
target's displayed health. These scene impacts are local cinematic state, not
realm combat authority. Pass a time in seconds as the capture's third argument
to inspect a cue, for example `51.2` for an arrow in flight.

The [Verse ritual video](../../bench/wow/2026-10-03/verse-anthropic-ritual.mp4)
contains 72 seconds at 1280 × 720 and 30 frames per second. The cinematic
camera opens on Claude and 12 cultists, then cuts behind the adventurer at
20 seconds. Seven programmed yells precede five directed bow shots. Every
frame time is checked for 13 visible red hostile nameplates.

To record the full scene, pass an `.mp4` output path to the same command:

```sh
CARGO_TARGET_DIR="$HOME/work/openagents-target-agent1" cargo run -p verse \
  --no-default-features --features capture --example wow_capture -- \
  "$HOME/wow-gym/verse-assets/pack.json" \
  bench/wow/2026-10-03/verse-anthropic-ritual.mp4
```

The recorder streams owned GPU frames into FFmpeg for H.264 encoding. It uses
no grading, subtitle filter, screen automation, or Blizzard executable. The
private imported asset pack stays outside the repository. The new renderer
owns geometry submission, skeletal poses, materials, lighting, shadows,
camera state, and overlays. Realm authority and authored replacement content
remain tracked in [#10406](https://github.com/OpenAgentsInc/openagents/issues/10406)
and [#10407](https://github.com/OpenAgentsInc/openagents/issues/10407).

The [Classic UI video](../../bench/wow/2026-10-03/verse-anthropic-ritual-classic-ui.mp4)
uses the client's `FRIZQT__.TTF` font and original
nameplate-border and status-bar textures. Names use proportional glyph spacing
and black outlines above textured red health bars. Programmed yells use outlined
cinematic text. The temporary backing boxes and connector lines are removed.
Run the importer with `--ui-only` to extract these assets into the private pack
before recording; the font and interface textures are not checked into Git.

## Playable wizard action bar

The native chamber unlocks its Classic action bar at the 20-second camera
handoff. It uses the retained Ruins combat schedule for Fire Bolt, Magic Missile,
and Fireball, including its mana, cooldown, homing, damage, and area effects.
Magic Missile and Fireball have one-second casts that movement interrupts.
This is the implemented Ruins spell subset and runtime tuning, not a complete
SRD interpreter. Bow impacts use the same hostile health state.

```sh
CARGO_TARGET_DIR="$HOME/work/openagents-target-agent1" cargo run -p verse \
  --no-default-features --features imported-desktop --example wow_play -- \
  "$HOME/wow-gym/verse-assets/pack.json"
```

Use **1** for the bow, **2** for Fire Bolt, **3** for Magic Missile, and **4**
for Fireball, or click their icons. **Tab** cycles living targets; click an NPC
to select it. **W/S** move forward and backward; **A/D** turn. **Q/E** strafe.
Hold the right mouse button to steer; **A/D** then strafe. Hold the left button
to orbit the camera independently. Hold both buttons to run forward, move the
mouse vertically to change pitch, and use the wheel to zoom. The arrow keys
mirror **WASD**; **Num Lock** or mouse button 4 toggles autorun. **Escape** closes the window. Refresh the private UI assets with the
importer's `--ui-only` mode to obtain the original Classic button frames and icons.
The [spell demo](../../bench/wow/2026-10-03/verse-wizard-actions.mp4) records the
handoff and all four actions through the same input-facing gameplay methods.
Append `--demo OUTPUT.mp4` to reproduce it, or `--proof OUTPUT.png` for a native
window capture. Chamber movement is bounded locally; this does not replace
realm authority or provide collision navigation for the full imported dungeon.


The SRD 5.2.1 catalog in `ruinsofatlantis/docs/srd/03-spells/README.md`
provides the utility spell references. The chamber reimplements five more wizard
spells in `verse-ruins::chamber_spells`: **5** Misty Step blinks forward up to
30 feet within the local floor bounds, **6** Thunderwave damages and pushes
nearby enemies in a forward cube, **7** Web restrains the selected target area,
**8** Grease knocks down actors in a target area, and **9** Light places a warm
light on the floor. Light illuminates world geometry through the owned renderer.
Web and Grease affect scripted locomotion as well as actor presentation.
SRD attribution remains in `crates/verse-ruins/NOTICE`.

These are deterministic real-time adaptations: Thunderwave deals 9 damage
without tabletop saving throws, Web lasts 12 seconds with one active web,
and Grease lasts 10 seconds. The displayed mana costs and cooldowns are MMO
tuning. The local runtime does not implement escape checks, concentration
checks, fire destruction of webs, or full dungeon visibility tests for teleport
placement. Light lasts one hour and replaces the previous light. The Classic
icons require another `--ui-only` import; imported art remains private.
The [utility spell video](../../bench/wow/2026-10-03/verse-utility-spells.mp4)
shows all five effects. Append `--utility-demo OUTPUT.mp4` to record them through the same
playable runtime. The recorder stages a close-range Thunderwave position.


The movement bindings follow Blizzard's [Classic manual](https://bnetcmsus-a.akamaihd.net/cms/template_resource/263A8NGR8HLZ1556919642368.pdf)
and the private client's `Interface/FrameXML/Bindings.xml`. The authored control
module reimplements the 1.12 camera rate law documented in the pinned
[Benilla camera reference](https://github.com/samwhosung/benilla/blob/cf891dc3756a36dc0af4376f861ffb0c847ba5e9/crates/benilla-app/src/player/camera.rs):
default yaw is 180 degrees per 800 pointer units and pitch is 90 degrees per
600 units, with an 89-degree pitch limit. Keyboard turning runs at 180 degrees
per second, reduced to 75% while translating. Run and backpedal speeds are
7 and 4.5 yards per second, converted to Verse meters. Diagonals preserve speed.
The wheel changes the zoom target by one yard per notch and glides at 8.33 yards
per second up to the default 15-yard limit. Smart follow recenters camera yaw
on movement input changes using the reference cosine transition.

World mouse drags capture and hide the pointer, then restore its original
position on release. Focus loss clears held input and releases capture. An
action-bar click does not start a camera drag; a world click selects on release
only when it was not dragged. This matches the input modes and default rate
parameters; OS pointer acceleration is platform-dependent. Full imported-dungeon
camera collision, jumping, swimming, and configurable client CVars are separate
from this chamber movement implementation.

The playable HUD uses the private 1.12 client's main-bar artwork, mirrored
gryphon caps, twelve 36×36 action slots with six-unit gaps, and 232×100
portrait frames with 119×12 health and mana fills. Layout scales from a
768-pixel reference height. Friz Quadrata supplies labels; Arial Narrow
supplies hotkeys and resource numbers. Portrait headshots are rendered by the
owned GPU pipeline. Cooldowns use radial swipes driven by source spell tuning.
The [native HUD capture](../../bench/wow/2026-10-03/verse-classic-hud.png)
shows the result. Refresh private assets with `wow-import --ui-only`.
Menu and bag artwork is decorative; this chamber has no inventory, experience,
or character-level system. Imported fonts and UI textures remain outside Git.

Press F2 to reset into agent-controlled combat, or F1 to reset into manual
combat. The same encounter runs in both modes. The local tactical controller
observes incoming casts, health, range, mana, and cooldowns, then selects
ordinary admitted actions. Its opening exercises the full kit; subsequent
actions prioritize shields, evasive movement, and damage. Press 0 to cast
Shield manually: one mana buys 18 absorption for four seconds, with an
eight-second cooldown. These are chamber MMO rules, not a full tabletop Shield
implementation. Cultists approach and cast telegraphed shadow bolts; Claude
becomes more dangerous below 25% health. Combat taunts are director cues.

Append `--agent` or `--combat` to start either mode directly, or
`--combat-demo OUTPUT.mp4` to record the full agent encounter. The recorder
runs the same controller and combat state at 30 frames per second. It retains
an outcome JSON and a native final-frame PNG beside the video. For visual
checks, `--combat-proof OUTPUT.png TIME` captures an encounter time between
20 and 120 seconds. The [agent combat recording](../../bench/wow/2026-10-03/verse-agent-combat.mp4)
ends in a close defeat: the adventurer falls after killing nine cultists, with
Claude at 4/400 health. Pending attacks finish without changing that result.

NPC locomotion selects the imported walking clip from actual encounter motion.
Clip changes blend local translations, quaternion rotations, and scales before
bone hierarchy evaluation; interrupted transitions begin from the current
blended pose. Bow attachments use the same blended hand palette. Death clips
play once and keep their final pose.

Refresh private assets with `wow-import --ui-only` to import Classic fire,
smoke, glow, spark, ribbon, rune, and web textures. Verse renders these through
camera-facing particles, velocity-aligned ribbons, and ground projections with
alpha or additive blending and no depth writes. Smoke sorts from back to front;
trails and impact sparks fade over their lifetimes. The shield uses a transparent
shell. The [updated combat recording](../../bench/wow/2026-10-03/verse-smooth-combat.mp4)
comes directly from the native renderer. These are authored chamber effects,
not complete playback of every Classic M2 emitter feature. Source textures
remain outside Git.

The chamber now uses low ambient light and localized green vessel and candle
illumination. Up to 32 point lights follow projectiles, hostile casts, shields,
teleports, and impacts. Fireballs create a stronger traveling light and a fading
explosion flash; two shadow slots follow the brightest nearby effects. The
[native dark combat recording](../../bench/wow/2026-10-03/verse-dark-combat.mp4)
shows the resulting illumination. Active cultists use combat-ready or directed
spell-ready poses between casts. Corpses keep their death pose after ECS removal,
rest above the floor, and have no overhead nameplate.

The [shorter combat recording](../../bench/wow/2026-10-03/verse-fast-combat.mp4)
ends after about 27 seconds of fighting, with all ten abilities used, ten
cultists defeated, and Claude at 12/135 health. Claude deals 18 damage per
normal hit and 45 while enraged; cultists deal 8 and cast more often. Cultists
have 15 health. Floating damage numbers use the imported Classic Friz font,
with yellow text above enemies and red text above the adventurer. Numbers rise
and fade, show actual health lost, and exclude damage absorbed by shields.

The bottom HUD now uses a compact, centered tray containing only the ten chamber
abilities. Classic icon frames, hotkeys, cooldowns, and tooltips remain; bags,
menu buttons, paging controls, end caps, and unused slots are removed. The
[native compact-bar capture](../../bench/wow/2026-10-03/verse-compact-bar.png)
shows the updated layout.

Claude now starts with 300,000 health. Each defeated cultist respawns at its
original spawn point 60 seconds after death with full health and a visible
nameplate. Respawning clears the previous life's corpse, control effects, and
pending attacks. The retained shorter-combat video records the earlier
135-health boss profile; the current profile no longer targets a close defeat.
