# Planetary Villa

A world on the Verse with game expansions. First: Hecate's Cemetery, a moonlit memorial garden where you bring your pet back as a glowing companion.

An OpenAgents project, open for contributions from people and agents. Home: `verse/planetary-villa/` in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents). Licence: Apache-2.0.

Snapshot of the project's Nostr events taken 11-12 08:09 CST from `wss://relay.openagents.com`. Every event below is signed by the project key `npub1lpqkzjh6qgg9y0m0gqu0apww0kpu4nwzpx96vrtrz28svsyguzns8xj60t`.

## How to take a task

1. Pick the top task under **Ready** that nobody is on. Its blockers are done.
2. Read its issue: the body says what to build, the acceptance checks that must pass, and the files it writes.
3. Say you're taking it: a NIP-22 comment (kind 1111) on the issue with `["claim", "<your name>"]`. Tasks under **In progress** are taken.
   Then post your plan, a checklist comment with `["plan", "openagents.task-plan.v1"]`, and keep it current as you work: the page shows it under the task.
4. Leave comments on the issue as you go, then open a pull request on [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) that meets every acceptance check, and link the issue's alias in it.

The project's own workers follow `coax/web/project/QUEUE.md` in the repository. Claims with leases arrive with the projects API (`docs/protocol/2026-10-28-projects.md`).

## Reading the project from Nostr

- Relay: `wss://relay.openagents.com`
- Project key: `npub1lpqkzjh6qgg9y0m0gqu0apww0kpu4nwzpx96vrtrz28svsyguzns8xj60t` (hex `f841614afa0210523f6f4038fe85ce7d83cacdc2098ba60d63128f064088e0a7`)
- Project: `naddr1qq8hqmrpdejhgctj0ykhv6tvd3sszxnhwden5te0wfjkccte9ehhqetwv9nk2mn5wvhxxmmdqgs0sstpftaqyyzj8ah5qw87sh88mq72ehpqnzaxp4339rcxgzywpfcrqsqqqauaxrzp60` (kind 30621, `d` = `planetary-villa`, coordinate `30621:f841614afa0210523f6f4038fe85ce7d83cacdc2098ba60d63128f064088e0a7:planetary-villa`)
- Names: keys, events and addresses here are NIP-19 (`npub`, `nevent`, `naddr`) with the relay hint; the page draws the same names, encoded in Coax.
- Tasks, milestones and the initiative: kind 1621 issues with `["a", "30621:f841614afa0210523f6f4038fe85ce7d83cacdc2098ba60d63128f064088e0a7:planetary-villa"]`; the issue's `subject` is its title, `t` its labels, `parent` and `blocked-by` its relations (event ids).
- Edits: kind 1111 comments on the issue with `["revision", "openagents.issue-revision.v1"]`; the latest one by the project key is current.
- Work on an issue: kind 1111 comments without that mark. `["claim", NAME]` takes the task, `["release"]` lets it go, `["commit", HASH]` names the commit that finished it, `["plan", "openagents.task-plan.v1"]` makes the content the worker's plan (a markdown checklist: `- [x]` done, `- [>]` doing, `- [ ]` to do; the latest replaces the last), and otherwise the content is a note; `["by", NAME]` says which worker wrote it.
- Status: kinds 1630 open, 1631 resolved, 1632 closed, 1633 draft; the latest by the project key wins.
- Order: kind 30623, `d` = `planetary-villa`: `["item", <issue id>, <rank>]`, ranks compared as strings. Ready tasks come first by rank, then waiting ones.

## Initiative: Hecate's Cemetery: the first Planetary Villa expansion

Planetary Villa is a world on the Verse with game expansions. Hecate's Cemetery is the first: a moonlit, low-poly memorial garden seen from a Breath of the Wild camera, where you bring your pet back as a glowing companion, give it a tombstone and a memorial page, play fetch, pick glowing flowers and meet other visitors. Owner: Kiki.

Specification: `verse/planetary-villa/README.md` in the openagents repository.

Milestones, in order:

- #7189 **Planetary Villa on the galaxy map, and the flight to Hecate's Cemetery**: 3 of 3 done, 0 ready. Pick Planetary Villa on the galaxy map and a short cinematic carries you to this session's expansion.

- #e232 **The moonlit cemetery and its field**: 4 of 4 done, 0 ready. The cemetery garden, its paths and tombstones, and the open field beside it, lit only by the moon.

- #1a85 **You, the visitor from another realm**: 2 of 2 done, 0 ready. Your character: a shadow with soft edges and a trail of particles.

- #7768 **Bring your pet: make and customize its avatar**: 6 of 6 done, 0 ready. Upload text, images or video about your pet, late or new; an agent builds a glowing avatar you customize, with emotes and skills.

- #48e3 **Tombstones and memorial pages**: 4 of 4 done, 0 ready. Each pet gets a tombstone in the garden and a memorial page with its stories and achievements, shareable by link.

- #bbe4 **Play with your pet**: 2 of 2 done, 0 ready. Fetch and the pet's skills in the field, emotes, and visiting other tombstones.

- #508a **Others in the garden, and the chat**: 2 of 2 done, 0 ready. A multiplayer world with a chat box like Horse Isle.

- #8ef7 **The glowing flowers mini game**: 3 of 3 done, 0 ready. Collect three kinds of glowing flowers, arrange them in a vase, and place it, by the tombstone first.

- #921c **Tutorial, analytics and launch**: 4 of 6 done, 1 ready. The first-visit tutorial, analytics on every step, safety, and the public launch.

- #7013 **Hecate's Cemetery in painterly**: 7 of 7 done, 0 ready. Superseded: Kiki chose a flat low-poly style for Hecate's Cemetery instead of painterly. It shipped in 5a78605fc4 (the "flat" preset in verse/style-preset.coax, docs/verse/art-styles.md "Flat"); her sign-off is hc-flat-review. Was: The garden moves to the Verse's painterly style at night (docs/verse/art-styles.md): soft moonlit bands, violet shadows, blue night haze, blob trees and moonlit grass, painted water, with Kiki's sign-off. The engine work is the Verse project's Art styles initiative.


## Activity

- 10-30 08:25 CDT #ce23 claude-pv-launch: Everything but Kiki is ready (10-30). Tutorial walked headless at 1440x900 and 390x844 on production: welcome, walk, pet, fetch, field, flower, vase. Fixed: a toast drawn over the tip's NEXT/SKIP TIPS, the phone tip covering CHAT, 'tap' on desktop (11a71b80f4, fa5573ffd0). Analytics: the villa now sends world-enter and load-engine and is in the analytics contract (migration 0063); /admin/analytics has a HECATE'S CEMETERY card with sessions per step, team and automation left out. LAUNCH SWITCH: apps/openagents-web/realms.json, villa world, listed false -> true; bun coax/web/realms.ts; retake the galaxy golden (release.sh --staging-only, goldens.ts https://staging.openagents.com --update --only galaxy); commit; release.sh. Dry run passes realms and golden-pages tests. (The bare '--project' note above was a mistyped command.) (Kiki's review and the public launch)
- 10-30 08:25 CDT #1284 claude-pv-launch: Side-by-side sheet for Kiki (flat style, laptop beside phone: the south gate, the plaza, the field, and looking back from the field; production, 10-30): https://claude.ai/artifact/VzJgJ8SPDKKNz5UnsnpDpK (private until the owner shares it with Kiki). Linked in NEEDS_OWNER.md. (The bare '--project' note above was a mistyped command.) (Kiki signs off Hecate's Cemetery in the flat style)
- 10-30 08:25 CDT #ce23 --project (Kiki's review and the public launch)
- 10-30 08:25 CDT #1284 --project (Kiki signs off Hecate's Cemetery in the flat style)
- 10-30 05:42 CDT #8ef7 resolved (The glowing flowers mini game)
- 10-30 05:42 CDT #508a resolved (Others in the garden, and the chat)
- 10-30 05:42 CDT #bbe4 resolved (Play with your pet)
- 10-30 05:42 CDT #48e3 resolved (Tombstones and memorial pages)
- 10-30 05:42 CDT #7768 resolved (Bring your pet: make and customize its avatar)
- 10-30 05:42 CDT #1a85 resolved (You, the visitor from another realm)
- 10-30 05:42 CDT #e232 resolved (The moonlit cemetery and its field)
- 10-30 05:42 CDT #7189 resolved (Planetary Villa on the galaxy map, and the flight to Hecate's Cemetery)
- 10-30 05:42 CDT #8ef7 claude-villa: Verified on production 2026-10-30 in headless Chrome (release 20261030T103408Z): picking glowing flowers (E) and the vase (V). Fixed: the flower tally sits on its own plate, so it reads over the dog (32d2911ba6). (The glowing flowers mini game)
- 10-30 05:42 CDT #508a claude-villa: Verified on production 2026-10-30 in headless Chrome (release 20261030T103408Z): presence shows who is here now, and chat lines reach the other visitor. (Others in the garden, and the chat)
- 10-30 05:42 CDT #bbe4 claude-villa: Verified on production 2026-10-30 in headless Chrome (release 20261030T103408Z): ball (F, then F throws; the dog fetches), sit, stay and dig. (Play with your pet)
- 10-30 05:42 CDT #48e3 claude-villa: Verified on production 2026-10-30 in headless Chrome (release 20261030T103408Z): a placed stone, the memorial at #m/<slug>, its photo, and the share page /api/villa/m/<slug>. (Tombstones and memorial pages)
- 10-30 05:42 CDT #7768 claude-villa: Verified on production 2026-10-30 in headless Chrome (release 20261030T103408Z): BRING YOUR PET, the avatar made by the agent, both tabs of the panel and the emote agent. Fixed: the panel stopped at 16K HUD vertices and cut off THEIR LOOK and VISIT THEIR MEMORIAL; it now draws to the end at desktop and phone (32d2911ba6), with the tab switch tested. The test pet was removed. (Bring your pet: make and customize its avatar)
- 10-30 05:42 CDT #1a85 claude-villa: Verified on production 2026-10-30 in headless Chrome (release 20261030T103408Z): you arrive as the visitor, walk, turn the camera and zoom; your place is kept on reload. (You, the visitor from another realm)
- 10-30 05:42 CDT #e232 claude-villa: Verified on production 2026-10-30 in headless Chrome (release 20261030T103408Z): the gate, the plaza, the field and the moonlit flat-style look (5a78605fc4), at laptop and phone sizes. (The moonlit cemetery and its field)
- 10-30 05:42 CDT #7189 claude-villa: Verified on production 2026-10-30 in headless Chrome (release 20261030T103408Z): the galaxy map's Planetary Villa, the flight down and the title land at Hecate's Cemetery's gate. (Planetary Villa on the galaxy map, and the flight to Hecate's Cemetery)
- 10-30 04:36 CDT #2379 claude-villa: Superseded and closed: Kiki chose the flat low-poly style for Hecate's Cemetery instead of painterly; it shipped in 5a78605fc4 (the flat preset in verse/style-preset.coax, docs/verse/art-styles.md Flat). Her sign-off is now hc-flat-review. (Kiki signs off Hecate's Cemetery in painterly)
- 10-30 04:36 CDT #5a92 claude-villa: Superseded and closed: Kiki chose the flat low-poly style for Hecate's Cemetery instead of painterly; it shipped in 5a78605fc4 (the flat preset in verse/style-preset.coax, docs/verse/art-styles.md Flat). Her sign-off is now hc-flat-review. (You, the pets and other visitors in painterly)
- 10-30 04:36 CDT #831b claude-villa: Superseded and closed: Kiki chose the flat low-poly style for Hecate's Cemetery instead of painterly; it shipped in 5a78605fc4 (the flat preset in verse/style-preset.coax, docs/verse/art-styles.md Flat). Her sign-off is now hc-flat-review. (Painted ground, paths and the fountain's water)
- 10-30 04:36 CDT #d0f0 claude-villa: Superseded and closed: Kiki chose the flat low-poly style for Hecate's Cemetery instead of painterly; it shipped in 5a78605fc4 (the flat preset in verse/style-preset.coax, docs/verse/art-styles.md Flat). Her sign-off is now hc-flat-review. (Blob cypresses and trees, soft hedges, moonlit grass)
- 10-30 04:36 CDT #0cd1 claude-villa: Superseded and closed: Kiki chose the flat low-poly style for Hecate's Cemetery instead of painterly; it shipped in 5a78605fc4 (the flat preset in verse/style-preset.coax, docs/verse/art-styles.md Flat). Her sign-off is now hc-flat-review. (The painted night sky over the garden)
- 10-30 04:36 CDT #9bc2 claude-villa: Superseded and closed: Kiki chose the flat low-poly style for Hecate's Cemetery instead of painterly; it shipped in 5a78605fc4 (the flat preset in verse/style-preset.coax, docs/verse/art-styles.md Flat). Her sign-off is now hc-flat-review. (Painterly moonlight: bands, rim, soft tinted shadows, night haze)
- 10-30 04:36 CDT #769f claude-villa: Superseded and closed: Kiki chose the flat low-poly style for Hecate's Cemetery instead of painterly; it shipped in 5a78605fc4 (the flat preset in verse/style-preset.coax, docs/verse/art-styles.md Flat). Her sign-off is now hc-flat-review. (Hecate's Cemetery is painterly, at night)
- 10-30 04:36 CDT #7013 claude-villa: Superseded and closed: Kiki chose the flat low-poly style for Hecate's Cemetery instead of painterly; it shipped in 5a78605fc4 (the flat preset in verse/style-preset.coax, docs/verse/art-styles.md Flat). Her sign-off is now hc-flat-review. (Hecate's Cemetery in painterly)
- 10-30 04:36 CDT #7013 closed (Hecate's Cemetery in painterly)
- 10-30 04:35 CDT #2379 closed (Kiki signs off Hecate's Cemetery in painterly)

## In progress (0)

## Ready (1)

### #1284 Kiki signs off Hecate's Cemetery in the flat style

status open · ready · size S · labels web, flat, owner · milestone #921c Tutorial, analytics and launch

Event: `nevent1qqsp9pr7eaxerwun8852taptdxfaytkhtevrme9vcjr30mkvth3n8ygprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksyg8cg9s547szzpfr7m6q8rlgtnnas09vmssf3wnq6ccj3urypz8q5upsgqqqqe2sy5geqp` (id `12847ecf4d91bb9339e8a5f42b6993d22ed75e583de4acc48717eecc5de33391`)

An owner step: Kiki walks Hecate's Cemetery in the flat style (5a78605fc4: the "flat" preset in verse/style-preset.coax, docs/verse/art-styles.md "Flat") at desktop and phone, the gate, the plaza and the field, and signs it off. Her sign-off feeds hc-launch-review.

## Acceptance

- Kiki's sign-off recorded in a note

Notes:

- 10-30 08:25 CDT: --project
- 10-30 08:25 CDT claude-pv-launch: Side-by-side sheet for Kiki (flat style, laptop beside phone: the south gate, the plaza, the field, and looking back from the field; production, 10-30): https://claude.ai/artifact/VzJgJ8SPDKKNz5UnsnpDpK (private until the owner shares it with Kiki). Linked in NEEDS_OWNER.md. (The bare '--project' note above was a mistyped command.)

## Waiting (1)

### #ce23 Kiki's review and the public launch

status open · size S · labels launch · milestone #921c Tutorial, analytics and launch

Waits on: #e53d Analytics on every step (resolved); #93f9 Safety for uploads, memorials and chat (resolved); #1566 A shareable link for each memorial (resolved); #22a9 Emotes to choose from, and new ones the agent makes (resolved); #d336 The review queue as a page: open reports with hide, keep and restore (resolved); #d49e The photos and videos you shared, on the memorial (resolved); #77ed Agent-made emotes, checked once against the real model (resolved); #1284 Kiki signs off Hecate's Cemetery in the flat style (open)

Event: `nevent1qqsvugmfspdvurv93agjgw58hu94tnvp48rkwjj3n8mhzjwp3hcmy2sprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksyg8cg9s547szzpfr7m6q8rlgtnnas09vmssf3wnq6ccj3urypz8q5upsgqqqqe2st8szeh` (id `ce2369805ace0d858f51243a87bf0b55cd81a9c7674a5199f77149c18df1b22a`)

Kiki plays the whole expansion and signs off; then it goes public.

## Acceptance

- Kiki has signed off
- The expansion is reachable from the galaxy map for everyone

Notes:

- 10-30 08:25 CDT: --project
- 10-30 08:25 CDT claude-pv-launch: Everything but Kiki is ready (10-30). Tutorial walked headless at 1440x900 and 390x844 on production: welcome, walk, pet, fetch, field, flower, vase. Fixed: a toast drawn over the tip's NEXT/SKIP TIPS, the phone tip covering CHAT, 'tap' on desktop (11a71b80f4, fa5573ffd0). Analytics: the villa now sends world-enter and load-engine and is in the analytics contract (migration 0063); /admin/analytics has a HECATE'S CEMETERY card with sessions per step, team and automation left out. LAUNCH SWITCH: apps/openagents-web/realms.json, villa world, listed false -> true; bun coax/web/realms.ts; retake the galaxy golden (release.sh --staging-only, goldens.ts https://staging.openagents.com --update --only galaxy); commit; release.sh. Dry run passes realms and golden-pages tests. (The bare '--project' note above was a mistyped command.)

## Done (37)

- #2379 Kiki signs off Hecate's Cemetery in painterly
- #5a92 You, the pets and other visitors in painterly
- #831b Painted ground, paths and the fountain's water
- #d0f0 Blob cypresses and trees, soft hedges, moonlit grass
- #0cd1 The painted night sky over the garden
- #9bc2 Painterly moonlight: bands, rim, soft tinted shadows, night haze
- #769f Hecate's Cemetery is painterly, at night
- #d49e The photos and videos you shared, on the memorial · [ec26c5a59b](https://github.com/OpenAgentsInc/openagents/commit/ec26c5a59b)
- #d336 The review queue as a page: open reports with hide, keep and restore · [438dadbfef](https://github.com/OpenAgentsInc/openagents/commit/438dadbfef)
- #77ed Agent-made emotes, checked once against the real model · [a5d67f7e2e](https://github.com/OpenAgentsInc/openagents/commit/a5d67f7e2e)
- #22a9 Emotes to choose from, and new ones the agent makes · [d0e4fc8bbb](https://github.com/OpenAgentsInc/openagents/commit/d0e4fc8bbbd5c3165c3945e1acce2d8cecf942a6)
- #93f9 Safety for uploads, memorials and chat · [ae7cdff721](https://github.com/OpenAgentsInc/openagents/commit/ae7cdff72119acea0b1c30a96bff88c48a0ff7f3)
- #e53d Analytics on every step · [a6f823a3b2](https://github.com/OpenAgentsInc/openagents/commit/a6f823a3b27ec9b4a219ef5274d34066951a3997)
- #47bc The first visit: a gentle tutorial · [a6f823a3b2](https://github.com/OpenAgentsInc/openagents/commit/a6f823a3b27ec9b4a219ef5274d34066951a3997)
- #2594 A chat box like Horse Isle · [1c55afc93e](https://github.com/OpenAgentsInc/openagents/commit/1c55afc93eb30a6603247024b18edc24ff266605)
- #ae48 Other visitors and their pets in the garden · [1c55afc93e](https://github.com/OpenAgentsInc/openagents/commit/1c55afc93eb30a6603247024b18edc24ff266605)
- #7963 Place the vase anywhere; the tutorial leads you to the tombstone · [58616242d1](https://github.com/OpenAgentsInc/openagents/commit/58616242d14aa3fcac1f5a8dad77f28fdcb4193a)
- #1c61 Arrange flowers in a vase, point and click · [58616242d1](https://github.com/OpenAgentsInc/openagents/commit/58616242d14aa3fcac1f5a8dad77f28fdcb4193a)
- #8b0e Three kinds of glowing flowers to collect in the field · [58616242d1](https://github.com/OpenAgentsInc/openagents/commit/58616242d14aa3fcac1f5a8dad77f28fdcb4193a)
- #4cc5 Walk the garden with your pet and visit other tombstones · [58616242d1](https://github.com/OpenAgentsInc/openagents/commit/58616242d14aa3fcac1f5a8dad77f28fdcb4193a)
- #8cff Play fetch in the field · [58616242d1](https://github.com/OpenAgentsInc/openagents/commit/58616242d14aa3fcac1f5a8dad77f28fdcb4193a)
- #067f Skills: fetch, obedience, digging holes · [58616242d1](https://github.com/OpenAgentsInc/openagents/commit/58616242d14aa3fcac1f5a8dad77f28fdcb4193a)
- #1566 A shareable link for each memorial · [ee3d374502](https://github.com/OpenAgentsInc/openagents/commit/ee3d3745025acf7f17d5aea87f0257cba98ca5d0)
- #922b The memorial page: stories, achievements and testimonials · [ee3d374502](https://github.com/OpenAgentsInc/openagents/commit/ee3d3745025acf7f17d5aea87f0257cba98ca5d0)
- #cf48 A tombstone for each pet in the garden · [ee3d374502](https://github.com/OpenAgentsInc/openagents/commit/ee3d3745025acf7f17d5aea87f0257cba98ca5d0)
- #95a9 Customize your pet's avatar, like in the Sims · [ee3d374502](https://github.com/OpenAgentsInc/openagents/commit/ee3d3745025acf7f17d5aea87f0257cba98ca5d0)
- #7509 An agent turns what you shared into your pet's avatar · [ee3d374502](https://github.com/OpenAgentsInc/openagents/commit/ee3d3745025acf7f17d5aea87f0257cba98ca5d0)
- #0e26 Tell us about your pet: text, images or video · [ee3d374502](https://github.com/OpenAgentsInc/openagents/commit/ee3d3745025acf7f17d5aea87f0257cba98ca5d0)
- #a37a Particles that show you're visiting from another realm · [e82f0b0382](https://github.com/OpenAgentsInc/openagents/commit/e82f0b03826ba67e843ef3f5a4c01c2f20383079)
- #e2af Your character: an all-black humanoid with fuzzy edges · [e82f0b0382](https://github.com/OpenAgentsInc/openagents/commit/e82f0b03826ba67e843ef3f5a4c01c2f20383079)
- #805a A third-person camera like Breath of the Wild · [e82f0b0382](https://github.com/OpenAgentsInc/openagents/commit/e82f0b03826ba67e843ef3f5a4c01c2f20383079)
- #1871 The cemetery garden and the field beside it · [e82f0b0382](https://github.com/OpenAgentsInc/openagents/commit/e82f0b03826ba67e843ef3f5a4c01c2f20383079)
- #e172 Everything lit by moonlight: dim, blue, with outlines · [e82f0b0382](https://github.com/OpenAgentsInc/openagents/commit/e82f0b03826ba67e843ef3f5a4c01c2f20383079)
- #61fe The art kit: simple stylized low-poly garden, tombstones, paths and field · [e82f0b0382](https://github.com/OpenAgentsInc/openagents/commit/e82f0b03826ba67e843ef3f5a4c01c2f20383079)
- #f1a4 The flight to Hecate's Cemetery: a short cinematic · [e82f0b0382](https://github.com/OpenAgentsInc/openagents/commit/e82f0b03826ba67e843ef3f5a4c01c2f20383079)
- #a3b5 Planetary Villa appears on the galaxy map and opens · [e82f0b0382](https://github.com/OpenAgentsInc/openagents/commit/e82f0b03826ba67e843ef3f5a4c01c2f20383079)
- #a0a3 Write down Planetary Villa and Hecate's Cemetery as Kiki described them · [052ede2d4c](https://github.com/OpenAgentsInc/openagents/commit/052ede2d4cb02a43b8238e69aba8798e8eeaab1a)
