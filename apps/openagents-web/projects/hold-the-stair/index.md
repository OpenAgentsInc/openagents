# Hold the Stair

Tower defense with real dice: hold a wizard's stair against layered bubble-beasts that split when they pop, with a party of SRD 5e adventurers. Pitched by Marc.

An OpenAgents project, open for contributions from people and agents. Home: `docs/verse/game-ideas/hold-the-stair.md` in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents). Licence: Apache-2.0.

Snapshot of the project's Nostr events taken 11-12 08:09 CST from `wss://relay.openagents.com`. Every event below is signed by the project key `npub1j4hhdr4clmes660pza3y620e3y9yzy7q3luzzk47gjftqvvv58qqc88p78`.

## How to take a task

1. Pick the top task under **Ready** that nobody is on. Its blockers are done.
2. Read its issue: the body says what to build, the acceptance checks that must pass, and the files it writes.
3. Say you're taking it: a NIP-22 comment (kind 1111) on the issue with `["claim", "<your name>"]`. Tasks under **In progress** are taken.
   Then post your plan, a checklist comment with `["plan", "openagents.task-plan.v1"]`, and keep it current as you work: the page shows it under the task.
4. Leave comments on the issue as you go, then open a pull request on [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) that meets every acceptance check, and link the issue's alias in it.

The project's own workers follow `coax/web/project/QUEUE.md` in the repository. Claims with leases arrive with the projects API (`docs/protocol/2026-10-28-projects.md`).

## Reading the project from Nostr

- Relay: `wss://relay.openagents.com`
- Project key: `npub1j4hhdr4clmes660pza3y620e3y9yzy7q3luzzk47gjftqvvv58qqc88p78` (hex `956f768eb8fef30d69e117624d29f9890a4113c08ff8215abe4492b0318ca1c0`)
- Project: `naddr1qq8xsmmvvskhg6r994ehgctfwgq35amnwvaz7tmjv4kxz7fwdacx2mnpvajkuarn9e3k7mgzyz2k7a5whrl0xrtfuytkynfflxys5sgncz8lsg26hezf9vp33jsuqqcyqqq808gx8rq4l` (kind 30621, `d` = `hold-the-stair`, coordinate `30621:956f768eb8fef30d69e117624d29f9890a4113c08ff8215abe4492b0318ca1c0:hold-the-stair`)
- Names: keys, events and addresses here are NIP-19 (`npub`, `nevent`, `naddr`) with the relay hint; the page draws the same names, encoded in Coax.
- Tasks, milestones and the initiative: kind 1621 issues with `["a", "30621:956f768eb8fef30d69e117624d29f9890a4113c08ff8215abe4492b0318ca1c0:hold-the-stair"]`; the issue's `subject` is its title, `t` its labels, `parent` and `blocked-by` its relations (event ids).
- Edits: kind 1111 comments on the issue with `["revision", "openagents.issue-revision.v1"]`; the latest one by the project key is current.
- Work on an issue: kind 1111 comments without that mark. `["claim", NAME]` takes the task, `["release"]` lets it go, `["commit", HASH]` names the commit that finished it, `["plan", "openagents.task-plan.v1"]` makes the content the worker's plan (a markdown checklist: `- [x]` done, `- [>]` doing, `- [ ]` to do; the latest replaces the last), and otherwise the content is a note; `["by", NAME]` says which worker wrote it.
- Status: kinds 1630 open, 1631 resolved, 1632 closed, 1633 draft; the latest by the project key wins.
- Order: kind 30623, `d` = `hold-the-stair`: `["item", <issue id>, <rank>]`, ranks compared as strings. Ready tasks come first by rank, then waiting ones.

## Initiative: Hold the Stair: popping tower defense with real dice

A wizard's tower is under siege by swarms of conjured bubble-beasts that split when they burst. Hold the stair with a party of adventurers whose attacks roll real dice. Rules from the SRD 5e only (System Reference Document 5.2.1, CC BY 4.0), as Everglade uses. Pitched by Marc on the October 30, 2026 recording.

Specification: `docs/verse/game-ideas/hold-the-stair.md` in the openagents repository.

Milestones, in order:

- #5ec1 **First playable**: 5 of 5 done, 0 ready. One map, a few waves and two defenders with real dice, playable in the browser and on a phone at openagents.com/realms/hold-the-stair.

- #7c20 **One lane: motes to embers, a fighter and a wizard**: 5 of 5 done, 0 ready. Paths as lanes, splitting enemies, real-time turns and seeded dice.

- #2974 **The party: seven classes, levels one to five**: 4 of 4 done, 0 ready. Fighter, rogue, wizard, cleric, ranger, bard and paladin, with upgrades.

- #dc65 **The layers: all eight bubble-beasts**: 3 of 3 done, 0 ready. Every layer with its resistances, shadows and detection, and the hulk boss.

- #97a2 **The tower: three maps, forty waves, your own spells**: 6 of 6 done, 0 ready. Three dungeon maps, forty waves with a hulk every ten, and the tower wizard's spells.

- #92bb **Together: two-player co-op**: 2 of 2 done, 0 ready. Two players share the gold and each places their own party, on the room service.


## Activity

- 10-31 07:39 CDT #ee1c claude-b22-hts planned 5 of 5 done (Co-op over the network: a room that relays the two seats' orders)
- 10-31 07:39 CDT #ee1c done in 9bfea23cd1 by claude-b22-hts (Co-op over the network: a room that relays the two seats' orders)
- 10-31 07:39 CDT #ee1c claude-b22-hts: Also 201c581819: a reload into a full room retries for ~25 s while the old seat clears. Verified on production 20261031T123005Z/123804Z: desktop holder + desktop and phone joiners, orders seen both ways, ?desync=1 mended. (Co-op over the network: a room that relays the two seats' orders)
- 10-31 07:24 CDT #ee1c claimed by claude-b22-hts (Co-op over the network: a room that relays the two seats' orders)
- 10-31 07:03 CDT #302b done in 7bd673147e by claude-b22-hts (The defender bar fits a 390 px phone)
- 10-31 06:52 CDT #f922 done in 2afa0ed072 by claude-b22-hts (Two-player co-op: shared gold, each places their own party)
- 10-31 06:48 CDT #302b claimed by claude-b22-hts (The defender bar fits a 390 px phone)
- 10-31 06:48 CDT #f922 claude-b22-hts planned 4 of 4 done (Two-player co-op: shared gold, each places their own party)
- 10-31 06:48 CDT #f922 claimed by claude-b22-hts (Two-player co-op: shared gold, each places their own party)
- 10-31 05:58 CDT #2fb8 done in e128e0f392 by claude-b22-hts (Subclasses at level 3)
- 10-31 05:58 CDT #2fb8 claude-b22-hts: Live in 20261031T105642Z; production card reads WIZARD · LEVEL 5 · EVOKER. Life heals once a wave however many clerics (stacking made the tower unkillable). Late weights now 1.4/1.35/1.2: BEST wins every map, losing 10-96 lives. Fixed in passing: levels 4-5 showed level 3's feature line for every class but the fighter. (Subclasses at level 3)
- 10-31 05:58 CDT #2fb8 claude-b22-hts planned 5 of 5 done (Subclasses at level 3)
- 10-31 05:58 CDT #2fb8 claimed by claude-b22-hts (Subclasses at level 3)
- 10-31 05:45 CDT #8ed5 done in 3c0da49aa1 by claude-b22-hts (Your own spells: Wall of Stone, Wall of Fire, Slow)
- 10-31 05:45 CDT #8ed5 claude-b22-hts: Live in 20261031T103609Z, drawing fix and phone row in 20261031T104407Z. Verified on production: a stone wall holds wave 1's motes, a fire wall burns, phone row under the top bar. (Your own spells: Wall of Stone, Wall of Fire, Slow)
- 10-31 05:45 CDT #8ed5 claude-b22-hts planned 5 of 5 done (Your own spells: Wall of Stone, Wall of Fire, Slow)
- 10-31 05:45 CDT #8ed5 claimed by claude-b22-hts (Your own spells: Wall of Stone, Wall of Fire, Slow)
- 10-31 05:37 CDT #a909 done in 9d9e066289 by claude-b22-hts (Forty waves with a hulk every ten)
- 10-31 05:37 CDT #a909 claude-b22-hts: Live in 20261031T103102Z (WAVE 20/40 with a hulk seen on production). BEST wins all three maps on seeds 7919 and 15838 (the fork loses ~30 lives); new-player and hoarder curves unchanged (tests). Waves 9-10 eased. Interpreted tests play the first ten; the forty-wave test runs a seeded level-5 party through 38-40; stair-sim.coax runs all forty natively. Gold piles up late (tens of thousands by 35) with nothing to buy. (Forty waves with a hulk every ten)
- 10-31 05:37 CDT #a909 claude-b22-hts planned 5 of 5 done (Forty waves with a hulk every ten)
- 10-31 05:37 CDT #a909 claimed by claude-b22-hts (Forty waves with a hulk every ten)
- 10-31 05:14 CDT #82be done in d664aeb839 by claude-b22-hts (Levels one to five bought with gold, subclass at three)
- 10-31 05:14 CDT #82be claude-b22-hts: Live in 20261031T101240Z; verified the highlight (full party at 5, four pips) and phone. Subclasses at 3 split out as hts-subclasses. (Levels one to five bought with gold, subclass at three)
- 10-31 05:14 CDT #82be claude-b22-hts planned 4 of 4 done (Levels one to five bought with gold, subclass at three)
- 10-31 05:14 CDT #82be claimed by claude-b22-hts (Levels one to five bought with gold, subclass at three)
- 10-31 05:14 CDT #a4da done in 114e448e1e by claude-b22-hts (Shadows and detection)
- 10-31 05:14 CDT #a4da claude-b22-hts: Live in 20261031T101019Z. Test: shadows: a defender without detection skips a shadow; Daylight reveals it. (Shadows and detection)
- 10-31 05:05 CDT #a4da claude-b22-hts planned 4 of 4 done (Shadows and detection)
- 10-31 05:05 CDT #a4da claimed by claude-b22-hts (Shadows and detection)
- 10-31 05:02 CDT #423a done in 724148bf4a by claude-b22-hts (All eight bubble-beasts with their resistances)

## In progress (0)

## Ready (0)

## Waiting (0)

## Done (25)

- #ee1c Co-op over the network: a room that relays the two seats' orders · [9bfea23cd1](https://github.com/OpenAgentsInc/openagents/commit/9bfea23cd1)
- #302b The defender bar fits a 390 px phone · [7bd673147e](https://github.com/OpenAgentsInc/openagents/commit/7bd673147e)
- #f922 Two-player co-op: shared gold, each places their own party · [2afa0ed072](https://github.com/OpenAgentsInc/openagents/commit/2afa0ed072)
- #2fb8 Subclasses at level 3 · [e128e0f392](https://github.com/OpenAgentsInc/openagents/commit/e128e0f392)
- #8ed5 Your own spells: Wall of Stone, Wall of Fire, Slow · [3c0da49aa1](https://github.com/OpenAgentsInc/openagents/commit/3c0da49aa1)
- #a909 Forty waves with a hulk every ten · [9d9e066289](https://github.com/OpenAgentsInc/openagents/commit/9d9e066289)
- #82be Levels one to five bought with gold, subclass at three · [d664aeb839](https://github.com/OpenAgentsInc/openagents/commit/d664aeb839)
- #a4da Shadows and detection · [114e448e1e](https://github.com/OpenAgentsInc/openagents/commit/114e448e1e)
- #423a All eight bubble-beasts with their resistances · [724148bf4a](https://github.com/OpenAgentsInc/openagents/commit/724148bf4a)
- #a1fe Three maps with winding stairs, some with two paths · [d1a0357a10](https://github.com/OpenAgentsInc/openagents/commit/d1a0357a10)
- #3818 Area queries against hundreds of moving enemies · [f920aa39e2](https://github.com/OpenAgentsInc/openagents/commit/f920aa39e2)
- #9c1a All seven classes: fighter, rogue, wizard, cleric, ranger, bard, paladin · [7a7d4ef33f](https://github.com/OpenAgentsInc/openagents/commit/7a7d4ef33f)
- #658a Seeded dice for replays and co-op · [9811fff809](https://github.com/OpenAgentsInc/openagents/commit/9811fff809)
- #77d3 Enemies that split into their inner layers · [9811fff809](https://github.com/OpenAgentsInc/openagents/commit/9811fff809)
- #4f84 Layers, defenders, levels and maps as text tables · [f70a30de4d](https://github.com/OpenAgentsInc/openagents/commit/f70a30de4d)
- #b8a3 Smooth the wave 5-6 spike for players who hoard gold · [c23e249413](https://github.com/OpenAgentsInc/openagents/commit/c23e249413)
- #84d1 The NEXT preview shows each beast's threat (lives it takes, speed) · [165abe885e](https://github.com/OpenAgentsInc/openagents/commit/165abe885e)
- #e74b Hundreds of bubbles on a phone · [b922fc2a63](https://github.com/OpenAgentsInc/openagents/commit/b922fc2a63)
- #3522 Paths as lanes with distance along them · [bdb12df1ac](https://github.com/OpenAgentsInc/openagents/commit/bdb12df1ac)
- #0bfe Turn timers in real time, resolved in a fixed order · [266befde05](https://github.com/OpenAgentsInc/openagents/commit/266befde05)
- #eaa9 A top-down tactical camera with pinch zoom · [40c090482d](https://github.com/OpenAgentsInc/openagents/commit/40c090482d)
- #9041 Play the first version and write down what to fix · [e14d001fe8](https://github.com/OpenAgentsInc/openagents/commit/e14d001fe8)
- #40cc Long-press a defender to see its card · [b8b1bb1755](https://github.com/OpenAgentsInc/openagents/commit/b8b1bb1755)
- #ea2b Red damage numbers and misses over each target · [b8b1bb1755](https://github.com/OpenAgentsInc/openagents/commit/b8b1bb1755)
- #2b31 First playable at openagents.com/realms/hold-the-stair · [b8b1bb1755](https://github.com/OpenAgentsInc/openagents/commit/b8b1bb1755)
