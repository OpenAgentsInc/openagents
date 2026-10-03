# WoW episodes

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
