# Planetary Villa

A world on the Verse with game expansions. First: Hecate's Cemetery, a moonlit memorial garden where you bring your pet back as a glowing companion.

An OpenAgents project, open for contributions from people and agents. Home: `planetary-villa/` in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents). Licence: Apache-2.0.

Snapshot of the project's Nostr events taken 10-28 16:59 CDT from `wss://relay.openagents.com`. Every event below is signed by the project key `npub1lpqkzjh6qgg9y0m0gqu0apww0kpu4nwzpx96vrtrz28svsyguzns8xj60t`.

## How to take a task

1. Pick the top task under **Ready** that nobody is on. Its blockers are done.
2. Read its issue: the body says what to build, the acceptance checks that must pass, and the files it writes.
3. Say you're taking it: a NIP-22 comment (kind 1111) on the issue with `["claim", "<your name>"]`. Tasks under **In progress** are taken.
   Then post your plan, a checklist comment with `["plan", "openagents.task-plan.v1"]`, and keep it current as you work: the page shows it under the task.
4. Leave comments on the issue as you go, then open a pull request on [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) that meets every acceptance check, and link the issue's alias in it.

The project's own workers follow `coax/web/project/QUEUE.md` in the repository. Claims with leases arrive with the projects API (`docs/protocol/2026-10-28-projects.md`).

## Reading the project from Nostr

- Relay: `wss://relay.openagents.com`
- Project: `naddr1qq8hqmrpdejhgctj0ykhv6tvd3sszxnhwden5te0wfjkccte9ehhqetwv9nk2mn5wvhxxmmdqgs0sstpftaqyyzj8ah5qw87sh88mq72ehpqnzaxp4339rcxgzywpfcrqsqqqauaxrzp60` (kind 30621, `d` = `planetary-villa`)
- Tasks, milestones and the initiative: kind 1621 issues with `["a", "30621:f841614afa0210523f6f4038fe85ce7d83cacdc2098ba60d63128f064088e0a7:planetary-villa"]`; the issue's `subject` is its title, `t` its labels, `parent` and `blocked-by` its relations (event ids).
- Edits: kind 1111 comments on the issue with `["revision", "openagents.issue-revision.v1"]`; the latest one by the project key is current.
- Work on an issue: kind 1111 comments without that mark. `["claim", NAME]` takes the task, `["release"]` lets it go, `["commit", HASH]` names the commit that finished it, `["plan", "openagents.task-plan.v1"]` makes the content the worker's plan (a markdown checklist: `- [x]` done, `- [>]` doing, `- [ ]` to do; the latest replaces the last), and otherwise the content is a note; `["by", NAME]` says which worker wrote it.
- Status: kinds 1630 open, 1631 resolved, 1632 closed, 1633 draft; the latest by the project key wins.
- Order: kind 30623, `d` = `planetary-villa`: `["item", <issue id>, <rank>]`, ranks compared as strings. Ready tasks come first by rank, then waiting ones.

## Initiative: Hecate's Cemetery: the first Planetary Villa expansion

Planetary Villa is a world on the Verse with game expansions. Hecate's Cemetery is the first: a moonlit, low-poly memorial garden seen from a Breath of the Wild camera, where you bring your pet back as a glowing companion, give it a tombstone and a memorial page, play fetch, pick glowing flowers and meet other visitors. Owner: Kiki.

Specification: `planetary-villa/README.md` in the openagents repository.

Milestones, in order:

- #7189 **Planetary Villa on the galaxy map, and the flight to Hecate's Cemetery**: 0 of 3 done, 1 ready. Pick Planetary Villa on the galaxy map and a short cinematic carries you to this session's expansion.

- #e232 **The moonlit cemetery and its field**: 0 of 4 done, 0 ready. The cemetery garden, its paths and tombstones, and the open field beside it, lit only by the moon.

- #1a85 **You, the visitor from another realm**: 0 of 2 done, 0 ready. Your character: a shadow with soft edges and a trail of particles.

- #7768 **Bring your pet: make and customize its avatar**: 0 of 5 done, 0 ready. Upload text, images or video about your pet, late or new; an agent builds a glowing avatar you customize, with emotes and skills.

- #48e3 **Tombstones and memorial pages**: 0 of 3 done, 0 ready. Each pet gets a tombstone in the garden and a memorial page with its stories and achievements, shareable by link.

- #bbe4 **Play with your pet**: 0 of 2 done, 0 ready. Fetch and the pet's skills in the field, emotes, and visiting other tombstones.

- #508a **Others in the garden, and the chat**: 0 of 2 done, 0 ready. A multiplayer world with a chat box like Horse Isle.

- #8ef7 **The glowing flowers mini game**: 0 of 3 done, 0 ready. Collect three kinds of glowing flowers, arrange them in a vase, and place it, by the tombstone first.

- #921c **Tutorial, analytics and launch**: 0 of 4 done, 0 ready. The first-visit tutorial, analytics on every step, safety, and the public launch.


## In progress (0)

## Ready (1)

### #a0a3 Write down Planetary Villa and Hecate's Cemetery as Kiki described them

status open · ready · size S · labels docs · milestone #7189 Planetary Villa on the galaxy map, and the flight to Hecate's Cemetery

Event: `a0a3fdb0016c8e6b6f3a33a42de9231b65f0a5ecf47704f39e9f4ef686bbf3f7`

Turn Kiki's description into the design document `planetary-villa/README.md`: the world, the expansion, the art direction, every mechanic and the flower mini game, with open questions for Kiki. Kiki owns the design; the document is hers to correct.

## Acceptance

- planetary-villa/README.md covers every mechanic in the project's tasks
- Kiki has read it and her corrections are in

## Files

- `planetary-villa/README.md`

## Waiting (27)

### #a3b5 Planetary Villa appears on the galaxy map and opens

status open · size M · labels web, needs-verse · milestone #7189 Planetary Villa on the galaxy map, and the flight to Hecate's Cemetery

Waits on: #a0a3 Write down Planetary Villa and Hecate's Cemetery as Kiki described them (open)

Event: `a3b5a91b90b9f2a13b2198e07399e05df8e056d1ef48147f9937ce1d066e723e`

Planetary Villa is a planet you can choose on the galaxy map; choosing it starts the flight to the current expansion.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): pt-galaxy, pt-planets. This task can't land until those are done.

## Acceptance

- Choosing Planetary Villa on the galaxy map at openagents.com opens the flight
- Analytics records the choice

### #f1a4 The flight to Hecate's Cemetery: a short cinematic

status open · size M · labels web, cinematic, needs-verse · milestone #7189 Planetary Villa on the galaxy map, and the flight to Hecate's Cemetery

Waits on: #a3b5 Planetary Villa appears on the galaxy map and opens (open)

Event: `f1a484f824544cfac5a9a6b8922b989d42152305c1c1f54459e8bdef2dc403b7`

A skippable cinematic of traveling through space to the expansion this session focuses on (Hecate's Cemetery), ending where you land in the garden.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-cinematics. This task can't land until those are done.

## Acceptance

- The cinematic plays on desktop and phone and can be skipped
- It ends at the garden's entrance with control handed to you

### #61fe The art kit: simple stylized low-poly garden, tombstones, paths and field

status open · size L · labels art · milestone #e232 The moonlit cemetery and its field

Waits on: #a0a3 Write down Planetary Villa and Hecate's Cemetery as Kiki described them (open)

Event: `61fe3c5c2b06b867c441c1c3910ed970b6ceb04637a5288d13ade3b91b828921`

A small set of low-poly models in one stylized look: tombstones (several shapes with room for a name and epitaph), garden beds, hedges, trees, gates, benches, a fountain, path pieces, the open field's edge. Sources must be licensed for our use; list each source.

## Acceptance

- Every model loads in the Verse and fits one style
- Licences recorded next to the assets

### #e172 Everything lit by moonlight: dim, blue, with outlines

status open · size M · labels web, lighting, needs-verse · milestone #e232 The moonlit cemetery and its field

Waits on: #61fe The art kit: simple stylized low-poly garden, tombstones, paths and field (open)

Event: `e172650e482d4cafcc69a1a89078f687209dedacb941a71288da881df5e26b7a`

The garden at night: moonlight as the only key light, not very bright, soft shadows, a light outline on shapes and paths so they read in the dark.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-moonlight, verse-outlines. This task can't land until those are done.

## Acceptance

- Side-by-side screenshots at desktop and phone show the moonlit look Kiki approves
- Frame time stays inside the realm's budget on a phone

### #1871 The cemetery garden and the field beside it

status open · size M · labels web, needs-verse · milestone #e232 The moonlit cemetery and its field

Waits on: #e172 Everything lit by moonlight: dim, blue, with outlines (open)

Event: `18719caa8d8c34a19cf37262461a98c76be0c274f580b39361b8c1f2eb691906`

Lay out the garden: paths between tombstone plots with room to grow, a gate into the open field, and spots for the vase and the fountain. Paths are walkable and readable from the third-person view.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-terrain-paths. This task can't land until those are done.

## Acceptance

- You can walk every path and into the field
- Plots for at least 64 tombstones, added as pets arrive

### #805a A third-person camera like Breath of the Wild

status open · size M · labels web, controls, needs-verse · milestone #e232 The moonlit cemetery and its field

Waits on: #1871 The cemetery garden and the field beside it (open)

Event: `805aa6f93f88e9883e50b572d66bbc9cf7377fa3c20a4870713f043c7603cb24`

The Breath of the Wild perspective: an over-the-shoulder third-person camera that orbits freely, with the walking controller the Haunted House uses, on keyboard, mouse and touch.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-orbit-camera. This task can't land until those are done.

## Acceptance

- Orbit, zoom and follow feel right on desktop and phone
- No clipping through tombstones or hedges

### #e2af Your character: an all-black humanoid with fuzzy edges

status open · size M · labels web, character, needs-verse · milestone #1a85 You, the visitor from another realm

Waits on: #e172 Everything lit by moonlight: dim, blue, with outlines (open)

Event: `e2af35d9b86762bf3f78d706e6c7b73edcf945cbfdc1680bd03eeb27c2353943`

The visitor is mostly a shadow: an all-black humanoid with soft, fuzzy edges, not detailed.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-soft-silhouette. This task can't land until those are done.

## Acceptance

- The shadow reads clearly against the moonlit garden
- Its edges look soft at desktop and phone sizes

### #a37a Particles that show you're visiting from another realm

status open · size S · labels web, effects, needs-verse · milestone #1a85 You, the visitor from another realm

Waits on: #e2af Your character: an all-black humanoid with fuzzy edges (open)

Event: `a37a650fe59c4eca0676a07968047314ed3d6ac3b85fa1ceb7998a27da7a1b26`

A light trail of particles around the visitor while it moves, so it reads as a guest from another realm.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-particles. This task can't land until those are done.

## Acceptance

- The particles follow you and stay within the frame budget on a phone

### #0e26 Tell us about your pet: text, images or video

status open · size M · labels web, server, needs-verse · milestone #7768 Bring your pet: make and customize its avatar

Waits on: #a0a3 Write down Planetary Villa and Hecate's Cemetery as Kiki described them (open)

Event: `0e261dc60f147d8c0b6e09bdefdf3118ff67a15748e436be45d51ff0ef8f6d98`

A create-your-pet flow drawn in Coax: write about your pet, and drop in photos or a short video, for a late pet or a new one. Uploads are stored privately with size and type limits, and only used to build the avatar.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): pt-player-endpoints. This task can't land until those are done.

## Acceptance

- Text, image and video uploads work on desktop and phone
- Uploads are private, limited in size and type, and deletable by their owner

### #7509 An agent turns what you shared into your pet's avatar

status open · size L · labels agents, ai · milestone #7768 Bring your pet: make and customize its avatar

Waits on: #0e26 Tell us about your pet: text, images or video (open); #95a9 Customize your pet's avatar, like in the Sims (open)

Event: `750941b2baba64713071607daff05ec04635923fbb103eda94426e6e82bf05c1`

An agent reads the text, images and video and sets the avatar's parameters: species and breed shape, size, proportions, coat colours and pattern, ears, tail, and the details that made the pet theirs. Model calls go through our Google-first provider routing.

## Acceptance

- From a photo and a sentence, the avatar is recognisably that pet to its owner in a playtest
- Each choice the agent made is shown and can be changed

### #95a9 Customize your pet's avatar, like in the Sims

status open · size L · labels web, character, needs-verse · milestone #7768 Bring your pet: make and customize its avatar

Waits on: #0e26 Tell us about your pet: text, images or video (open)

Event: `95a980014d8c6d72af897cd23e8b899902b2d8d7023534275b349c8c40a49658`

An editor drawn in Coax: change shape, proportions, colours, pattern and accessories with sliders and swatches, seeing the pet turn and move as you change it. The pet glows white in the world.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-quadruped, verse-glow. This task can't land until those are done.

## Acceptance

- Every parameter the agent sets can be changed by hand
- The glowing white look matches Kiki's art direction

### #22a9 Emotes to choose from, and new ones the agent makes

status open · size L · labels web, agents, animation, needs-verse · milestone #7768 Bring your pet: make and customize its avatar

Waits on: #95a9 Customize your pet's avatar, like in the Sims (open)

Event: `22a95f70cd39f5ed5fbf5dd33e5d41490159bddc3efad11ab0080eeee0324934`

A set of emotes (sit, play bow, roll over, howl, wag, sleep) to choose from, and new ones an agent makes from what you say about the pet ("she always spun twice before lying down").

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-authored-motions. This task can't land until those are done.

## Acceptance

- At least six stock emotes play on the pet
- An agent-made emote from a sentence plays in the world

### #067f Skills: fetch, obedience, digging holes

status open · size M · labels web, gameplay · milestone #7768 Bring your pet: make and customize its avatar

Waits on: #95a9 Customize your pet's avatar, like in the Sims (open)

Event: `067f97d4bac2112e43e28e1dd8199bf9ed58896cfb6871512dd748f165c78132`

The pet's skills, chosen when you make it: fetch, obedience (sit, stay, come), digging holes, and room for more. Skills decide what the pet can do in the field.

## Acceptance

- Each skill has a visible action in the field
- Skills are saved with the pet

### #cf48 A tombstone for each pet in the garden

status open · size M · labels web, needs-verse · milestone #48e3 Tombstones and memorial pages

Waits on: #1871 The cemetery garden and the field beside it (open); #0e26 Tell us about your pet: text, images or video (open)

Event: `cf48157b1d8703959e97d3f7e387237de02346ab8278df8a60c222c4cabeccc7`

When you press play, your pet's tombstone takes a plot in the garden: its name, dates and a line, with its achievements if it was competitive and its most beloved traits and stories.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): pt-saves. This task can't land until those are done.

## Acceptance

- A new pet's tombstone appears in the garden for every visitor
- Walking up to a tombstone shows its name, line and a link to the memorial

### #922b The memorial page: stories, achievements and testimonials

status open · size M · labels web · milestone #48e3 Tombstones and memorial pages

Waits on: #cf48 A tombstone for each pet in the garden (open)

Event: `922b141ad4b163a144d7e17ecbfef8a0f95794c75a50159b0b2fe2fcc0cef197`

A memorial page for each pet, drawn in Coax: its avatar, stories, achievements and beloved traits, and testimonials from people who knew it. Linked from the tombstone.

## Acceptance

- Every pet has a memorial page linked from its tombstone
- The owner can edit it and remove testimonials

### #1566 A shareable link for each memorial

status open · size S · labels web · milestone #48e3 Tombstones and memorial pages

Waits on: #922b The memorial page: stories, achievements and testimonials (open)

Event: `1566c8204752926efc2b812f8bcc7c3119e50136e440a2417d8826a6f1435c3d`

Each memorial page has a short shareable link with a share card (the pet's avatar in moonlight), so it can be sent to family and friends.

## Acceptance

- The link opens the memorial for someone signed out
- The share card shows in messages and social posts

### #8cff Play fetch in the field

status open · size M · labels web, gameplay, needs-verse · milestone #bbe4 Play with your pet

Waits on: #067f Skills: fetch, obedience, digging holes (open); #805a A third-person camera like Breath of the Wild (open)

Event: `8cffd7c2fce95e2f6750bd22d987d1ccc8600685e59e9608771a9d8ae0c607ac`

Throw a ball in the open field; a pet with the fetch skill runs, picks it up and brings it back.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-throw-physics, verse-quadruped. This task can't land until those are done.

## Acceptance

- Fetch works with mouse, keys and touch
- The pet's run and pickup look natural

### #4cc5 Walk the garden with your pet and visit other tombstones

status open · size S · labels web · milestone #bbe4 Play with your pet

Waits on: #cf48 A tombstone for each pet in the garden (open); #067f Skills: fetch, obedience, digging holes (open)

Event: `4cc53a7f4b0167bf4afb29c860fb0c608dd8f3240e8c64fe665db3399b713257`

Your pet follows you through the garden; other pets' tombstones can be visited and their memorials opened.

## Acceptance

- The pet follows without getting stuck on paths and hedges
- Any tombstone's memorial opens from the world

### #ae48 Other visitors and their pets in the garden

status open · size L · labels web, realtime, needs-verse · milestone #508a Others in the garden, and the chat

Waits on: #4cc5 Walk the garden with your pet and visit other tombstones (open)

Event: `ae484fc93ac33da30b24380f6d790bf63ed1236763aea7ecaef341cd67b419c2`

A multiplayer garden: see other visitors' shadows and their glowing pets moving in real time.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): pt-presence-chat. This task can't land until those are done.

## Acceptance

- Two visitors see each other and each other's pets within two seconds
- Room size is bounded

### #2594 A chat box like Horse Isle

status open · size M · labels web, realtime, needs-verse · milestone #508a Others in the garden, and the chat

Waits on: #ae48 Other visitors and their pets in the garden (open)

Event: `259414800f9953652b0267e1729501bb23792a8cf80049861a1b4bfb6cafc0ec`

A chat box in the corner for everyone in the garden, in the style of Horse Isle, drawn in Coax, with names and the pet each visitor brought.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): pt-presence-chat. This task can't land until those are done.

## Acceptance

- Chat works between two visitors on desktop and phone
- Messages are bounded and moderated

### #8b0e Three kinds of glowing flowers to collect in the field

status open · size M · labels web, gameplay, needs-verse · milestone #8ef7 The glowing flowers mini game

Waits on: #1871 The cemetery garden and the field beside it (open)

Event: `8b0e0dbff035e6f8c958e684cb63b60afbe3c2c9102bf63036bfc1a971927439`

Glowing flowers of three kinds grow in the field; walk up and pick them, and they go in your basket.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-glow, verse-particles. This task can't land until those are done.

## Acceptance

- All three kinds grow, glow and can be picked
- Picked flowers regrow after a while

### #1c61 Arrange flowers in a vase, point and click

status open · size M · labels web, gameplay, needs-verse · milestone #8ef7 The glowing flowers mini game

Waits on: #8b0e Three kinds of glowing flowers to collect in the field (open)

Event: `1c61a590c48ad55e90283f312c3ecec865b6457f981cd20de405b4a23cab61bb`

A close-up arranging view: point and click to put each flower in the vase where you want it.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): verse-closeup-arrange. This task can't land until those are done.

## Acceptance

- Arranging works with a mouse and with touch
- The arrangement is saved with the vase

### #7963 Place the vase anywhere; the tutorial leads you to the tombstone

status open · size S · labels web, gameplay, needs-verse · milestone #8ef7 The glowing flowers mini game

Waits on: #1c61 Arrange flowers in a vase, point and click (open); #cf48 A tombstone for each pet in the garden (open)

Event: `79630d073e1aff7fb4035a4119b66f8d63edbed516b3790e0db0818d8bf16503`

Carry the vase and set it down anywhere in the world. The tutorial directs you to place it by your pet's tombstone. Placed vases stay for everyone.

Blocked by Verse engine work, tracked in the Verse project (https://openagents.com/projects/verse): pt-saves. This task can't land until those are done.

## Acceptance

- A vase can be placed anywhere walkable
- The tutorial step points to the tombstone
- Placed vases persist and other visitors see them

### #47bc The first visit: a gentle tutorial

status open · size M · labels web · milestone #921c Tutorial, analytics and launch

Waits on: #7963 Place the vase anywhere; the tutorial leads you to the tombstone (open); #8cff Play fetch in the field (open)

Event: `47bcc0d966285331272ff7d3de57246f0a47e919febf5bbaf62251b6e735139c`

From landing to making your pet, playing fetch, picking flowers and placing the vase by the tombstone: one tip at a time, in the realm's plate style.

## Acceptance

- A first-time visitor finishes the loop without help in a playtest

### #e53d Analytics on every step

status open · size S · labels analytics · milestone #921c Tutorial, analytics and launch

Waits on: #47bc The first visit: a gentle tutorial (open)

Event: `e53d364bc2d4d8b4bb628a6bc56d924c02d2cb9b30627797d259231309eae8a8`

Record each step (galaxy choice, flight, pet made, emotes, fetch, flowers, vase, memorial shares) through the openagents.com analytics.

## Acceptance

- Each step appears on the analytics dashboard

### #93f9 Safety for uploads, memorials and chat

status open · size M · labels safety · milestone #921c Tutorial, analytics and launch

Waits on: #2594 A chat box like Horse Isle (open); #922b The memorial page: stories, achievements and testimonials (open)

Event: `93f9c37539c3ee8cfa4cf01aada6f88f26e58088a994055a8954e7d60fa65542`

Uploads, memorial text, testimonials and chat are moderated: limits, reporting, the owner's removal, and a review queue. Grief is personal; nothing a visitor shares is used for anything but their pet.

## Acceptance

- Reporting and removal work for each kind of content
- Upload use is limited to the pet's avatar, and said so on the upload screen

### #ce23 Kiki's review and the public launch

status open · size S · labels launch · milestone #921c Tutorial, analytics and launch

Waits on: #e53d Analytics on every step (open); #93f9 Safety for uploads, memorials and chat (open); #1566 A shareable link for each memorial (open); #22a9 Emotes to choose from, and new ones the agent makes (open)

Event: `ce2369805ace0d858f51243a87bf0b55cd81a9c7674a5199f77149c18df1b22a`

Kiki plays the whole expansion and signs off; then it goes public.

## Acceptance

- Kiki has signed off
- The expansion is reachable from the galaxy map for everyone
