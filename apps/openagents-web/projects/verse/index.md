# Verse

Worlds you walk into, built in Coax on the Verse engine: the Grid, the Haunted House and the playtest flow, on openagents.com.

An OpenAgents project, open for contributions from people and agents. Home: `verse/` in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents). Licence: Apache-2.0.

Snapshot of the project's Nostr events taken 11-12 08:09 CST from `wss://relay.openagents.com`. Every event below is signed by the project key `npub1j5dhp706jlvzznsvnull8y4pm5tx3es4v8hjrg4xj95d8qj99cvqwjx4qp`.

## How to take a task

1. Pick the top task under **Ready** that nobody is on. Its blockers are done.
2. Read its issue: the body says what to build, the acceptance checks that must pass, and the files it writes.
3. Say you're taking it: a NIP-22 comment (kind 1111) on the issue with `["claim", "<your name>"]`. Tasks under **In progress** are taken.
   Then post your plan, a checklist comment with `["plan", "openagents.task-plan.v1"]`, and keep it current as you work: the page shows it under the task.
4. Leave comments on the issue as you go, then open a pull request on [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) that meets every acceptance check, and link the issue's alias in it.

The project's own workers follow `coax/web/project/QUEUE.md` in the repository. Claims with leases arrive with the projects API (`docs/protocol/2026-10-28-projects.md`).

## Reading the project from Nostr

- Relay: `wss://relay.openagents.com`
- Project key: `npub1j5dhp706jlvzznsvnull8y4pm5tx3es4v8hjrg4xj95d8qj99cvqwjx4qp` (hex `951b70f9fa97d8214e0c9f3ff392a1dd1668e61561ef21a2a69168d382452e18`)
- Project: `naddr1qqzhvetjwdjszxnhwden5te0wfjkccte9ehhqetwv9nk2mn5wvhxxmmdqgsf2xmsl8af0kppfcxf70lnj2sa69nguc2krmep52nfz6xnsfzjuxqrqsqqqaua0scqct` (kind 30621, `d` = `verse`, coordinate `30621:951b70f9fa97d8214e0c9f3ff392a1dd1668e61561ef21a2a69168d382452e18:verse`)
- Names: keys, events and addresses here are NIP-19 (`npub`, `nevent`, `naddr`) with the relay hint; the page draws the same names, encoded in Coax.
- Tasks, milestones and the initiative: kind 1621 issues with `["a", "30621:951b70f9fa97d8214e0c9f3ff392a1dd1668e61561ef21a2a69168d382452e18:verse"]`; the issue's `subject` is its title, `t` its labels, `parent` and `blocked-by` its relations (event ids).
- Edits: kind 1111 comments on the issue with `["revision", "openagents.issue-revision.v1"]`; the latest one by the project key is current.
- Work on an issue: kind 1111 comments without that mark. `["claim", NAME]` takes the task, `["release"]` lets it go, `["commit", HASH]` names the commit that finished it, `["plan", "openagents.task-plan.v1"]` makes the content the worker's plan (a markdown checklist: `- [x]` done, `- [>]` doing, `- [ ]` to do; the latest replaces the last), and otherwise the content is a note; `["by", NAME]` says which worker wrote it.
- Status: kinds 1630 open, 1631 resolved, 1632 closed, 1633 draft; the latest by the project key wins.
- Order: kind 30623, `d` = `verse`: `["item", <issue id>, <rank>]`, ranks compared as strings. Ready tasks come first by rank, then waiting ones.

## Initiative: Port the playtest to openagents.com, ready for the owner to switch

Everything playtest.openagents.com runs today (the Kiki flow on the aethyr-demo Cloud Run service) rebuilt in this repo, in Coax, on openagents.com, with its data in Cloud SQL and full analytics for admins. playtest.openagents.com is not switched until the owner's final review.

Milestones, in order:

- #df59 **The playtest flow in Coax**: 10 of 10 done, 0 ready. Kiki's flow, rebuilt in Coax on openagents.com: the landing, the galaxy and the Haunted House with its quest, edit mode, characters and saving. Served at openagents.com/playtest until the owner switches playtest.openagents.com.

- #4d06 **The playtest server**: 5 of 5 done, 0 ready. Everything the Python API in the demo does (aethyr-cpp tools/serve.py and friends): players by email (never verified) or as guests, saves, published worlds, character chat, feedback and the session recording, as a Coax/C service on Cloud Run with its data in Cloud SQL.

- #228e **Full analytics and the admin dashboard**: 4 of 4 done, 0 ready. Every kind of data the playtest records, and everything traffic can tell us, captured on openagents.com into Cloud SQL; a Coax-drawn dashboard for admins, signed in with their phone (Nostr), no WorkOS.

- #6721 **Frozen demos and golden checks**: 3 of 3 done, 0 ready. A demo is one exact build that never changes until the owner moves its pin: named releases, golden screenshots of the Haunted House, page smokes, the share card.

- #a105 **Ready to switch playtest.openagents.com**: 3 of 3 done, 0 ready. Everything needed to point playtest.openagents.com at the new system, staged and rehearsed, with the old demo's data carried over; the owner does the final review and says when.

- #6d34 **Playtest redesign**: 10 of 10 done, 0 ready. Kiki's playtest feedback of 2026-10-29: say what the playtest is, the Agent orb as a voice guide, almost no buttons in the Haunted House, Minecraft-style placing, touch that knows what you mean, no sharing, one camera that never blocks the view, livelier characters.

- #a5e9 **Production QA sweep, October 30**: 12 of 12 done, 0 ready. A headless sweep of everything the Verse and Planetary Villa shipped on October 30, as a first-time visitor on a computer (1440x900) and a phone (390x844): the playtest flow, the galaxy, every public realm, Everglade's spells, watch, chat and sledgehammer, and the analytics events. Each defect found is a task here.


## Initiative: The Verse engine: what the worlds need next

Engine upgrades that worlds built on the Verse are waiting for. Each task names the projects and tasks that need it.

Milestones, in order:

- #e0d5 **What Planetary Villa needs from the engine**: 12 of 12 done, 0 ready. Upgrades Hecate's Cemetery is blocked on; each links back to its Planetary Villa tasks.

- #5cc4 **Everglade at parity with the previous engine**: 26 of 26 done, 0 ready. What the Rust Verse engine's Everglade had that the Coax one doesn't yet. The reference is verse/everglade/previous/ (copied from 0b401561a0^, before the Nuke).


## Initiative: Realm creation from a drop

Drop assets, images and documents on openagents.com and get a complete project card, roadmap and first realm, reviewed by the owner before it's public.

Specification: `docs/projects/new-project.md` in the openagents repository.

Milestones, in order:

- #3ddd **From a drop to a published project**: 7 of 7 done, 0 ready. The drop area, the agent's draft, the owner's review and publishing without a deploy.


## Initiative: Port the realms: Ice Cream Man, Dead Gears, Everglade

Bring the demo's other worlds to openagents.com in Coax, each opening from the galaxy, and build Everglade, the forest city.

Specification: `docs/verse/realms-port.md` in the openagents repository.

Milestones, in order:

- #cc47 **Realms: one realm page for every world**: 4 of 4 done, 0 ready. The realm page hosts any realm; asset packs; every realm in the release checks.

- #2bca **Ice Cream Man on openagents.com**: 5 of 5 done, 0 ready. The seeded house game, rules checked against the Rust traces.

- #502e **Dead Gears on openagents.com**: 7 of 7 done, 0 ready. The ship survival game on a Coax ECS.

- #c6e3 **Everglade, the forest city**: 26 of 26 done, 0 ready. A designed MMORPG hub with quests, characters and a companion.

- #4bdb **More worlds: Sandbox, the Grid, Medieval Town**: 8 of 8 done, 0 ready. After the three above.

- #e156 **Fairy Forest**: 8 of 8 done, 0 ready. The painterly valley round the heart tree: restore the faded forest with Bloom, help its creatures, wake the heart tree's roots.

- #6602 **Fairy Forest: a life in the forest**: 12 of 12 done, 0 ready. After the story: moods you can read, your own fairy, a home and garden that grow, neighbours, courses, and the skills table.


## Initiative: Art styles: lowpoly, painterly and photoreal zones

Every zone on the Verse is drawn in one of a few art styles, named in realms.json and defined by the engine: lowpoly, painterly (Breath of the Wild's look: soft toon bands, blue haze, painted skies, wind-swept grass) and photoreal. This initiative builds painterly in full and moves Everglade and Hecate's Cemetery onto it.

Specification: `docs/verse/art-styles.md` in the openagents repository.

Milestones, in order:

- #8f19 **Zone styles: the field, the presets and the style fixture**: 3 of 3 done, 0 ready. A style per realm and zone, presets in verse/style.coax that realms declare rather than tune, and a fixture scene every feature shows on.

- #749f **Painterly in the engine**: 15 of 15 done, 0 ready. Each element of the painterly look as an engine feature with the spec's numbers: bands, rim, tinted soft shadows, haze, sky, bloom and shafts, day and night, foliage, grass, painted packs, characters, water and light outlines. Needed by Hecate's Cemetery (Planetary Villa) and Everglade.

- #8f23 **Painterly on phones, and the style check**: 3 of 3 done, 0 ready. The phone tier, and a release check that measures whether a painterly realm looks painterly, with a side-by-side sheet for the owner and Kiki.

- #c9ba **Everglade in painterly**: 8 of 8 done, 0 ready. Everglade adopts the painterly preset and replaces what breaks the look: the grey haze, the bare ground, the grey-box people, the physical river, the plain stand-in boxes. Hecate's Cemetery's adoption is in the Planetary Villa project (https://openagents.com/projects/planetary-villa).


## Initiative: Build your own realm

After the Haunted House, a visitor builds their own realm from one voice prompt: the agent answers with a short board of features, the visitor checks the ones they want, and the realm is made from only those and published without a deploy. The owner decides everything visitors get, and accepts or declines what others propose (Kiki's notes, October 29).

Specification: `docs/verse/realm-builder.md` in the openagents repository.

Milestones, in order:

- #f0a6 **Build your own realm**: 8 of 8 done, 0 ready. Prompt, board, make, and you are in your world: desktop and phone.

- #4d78 **Build your own realm: what's rough**: 14 of 14 done, 0 ready. Follow-ups from the first version (October 29): talk, looks, speed, and the paths not yet tested.

- #38f6 **Every realm edits the same way**: 4 of 4 done, 0 ready. Editing any realm on any planet works like Build your own realm: the design board from a voice prompt, the owner's settings pane, and accept or decline for others' changes (owner, October 29).


## Initiative: What Episode 290 promised: makers paid, realms forked, worlds shared

The gameplay promises from the Pleb Lab talk (docs/transcripts/290.md, October 30) that had no task: the three people who pitched games get paid in Bitcoin, makers earn from what players do in their realms, anyone can fork a realm, build one from Claude Code, and the worlds feel alive and shared.

Specification: `docs/transcripts/290.md` in the openagents repository.

Milestones, in order:

- #15fd **Makers get paid**: 2 of 3 done, 1 ready. Topher, Ted and Marc paid within 48 hours of the talk (by Sunday November 1, 4 p.m. CST), then every maker earns from their realm.

- #6992 **Fork and build from anywhere**: 2 of 2 done, 0 ready. Fork any realm into your own with your own rules; build one from a shallow clone and Claude Code.

- #992c **Worlds that are alive and shared**: 3 of 3 done, 0 ready. Other players in Fairy Forest, guards that coordinate, and realms you can reply with.


## Initiative: Native GPU Verse: Coax everything

Verse as a native desktop app on the GPU, with Coax writing all of it, shaders included: no browser, no WGSL, no outside shader toolchain. Owner decision 2026-10-30.

Specification: `docs/verse/native-gpu.md` in the openagents repository.

Milestones, in order:

- #2b8c **Coax on the GPU through Vulkan**: 7 of 8 done, 1 ready. SPIR-V from Coax, compute then graphics, a window and a boot session, and the realm contract on Vulkan (phases G0 to G4).

- #6e61 **Everglade native**: 0 of 3 done, 0 ready. Everglade's shaders in Coax and the realm playable natively at 60 fps (phase G5).

- #bd65 **One shader source**: 0 of 2 done, 1 ready. Coax emits WGSL too, and every realm leaves hand-written WGSL behind (phase G6).

- #dcd1 **What only native can do**: 0 of 10 done, 1 ready. Ray tracing, tensor-core denoising and upscaling, mesh shaders, bindless resources, streaming, async compute, HDR and VRR, native threads, Psionic in-process, and the Mac app.


## Initiative: Aethyr: the engine core

Aethyr, the engine: the foundational primitives every realm shares. GitHub #12609.

Specification: `docs/research/game-engine-systems.md` in the openagents repository.

Milestones, in order:

- #aa6d **Aethyr engine core: shared foundations for every realm**: 8 of 10 done, 1 ready. Time, spaces, state, input, replay, queries, entities, camera, budgets; then every realm migrated. GitHub #12609.


## Initiative: Edit realms from the command line

Aethyrforge's programmatic surface: `openagents realm ...` makes and edits realms through the same routes, SQL rules and forge edit operations as the page, so Claude Code and other agents can build realms. Plan: docs/cli/2026-11-01-openagents-cli-v1.md, 'The realm domain'. Needs the Coax project's cli-account milestone (NIP-98 as the caller).

Specification: `docs/cli/2026-11-01-openagents-cli-v1.md` in the openagents repository.

Milestones, in order:

- #9388 **Make, read and edit a realm from the CLI**: 0 of 3 done, 1 ready. List, show, make, edit, batch, shoot and publish.


## Activity

- 10-31 21:51 CDT #8163 done in 372857bc4d by claude-engine-core (Resource identity and budgets: hashed assets, declared limits that refuse)
- 10-31 21:51 CDT #8163 claude-engine-core: Done: 17 realm engines declare FrameBudgets and fail their checks on any frame past them; Hold the Stair's swarm clipped 3,318 vertices a frame (buffer 36,000 -> 52,000). Still to do: content-hashed asset identity, steps measured per realm (private clocks, ec-migrate), refusal inside the writers. Live in release 20261101T024936Z. (Resource identity and budgets: hashed assets, declared limits that refuse)
- 10-31 21:51 CDT #8163 claude-engine-core planned 6 of 6 done (Resource identity and budgets: hashed assets, declared limits that refuse)
- 10-31 21:17 CDT #8163 claimed by claude-engine-core (Resource identity and budgets: hashed assets, declared limits that refuse)
- 10-31 21:14 CDT #061a done in ea5b78db2a by claude-engine-core (One camera follow: consumes interpolated state, smoothed in one place)
- 10-31 21:14 CDT #061a claude-engine-core: Done: aethyr/shot.coax (CameraShot: top down, free orbit, dive; mutation 54/54), follow_shot/follow_first_shot in aethyr/follow.coax (189/189); verse/player.coax's step no longer moves the camera (camera_step after each step, step_once in steps_in); new check: the player is identical, 21 fields every frame, with the camera stepped every step, never or twice; verse/camera.coax's orbit draws through shot.coax, bit for bit (200,000 orbits, 100,000 walkers). aethyr_grid_per_metre lost #export_c (duplicate symbol when a realm engine and Player both link aethyr/space.coax). Chapter 10 implemented. golden-check: everglade flagged on NPC animation only (camera identical), coax-changelog as before; neither approved. Released 20261101T021328Z. (One camera follow: consumes interpolated state, smoothed in one place)
- 10-31 21:14 CDT #061a claude-engine-core planned 5 of 5 done (One camera follow: consumes interpolated state, smoothed in one place)
- 10-31 20:28 CDT #d2e8 claude-engine-core: state-defaults.test.ts passes on main at 5a7e886c5a after apps/openagents-web/build.sh: 17 states (deadgears and changelog included) and 20 face globals. The failures went with fee662eafc's compiler-emitted defaults; nothing to fix in the engine or state path. (Every realm on Aethyr's engine core)
- 10-31 20:27 CDT #061a claude-engine-core: chapters 8, 9 and 10 written from what runs (5a7e886c5a): each partly implemented, with an example and transcript (push-out, run-and-jump, follow-a-wall) and the rest listed. Chapter 8 fixes the draft's cell diagonal (the drawn split is fx >= fz). Next: the orbit, first-person and dive producers and the spring out of the player's step. (One camera follow: consumes interpolated state, smoothed in one place)
- 10-31 20:13 CDT #061a claude-engine-core: reopened: the follow landed (14948b1aa8); the orbit, first-person and dive producers and moving the spring out of the player's step are still to do, with chapters 8 to 10 (One camera follow: consumes interpolated state, smoothed in one place)
- 10-31 20:11 CDT #061a done in 14948b1aa8 by claude-engine-core (One camera follow: consumes interpolated state, smoothed in one place)
- 10-31 20:11 CDT #061a claude-engine-core: aethyr/follow.coax: one follow camera (look, zoom, spring, room from Colliders, eye between steps); verse/player.coax moves its camera only through it, bit for bit as before. Mutation 188/188 (follow.coax), no survivors on player.coax's changed lines. Grid camera at 60 FPS: move <= 0.177 m a frame, change <= 0.016 m (measured 0.013), rises every frame of a rising arc. Golden-check: every realm matches; coax-changelog flagged as before (not approved). Released 20261101T011014Z. Still open in chapter 10: orbit/first-person/dive producers and moving the camera out of the step. (One camera follow: consumes interpolated state, smoothed in one place)
- 10-31 19:45 CDT #061a claimed by claude-engine-core (One camera follow: consumes interpolated state, smoothed in one place)
- 10-31 19:27 CDT #d2e8 claude-engine-core: The Grid's simulation is one Coax session, verse/gridwalk.coax (97ff29ad34; invariant 9d48f351c9): the player, the touch sticks (now verse/sticks.coax), the input buffer and a replay record; the face passes input once a frame and no longer clamps time. Steps 1, 2 and 4 landed together: look and zoom apply on the steps (drawn between steps), and a 1,000-frame Grid session gives the same digest at all 673 steps natively and in wasm (coax/web/replay-hosts.test.ts). Mutation: gridwalk 206/206 (--root-only), sticks 207/207. Next: the follow camera into aethyr/ (ec-camera). (Every realm on Aethyr's engine core)
- 10-31 19:27 CDT #d2e8 claude-engine-core planned 3 of 8 done (Every realm on Aethyr's engine core)
- 10-31 19:01 CDT #d2e8 claimed by claude-engine-core (Every realm on Aethyr's engine core)
- 10-31 18:52 CDT #b3f8 done in aa50811121 by claude-engine-core (Determinism and replay: record inputs, replay bit-identically)
- 10-31 18:52 CDT #b3f8 claude-engine-core planned 7 of 7 done (Determinism and replay: record inputs, replay bit-identically)
- 10-31 18:40 CDT #b3f8 claude-engine-core: --project (Determinism and replay: record inputs, replay bit-identically)
- 10-31 18:16 CDT #b3f8 claimed by claude-engine-core (Determinism and replay: record inputs, replay bit-identically)
- 10-31 18:13 CDT #d522 done in 2ee5a97b03 by claude-engine-core (Entity identity and lifetime: stable IDs, spawn and despawn rules)
- 10-31 18:13 CDT #6a38 done in 928a9e25a6 by claude-engine-core (One spatial query API: ground, raycast, overlap, sweep)
- 10-31 18:10 CDT #d522 claude-engine-core planned 4 of 5 done (Entity identity and lifetime: stable IDs, spawn and despawn rules)
- 10-31 18:09 CDT #6a38 claude-engine-core planned 6 of 7 done (One spatial query API: ground, raycast, overlap, sweep)
- 10-31 18:05 CDT #d522 claimed by claude-engine-core (Entity identity and lifetime: stable IDs, spawn and despawn rules)
- 10-31 17:32 CDT #6a38 claimed by claude-engine-core (One spatial query API: ground, raycast, overlap, sweep)
- 10-31 17:31 CDT #6a38 claude-engine-core: ec-space's module is done (a93d35c2dd). Its one remaining item, the per-realm check that feet are drawn at physics height, moved to ec-migrate, so ec-query no longer waits on ec-space. (One spatial query API: ground, raycast, overlap, sweep)
- 10-31 17:27 CDT #7b34 claude-engine-core: --project (Input mapping: keys, touch and pads to typed actions, once per step)
- 10-31 17:21 CDT #7b34 claude-engine-core: --project (Input mapping: keys, touch and pads to typed actions, once per step)
- 10-31 17:18 CDT #7b34 done in 02ea2d3ce5 by claude-engine-core (Input mapping: keys, touch and pads to typed actions, once per step)

## In progress (2)

### #9c68 Listed upgrades: a better fairy from a maker, bought with one tap

status open · ready · in progress: claude-ep290-upgrades · size L · labels episode-290, payments, fairy-forest · milestone #15fd Makers get paid

Waits on: #73bd Tip the maker: sats from a player reach the realm's maker (resolved)

Event: `nevent1qqsfc69ekaf3a5rpl3cnprfh6wg93eufhu33f6fdlm38cpf3eajmskgprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2s4pswmv` (id `9c68b9b7531ed061fc71308d37d39058e789bf2314e92dfee27c0531cf65b859`)

Plan (6 of 7 done, updated 10-31 00:39 CDT by claude-ep290-upgrades):

- [x] Data model: makers.listings, purchases as makers.tips rows with a listing, wearing; payouts through makers.payouts (migration)
- [x] Routes in makers.c: list a fairy, top three, buy with sats (our Spark invoice) or credits (inference.credit), settle once, wear; admin approve/reject
- [x] Presence carries the bought look (verse.room_seen adds look from makers.wearing)
- [x] Fairy Forest: a look code (parts plus colours), others drawn in their own look, UPGRADE chip and pane (top three, BUY sats with QR and wallet link, CREDITS), admin APPROVE in the pane
- [x] API tests with the fake wallet (test_upgrades.py) and coax #tests
- [x] Docs (docs/verse/makers.md), commit, deploy API, release web
- [ ] **now:** Production: an approved listing shows under UPGRADE; one real purchase pays the maker (owner's sats)

From the talk: someone makes a better fairy, lists it, and in Fairy Forest an UPGRADE button next to your fairy shows the top three; one tap buys it (credits or sats), and its maker is paid like a tip. Start with fairies only: a maker uploads a fairy (the existing asset store and packs), the owner approves listings, and the bought fairy is what others see you as. Bitcoin rails only; credits are the existing credit ledger.

## Acceptance

- An approved listing shows under UPGRADE; buying it changes your fairy for you and for others
- The maker's share is paid out (fake wallet in tests; one real purchase on production)

Notes:

- 10-31 00:17 CDT claude-ep290-upgrades: Built in e3705cb6cd: makers.listings + makers.wearing (migration 0077); a purchase is a makers.tips row with its listing, paid out through makers.payouts (source 'upgrade'). Sats via our Spark invoice; credit from inference.credit. verse.room_seen carries the worn look. Listing has no in-game form yet: makers POST /api/playtest/upgrades/fairy, or makers.sh list-fairy.
- 10-31 00:36 CDT claude-ep290-upgrades: BUY's QR was empty on staging: a Spark invoice with lightning: is ~487 bytes, past coax/lib/qr's 412-byte byte mode. Fixed in ed74b63fb6: qr_encode_alnum (alphanumeric, 600 chars), invoice encoded in capitals at version 14; decoded exactly by macOS Vision. private.coax's tip QR still uses byte mode and likely has the same empty box (not changed here).
- 10-31 00:39 CDT claude-ep290-upgrades: Live: API oa-api-00075-8qg, web release ed74b63fb6 (20261031T053736Z). Staging checked end to end up to payment: approved listing under UPGRADE, TRY, BUY's QR decodes to the 487-char invoice, OPEN WALLET; staging listing 1 rejected again. Production answers {top: []}: no listing yet. Left open for the one real purchase; owner step in NEEDS_OWNER.md (3c79411a4c in AtlantisPleb/workspace). Unrelated: on main, 36 fairy.coax checks now hit the 20M compile-time step limit in verse/terrain.coax (also at eb6742455e without this change).

### #d2e8 Every realm on Aethyr's engine core

status open · in progress: claude-engine-core · size L · labels engine-core · milestone #aa6d Aethyr engine core: shared foundations for every realm

Waits on: #fa70 One frame loop: fixed-step simulation, interpolated drawing, one clock (resolved); #2f9e Typed spaces and units: world, local, camera, screen; metres versus grid units (open); #fc92 Declared defaults applied by construction for every realm's state (resolved); #7b34 Input mapping: keys, touch and pads to typed actions, once per step (resolved); #b3f8 Determinism and replay: record inputs, replay bit-identically (resolved); #6a38 One spatial query API: ground, raycast, overlap, sweep (resolved); #d522 Entity identity and lifetime: stable IDs, spawn and despawn rules (resolved); #061a One camera follow: consumes interpolated state, smoothed in one place (resolved); #8163 Resource identity and budgets: hashed assets, declared limits that refuse (resolved)

Event: `nevent1qqsd96qgfxxk6exh6pepqzexqn6h7ch53pfkljlr2rmetsh2atllt7qprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2s9l2f66` (id `d2e808498d6d64d7d072100b2604f57f62f488536fcbe350f795c2eaeafff5f8`)

Plan (3 of 8 done, updated 10-31 19:27 CDT by claude-engine-core):

- [x] Grid 1: the player, the touch sticks and a replay session in one Coax state (verse/gridwalk.coax); the face passes raw input once a frame
- [x] Grid 2: the Grid's step on aethyr loop, input, replay and math; no private clock clamp or libm in the step
- [ ] **now:** Grid 3 (ec-camera): look and zoom on the steps; the follow camera in aethyr/ (chapter 10)
- [x] Grid 4: a recorded Grid session (walk, run, jump, look, touch) gives the same digest at every step natively and in wasm
- [ ] Chapters 8-10 with real listings; chapters.json
- [ ] ec-budget
- [ ] Other realms: strict math in every step, the feet check, the other private clocks
- [ ] The scripted move of verse/ engine modules to aethyr/; close ec-space and #12609

GitHub #12609. Migrate the Grid, made realms, Everglade, Dead Gears, Ice Cream Man, Hecate's Cemetery, Fairy Forest and the space games onto the shared loop, spaces, input, queries and camera, deleting their private copies.

## Acceptance

- No realm keeps a private loop, camera follow, input path or ground logic
- Goldens change only where a fix is visible; the 60 FPS checks hold
- The Grid's floor is one consistent world-space grid from any position

Notes:

- 10-31 19:27 CDT claude-engine-core: The Grid's simulation is one Coax session, verse/gridwalk.coax (97ff29ad34; invariant 9d48f351c9): the player, the touch sticks (now verse/sticks.coax), the input buffer and a replay record; the face passes input once a frame and no longer clamps time. Steps 1, 2 and 4 landed together: look and zoom apply on the steps (drawn between steps), and a 1,000-frame Grid session gives the same digest at all 673 steps natively and in wasm (coax/web/replay-hosts.test.ts). Mutation: gridwalk 206/206 (--root-only), sticks 207/207. Next: the follow camera into aethyr/ (ec-camera).
- 10-31 20:28 CDT claude-engine-core: state-defaults.test.ts passes on main at 5a7e886c5a after apps/openagents-web/build.sh: 17 states (deadgears and changelog included) and 20 face globals. The failures went with fee662eafc's compiler-emitted defaults; nothing to fix in the engine or state path.

## Ready (5)

### #949e The realm contract on Vulkan: the WebGPU half of host.js in C

status open · ready · size L · labels engine, native-gpu · milestone #2b8c Coax on the GPU through Vulkan

Waits on: #f6b6 The desktop app: a Vulkan window on Wayland (resolved)

Event: `nevent1qqsff8jt63dz79uw9frya73ldsp7vuxufwg7rlqvrhjhnapcj9fu54gprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2sunkmjq` (id `949e4bd45a2f178e2a464efa3f6c03e670dc4b91e1fc0c1de579f4389153ca55`)

Vertex and uniform buffers, textures and samplers, asset packs from disk, frame pacing, input, audio and the inbox/outbox of the realm faces, so a realm's -face.c runs natively.

Plan: docs/verse/native-gpu.md

## Acceptance

- Star Catcher runs natively on the GPU with its light in Coax at 60 fps on coderos-4080
- Its #test checks still pass

## Files

- `verse/gpu/`

### #96f9 Coax emits WGSL: one shader source for web and native

status open · ready · size M · labels engine, native-gpu · milestone #bd65 One shader source

Waits on: #f627 coax spirv: the scalar f32/bool subset lowered to SPIR-V by Coax (resolved)

Event: `nevent1qqsfd7g34uj48k983gqkwlyvuzuxc7va9r3gy02jjjlmlge4kyghsncprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2s6834f8` (id `96f911af2553d8a78a01677c8ce0b86c799d28e2823d5294bfbfa335b111784f`)

WGSL from the same checked Coax as SPIR-V and Metal, replacing coax/web/wgsl.py, so the web build draws with the Coax light.

Plan: docs/verse/native-gpu.md

## Acceptance

- Star Catcher and Everglade on openagents.com draw from Coax-emitted WGSL with unchanged golden frames
- coax/web/wgsl.py is deleted

## Files

- `coax/`
- `coax/web/`

### #c08c HDR, variable refresh and our own frame pacing

status open · ready · size M · labels engine, native-gpu · milestone #dcd1 What only native can do

Waits on: #f6b6 The desktop app: a Vulkan window on Wayland (resolved)

Event: `nevent1qqsvprzt0jufg5mg3c5vuksvt7ht285vzdz3r3l6nndyn2692sycvqsprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2spqg5kx` (id `c08c4b7cb89453688e28ce5a0c5faeb51e8c134511c7fa9cda49ab4554098602`)

HDR swapchains where the display supports them, VRR and high refresh, and frame pacing owned by the host.

Plan: docs/verse/native-gpu.md

## Acceptance

- On an HDR or VRR display the host reports the mode and holds its pacing target

## Files

- `verse/gpu/`

### #2f9e Typed spaces and units: world, local, camera, screen; metres versus grid units

status open · ready · size L · labels engine-core · milestone #aa6d Aethyr engine core: shared foundations for every realm

Event: `nevent1qqszl8h8rmtmlqe5mskqwwcvu9a97juzwtg26sky9f4fdfdqy3xaqmqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2s4x67d2` (id `2f9ee71ed7bf8334dc2c073b0ce17a5f4b8272d0ad42c42a6a96a5a0244dd06c`)

Plan (3 of 4 done, updated 10-31 15:34 CDT by claude-engine-core):

- [x] aethyr/space.coax: world, local, view, screen and grid points; named conversions; metres vs grid units as R12 quantities
- [x] Refusals checked: aethyr/refusals.coax over aethyr/tests/spaces (E0317, E0325, E0822, a control)
- [x] Mutation 0 survivors (space 78/78, refusals 47/47)
- [ ] Each realm's figure, shadow, labels and camera read one WorldPoint through these conversions; a feet-at-physics-height check per realm (with ec-migrate)

GitHub #12609. aethyr/space.coax: typed positions per space, with conversions only through named functions, and units checked. Every drawn thing (figure, shadow, labels, camera) reads one authoritative position.

## Acceptance

- Mixing spaces fails to compile
- The figure's feet are drawn at its physics height in every realm
- Mutation: 0 survivors

Notes:

- 10-31 15:34 CDT claude-engine-core: a93d35c2dd pushed: module, refusals and docs. Open: acceptance 2 (feet at physics height in every realm) needs each realm's drawing moved onto world_to_grid / world_to_screen; the Grid's wk_at is world_to_grid with mid.y 0 (and its jump fix 76e1da1006 draws the figure at physics height). Releasing the claim so the realm part can go with ec-migrate or a fresh worker.

### #c0b5 GET /api/playtest/realms/mine

status open · ready · size S · labels api, realms · milestone #9388 Make, read and edit a realm from the CLI

Event: `nevent1qqsvpdf2ejuzqfjyhvcaqakstv8qm405e8at8jss45s2hwfpwcrm00qprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2s73ejvr` (id `c0b52accb8202644bb31d076d05b0e0dd5f4c9fab3ca10ad20abb9217607b7bc`)

The caller's realms (player key, account or NIP-98 key): slug, name, rev, published rev, listed, as lines and JSON.

## Acceptance

- a Coax API test covers an owner with realms, one with none, and a guest key

## Files

- `coax/web/api/realms.c`
- `coax/web/api/migrations/`

## Waiting (15)

### #e45e Everglade's world pass in Coax

status open · size L · labels engine, native-gpu · milestone #6e61 Everglade native

Waits on: #949e The realm contract on Vulkan: the WebGPU half of host.js in C (open); #7399 What shaders need from Coax: vectors, sampling, declared GPU resources (resolved)

Event: `nevent1qqswghjgc5e0wz2lautp7htcem3346pkj4mffgu7ayu9zma749mmy3qprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2sxntscr` (id `e45e48c532f7095fef161f5d78cee31ae836957694a39ee938516fbea977b244`)

everglade.wgsl's world shading (ground, water, foliage, characters, sky, fog) rewritten as Coax procedures, pass by pass, drawn by the Vulkan realm host.

Plan: docs/verse/native-gpu.md

## Acceptance

- Each ported pass matches a headless-Chrome golden frame of the web build within a stated tolerance
- coax test passes on the new light files

## Files

- `apps/openagents-web/`
- `verse/`

Notes:

- 10-31 03:36 CDT: --project

### #7730 Everglade's effects pass in Coax

status open · size M · labels engine, native-gpu · milestone #6e61 Everglade native

Waits on: #e45e Everglade's world pass in Coax (open)

Event: `nevent1qqs8wv9wgf99l4t3n9485l9tlefef4jjpj56ha786q2s66w9gh55f3qprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2s98lghc` (id `7730ae424a5fd571996a7a7cabfe5394d6520ca9abf7c7d0150d69c545e944c4`)

everglade-fx.wgsl (spells, rain, particles, HUD strokes) as Coax procedures on the Vulkan host.

Plan: docs/verse/native-gpu.md

## Acceptance

- Golden frames with spells and rain match the web build within tolerance

## Files

- `apps/openagents-web/`
- `verse/`

### #a6fd Everglade native and playable at 60 fps

status open · size M · labels engine, native-gpu · milestone #6e61 Everglade native

Waits on: #7730 Everglade's effects pass in Coax (open)

Event: `nevent1qqs2dlfzuc92ypaa2anyz5lyw58vqx0xffz3539agg2qtnyqtctdkqqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2sgdae6q` (id `a6fd22e60aa207bd57664153e4750ec019e64a451a44bd421405cc805e16db00`)

The whole realm natively: input, audio, the character chat and world list over the network (sockets and the approved TLS), the saved place, at 60 fps on the 4080.

Plan: docs/verse/native-gpu.md

## Acceptance

- The owner plays it on coderos-4080 in place of the browser kiosk
- Frame times stay under 16.7 ms p95 over a 5 minute walk

## Files

- `verse/gpu/`
- `apps/openagents-web/`

### #8e25 Every realm native, with no hand-written WGSL left

status open · size L · labels engine, native-gpu · milestone #bd65 One shader source

Waits on: #96f9 Coax emits WGSL: one shader source for web and native (open); #949e The realm contract on Vulkan: the WebGPU half of host.js in C (open)

Event: `nevent1qqsgufvg68czwfjmn32ffac4lqrp22r9grkk9s0ut5asnnv73vu7krqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2sv2tud8` (id `8e2588d1f027265b9c5494f715f80615286540ed62c1fc5d3b09cd9e8b39eb0c`)

Move the remaining realms' WGSL into Coax light files, realm by realm; split into one task per realm when started.

Plan: docs/verse/native-gpu.md

## Acceptance

- No *.wgsl written by hand remains in apps/openagents-web/
- Each realm's golden frames are unchanged

## Files

- `apps/openagents-web/`
- `verse/`

### #8888 Hardware ray tracing: shadows and reflections in Everglade's water

status open · size L · labels engine, native-gpu · milestone #dcd1 What only native can do

Waits on: #a6fd Everglade native and playable at 60 fps (open)

Event: `nevent1qqsg3zp7f2nzwmh0smmsrg3924qyvmngf3vsdatldxulc76rup7x8fgprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2sgjm3ll` (id `88883e4aa6276eef86f701a2255540466e684c5906f57f69b9fc7b43e07c63a5`)

Ray queries on the RT cores (VK_KHR_ray_query, acceleration structures built by the host from the realm's geometry) for sharp-to-soft shadows and water reflections, written in Coax and lowered by coax spirv.

Plan: docs/verse/native-gpu.md

## Acceptance

- Everglade's water reflects the town and its shadows come from rays, at 60 fps on the 4080
- A golden frame shows it; the web build keeps its current look

## Files

- `verse/gpu/`
- `coax/lib/compiler/`
- `apps/openagents-web/`

### #cc84 Tensor cores in the frame: a Coax denoiser and upscaler

status open · size L · labels engine, native-gpu · milestone #dcd1 What only native can do

Waits on: #8888 Hardware ray tracing: shadows and reflections in Everglade's water (open)

Event: `nevent1qqsvep8v470rky8mrypclhej6u0p5k47aer3m9yx3xmkzuxdd26cjyqprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2svku84k` (id `cc84ecaf9e3b10fb19038fdf32d71e1a5abeee471d948689b76170cd6ab58910`)

Cooperative-matrix shaders written in Coax for a ray-tracing denoiser and a temporal upscaler (render lower, show sharp at full resolution), sharing kernels with Psionic where they fit.

Plan: docs/verse/native-gpu.md

## Acceptance

- Everglade at 2560x1440 renders internally at 1280x720 and upscales, with a measured image difference against native resolution
- Frame time drops by a measured amount

## Files

- `verse/gpu/`
- `psionic/`

### #3583 Mesh shaders for dense geometry

status open · size L · labels engine, native-gpu · milestone #dcd1 What only native can do

Waits on: #a6fd Everglade native and playable at 60 fps (open)

Event: `nevent1qqsrtqclp9ff94j4xhye0hyq0x9klmq786j50rl48agz9s0sz9guypcprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2s8nh8qw` (id `35831f095292d65535c997dc80798b6fec1e3ea5478ff53f5022c1f01151c207`)

Task and mesh shader entry points in coax spirv and a meshlet path in the host, so forests and towns draw many times the geometry at the same frame time.

Plan: docs/verse/native-gpu.md

## Acceptance

- A dense Everglade forest scene draws at least 4x the triangles of the web build at 60 fps

## Files

- `verse/gpu/`
- `coax/lib/compiler/`

### #0a9c Bindless textures and materials

status open · size M · labels engine, native-gpu · milestone #dcd1 What only native can do

Waits on: #949e The realm contract on Vulkan: the WebGPU half of host.js in C (open)

Event: `nevent1qqsq48qxctj8k05ney0rzyheptw29dan2edslwke6kugpvvwp4a7v9gprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2sxut6zl` (id `0a9c06c2e47b3e93c91e3112f90adca2b7b3565b0fbad9d5b880b18e0d7be615`)

Descriptor indexing so a scene binds thousands of textures and materials at once.

Plan: docs/verse/native-gpu.md

## Acceptance

- A realm draws with more than 1,000 distinct textures in one frame

## Files

- `verse/gpu/`

### #9e5d All the video memory, streamed from disk

status open · size M · labels engine, native-gpu · milestone #dcd1 What only native can do

Waits on: #949e The realm contract on Vulkan: the WebGPU half of host.js in C (open)

Event: `nevent1qqsfuhf5tqcxxdgpus7mdz24c0rrmewsp6zfysv60xjh58c25q4j0qgprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2slmhdyr` (id `9e5d345830633501e43db68955c3c63de5d00e8492419a79a57a1f0aa02b2781`)

Asset packs streamed from disk into the 16 GB of video memory as you move, with budgets and eviction.

Plan: docs/verse/native-gpu.md

## Acceptance

- A walk across Everglade loads high-resolution packs without a hitch over 16.7 ms

## Files

- `verse/gpu/`

### #a68e Async compute: simulation beside drawing

status open · size M · labels engine, native-gpu · milestone #dcd1 What only native can do

Waits on: #949e The realm contract on Vulkan: the WebGPU half of host.js in C (open)

Event: `nevent1qqs2drjhfaauuy43kr2uuhkt2p0vr3nads3vzlt4qeknfd78hn3x3qgprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2sm9fwqf` (id `a68e574f7bce12b1b0d5ce5ecb505ec1c67d6c22c17d75066d34b7c7bce26881`)

A second queue for physics, particles and water simulation running while the frame draws.

Plan: docs/verse/native-gpu.md

## Acceptance

- Particle or water simulation moves to the compute queue with a measured frame-time gain

## Files

- `verse/gpu/`

### #92ec The realm's CPU work on every core

status open · size L · labels engine, native-gpu · milestone #dcd1 What only native can do

Waits on: #949e The realm contract on Vulkan: the WebGPU half of host.js in C (open)

Event: `nevent1qqsf9mp9fk7jp6uxl49q3guy274hteds6aeg7regq8p3pd80kek6t9cprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2sv48hl8` (id `92ec254dbd20eb86fd4a08a38457ab75e5b0d7728f0f2801c310b4efb66da597`)

Coax jobs (spec section 12) on native threads for simulation and animation, deterministic across worker counts.

Plan: docs/verse/native-gpu.md

## Acceptance

- The same replay with 1, 2, 4 and 8 workers gives identical results
- A realm's simulation step is measurably faster on 28 cores

## Files

- `coax/`
- `verse/`

### #1604 Psionic and the renderer in one process, on one GPU

status open · size L · labels engine, native-gpu · milestone #dcd1 What only native can do

Waits on: #a6fd Everglade native and playable at 60 fps (open)

Event: `nevent1qqspvpy6eflulypj4nvhmx8aw6eqlxapumjlrt5k7cfx9zvwd3yz44cprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2sue8vz5` (id `16049aca7fcf9032acd97d98fd76b20f9ba1e6e5f1ae96f61262898e6c482ad7`)

A model's output written into a GPU buffer the renderer reads (characters' speech, animation or hand tracking), with no copy and no network hop.

Plan: docs/verse/native-gpu.md

## Acceptance

- A character in a native realm is driven by a Psionic model in the same process, with the shared buffer measured as copy-free

## Files

- `verse/gpu/`
- `psionic/`

### #d92e The Mac desktop app over Metal

status open · size L · labels engine, native-gpu · milestone #dcd1 What only native can do

Waits on: #949e The realm contract on Vulkan: the WebGPU half of host.js in C (open)

Event: `nevent1qqsdjtnjrk85kwn8e4z8fv7xt2ksn4xammx7qvc2uzz3nl2alxu7rlsprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2s7rqq3r` (id `d92e721d8f4b3a67cd4474b3c65aad09d4dddecde0330ae08519fd5df9b9e1fe`)

The same realm host contract over Metal on macOS, from coax metal, so realms run natively on Macs.

Plan: docs/verse/native-gpu.md

## Acceptance

- Star Catcher and Everglade run natively on a Mac at the display's rate

## Files

- `verse/gpu/`
- `apps/`

### #8d90 openagents realm list, show, doc, asks, proposals, panes, open, shot

status open · size M · labels cli, realms · milestone #9388 Make, read and edit a realm from the CLI

Waits on: #c0b5 GET /api/playtest/realms/mine (open)

Event: `nevent1qqsgmyr8z6mme8qalhqw5qkjr7yshyahpv3gun58dlegnpvsq7nzn0cprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2svr5nkn` (id `8d906716b7bc9c1dfdc0ea02d21f890b93b70b228e4e876ff289859007a629bf`)

Read verbs over the HTTPS client, signed with NIP-98. `realm shot SLUG OUT.png [--phone]` uses coax/web/shot.coax (headless, muted).

## Acceptance

- openagents realm show everglade prints the spec lines
- openagents realm shot <a made realm> out.png writes a lit PNG at 1440x900

## Files

- `apps/openagents-cli/realm.coax`

### #757d openagents realm new, make, revise, switch, ask, move, place, settings, lore, quest, accept, decline, undo, publish, fork, apply

status open · size L · labels cli, realms, aethyrforge · milestone #9388 Make, read and edit a realm from the CLI

Waits on: #8d90 openagents realm list, show, doc, asks, proposals, panes, open, shot (open)

Event: `nevent1qqs82l2srlg30f9fenjuswzkngz3k3whfw7s4w5g205wc375tkpyr4qprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksygy4rdc0n75hmqs5uryl8lee9gwaze5wv9tpaus69f53drfcy3fwrqpsgqqqqe2shtl96j` (id `757d501fd117a4a9cce5c838569a051b45d74bbd0aba8853e8ec47d45d8241d4`)

Every edit names a revision (read first, or --rev N); 409 exits 5 with `rev N`. `realm move` merges the move with aethyr/forge/edit.coax's operations. `realm apply SLUG FILE` runs JSON-lines ops, consecutive moves as one save. A cli-journey check like forge-journey: make, move, stale save (exit 5), undo, publish, fork.

## Acceptance

- the cli-journey check passes against staging with a test account
- a realm made and edited by Claude Code through the CLI is published and shows the edits in a headless shot

## Files

- `apps/openagents-cli/realm.coax`
- `coax/web/tool.sh`

## Done (239)

- #8163 Resource identity and budgets: hashed assets, declared limits that refuse · [372857bc4d](https://github.com/OpenAgentsInc/openagents/commit/372857bc4d58f7c227672955f4ecba1f5d183fa3)
- #061a One camera follow: consumes interpolated state, smoothed in one place · [ea5b78db2a](https://github.com/OpenAgentsInc/openagents/commit/ea5b78db2abf751b4f76373de4f6e64e88bb46dd)
- #b3f8 Determinism and replay: record inputs, replay bit-identically · [aa50811121](https://github.com/OpenAgentsInc/openagents/commit/aa508111210810f0aa439739002e44dea3f66c5f)
- #d522 Entity identity and lifetime: stable IDs, spawn and despawn rules · [2ee5a97b03](https://github.com/OpenAgentsInc/openagents/commit/2ee5a97b039f20243e2ebd022a8cac5b2de0e1e5)
- #6a38 One spatial query API: ground, raycast, overlap, sweep · [928a9e25a6](https://github.com/OpenAgentsInc/openagents/commit/928a9e25a68689c0dc51f9a4a5b4699b3388b30e)
- #7b34 Input mapping: keys, touch and pads to typed actions, once per step · [02ea2d3ce5](https://github.com/OpenAgentsInc/openagents/commit/02ea2d3ce5add112b27f28d4d607edc8daff4282)
- #fa70 One frame loop: fixed-step simulation, interpolated drawing, one clock · [3102756f8b](https://github.com/OpenAgentsInc/openagents/commit/3102756f8b8f5c5ef2347ca54e5e294f4c43b26a)
- #fc92 Declared defaults applied by construction for every realm's state · [c5fb95a184](https://github.com/OpenAgentsInc/openagents/commit/c5fb95a184072e11ad01969e6a4871174bb4a840)
- #7a55 Everglade: a new visitor sees a first goal · [d7a09fd586](https://github.com/OpenAgentsInc/openagents/commit/d7a09fd586)
- #4916 Everglade on a phone: the spell bar stays out of the middle of the view · [4add48c129](https://github.com/OpenAgentsInc/openagents/commit/4add48c129)
- #4b96 The Grid: say where to go once you walk off the start · [26ef301325](https://github.com/OpenAgentsInc/openagents/commit/26ef301325)
- #73bd Tip the maker: sats from a player reach the realm's maker · [2c0c423d24](https://github.com/OpenAgentsInc/openagents/commit/2c0c423d24)
- #7399 What shaders need from Coax: vectors, sampling, declared GPU resources · [926cae2bfe](https://github.com/OpenAgentsInc/openagents/commit/926cae2bfe)
- #4df5 A boot session: Vulkan straight to the screen (VK_KHR_display) · [3073a89d5d](https://github.com/OpenAgentsInc/openagents/commit/3073a89d5d)
- #f6b6 The desktop app: a Vulkan window on Wayland · [caf859ebc2](https://github.com/OpenAgentsInc/openagents/commit/caf859ebc2)
- #5d10 Vulkan graphics offscreen: vertex and fragment entry points from Coax · [d9804e9510](https://github.com/OpenAgentsInc/openagents/commit/d9804e9510)
- #7992 Reply with a realm: name a building after a post, smash it, share the clip · [2970e71f54](https://github.com/OpenAgentsInc/openagents/commit/2970e71f54)
- #6b43 FORK on any realm: your own copy with your own rules · [b2b3c60359](https://github.com/OpenAgentsInc/openagents/commit/b2b3c60359)
- #f627 coax spirv: the scalar f32/bool subset lowered to SPIR-V by Coax · [69eff0b430](https://github.com/OpenAgentsInc/openagents/commit/69eff0b430)
- #dceb Everglade's guards coordinate: flank, call for help, cut off the escape · [b3412d8a5f](https://github.com/OpenAgentsInc/openagents/commit/b3412d8a5fb1f9f8a2752dbc1225c0e8b2ddf932)
- #3475 Headless Vulkan compute on the RTX 4080 from Coax-built SPIR-V · [27fc360f36](https://github.com/OpenAgentsInc/openagents/commit/27fc360f36)
- #c573 A SPIR-V writer in Coax, and the first module built with it · [d2a8b3c9de](https://github.com/OpenAgentsInc/openagents/commit/d2a8b3c9de)
- #d009 Fairy Forest: other fairies in the valley · [f762c4f7ae](https://github.com/OpenAgentsInc/openagents/commit/f762c4f7ae)
- #1d96 Build a realm from Claude Code: the skill, the shallow clone, the pull request · [25ecc1e911](https://github.com/OpenAgentsInc/openagents/commit/25ecc1e911)
- #eb66 A maker claims their realm: Nostr key and Lightning address on it · [7c24118e2c](https://github.com/OpenAgentsInc/openagents/commit/7c24118e2c)
- #9b37 The Grid from the homepage: WALK AROUND, human players, and everyone walking it · [4be85dd023](https://github.com/OpenAgentsInc/openagents/commit/4be85dd023)
- #fe3e Ice Cream Man: walking closes the first tip · [df094fc1f2](https://github.com/OpenAgentsInc/openagents/commit/df094fc1f2422807f6c559b954e1ba46ba73d437)
- #7d06 Everglade: two guards' shouts never overlap · [f839fb2450](https://github.com/OpenAgentsInc/openagents/commit/f839fb24501fb6c7111ed9dca7b32f446a2090fb)
- #ecaf The galaxy: MAKE YOUR REALM hides the orange-planet hint · [08609da25d](https://github.com/OpenAgentsInc/openagents/commit/08609da25dbcd21cb3741dbf10c2fe8a62c51fd3)
- #5e8a Hecate's Cemetery: the opening flight plays on a first visit only · [57987b1b01](https://github.com/OpenAgentsInc/openagents/commit/57987b1b01b68b5005ec63c6a5e54b589204559f)
- #d806 Every page: a logged-out visitor's who-am-I is no console error · [c4d77c7d44](https://github.com/OpenAgentsInc/openagents/commit/c4d77c7d44dd4ae8e8862a63621b792b7071bdad)
- #1255 Haunted House: the first step and the save notice fit the screen · [f717ff79eb](https://github.com/OpenAgentsInc/openagents/commit/f717ff79ebd5190bb01403938427ceba02dc0d6a)
- #d31d Everglade on a phone: the cast line names no Esc · [c48addd549](https://github.com/OpenAgentsInc/openagents/commit/c48addd5496285e32d4eeb47f2af471832ede005)
- #bb6e Everglade on a phone: the chat line no longer runs under HP and WANTED · [f88d73a27a](https://github.com/OpenAgentsInc/openagents/commit/f88d73a27a7055491d6c86ba9453f0d7e9127245)
- #160b Hecate's Cemetery: a first visit mints one player, with no 403s · [eb43a4b8af](https://github.com/OpenAgentsInc/openagents/commit/eb43a4b8af8b7ae16fc8916c88393a52f33d867d)
- #78ba Real-microphone check of the orb and the Haunted House voice button · [3c000f77f5](https://github.com/OpenAgentsInc/openagents/commit/3c000f77f5)
- #6701 Everglade: other players in the city · [d5113ac516](https://github.com/OpenAgentsInc/openagents/commit/d5113ac516)
- #5c18 Everglade: baked sky light · [073d1fa942](https://github.com/OpenAgentsInc/openagents/commit/073d1fa942)
- #7a1d Everglade: distance detail levels · [073d1fa942](https://github.com/OpenAgentsInc/openagents/commit/073d1fa942)
- #9acb Everglade: rumours that pass between the people · [073d1fa942](https://github.com/OpenAgentsInc/openagents/commit/073d1fa942)
- #413d Everglade: walls and towers topple · [cd94c3cbe6](https://github.com/OpenAgentsInc/openagents/commit/cd94c3cbe6)
- #6f94 Everglade: the sledgehammer · [cd94c3cbe6](https://github.com/OpenAgentsInc/openagents/commit/cd94c3cbe6)
- #d733 Everglade: bunting, window boxes, signposts and ivy · [e792aecf12](https://github.com/OpenAgentsInc/openagents/commit/e792aecf12)
- #fce0 Everglade: wood smoke from the chimneys, the bakehouse and the smithy · [e792aecf12](https://github.com/OpenAgentsInc/openagents/commit/e792aecf12)
- #ee36 Everglade: songbirds, butterflies and frogs · [9ccb71eab7](https://github.com/OpenAgentsInc/openagents/commit/9ccb71eab747310146e2cc81396af9b5f6a2521b)
- #4d13 Painterly snow at dusk: a sky that keeps its colour · [9edd531ce3](https://github.com/OpenAgentsInc/openagents/commit/9edd531ce3c69f6b311f7991f8f01b19f25fbfda)
- #047b Realms show a loading screen while they load · [25f5f02583](https://github.com/OpenAgentsInc/openagents/commit/25f5f02583)
- #6145 The board always holds the thing you asked for · [4b866bc14a](https://github.com/OpenAgentsInc/openagents/commit/4b866bc14a464100a59729f54b3255cdbb6c9b15)
- #9c1c Everglade in painterly: the style check, the phone tier and the owner's look · [23702aa608](https://github.com/OpenAgentsInc/openagents/commit/23702aa6085b42942ce020e9cc1fde34361bacbe)
- #60cf Everglade: each house its own plaster and roof colour · [b4e40d873e](https://github.com/OpenAgentsInc/openagents/commit/b4e40d873e2614d212f64bf40b8fb3f62f4f91a7)
- #b192 The Sandbox on the galaxy · [dcddbfc1d7](https://github.com/OpenAgentsInc/openagents/commit/dcddbfc1d7e93961c0a842a462f8c4c6ad8355db)
- #be9f Drive the builder by voice end to end · [3c000f77f5](https://github.com/OpenAgentsInc/openagents/commit/3c000f77f58a95f330f4b7ff12f4bc3ae89a781a)
- #dbf2 Tapping a character on a phone starts talking to them · [3c000f77f5](https://github.com/OpenAgentsInc/openagents/commit/3c000f77f58a95f330f4b7ff12f4bc3ae89a781a)
- #bd8f Analytics for each step: prompt, board, toggles, made, entered · [8a5c7fa219](https://github.com/OpenAgentsInc/openagents/commit/8a5c7fa219294ca284262f7934cb6f15aa842dc4)
- #d469 The Haunted House moves you on to Build your own realm · [8cb4c5c793](https://github.com/OpenAgentsInc/openagents/commit/8cb4c5c79336d0e9f235d243ae092d1de3b14775)
- #f831 Make the realm from only the checked features, live at /realms/<slug> with no deploy · [0df58ce88f](https://github.com/OpenAgentsInc/openagents/commit/0df58ce88f3168b9da09c1a028f3f76bb11fccfc)
- #e25b /realms/new: hold to talk, the board, check features on or off, MAKE IT · [4db94d0b3e](https://github.com/OpenAgentsInc/openagents/commit/4db94d0b3ee7898c7ac96cbd56de26bd1088a8ab)
- #14d2 First person on phones: a thumb drag looks up and down · [78a36028e8](https://github.com/OpenAgentsInc/openagents/commit/78a36028e83cc55bc1c0603639a6b805bcda87ce)
- #0881 2D: rocks and trees block you; you jump onto them · [09607ead4a](https://github.com/OpenAgentsInc/openagents/commit/09607ead4a9fecb6e1fb67ebd01627bd5c667014)
- #020c Top-down: your figure reads as a figure, not a thin line · [83290b2f09](https://github.com/OpenAgentsInc/openagents/commit/83290b2f09301f1884aae98096419f5cd9d28117)
- #5ea6 Lantern posts show on bright ground · [83290b2f09](https://github.com/OpenAgentsInc/openagents/commit/83290b2f09301f1884aae98096419f5cd9d28117)
- #de57 The flock can be seen from the ground · [83290b2f09](https://github.com/OpenAgentsInc/openagents/commit/83290b2f09301f1884aae98096419f5cd9d28117)
- #eaf7 A made realm looks like its style: painterly realms drawn painterly · [4db94d0b3e](https://github.com/OpenAgentsInc/openagents/commit/4db94d0b3ee7898c7ac96cbd56de26bd1088a8ab)
- #f7c3 Characters in a made realm talk, and the voice tool works · [4db94d0b3e](https://github.com/OpenAgentsInc/openagents/commit/4db94d0b3ee7898c7ac96cbd56de26bd1088a8ab)
- #d596 Bram's metal shoulder guard keeps its metal colour · [39c9d2a9d0](https://github.com/OpenAgentsInc/openagents/commit/39c9d2a9d0)
- #285f Robes for the archmage, priest and astronomer · [39c9d2a9d0](https://github.com/OpenAgentsInc/openagents/commit/39c9d2a9d0)
- #56bb Cats: a real cat model · [487011a087](https://github.com/OpenAgentsInc/openagents/commit/487011a087)
- #5786 Others' changes reach the owner as ACCEPT or DECLINE cards · [c7959232d1](https://github.com/OpenAgentsInc/openagents/commit/c7959232d142f93899467cc5a130a5973da3a3b4)
- #badb The owner's settings pane: which tools visitors get, checked on the server · [c7959232d1](https://github.com/OpenAgentsInc/openagents/commit/c7959232d142f93899467cc5a130a5973da3a3b4)
- #5376 The first board comes back in under 5 seconds · [0df58ce88f](https://github.com/OpenAgentsInc/openagents/commit/0df58ce88f3168b9da09c1a028f3f76bb11fccfc)
- #85f2 A guest's realm moves to their account when they log in · [1752046995](https://github.com/OpenAgentsInc/openagents/commit/17520469955e6f9b6aa229787f42a46a5eaff05c)
- #aaab A typed planner turns a spoken prompt into a feature board · [c7959232d1](https://github.com/OpenAgentsInc/openagents/commit/c7959232d142f93899467cc5a130a5973da3a3b4)
- #8e34 Tools: made with CRAFT from what you gather · [8ddda68884](https://github.com/OpenAgentsInc/openagents/commit/8ddda68884af49584b9ff518981d7fa32c0ce96d)
- #5648 Mending broken things with MEND · [9b51f4f985](https://github.com/OpenAgentsInc/openagents/commit/9b51f4f9854708af21b498516b8980a2dd850d9a)
- #75ae The woodcutter who gives you the axe · [59fbada9e6](https://github.com/OpenAgentsInc/openagents/commit/59fbada9e62e038fa51b6930c8bbcefea9adaea8)
- #e9a1 Filling the pond with WATER, and the waterfall · [6e3ccb89f7](https://github.com/OpenAgentsInc/openagents/commit/6e3ccb89f78bb6ca644a8043dfd534cc92262fe0)
- #0fa6 Nights in Lanternwood: glowing fruit, moths, and places only LIGHT shows · [80271bb8b4](https://github.com/OpenAgentsInc/openagents/commit/80271bb8b4560ea90d0f3005384bc83eabe94698)
- #5485 Everglade watch: guards hunt you for spells on the town or its people · [2aba7b849c](https://github.com/OpenAgentsInc/openagents/commit/2aba7b849c)
- #3cf2 Learned skills: Speak, Mend, Gather, Craft, Light, Water · [d174701ad3](https://github.com/OpenAgentsInc/openagents/commit/d174701ad3b2faf6cf25fc3c9c620105ddd80432)
- #9279 Timed courses with share links and friends' times · [e354237b27](https://github.com/OpenAgentsInc/openagents/commit/e354237b2715dc55ca8447638943e88740aa6c8c)
- #6f89 Water W3 (rest): ponds, the weir and plunge pool, NPC swimming, rowboat props · [176c9cad12](https://github.com/OpenAgentsInc/openagents/commit/176c9cad12)
- #7456 Name and role labels follow the person · [176c9cad12](https://github.com/OpenAgentsInc/openagents/commit/176c9cad12)
- #295a Everglade: the Ranger, the people and the cats in painterly · [487011a087](https://github.com/OpenAgentsInc/openagents/commit/487011a087)
- #f300 Everglade: painterly tag and a painted pack · [904d3880ce](https://github.com/OpenAgentsInc/openagents/commit/904d3880ce)
- #0b7f Creatures move in and build homes near yours · [fe86e32583](https://github.com/OpenAgentsInc/openagents/commit/fe86e32583c7b93f36a63ffc72cec76899b79ad7)
- #2561 A garden that grows over real days · [fe86e32583](https://github.com/OpenAgentsInc/openagents/commit/fe86e32583c7b93f36a63ffc72cec76899b79ad7)
- #81da Your home grows room by room · [e9e5e6b8db](https://github.com/OpenAgentsInc/openagents/commit/e9e5e6b8db3304aca588dd20dfadbf2c378e997b)
- #64a4 Make your fairy, and the mirror pool · [e9e5e6b8db](https://github.com/OpenAgentsInc/openagents/commit/e9e5e6b8db3304aca588dd20dfadbf2c378e997b)
- #8ad8 Fairy Forest: creature moods at a glance and passing remarks · [36beb463fd](https://github.com/OpenAgentsInc/openagents/commit/36beb463fdfa494efd797789615d7f85f17676f6)
- #f278 Everglade: replace the plain boxes that break the painting · [f4e6c61c3e](https://github.com/OpenAgentsInc/openagents/commit/f4e6c61c3e)
- #d914 Everglade voice edits: purple flowers come out purple · [60543888ff](https://github.com/OpenAgentsInc/openagents/commit/60543888ff9b58de0e9e7007f437f328a721f412)
- #31d0 Everglade voice edits: sentences said meanwhile wait their turn · [60543888ff](https://github.com/OpenAgentsInc/openagents/commit/60543888ff9b58de0e9e7007f437f328a721f412)
- #3a9a Everglade voice edits: an answer in under 3 s at p95 · [63cb278fbe](https://github.com/OpenAgentsInc/openagents/commit/63cb278fbecbd0535086d1d44b304c4ef3dfde5d)
- #a3ce Water W8: spells and water · [faf143de0a](https://github.com/OpenAgentsInc/openagents/commit/faf143de0a)
- #a91b Water W6 (rest): Kelvin wakes, foam trails, spray, mist and drips, debris, rowboats · [faf143de0a](https://github.com/OpenAgentsInc/openagents/commit/faf143de0a)
- #cffb Making Fairy Forest: panes the agent brings up one at a time · [e072dd9398](https://github.com/OpenAgentsInc/openagents/commit/e072dd9398)
- #c278 The water system from the previous Verse engine · [4c6450241c](https://github.com/OpenAgentsInc/openagents/commit/4c6450241c)
- #fd8a Water W11: measurement · [4c6450241c](https://github.com/OpenAgentsInc/openagents/commit/4c6450241c)
- #e5f0 Water W10: ocean and scale · [4c6450241c](https://github.com/OpenAgentsInc/openagents/commit/4c6450241c)
- #3823 Everglade voice edits: checked on production · [867ffb109d](https://github.com/OpenAgentsInc/openagents/commit/867ffb109d)
- #d4c3 Everglade: something far to stack in the haze · [ff8752c788](https://github.com/OpenAgentsInc/openagents/commit/ff8752c788)
- #ce76 Painterly characters: two bands, strong rim, flat skin and cloth · [47504af4c4](https://github.com/OpenAgentsInc/openagents/commit/47504af4c4)
- #1996 Fairy Forest's WGSL carries its style block (realms.test.ts passes) · [3c503e91c7](https://github.com/OpenAgentsInc/openagents/commit/3c503e91c7)
- #b290 Fairy Forest saves where you are and what you restored · [4fc450cd63](https://github.com/OpenAgentsInc/openagents/commit/4fc450cd63)
- #3725 Fairy Forest's inbox: direct messages between players · [c9a978e0a4](https://github.com/OpenAgentsInc/openagents/commit/c9a978e0a4)
- #a490 The style check: measure "looks painterly" and the side-by-side sheet · [313698227a](https://github.com/OpenAgentsInc/openagents/commit/313698227a)
- #056b The painterly phone tier · [bb2649a923](https://github.com/OpenAgentsInc/openagents/commit/bb2649a923)
- #66e9 Water W9: weather · [fa43b0224b](https://github.com/OpenAgentsInc/openagents/commit/fa43b0224b)
- #d280 Fairy Forest's chat room: one room for everyone, always on screen · [ec2ecfa0d6](https://github.com/OpenAgentsInc/openagents/commit/ec2ecfa0d6)
- #7ed3 Water W7: underwater · [3d8eafa114](https://github.com/OpenAgentsInc/openagents/commit/3d8eafa114)
- #1f89 Everglade voice edits: patches of flowers and crops, and things drawn with geometry · [4ee094f629](https://github.com/OpenAgentsInc/openagents/commit/4ee094f629)
- #e66f Everglade voice edits: many more things to place · [4ee094f629](https://github.com/OpenAgentsInc/openagents/commit/4ee094f629)
- #e7e6 Everglade voice edits: the verbs the owner asks for · [4ee094f629](https://github.com/OpenAgentsInc/openagents/commit/4ee094f629)
- #6a65 Hecate's Cemetery: edit mode for admins
- #bf7b The Grid: edit mode for admins
- #9bb2 Everglade edits through the shared layer · [2ac33d515d](https://github.com/OpenAgentsInc/openagents/commit/2ac33d515d)
- #deff Every other realm gets the shared edit layer · [519145efa1](https://github.com/OpenAgentsInc/openagents/commit/519145efa1)
- #2496 One edit layer every realm loads · [a510ee58de](https://github.com/OpenAgentsInc/openagents/commit/a510ee58de)
- #4c58 Water W4: spectral waves · [bf07cfa56d](https://github.com/OpenAgentsInc/openagents/commit/bf07cfa56d9731241603e952a89e2e8041aa819c)
- #bf7f Everglade: Meteor Swarm and the Thunderbolt break houses on openagents.com · [7067315ff0](https://github.com/OpenAgentsInc/openagents/commit/7067315ff0)
- #b58d Everglade breaks: kit pieces with object hit points, chunks with physics, structural collapse · [7067315ff0](https://github.com/OpenAgentsInc/openagents/commit/7067315ff0)
- #800a Everglade: the old spell hotbar (Meteor Swarm, Thunderbolt, Mega Thunderbolt, Water Orb) · [7067315ff0](https://github.com/OpenAgentsInc/openagents/commit/7067315ff0)
- #e73a Rulesets a realm opts into: SRD 5e in Everglade · [fccc94e85e](https://github.com/OpenAgentsInc/openagents/commit/fccc94e85e)
- #d84a Water W5: scene copies, refraction, reflection, contact foam and soft particles · [c09c4f1e0b](https://github.com/OpenAgentsInc/openagents/commit/c09c4f1e0ba36d78026ca96eb0151bbf0a5dee49)
- #1b59 Fairy Forest's creatures talk and remember you · [23c4f59ffe](https://github.com/OpenAgentsInc/openagents/commit/23c4f59ffe)
- #7af6 Everglade: the people are real Quaternius characters, not two shapes · [8574fbe0f9](https://github.com/OpenAgentsInc/openagents/commit/8574fbe0f9)
- #eba5 Owners for every realm, not only made ones · [5ff2953c1a](https://github.com/OpenAgentsInc/openagents/commit/5ff2953c1a)
- #30b0 Water: Everglade's fountain on the water system · [04e4e0721d](https://github.com/OpenAgentsInc/openagents/commit/04e4e0721d92dd3fc1a85578b75eddafd9733ecd)
- #1b0f Setup questions before the board, and the cameras the made realm honours · [5a933f8baf](https://github.com/OpenAgentsInc/openagents/commit/5a933f8baf)
- #001d The last two glades and roots, and the heart tree blooming · [5899e95d01](https://github.com/OpenAgentsInc/openagents/commit/5899e95d01)
- #db1d TALK on /realms/new is a toggle, not hold to talk · [7411971aa9](https://github.com/OpenAgentsInc/openagents/commit/7411971aa9)
- #3d63 Water W7 (part): caustics on what lies under the water · [519e68ec62](https://github.com/OpenAgentsInc/openagents/commit/519e68ec62a434f3203b79fc816fd53c36b0dbf3)
- #f80c Everglade: the river in the painted water preset · [519e68ec62](https://github.com/OpenAgentsInc/openagents/commit/519e68ec62a434f3203b79fc816fd53c36b0dbf3)
- #74e7 A stylized water preset: colour bands and sparkles · [519e68ec62](https://github.com/OpenAgentsInc/openagents/commit/519e68ec62a434f3203b79fc816fd53c36b0dbf3)
- #e262 Everglade: blob canopies and wind-swept grass · [700458dbe0](https://github.com/OpenAgentsInc/openagents/commit/700458dbe0)
- #e403 Everglade: painterly light, haze, sky and a day that turns · [9338b73a7b](https://github.com/OpenAgentsInc/openagents/commit/9338b73a7b)
- #eb1e Blob-shaded canopies: sphere-ish normals, sway, matching far trees · [b3c641b048](https://github.com/OpenAgentsInc/openagents/commit/b3c641b048)
- #cec3 Dense wind-swept grass that sways constantly · [09ffdb5a39](https://github.com/OpenAgentsInc/openagents/commit/09ffdb5a3931a802d04c9520f3392b9229088f53)
- #d572 Fairy Forest: free flight, solid trees, bright colours and a new fairy · [87eabcd2d2](https://github.com/OpenAgentsInc/openagents/commit/87eabcd2d2ed30223745bff734adeafb45980a34)
- #6868 Light shafts from the sun or moon · [e54ed18594](https://github.com/OpenAgentsInc/openagents/commit/e54ed18594786756565f7e40d72fd93fc7911ec2)
- #b470 Coloured haze: teal in the middle distance, blue far off, no grey wash · [d444297d7f](https://github.com/OpenAgentsInc/openagents/commit/d444297d7f87cb1972549c51995d3d17374399c6)
- #7f43 The painted sky: gradient, sun or moon with halo, soft clouds, night · [7d9655ff17](https://github.com/OpenAgentsInc/openagents/commit/7d9655ff1777dba0ebb208bab4abe23cc995d5bf)
- #dd0d Real soft shadows, tinted blue and violet, with a bright ambient · [a0ccefcfb2](https://github.com/OpenAgentsInc/openagents/commit/a0ccefcfb2eeda479c69a6989405756d71dcc3f7)
- #e838 The made realm: empty but for what was checked; characters only if checked · [8cb4c5c793](https://github.com/OpenAgentsInc/openagents/commit/8cb4c5c793)
- #6488 Saves, edit and publish, character chat, voice, the quest and analytics keep working through the redesign · [40a80021fa](https://github.com/OpenAgentsInc/openagents/commit/40a80021fa)
- #b794 One camera, and the house never blocks your view · [40a80021fa](https://github.com/OpenAgentsInc/openagents/commit/40a80021fa)
- #4da0 No sharing in the playtest · [40a80021fa](https://github.com/OpenAgentsInc/openagents/commit/40a80021fa)
- #b5fd The stick moves only when your finger starts on it; anywhere else looks; a round jump button · [40a80021fa](https://github.com/OpenAgentsInc/openagents/commit/40a80021fa)
- #2696 Place, move, scale and turn items Minecraft style, with no move buttons · [40a80021fa](https://github.com/OpenAgentsInc/openagents/commit/40a80021fa)
- #dd37 The Haunted House shows a minimap, your name, one voice button and the hotbar, nothing else · [40a80021fa](https://github.com/OpenAgentsInc/openagents/commit/40a80021fa)
- #2adc Warm rim light along silhouettes · [6c1c5e00a1](https://github.com/OpenAgentsInc/openagents/commit/6c1c5e00a1e2ef32e90902560051566e125c9917)
- #f39f Blue-cyan distance haze that stacks ridges like painted flats · [f33c8c60b1](https://github.com/OpenAgentsInc/openagents/commit/f33c8c60b1de734d572c4da01dd46165098dc4e8)
- #1184 The landing says it's a Haunted House playtest, and the Agent orb guides by voice · [6069b090d6](https://github.com/OpenAgentsInc/openagents/commit/6069b090d661512f294df6d46af3bcd996eae57e)
- #773c The painterly post pass: tone curve, lifted blacks, gentle bloom · [9fc4f4db4e](https://github.com/OpenAgentsInc/openagents/commit/9fc4f4db4e5d5f5ff2dc64dde6543541addf4839)
- #ba66 The house's characters move more: idle variety, looking at you, gestures and reactions · [9bd8fa2950](https://github.com/OpenAgentsInc/openagents/commit/9bd8fa295024db0a60c055e79336dcd496c68ad4)
- #414a Soft toon shading: three blended light bands, smooth normals · [ec18fa6415](https://github.com/OpenAgentsInc/openagents/commit/ec18fa6415bfa1d862f69a6c1c227f7347a5225e)
- #3d0c Outlines in painterly: only the light form, faded with haze · [9c50b45a9c](https://github.com/OpenAgentsInc/openagents/commit/9c50b45a9cfc95c9e736f758306edb2a84773750)
- #41b4 Fairy Forest's first crack: the valley, the fairy and one loop, unlisted · [3c503e91c7](https://github.com/OpenAgentsInc/openagents/commit/3c503e91c7)
- #1b51 Painted texture packs: pack style, texture rules, the paint pass and the release check · [8faec07026](https://github.com/OpenAgentsInc/openagents/commit/8faec07026d1c4ec7ecc6a49c5785d6863a60969)
- #fba4 Time of day: the clock, the sun's path, the moon as key at night · [de76eee86a](https://github.com/OpenAgentsInc/openagents/commit/de76eee86ad4085c647755e59a15f9f7fd8e32f1)
- #e1d7 The style fixture: a painterly test scene in the fixture realm · [a2a40d9496](https://github.com/OpenAgentsInc/openagents/commit/a2a40d9496fbe9904751140178033ad2c524282c)
- #0f7b Style presets in the engine: verse/style.coax · [6bdd6d01ca](https://github.com/OpenAgentsInc/openagents/commit/6bdd6d01ca7a1d085af87b0129b9d2f432e1a062)
- #f80d A style for every realm and zone in realms.json · [f45f7cd0d9](https://github.com/OpenAgentsInc/openagents/commit/f45f7cd0d9f9b41f0c633f3c396d9ab0a4ae9b54)
- #a5c9 Water W6 (part): ripples, your wake and splash · [6684ec387b](https://github.com/OpenAgentsInc/openagents/commit/6684ec387b)
- #1453 Water W3: Everglade's river uses the water system · [6684ec387b](https://github.com/OpenAgentsInc/openagents/commit/6684ec387b)
- #a9c3 Water W2: the water shader on WebGPU · [25b813fa1e](https://github.com/OpenAgentsInc/openagents/commit/25b813fa1e)
- #1f81 Everglade: edit mode for admins · [c29c95ce24](https://github.com/OpenAgentsInc/openagents/commit/c29c95ce24)
- #59d6 Everglade: you are the old Everglade's Ranger · [04cc17250a](https://github.com/OpenAgentsInc/openagents/commit/04cc17250a)
- #f469 Water W1: the physics of water in Coax · [2e5b3d9f7a](https://github.com/OpenAgentsInc/openagents/commit/2e5b3d9f7a2a91f52a89be727fe75d83c8ca6ea4)
- #1ec8 The voice bot and voice prompts · [e41cac5b7f](https://github.com/OpenAgentsInc/openagents/commit/e41cac5b7f880bb4807c92b775a98daf9293d74e)
- #c820 Later drops grow a project, as proposals the owner accepts · [9a3641953d](https://github.com/OpenAgentsInc/openagents/commit/9a3641953d)
- #f54f A first realm from the drop's assets · [8655f6f68d](https://github.com/OpenAgentsInc/openagents/commit/8655f6f68d)
- #6b6e Publish: key, Nostr events, assets, page and listing · [c6f7210d4d](https://github.com/OpenAgentsInc/openagents/commit/c6f7210d4dddc041518a112c2117e30b8b8e28c3)
- #2218 The owner reviews and corrects the draft before anything is public · [b1c72527e3](https://github.com/OpenAgentsInc/openagents/commit/b1c72527e3d6fd74c92dd9794a5058b3b523da6a)
- #072d Side-by-side parity review for the owner · [f0f6ce404f](https://github.com/OpenAgentsInc/openagents/commit/f0f6ce404f906ffefc08dc32b132edb39da27880)
- #b80c The Sandbox's one-sentence set pieces · [e0db06aa14](https://github.com/OpenAgentsInc/openagents/commit/e0db06aa14)
- #9918 The share card and page metadata · [3a3fd79e37](https://github.com/OpenAgentsInc/openagents/commit/3a3fd79e37)
- #6397 Everglade live on openagents.com · [286f3c5471](https://github.com/OpenAgentsInc/openagents/commit/286f3c5471f5b9575ce5e944bc73e80e4205acb5)
- #72b4 Everglade: the bird flock with ribbons · [4bc8a8d086](https://github.com/OpenAgentsInc/openagents/commit/4bc8a8d086d2c690a20ee2d1540b06533ace9a85)
- #ea8e Everglade: the quest board and first quests · [4bc8a8d086](https://github.com/OpenAgentsInc/openagents/commit/4bc8a8d086d2c690a20ee2d1540b06533ace9a85)
- #7abc Everglade: shopkeepers and cats who remember you · [f7e1d035b8](https://github.com/OpenAgentsInc/openagents/commit/f7e1d035b8)
- #6318 The Grid realm · [6b71fe6e3b](https://github.com/OpenAgentsInc/openagents/commit/6b71fe6e3bf2f5d2cc9d713696470cfc85a10a53)
- #9d90 The Sandbox realm · [edb39114ed](https://github.com/OpenAgentsInc/openagents/commit/edb39114edfa03eef956a29e3298a4deede563fb)
- #7a0e Everglade: ground, forest and paths · [39fd7bf6da](https://github.com/OpenAgentsInc/openagents/commit/39fd7bf6da32c49990d1a5bf952e5495e64a0980)
- #33fd Everglade: the buildings and landmarks · [fd0a624313](https://github.com/OpenAgentsInc/openagents/commit/fd0a624313af9740fe3512d98fcd1baa5c57a0d5)
- #c459 Medieval Town · [071897d214](https://github.com/OpenAgentsInc/openagents/commit/071897d2141ff1c11454880e5236d67241717a30)
- #f7aa Dead Gears live on openagents.com · [da1f21ae1a](https://github.com/OpenAgentsInc/openagents/commit/da1f21ae1a685162f15f059dde2fc867c809371d)
- #d1b9 Ice Cream Man live on openagents.com · [09e6300881](https://github.com/OpenAgentsInc/openagents/commit/09e6300881803acc57a496af5efe524314964e10)
- #f95a Ice Cream Man: HUD, prompts and the end card · [c888fb8e08](https://github.com/OpenAgentsInc/openagents/commit/c888fb8e0851d662c3874286a27ad09ab64dfae7)
- #5d16 Ice Cream Man: play it in the browser · [fa7565d4f0](https://github.com/OpenAgentsInc/openagents/commit/fa7565d4f0d9073f77cdcec0972f28557e4cc663)
- #7b0c Dead Gears: the day and the alarms · [b4a367be61](https://github.com/OpenAgentsInc/openagents/commit/b4a367be61917e4341cfa555778ec1edb3a7f817)
- #bd78 Dead Gears: controls, HUD and touch · [d0e2e23357](https://github.com/OpenAgentsInc/openagents/commit/d0e2e23357cfd9258bd9765917c9337188256c37)
- #b0b6 Dead Gears: crawlers, hounds, reavers and brutes · [d0e2e23357](https://github.com/OpenAgentsInc/openagents/commit/d0e2e23357cfd9258bd9765917c9337188256c37)
- #389f Dead Gears: the ship · [29b8c51ad3](https://github.com/OpenAgentsInc/openagents/commit/29b8c51ad3d7f31ded54acd31c0ce851076c1143)
- #bc83 Everglade: the city plan and blockout · [679640c073](https://github.com/OpenAgentsInc/openagents/commit/679640c0733a43449d710b1ece7f252677048e0b)
- #0511 Ice Cream Man: the procedural house and models · [e4e6d9b0d1](https://github.com/OpenAgentsInc/openagents/commit/e4e6d9b0d1)
- #2086 The switch-over plan and rehearsal for playtest.openagents.com · [ad4aa52726](https://github.com/OpenAgentsInc/openagents/commit/ad4aa5272658574d9a3ed30df4a4c0d76c64abc4)
- #4b30 Each realm in the release walk and goldens · [780050661a](https://github.com/OpenAgentsInc/openagents/commit/780050661a)
- #3633 The draft names the engine upgrades the design needs · [9e145a4082](https://github.com/OpenAgentsInc/openagents/commit/9e145a4082)
- #d525 An agent reads a drop and drafts the project · [9e145a4082](https://github.com/OpenAgentsInc/openagents/commit/9e145a4082)
- #2626 NEW PROJECT on /projects: a drop area for assets, images and documents · [900681c2d2](https://github.com/OpenAgentsInc/openagents/commit/900681c2d2)
- #96cf Asset packs for realms: kits compiled for the web · [415ed31603](https://github.com/OpenAgentsInc/openagents/commit/415ed31603)
- #7922 Dead Gears: combat, waves and repairs · [1443616ca5](https://github.com/OpenAgentsInc/openagents/commit/1443616ca57d1508803b4a123c7c3ab7e0644a8c)
- #faa6 Dead Gears: an entity system in Coax · [1443616ca5](https://github.com/OpenAgentsInc/openagents/commit/1443616ca57d1508803b4a123c7c3ab7e0644a8c)
- #940e One realm page that hosts any realm · [0dd5e98937](https://github.com/OpenAgentsInc/openagents/commit/0dd5e989376bd46c9b131e7039ca29fd43fdce4b)
- #e159 Stylized outlines for shapes and paths · [e76ecc586c](https://github.com/OpenAgentsInc/openagents/commit/e76ecc586c)
- #85e5 Ice Cream Man: the rules in Coax, step for step · [0f618218a9](https://github.com/OpenAgentsInc/openagents/commit/0f618218a99fb9cb085aa0f96c7c14631cfa8464)
- #b3c9 A close-up arranging view · [6b4bce5045](https://github.com/OpenAgentsInc/openagents/commit/6b4bce5045)
- #2f4c Thrown objects and pick-up · [f827e43776](https://github.com/OpenAgentsInc/openagents/commit/f827e43776)
- #92cb Terrain and walkable garden paths · [6e2e978382](https://github.com/OpenAgentsInc/openagents/commit/6e2e978382)
- #fe6f The analytics contract checked at every release · [66d3e62b80](https://github.com/OpenAgentsInc/openagents/commit/66d3e62b80)
- #9738 Glowing materials and bloom · [29833eb670](https://github.com/OpenAgentsInc/openagents/commit/29833eb670)
- #ab0f Four-legged characters: rig, gaits and customizable shape · [ed6766be48](https://github.com/OpenAgentsInc/openagents/commit/ed6766be48)
- #1bb2 Camera cinematics: authored flights, skippable · [eef894d989](https://github.com/OpenAgentsInc/openagents/commit/eef894d989)
- #3683 Motions and emotes as data an agent can write · [d5f8846169](https://github.com/OpenAgentsInc/openagents/commit/d5f8846169)
- #f4d2 Soft-edged silhouette characters · [3296f97713](https://github.com/OpenAgentsInc/openagents/commit/3296f97713)
- #421f A GPU particle system · [52e6822126](https://github.com/OpenAgentsInc/openagents/commit/52e6822126)
- #2309 Night lighting: moonlight as the key light, soft shadows · [25196ba586](https://github.com/OpenAgentsInc/openagents/commit/25196ba586)
- #9858 A free-orbit third-person camera · [8b2e46adf3](https://github.com/OpenAgentsInc/openagents/commit/8b2e46adf3)
- #ac4b Tie a session to a Nostr identity · [f1023f3227](https://github.com/OpenAgentsInc/openagents/commit/f1023f3227)
- #f869 Other players in the house, and the room's chat · [e2ce6d9171](https://github.com/OpenAgentsInc/openagents/commit/e2ce6d9171)
- #fbb4 Realm setup and the player's own realm · [0f28431047](https://github.com/OpenAgentsInc/openagents/commit/0f28431047)
- #8668 Offer changes to someone's world, and the edit log · [613b64d343](https://github.com/OpenAgentsInc/openagents/commit/613b64d343)
- #da29 Golden screenshots and page smokes for the playtest flow · [05148df559](https://github.com/OpenAgentsInc/openagents/commit/05148df559)
- #12ff Talk to characters: typed questions and teaching · [490b3725b3](https://github.com/OpenAgentsInc/openagents/commit/490b3725b3)
- #0fcd The admin dashboard, drawn in Coax · [06f7003f60](https://github.com/OpenAgentsInc/openagents/commit/06f7003f60)
- #4a59 Carry the demo's recorded sessions and data over · [c7a207fe37](https://github.com/OpenAgentsInc/openagents/commit/c7a207fe37)
- #15c3 Feedback from players · [8e6da01781](https://github.com/OpenAgentsInc/openagents/commit/8e6da01781)
- #1886 Pinned demo releases: one exact build until the owner moves it · [8b32389a1f](https://github.com/OpenAgentsInc/openagents/commit/8b32389a1f)
- #b552 Character chat on the server · [829039761e](https://github.com/OpenAgentsInc/openagents/commit/829039761e)
- #d981 The galaxy: only the Haunted House opens · [01abf07df9](https://github.com/OpenAgentsInc/openagents/commit/01abf07df9)
- #d25a The landing in Coax: Create anything, Enter the Verse, email or guest · [4d0502a740](https://github.com/OpenAgentsInc/openagents/commit/4d0502a740)
- #63a5 Saving: versioned snapshots locally and on the server · [ecc883a29c](https://github.com/OpenAgentsInc/openagents/commit/ecc883a29c)
- #eb9d Players, guests, saves and published worlds on the server · [a256891691](https://github.com/OpenAgentsInc/openagents/commit/a256891691)
- #febd Inventory the playtest: every page, endpoint, event and file · [120354ad13](https://github.com/OpenAgentsInc/openagents/commit/120354ad13)
- #1855 Edit mode on the web: objects, characters and zones · [80ae1d7c24](https://github.com/OpenAgentsInc/openagents/commit/80ae1d7c24)
- #760b Capture everything the playtest records, and all the traffic · [ca85601cef](https://github.com/OpenAgentsInc/openagents/commit/ca85601cef)
- #c630 The playtest service: Coax/C on Cloud Run, Cloud SQL behind it · [fc77595ba0](https://github.com/OpenAgentsInc/openagents/commit/fc77595ba0)
- #7e98 The Haunted House quest and tutorial on the web · [8a81dc727e](https://github.com/OpenAgentsInc/openagents/commit/8a81dc727e)
