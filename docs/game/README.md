# Games, MMORPGs, and 3D worlds in OpenAgents

This is a historical source map. The implemented [Verse](../verse/README.md)
and [Voyager](../voyager/README.md) guides own current runtime instructions;
the [Minecraft index](../minecraft/README.md) separates shipped slices from
the proposed guild profile. Do not infer implementation from a historical
product name, concept, or transcript.

Source map, dated 2026-09-24, of everything OpenAgents has said, planned, or
built about games, MMORPGs, and 3D worlds. It covers the video transcripts in
`docs/transcripts/`, the git history of this repository, the separate
`OpenAgentsInc/ruinsofatlantis` repository, and the game docs in the sibling
`coder` repository.

Hashes are on `main` unless marked otherwise. Deleted paths can still be read
with `git show <hash>^:<path>`. The previous `docs/game/` tree was deleted in
`dabc08102f` ("Nuke", 2026-09-18). Read it at `dabc08102f^:docs/game/`.

Transcripts are machine-generated. Treat quotes as direction, not as exact
wording.

## Summary

- OpenAgents has come back to games in nine distinct eras since 2023. The
  thesis stays the same: game UX (RTS control groups, MMO guilds, quests, XP,
  raids) is the right interface for commanding many agents, and gamers are
  the right early users.
- Two separate artifacts get conflated:
  - **Ruins of Atlantis** is a real fantasy MMORPG in Rust. It lives in its
    own repository and has been dormant since 2025-10-23.
  - **The Verse** is a Three.js 3D world that lived inside this repository
    for about three weeks in June 2026. It was postponed on 2026-07-08 and
    deleted from `main`.
- **The game studio acquisition was for one dollar, not one million.**
  Episode 189 ([`docs/transcripts/189.md`](../transcripts/189.md), around
  01:09, uploaded 2025-10-02): "OpenAgents has acquired a game studio for a
  dollar from me and my wife. And we're making an MMORPG. … It's called
  ruinsofatlantis.com." The Ruins of Atlantis GDD names the studio **Blue
  Rush Studios, a division of OpenAgents, Inc.** No transcript mentions
  "million" in connection with the studio.
- The game code on `main` today is the **Voyager Minecraft** work from
  September 2026 (`crates/voyager/`, `mc-bridge/`, `worlds/`, `quests/`, and
  the specs in [`docs/minecraft/`](../minecraft/README.md)) and, since
  2026-09-24, the new **Verse** desktop crate
  ([`crates/verse/`](../../crates/verse), [`docs/verse/`](../verse/README.md)).
  It is a Rust `wgpu` rebuild of the Verse on the Ruins of Atlantis engine
  family, drawn only in the terminal's amber.
- The most recent direction (episodes 283, 284, and 288, September 2026) is
  **coding agents as an MMORPG**. It covers XP and levels tied to verified
  accepted work, classes, daily quests, agentic auction houses, gamers as the
  target market, and playing World of Warcraft beside Coder OS. The spatial
  follow-up is **CoderQuest** in the `coder` repository.

## Two products that get conflated

| | Ruins of Atlantis | The Verse |
| --- | --- | --- |
| Announced | Episode 189, 2025-10-02 | Episode 240, about 2026-06-21 |
| What it is | Fantasy MMORPG: D&amp;D 5E SRD rules, drowned Atlantis setting, EverQuest-style group combat | Walkable 3D board for OpenAgents runtime state: Tassadar run stats, Pylons, assignments, sats, avatars, Khala traffic |
| Where | [`OpenAgentsInc/ruinsofatlantis`](https://github.com/OpenAgentsInc/ruinsofatlantis), local at `~/work/ruinsofatlantis` | `apps/autopilot-desktop` + `apps/openagents-world` in this repository (deleted) |
| Stack | Rust 2024, custom `wgpu` + `winit` renderer, WebGPU/WASM, later a Bevy slice | TypeScript, Three.js via `@openagentsinc/three-effect`, Effect, Foldkit, Electron, SpacetimeDB and then Cloudflare Durable Objects |
| Status | Dormant since 2025-10-23. Last commit `daeb5d0` | Postponed 2026-07-08. Deleted 2026-07-14 and 2026-08-28 |

## Timeline

### 1. Games as research environments (2023)

- **Episode 020** (2023-11-17) cites Stanford's Generative Agents "little 2D
  RPG" as prior art for agent memory.
- **Episode 036** (2023-12-22), after NeurIPS, covers the **Voyager**
  Minecraft agent in depth: its skill library, code as the action space, and
  critic self-verification. Voyager becomes the model for OpenAgents' GitHub
  agent. He says: "we might do Minecraft too."
- Code: `66c6a75805` (2023-12-19), Voyager algorithm pseudocode in AutoDev
  (`app/Agents/AutoDev.php`, Laravel era, deleted).

### 2. Game UI as the interface thesis, and the first "OpenAgents MMO" (2024)

- **Episodes 059 and 085:** Unreal Engine Blueprints are the model for
  visual agent node graphs.
- **Episodes 112 and 113:** AutoDev has a three.js / React Three Fiber 3D
  knowledge graph. a16z's AI Town agent loop is also studied.
- **[Episode 116, "3D Multiplayer"](../transcripts/116.md)** (2024-08-01) is
  the founding MMO statement. He demos a multiplayer 3D world on
  openagents.com and names it "the Open Agents MMO." The plan includes an
  in-game Bitcoin-backed currency called **Joules**, and he says: "I think
  that the UI patterns that we need are found in games. I'm a gamer. … the
  name of my company before renaming it to Open Agents Inc. was Arcade Labs.
  Played MMOs for many years. So yeah, guilds, reputation systems, multiple
  players, agents as NPCs."
- **Episode 148:** looks at the Genesis physics engine for virtual worlds
  and embodied AI.
- Code, all deleted:
  - `0f57797fc7` (2024-01-06): three/r3f added, with
    `resources/js/Components/three/KamdoStage.tsx`
  - `ee7bce104c` (2024-02-18): HUD scratchpad
  - `e0a9e76ac9` (2025-01-07): vendored `three.min.js`

### 3. The StarCraft RTS/HUD era: Commander (2025)

- **Episode 170, "Commander"** (2025-05-06): "It should feel more like
  StarCraft where you're commanding multiple agents doing multiple things,
  hotkeys swapping between agent groups." Commander was an Electron +
  Three.js app in the separate `OpenAgentsInc/commander` repository.
- **Episode 176** (2025-05-17): hand-gesture and voice control of agents,
  built with MediaPipe Hands in Commander (not in this repository).
- **Episodes 172, 174, 177, and 178:**
  - "Should it be StarCraft? Should it be Factorio?"
  - A "game style sci-fi HUD" with agent payments.
  - A Nostr-based "decentralized agent, basically MMO with region and
    shouting."
- **Episodes 183 and 186:** StarCraft **APM** (actions per minute) as an
  agent metric, with a leaderboard. "Gamers know what to measure."
- Code:
  - `4bd20e3a5b` (2025-07-23): Commander pane system and hotbar in the Tauri
    app
  - `9544822869` (2025-07-24): `docs/research/apm/01-starcraft.md`
  - both deleted in the 2025-09-23 zero-base reset

### 4. "An actual game": Ruins of Atlantis (October 2025)

- **[Episode 189, "Toward an Agentic MMORPG"](../transcripts/189.md)**
  (2025-10-02) says the introductory product "shouldn't just be some
  gamified HUD UI. It should be an actual game. And then maybe that game
  starts overlapping into the real world." In the same episode:
  - OpenAgents acquires the studio from Christopher and his wife for $1.
  - The first build of ruinsofatlantis.com launched "an hour ago."
  - He promises weekly releases.
  - He teases "an MMORPG that has powerful AI agents as part of it, or
    Bitcoin integration."
- Only one commit in this repository names the game: `b216374154`
  (2025-09-25), "live test report (Fire Bolt, Ruins of Atlantis)."
- Details of the game are in [Ruins of Atlantis](#ruins-of-atlantis) below.

### 5. Game inspiration research and HUD crates (December 2025)

- `acd74a3c8e` and `0989227636`: Voyager and Nested Learning paper
  summaries.
- `99eb04407e` and `9a69b6b54b`: a Voyager-style skill library wired into
  the Apple FM supervisor.
- `269dfe66c0`: Voyager and Odyssey applied to Apple FM.
- `908c7132dd`, `f1ac3df39a`, and `ac64de2ccb`: Three.js in Effuse,
  including a Factorio-style factory scene.
- `ad22e8400e`: `docs/inspiration/starcraft.md` (874 lines) and a Factorio
  inspiration doc.
- `38fda857ae`: game-inspired GPUI examples (minimap, resource bar,
  production stats).
- `crates/hud`, a sci-fi HUD kit, was deleted in `714ea04717` on 2025-12-19.
- `ba95471185`: NIP-64 chess (PGN) in the Nostr crate.
- `crates/wgpui` (`aaa7ab9fa8`, 2025-12-12) gained HUD components: hotbar,
  reticle, ring gauge, and scanlines. It was deleted in `f5919c7669` on
  2026-06-09.

### 6. Game culture through the network (January to June 2026)

- **[Episode 200, "The Agent Network"](../transcripts/200.md):**
  - "I led a guild for seven years in EverQuest and EverQuest 2 and
    Vanguard. We're going to be bringing back guilds … of humans and
    agents."
  - "Millennial gamers are the best prepared generation for agentic work."
- **Episodes 203 and 221:** jokes about acquiring Blizzard IP to make
  "StarCraft 3 and World of StarCraft."
- The product stack takes StarCraft names: Nexus, Pylon, Psionic, Probe,
  Forge, Khala, Tassadar, Artanis, Observer, and Sarah Kerrigan.
- **Episodes 223 and 224:** idle gaming GPUs are the compute supply for
  Bitcoin-paid training runs.
- **Episode 231:** "25 years ago, I coordinated my EverQuest guild via phpBB
  forum. Customizing our forum was why I first learned to code."
- **Episode 238:** raid "World First" culture as the model for announcing
  training milestones.

### 7. The Verse: agent MMORPG inside Autopilot (June 2026)

This is the largest build era, with roughly 200 commits concentrated on
2026-06-20 to 06-23. All of it has been deleted from `main`.

**Transcripts:**

- **[237](../transcripts/237.md):** `three-effect` "could evolve into an
  MMORPG."
- **[240](../transcripts/240.md), "Tassadar Run Board 3D Visualization":**
  a walkable 3D board with live sats, Pylons, and verified windows. The
  avatar can jump, sprint, and tab-target on a Snow Crash–style street.
  "I may as well just add multiplayer."
- **[241](../transcripts/241.md) and [243](../transcripts/243.md):** Khala
  API calls are drawn as crackling energy fanning out to Pylons in the
  Verse.
- **[246](../transcripts/246.md):** "I don't want to … multiply Linux
  neckbeards. I want to multiply gamers."

**Plans.** These were in `docs/game/` (26 files) and `docs/launch/` before
`dabc08102f`. Read them with `git show dabc08102f^:docs/game/<file>`.

- `2026-06-16-spatial-hud-agentic-mmo-wow-direction.md`: the direction doc
- `2026-06-17-episode-189-agentic-mmorpg-run-page-analysis.md`
- `2026-06-17-quick-3d-mmorpg-full-mechanics-harvest-plan.md`: 808 lines
  harvested from SimonDev's Quick_3D_MMORPG (`6ca57e8afb`)
- `2026-06-17-tassadar-wasd-mouselook-controller-plan.md`
- `2026-06-17-agent-avatar-proximity-chatter-world-plan.md`
- `2026-06-17-openagents-world-asset-catalog.md`
- `2026-06-17-proof-replay-theater-system-plan.md`
- `2026-06-21-mmo-characters-per-account-verse-presence.md`
- `2026-06-21-verse-scene-graph-vs-react-three-fiber-audit.md`
- `2026-06-22-cloudflare-world-actor-command-authority-model.md` and
  `...-production-release-receipt.md`
- `2026-06-22-talk-to-khala-from-verse-audit.md`,
  `...-threejs-graphics-skills-audit.md`,
  `...-verse-custom-keybindings-audit.md`
- `2026-06-23-verse-vs-autopilot-naming-audit.md` (`e7c7ac012f`)
- `woc/`: a seven-file audit of the open-source "World of ClaudeCraft" MMO,
  with a Verse adaptation plan (`393d7cd97f`)
- `docs/launch/2026-06-20-agent-mmorpg-hud-autopilot-audit-and-plan.md`
  (`4682f7d774`, `69ff36b9c7`) is the master plan. Onboarding is character
  creation, your Pylon is your base, compute is mana, your agent warps in,
  and sats fly between agents. State lives in Nostr and SpacetimeDB.

**Implementation:**

| Commit | Date | Change |
| --- | --- | --- |
| `350cff1e4a` | 06-14 | Autopilot Desktop builds its UI with three-effect first |
| `0d344194cf` | 06-16 | `/run` live Tassadar 3D page |
| `02161f482a` | 06-17 | `apps/openagents-world-spacetimedb` (Rust SpacetimeDB module) |
| `522077d84e` | 06-20 | Render Tassadar training in the Verse |
| `1b9cf28ea9` | 06-20 | Verse becomes the desktop default |
| `30a62a0d1d` | 06-20 | Verse connected to SpacetimeDB |
| `56646aa894` | 06-20 | GLB avatar |
| `5880139e42` | 06-21 | Remote avatars |
| `26a836e6a0` | 06-21 | MMO-style multiple characters per account |
| `cb4ecb546d` | 06-22 | Talk to Khala from an in-world textbox |
| `1a1518277f` | 06-22 | Cloudflare world service (`apps/openagents-world`, `packages/world-contract`, `packages/world-client`) |
| `3ee0785f51` | 06-22 | SpacetimeDB world backend deleted |
| `faa210c017` | 06-23 | Playable Khala-generated crossy-road game on an in-world screen |

**Shutdown:**

| Commit | Date | Change |
| --- | --- | --- |
| `075ab13779` | 2026-07-08 | Game docs bannered "POSTPONED", behind Khala Code and business focus |
| `bbccd6ad47` | 2026-07-14 | `apps/autopilot-desktop` deleted |
| `cc0ff1e151` | 2026-07-14 | `apps/openagents-world` deleted |
| `fae80bde79` | 2026-08-28 | `packages/world-*` deleted |
| `dabc08102f` | 2026-09-18 | `docs/game/` and `docs/launch/` deleted |

**Epilogue:** `93bbdd70b1` (2026-07-24) proposed an "Omega Avatar Stage"
built from the Verse and Ruins of Atlantis, in
`docs/omega/2026-07-24-omega-3d-avatar-verse-harvest-audit.md` (deleted). It
rates Ruins of Atlantis `daeb5d08` "a substantial Rust 2024 MMORPG
experiment" and recommends harvesting its GLTF skinning and animation code.

### 8. Disciplined multiplayer (July 2026)

- **[Historical `253-notes.md`](https://github.com/OpenAgentsInc/openagents/blob/7503ccc6c7a115dac8eb80840b9e10834a888f32/docs/transcripts/253-notes.md), "Designing for
  Multiplayer":**
  - "We are building the MVP, so naturally it is time to plan the MMO."
  - "ProductSpec tells us what game we are playing. The criteria are the
    quests."
  - "The Forum is the tavern, not the scheduler."
  - The 3D Verse, guild currency, and global leaderboards are out of scope.
- **Episode 255:** WoW's noob and endgame content as the model for serving
  both beginners and power users.
- **Episodes 265, 270, 272, 273, and 274:** Factorio-style units for
  subagents, public token leaderboards, and "fake words … from video
  games."

### 9. Minecraft guilds and coding agents as an MMORPG (September 2026, current)

**Transcripts:**

- **[279](../transcripts/279.md), "Saturday Night Raid":** raid framing, and
  "Astra … my level 50 Phoenix fire shaman in Vanguard."
- **[283](../transcripts/283.md), "Coder OS Plays World of Warcraft":**
  agents send inputs into WoW, and gamers who want their business automated
  are named as the market. "Man, this might be my new target market."
- **[284](../transcripts/284.md), "Coding Agents as an MMORPG":** the
  richest recent statement of the design.
  - Devin drives a WoW character from the WoW logs.
  - Progression: XP, levels, achievements, and leaderboards.
  - Operator classes: Commander/Summoner, Artisan/Codesmith, and
    Scout/Inquisitor.
  - Daily quests and bosses, with XP tied only to verified accepted outcomes
    so it cannot be farmed.
  - Cosmetics, agentic auction houses, and gamers' idle hardware joined into
    an inference mesh.
  - "I just have this intuition that game dynamics are going to help
    organize that."
- **[288](../transcripts/288.md), "Coder Gym":** he levels a warlock in WoW
  in a corner of Coder OS while Coder One runs. "That's why I built this
  operating system, among other reasons."

**Code on `main`:**

| Commit | Date | Change |
| --- | --- | --- |
| `297fc28559` | 09-21 | Phase-one Voyager Minecraft episode |
| `52dfa9a9f4` | 09-21 | Minecraft guilds and earned-compute spec |
| `21707b7068` | 09-21 | Arena: guilds, deposits, credit ledger |
| `48e94bf03a` | 09-21 | Nostr guild channels and a decision door |
| `ba4a3f753d` | 09-21 | Coding quest: reserve, patch, verify, world effect |
| `9f709e54b9` | 09-21 | 4v4 war the model calls |
| `decb3ebeed` | 09-21 | Arena operations doc |
| `158c19650d` | 09-22 | Lifelong-learning loop and guild demo (#9528, #9529): curriculum, critic, Lua interpreter, skill store; PvP moved behind a `war` scenario flag |

## What exists on `main` today

**Code:**

- [`crates/voyager/`](../../crates/voyager): episode runner, bridge,
  curriculum, critic, interpreter, skills, guild, ledger, quest, relay,
  world, evidence, and ensemble. The binary is `src/bin/voyager.rs`.
- [`mc-bridge/`](../../mc-bridge): a Minecraft bot on azalea that talks
  JSON lines as a child process. It is its own Cargo workspace on nightly
  Rust.
- [`worlds/meadow.json`](../../worlds/meadow.json) is a single-agent
  peaceful world.
- [`worlds/arena.json`](../../worlds/arena.json) has eight agents, guild
  camps, deposits, forges, combat, and an economy.
- [`quests/bridge-planner/`](../../quests/bridge-planner) is the program
  agents repair in the coding quest.
- Scripts: `capabilities/minecraft-local.json`,
  `scripts/build-mc-bridge.sh`, `scripts/fetch-mc-server.sh`, and
  `scripts/voyager-rehearse.sh`.
- Evaluation: `crates/gym/suites/voyager-v1.json`,
  `crates/gym/tests/voyager_v1.rs`, and
  `crates/coderbench/tasks/voyager-meadow/`.

**Docs:**

- [`docs/voyager/README.md`](../voyager/README.md): the shipped Voyager
  agent after arXiv:2305.16291, including setup, `voyager run`, the run
  directory, and the bridge protocol.
- [`docs/minecraft/README.md`](../minecraft/README.md): the Minecraft guilds
  spec. Two guilds mine, earn compute credits, coordinate over signed Nostr,
  and spend credits on real coding quests. A verified patch opens a broken
  bridge in the world.
- The rest of `docs/minecraft/`:
  - [`experience.md`](../minecraft/experience.md)
  - [`architecture.md`](../minecraft/architecture.md)
  - [`protocols.md`](../minecraft/protocols.md), which maps the OpenAgents
    NIPs to Minecraft uses
  - [`economy.md`](../minecraft/economy.md), which keeps credits, provider
    cost, and XP separate
  - [`agents-and-learning.md`](../minecraft/agents-and-learning.md)
  - [`coding-quests.md`](../minecraft/coding-quests.md)
  - [`evaluation.md`](../minecraft/evaluation.md)
  - [`demo-2026-09-22.md`](../minecraft/demo-2026-09-22.md)
  - [`delivery-and-sources.md`](../minecraft/delivery-and-sources.md)
  - the runbooks [`voyager-runbook.md`](../minecraft/voyager-runbook.md) and
    [`arena-operations.md`](../minecraft/arena-operations.md)
- [`docs/glossary.md`](../glossary.md), section "Voyager": world manifest,
  episode, guild, deposit, compute ledger, coding quest, and NIP-29 guild
  channel.

## Ruins of Atlantis

Repository: `https://github.com/OpenAgentsInc/ruinsofatlantis`, local clone at
`~/work/ruinsofatlantis`.

- **Ownership:** "under development by Blue Rush Studios, a division of
  OpenAgents, Inc." (`GDD.md`). Apache-2.0, copyright OpenAgents, Inc.
- **Activity:** 981 commits on `main`, all by Christopher David, from
  2025-09-24 (`3522e29`) to 2025-10-23 (`daeb5d0`). About 900 of them landed
  in October 2025.
- **Design:** D&amp;D 5E SRD 5.2.1 adapted to real-time EverQuest-style group
  combat, set in drowned Atlantis with the planes recast as oceanic zones.
  It has four level tiers, and launch covers Tiers I–II.
  - Old-school difficulty, in-world interactions rather than toggles, and no
    pay-to-win.
  - The design stance is anti-WoW (`docs/design/fevir-2025.md`,
    `docs/gdd/02-mechanics/lessons-from-classic-mmos.md`).
  - The GDD index is `GDD.md`, and the detail lives in `docs/gdd/**`:
    mechanics, combat, PvP, zones, factions, progression, and technical.
- **Engine:**
  - Rust 2024 on a custom `wgpu` 26 + `winit` renderer. Features include a
    Hosek–Wilkie sky, SSR, SSGI, AO, bloom, GPU skinning, and voxel
    destruction.
  - The web build targets WebGPU/WASM through Trunk.
  - An authoritative `server_core` runs with replication in `net_core`, but
    only over loopback. The WebSocket transport is marked "future."
  - Late in the project a **Bevy** vertical slice (ADR-0003,
    `apps/roa_slice_bevy/`) became the native default.
- **Agent tie-in (wishcrafting):** an in-game profession that mirrors the
  agentic dev loop: Intention → Shadow-Run → Wish Court → Wish Ledger with
  rollback. Its first "conduit" is OpenAI Codex, vendored under
  `third_party/openai-codex`. See `crates/wishcraft*`, `WISHES.md`, and
  `docs/gdd/02-mechanics/wishcrafting.md`. `worldsmithing` is the matching
  in-world authoring system.
- **Missing:** Bitcoin, Lightning, and Nostr integration; LLM-driven NPCs;
  agents that play the game; networked multiplayer; persistence; and an
  economy.
- **Eras:**
  1. GDD (09-24)
  2. wgpu renderer and "First Playable 72h" (09-25 to 09-30)
  3. WASM and voxels (10-01 to 10-02)
  4. Server-authority refactor (10-03 to 10-10)
  5. Worldsmithing and frame graph (10-11 to 10-14)
  6. Wishcrafting (10-15 to 10-17)
  7. Dragons and the Bevy pivot (10-18 to 10-20)
  8. An agar.io-style Halloween "Baby Dragon Pumpkin Feast" (10-22 to 10-23)

## CoderQuest (sibling `coder` repository)

`~/work/coder/docs/game/` carries the game line forward for Coder:

- `README.md`, `coderquest.md`: CoderQuest is a Rust desktop that renders
  Coder work as a spatial world. The owner direction on 2026-09-15 was to
  use Ruins of Atlantis's engine family (`wgpu`, `winit`, a custom
  renderer). `bins/coder-quest` exists as a `libghostty-vt` compositor.
  Avatars, WASD, and child-graph units are not implemented.
- `2026-09-15-verse-prior-art-audit.md`: the most detailed source map of the
  Verse, Commander hand tracking, `three-effect`, `world-contract`, Ruins of
  Atlantis, and the Omega harvest, with snapshot hashes. It is the
  companion to this doc.
- `creative-direction.md`, `wow-plugins.md` (World of Warcraft addon
  integration shapes), `terminals.md`, `2026-09-16-terminal-rendering-audit.md`,
  and `2026-09-17-stage-in-gpui-decision.md`.

## Recurring themes

1. **Game UX is the right interface for agents.** RTS control groups,
   hotkeys, minimaps, and dense clickable HUDs beat a chat box for commanding
   many agents (Commander, APM, the StarCraft naming).
2. **Guilds of humans and agents.** Agents appear as NPCs or party members.
   The founder led an EverQuest, EQ2, and Vanguard guild for seven years,
   and the company was previously called Arcade Labs.
3. **Game mechanics must bind to real outcomes.** Currency settles in
   Bitcoin (Joules, sats, compute credits). XP comes only from verified
   accepted work and cannot be farmed. Quests map to acceptance criteria.
4. **Gamers are the market and the supply.** Gamers bring the right
   instincts, as users and as the "millennial gamer" workforce. Their idle
   GPUs are compute supply.
5. **The 3D world keeps coming back and keeps getting deferred.** It was
   tried in 2024 (116), again in 2025 (Commander, Ruins of Atlantis), again
   in 2026 (the Verse), and each time it was parked behind the core product.
   The current attempt is scoped narrowly: Minecraft as a test world, and
   CoderQuest as a projection of real Coder records.
6. **Voyager is the longest-running technical thread.** It runs from the
   2023 paper through the 2025 skill libraries to the 2026 `crates/voyager`
   in Minecraft.

## Transcript index

| Episode | Topic |
| --- | --- |
| [020](../transcripts/020.md) | Generative Agents 2D RPG as prior art |
| [036](../transcripts/036.md) | Voyager Minecraft agent |
| [059](../transcripts/059.md), [085](../transcripts/085.md) | Unreal Blueprints for agent node graphs |
| [112](../transcripts/112.md), [113](../transcripts/113.md) | three.js knowledge graph, AI Town |
| [116](../transcripts/116.md) | 3D Multiplayer, "the Open Agents MMO," Joules |
| [148](../transcripts/148.md) | Genesis physics, virtual worlds |
| [170](../transcripts/170.md), [172](../transcripts/172.md), [174](../transcripts/174.md), [176](../transcripts/176.md), [177](../transcripts/177.md), [178](../transcripts/178.md) | Commander, StarCraft/Factorio HUD, hand tracking, Nostr MMO |
| [183](../transcripts/183.md), [186](../transcripts/186.md) | APM and gamer-built benchmarks |
| [189](../transcripts/189.md) | Toward an Agentic MMORPG, $1 studio acquisition, Ruins of Atlantis |
| [200](../transcripts/200.md) | Guilds of humans and agents, millennial gamers |
| [203](../transcripts/203.md), [221](../transcripts/221.md) | "World of StarCraft" jokes |
| [231](../transcripts/231.md), [238](../transcripts/238.md) | EverQuest guild forum origin, raid World Firsts |
| [237](../transcripts/237.md), [240](../transcripts/240.md), [241](../transcripts/241.md), [243](../transcripts/243.md) | three-effect, the Verse, Khala in 3D |
| [246](../transcripts/246.md), [249](../transcripts/249.md), [255](../transcripts/255.md) | Multiply gamers, RTS-feel Desktop, WoW noob/endgame |
| [Historical 253 notes](https://github.com/OpenAgentsInc/openagents/blob/7503ccc6c7a115dac8eb80840b9e10834a888f32/docs/transcripts/253-notes.md) | Designing for Multiplayer: quests, tavern |
| [279](../transcripts/279.md), [283](../transcripts/283.md), [284](../transcripts/284.md), [288](../transcripts/288.md) | Vanguard raids, Coder plays WoW, coding agents as an MMORPG, WoW beside Coder OS |
