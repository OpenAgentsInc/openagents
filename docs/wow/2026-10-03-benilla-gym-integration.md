# benilla in the gym: a World of Warcraft world for our agent player

Status: proposal, 2026-10-03. Source reviewed: `samwhosung/benilla` at `cf891dc3`
(cloned read-only to `~/work/projects/repos/benilla`, listed in the workspace
`projects/manifest.txt`).

## Verdict

It fits, as a second world behind the same agent loop. Not as a Minecraft
replacement, and not through benilla's renderer.

benilla is a World of Warcraft 1.12.1 client written from scratch in Rust, not a
Minecraft project (`README.md` lines 1–14). What makes it useful to us is one crate:
`benilla-protocol`, a Bevy-free, headless library that logs in over SRP6, enters the
world server, decodes the server's stream into typed events, and sends the game's
actions (move, cast, attack, loot, quest, trade, chat and more). That is the same job
`mc-bridge` (azalea) does for Voyager in Minecraft. A `wow-bridge` built on it can
speak Voyager's existing one-JSON-object-per-line protocol, so the curriculum, Lua
interpreter, critic, skill library, ledger, ATIF trace and coderbench grading carry
over unchanged.

The hard part is the server, not the client. A vmangos server needs data extracted
from a legally obtained 1.12.1 game install, and we don't have one. That is the
blocking owner step (see "Risks").

## What benilla is

- **A full 1.12.1 (build 5875) client:** formats, world renderer, models, movement,
  networking, a FrameXML and Lua UI engine, and audio (`README.md` "What's inside").
  It ships no Blizzard assets; it reads the user's own install through `WOW_DATA`.
- **Licensing:** MIT or Apache-2.0 for its own code (`README.md`, last paragraph).
  `third_party/` (kira, a patched Lua 5.1) keeps upstream licenses. We can depend on
  `benilla-protocol` without license trouble.
- **The split we need:** `docs/MAP.md` marks which crates use Bevy.
  `benilla-protocol`, `benilla-srp`, `benilla-dbc`, `benilla-mpq` and the other format
  readers are "no Bevy". The renderer, `benilla-app`, is the only heavy part, and a
  headless agent doesn't need it.
- **The headless session API** (`crates/benilla-protocol/src/world/session.rs`, 73
  public functions):
  - **Session:** `connect`, `char_enum`, `create_character`, `player_login` and `logout`.
  - **Movement:** `start_forward`, `heartbeat`, `stop`, `teleport_ack`, `worldport_ack`
    and `move_spline_done`.
  - **Combat and targeting:** `cast_spell`, `cast_spell_at_dest`, `attack_swing` and
    `set_selection`.
  - **Items and looting:** `loot`, `autostore_loot_item`, `use_item`, `auto_equip_item`,
    `buy_item` and `sell_item`.
  - **Quests:** `questgiver_hello`, `questgiver_accept_quest` and
    `questgiver_complete_quest`.
  - **Social:** `send_chat`, `group_invite`, `join_channel` and `gameobj_use`.
- **Events:** `events.rs` decodes the stream into `SessionEvent`s: object
  create/move/values/destroy, teleports, combat, loot, quests, reputation and more.
- **Debug CLIs:** `benilla-world` and `benilla-auth`.
  - **What `benilla-world` does:** it logs in, creates a Human Warrior if the account has
    none, streams updates, and can `--walk`, `--say` GM dot-commands, or run one live probe
    per wire (`--attack`, `--spells`, `--loot`, `--quest` and others).
  - **Where it's documented:** `crates/benilla-protocol/src/bin/benilla-world/main.rs`,
    lines 1–80.
- **Server:** it runs against vmangos with Warden (anticheat) off, the default there.
  cMaNGOS and other 1.12.1 cores speak the same protocol (`README.md` "Running it").
- **Toolchain:** pinned Rust 1.98.1 (`rust-toolchain.toml`); ours is 1.97.1.

## What our Minecraft gym and agent player are today

- **Voyager**, `crates/voyager` (`docs/voyager/README.md`), runs supervised episodes.
  1. A curriculum proposes tasks.
  2. Programs are Lua (5.4 via `mlua`), run in a bounded interpreter, and may only call
     the host's typed vocabulary.
  3. A critic checks before/after state, using a `verify` spec, or a Jev `noul` verdict
     through the decision door.
  4. Passing programs bank into a digested, versioned skill library.
- **The bridge:** `mc-bridge` is a separate, supervised helper process built on azalea.
  It runs on nightly Rust because azalea needs it, which is why it's a child process, and
  it speaks one JSON object per line: `join`, `state`, `say`, `goto`, `explore`, `mine`,
  `wait`, `disconnect` and `shutdown`, plus events. The orchestrator side is
  `crates/voyager/src/bridge.rs` (`Bridge::start`, `call`, `join`, `state`, `goto`,
  `mine`, …).
- **Worlds:** `worlds/*.json` manifests pin the server version, seed, rules, setup
  commands, roster, deposits, economy and curriculum. A manifest's SHA-256 is the world's
  identity (`worlds/meadow.json`).
- **Grading:** `crates/coderbench/tasks/voyager-meadow/task.json` grades a run by path.
  The trace must contain `mc-bridge:join → state → explore → mine` and end `ended`. Its
  capability is `minecraft-local`.
- **Guild arena:** `docs/minecraft/README.md`, with `arena-operations.md`, adds two guilds
  that mine, earn compute credits, coordinate over Nostr and complete real coding quests.
  Jev picks work, and Rust owns budgets, verification and rules.
- **Placement:** episodes run locally (a supervised server plus the helper on one
  machine). There is no Boat or GCE path for Voyager yet.

## Fit

| Question | Answer |
| --- | --- |
| Can our agent player act in WoW? | Yes, headlessly. `benilla-protocol` covers login, movement, combat, spells, loot, quests, vendors, chat and groups, and it decodes the world stream into typed events. |
| Same architecture as Minecraft? | Yes. Make `wow-bridge` a supervised child process on benilla's toolchain, speaking Voyager's JSON-line protocol. The toolchain mismatch is handled the way `mc-bridge`'s nightly already is. |
| What changes in Voyager? | A second vocabulary (WoW ops), a world kind (`voyager.world/v1` with `wow:` instead of `minecraft:`), and critic specs for WoW state (XP, level, quest status, items, kills, position). |
| Rendering needed? | No. Spectating works by joining the same server with benilla's full client (or any 1.12.1 client), as Minecraft spectators do today. |
| What does WoW give us that Minecraft doesn't? | A huge authored world with quests: thousands of tasks, each with its own completion state, rewards and dependencies, checkable from server state. Also group content (parties, dungeons) as a natural multi-agent test, an economy (vendors, auction house, mail), and a stable 2006 protocol that won't move under us. |
| What is worse? | No free server data, so an owner step is needed. Movement has no client-side pathfinding: vmangos has mmaps server-side, but the headless client must plan routes itself. Episode reset is coarser than a Minecraft seed. |

## Integration design

### 1. `wow-bridge` (new helper, own workspace like `mc-bridge`)

- **Build:** a small binary on Rust 1.98.1 depending on `benilla-protocol` (and
  `benilla-dbc`/`benilla-mpq` later, for map and creature names). It's built by
  `scripts/build-wow-bridge.sh`, the twin of `build-mc-bridge.sh`.
- **The vocabulary,** one request per line:

  ```jsonc
  {"id":1,"op":"join","args":{"auth":"127.0.0.1:3724","account":"gym1","password":"…","character":"Voyager","create":{"race":"human","class":"warrior"}}}
  {"id":2,"op":"state","args":{"radius":40}}          // self (pos, map, level, xp, hp/mana, gold), nearby units/objects with entry ids, quest log, bags
  {"id":3,"op":"say","args":{"text":"hello"}}
  {"id":4,"op":"goto","args":{"x":-8913.2,"y":-137.4,"z":82.2,"seconds":60}}
  {"id":5,"op":"target","args":{"entry":6,"nearest":true}} // or guid
  {"id":6,"op":"attack","args":{"seconds":30}}       // melee until the target dies or time runs out
  {"id":7,"op":"cast","args":{"spell":78,"target":"current"}}
  {"id":8,"op":"loot","args":{"all":true}}
  {"id":9,"op":"quest","args":{"giver":"nearest","action":"accept","quest":783}} // accept | complete | reward
  {"id":10,"op":"use","args":{"bag":0,"slot":1}}
  {"id":11,"op":"vendor","args":{"action":"sell_junk"}}
  {"id":12,"op":"gm","args":{"command":".tele northshire"}} // episode setup and reset only; refused to programs
  {"id":13,"op":"wait","args":{"seconds":2}}
  {"id":14,"op":"disconnect"}
  ```

- **Events:** `chat`, `feedback` (narration), `death`, `level_up`, `quest_complete`,
  `loot`, `disconnect` and `server_exit`, mapped from benilla's `SessionEvent`s.
- **Movement:** `goto` drives `start_forward` and `heartbeat` along a straight line,
  re-aiming each heartbeat. It falls back to waypoint lists for anything that needs
  routing. A real planner (navmesh from benilla's ADT/WMO readers, or recastnavigation
  data from vmangos's mmaps) is a later step, not v1.

### 2. Voyager changes

- **`World` kind:** `voyager.world/v1` gains a `wow` section: realm address, account pool,
  character template (race, class, level), start location, GM setup commands (`.tele`,
  `.levelup`, `.additem`, `.quest add`), and the episode bounds. The manifest's digest
  stays the world identity.
- **Lua host functions:** add `goto`, `target`, `attack`, `cast`, `loot`, `quest`, `use`,
  `vendor`, `state` and `say` for WoW worlds. `gm` is setup-only and never exposed to
  programs.
- **Critic `verify` kinds:** `xp_gained`, `level_at_least`, `quest_status` (accepted,
  complete or turned in), `item_count`, `killed` (entry, count), `at_position`
  (map + radius), and the existing `noul` for anything mechanical rules don't cover.

### 3. Episode reset and seeding

- **One account and character per episode** from a pool. The episode starts with GM setup
  commands from the manifest (teleport, level, gear, quest state), then hands control to
  the curriculum.
- **Reset:** delete or recreate the character, or restore it from a snapshot of the
  character database rows. vmangos keeps characters in MySQL, so a per-episode snapshot
  is cheap.
- **Determinism is weaker than Minecraft's seed:** creature respawns, random loot and
  pathing jitter vary. Grade on outcomes (quest turned in, item obtained), not exact
  traces, the same way `voyager-meadow` grades the path and the ending.

### 4. Scoring and grading

- **ATIF trace:** every bridge exchange lands as a call, and every event as a step, as
  in Minecraft.
- **A coderbench task family `wow`:** for example `wow-northshire-first-quests`, with
  capability `wow-local`. The path is `wow-bridge:join → state → quest accept → attack →
  loot → quest complete`, and the endings are `ended`.
- **Metrics:** quests completed per hour, deaths, XP per action, and cost per completed
  quest (the efficiency report's "cost per checked result").

### 5. Running in parallel (GCE, Boat)

- **One server:** a vmangos realm (realmd, mangosd, MySQL) on one GCE VM, holding the
  extracted data (maps, vmaps, mmaps, dbc), roughly 4–6 GB.
- **Many agents:** many headless `wow-bridge` agents on pool hosts or Boat sandboxes,
  each with its own account. 1.12 servers handle hundreds of players, so 20–50 parallel
  agents on one realm is ordinary load.
- **Episode isolation:** different start zones or phased instances, plus per-account
  character reset.
- **Spectating:** any 1.12.1 client, including benilla itself with the owner's data, can
  log in to watch.

## Risks and unknowns

- **Game data (blocking):** vmangos needs maps, vmaps, mmaps and dbc extracted from an
  English 1.12.1 (build 5875) client. benilla's renderer needs the same install. We
  don't have one here, and we must not obtain or distribute Blizzard data ourselves; the
  owner must supply a legally obtained install. Without it, nothing past the login
  smoke can run.
- **Terms:** private-server play of a Blizzard game is a legal grey area. Keep it
  local or private, never public, and treat it as a research environment.
- **Pathfinding:** a headless client has no navmesh. v1 uses straight lines and
  waypoints, so complex terrain needs a planner (see the design above).
- **Server cost and ops:** vmangos is C++ with MySQL, a heavier service than a
  Minecraft jar. Building it and extracting data the first time takes about an hour.
- **Upstream churn:** benilla is moving fast (tagged releases), so pin a tag.

## Smoke result (2026-10-03, about 2 minutes)

- `cargo build -p benilla-protocol --bin benilla-world` built in 10.9 s on this Mac. The
  protocol crate needs no Bevy, and the binary is 3.1 MB.
- `benilla-world --help` runs. Dialling `127.0.0.1:3724` returns
  `nothing answered at 127.0.0.1:3724`.
- No live login was attempted: no 1.12.1 server is available, because one needs the
  game data above.

## Issue-sized steps

1. **Owner:** provide a legally obtained English 1.12.1 (5875) client's `Data`
   folder, kept private and never committed.
2. **vmangos on one GCE VM:** extract maps, vmaps, mmaps and dbc, create 20 gym accounts,
   GM on one setup account, and a runbook in `docs/wow/`.
3. **`wow-bridge` v1:** `join`, `state`, `say`, `goto` (straight line), `wait` and
   `disconnect` on the JSON-line protocol, with a live test against the realm.
4. **Voyager world kind `wow`, part 1:** manifest section, GM setup, Lua host functions
   for the v1 ops, and a `northshire.json` world.
5. **Combat and quests in the bridge:** `target`, `attack`, `cast`, `loot` and `quest`,
   plus the critic kinds `quest_status`, `killed`, `item_count` and `xp_gained`.
6. **coderbench task `wow-northshire-first-quests`:** path grading, capability
   `wow-local`, and metrics in the efficiency report.
7. **Parallel episodes:** an account pool, per-episode character reset from DB snapshots,
   and `wow-bridge` on pool hosts and Boat sandboxes against the one realm.
8. **Routing:** a navmesh planner from vmangos mmaps or benilla's ADT/WMO readers.
9. **Guild arena in WoW:** two parties, quest-funded compute credits and the existing
   Nostr coordination, mirroring `docs/minecraft/README.md`.
