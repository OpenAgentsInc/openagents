# Reuse what exists

Owner rule, 2026-11-11: **never hand-roll something the repository already
has.** Before you write a text box, a button, an undo stack, a camera, a
controller, a parser, a hash or a model call, find the shared one in the
table below and use it. If it falls short, extend the shared one so every
app gets the fix. Never write a second copy, and never write a "small local
version for now".

If nothing fits, say so in your commit message and put the new code where the
next app can reuse it (`components/`, `aethyr/`, `coax/lib/`, `everglade/`),
never inside one page.

Why: a hand-rolled copy is missing what the shared one already got right.
WorldBuilder's chat box was hand-rolled and lost Cmd+A, Cmd+C and Cmd+V; its
undo was hand-rolled and had no redo, while Aethyrforge's edit log had both.

## Where to look

| You need | Use | Where |
| --- | --- | --- |
| A text box: typing, selection, caret, undo, paste, IME | The edit buffer | `components/core/edit.coax` |
| Cmd/Ctrl+A, C, X, V and word deletion in a box on the website | The site's text keys | `apps/openagents-web/textkeys.coax` |
| Buttons, toggles, sliders, tabs, lists, panels, bars | The control kit, drawn in a skin | `components/kit/controls.coax`, `components/skin/` |
| A chat composer, transcript, Markdown, tool chips | The chat set | `components/zeron/`, `components/core/markdown.coax` |
| Measuring and wrapping text, fonts | Layout | `components/layout/` |
| A game HUD: bars, hotbar, windows, numbers | The HUD sets | `components/kit/hud.coax`, `everglade/hud/`, `verse/hud.coax` |
| Drawing UI on the GPU or in a terminal | Backends | `components/backend/` |
| Editing a world: typed edits, undo and redo, revision-checked saves | Aethyrforge's edit log | `aethyr/forge/edit.coax`, `aethyr/forge/doc.coax` |
| Publishing or packing a realm | Aethyrforge | `aethyr/forge/publish.coax`, `aethyr/forge/pack.coax` |
| The player character, walking, WASD, jump, touch stick | The Everglade zone controller | `everglade/zone.coax`, `everglade/zone-touch.coax`, `aethyr/walk.coax`, `aethyr/player.coax` |
| A follow, orbit or free camera | Aethyr's cameras | `aethyr/follow.coax`, `aethyr/orbit.coax`, `aethyr/camera.coax`, `aethyr/cam/` |
| Characters, rigs, faces, poses, materials | Aethyr's character | `aethyr/character/`, `aethyr/character-appearance.coax` |
| Ground, terrain, heights, ground rays | Aethyr's terrain | `aethyr/terrain.coax`, `aethyr/terrain-tile.coax`, `aethyr/ground-query.coax` |
| Collision, contact, queries | Aethyr | `aethyr/collide.coax`, `aethyr/contact.coax`, `aethyr/query.coax` |
| Scattering things, grass, props | Aethyr | `aethyr/scatter.coax`, `aethyr/render/` |
| Rendering, lighting, shadows, quality tiers | Aethyr's renderer and governor | `aethyr/render/`, `aethyr/quality.coax`, `aethyr/frame-pacing.coax` |
| Multiplayer: transport, replication, prediction, rewind | Aethyr's netcode | `aethyr/net/` |
| Steering and pathfinding | Aethyr | `aethyr/ai/`, `aethyr/nav/` |
| The Grid | The Grid and its session | `verse/grid.coax`, `verse/gridwalk.coax` |
| 3D models | The asset registry | `assets/registry/` (read its README) |
| Hashes, signatures, encryption | Coax crypto | `coax/lib/crypto/` |
| Nostr events, NIPs, NIP-98, zaps | Coax Nostr | `coax/lib/nostr/` |
| Bitcoin and Lightning | Coax Bitcoin and Lightning | `coax/lib/bitcoin/`, `coax/lib/lightning/`, `coax/lib/lnaddr/` |
| A wire format: messages between apps, the API, rooms or devices | The protocol's schemas, with generated Coax codecs | `proto/` (read its README); add a new package there rather than inventing JSON shapes in a page |
| JSON, protobuf, base64, Unicode, regex | Coax formats | `coax/lib/json/`, `coax/lib/proto/`, `coax/lib/text/` |
| HTTP, sockets, processes, files, time | Coax system | `coax/lib/net/`, `coax/lib/sys/`, `coax/lib/clock.coax` |
| Browser storage | The host's byte storage | `coax/web/STORAGE.md` |
| A model call (Gemini, embeddings) | The API's Vertex client | `coax/web/api/inference.c`, `coax/lib/systemone/` |
| A decision (classify, route, rank) | The Decisions API | `coax/lib/systemone/` |
| Agents: engines, transcripts, tiers | Coax agent | `coax/lib/agent/` |
| Tensors, autodiff, training, inference | Psionic | `psionic/lib/` (read `psionic/README.md` and its book) |
| QR codes, images, media | Coax | `coax/lib/qr/`, `coax/lib/image/`, `coax/lib/media/` |
| A terminal UI | Coax TUI | `coax/lib/tui/` |

## How to check before writing

1. Search for the thing by what it does, not by your name for it:
   `git grep -n -i "redo\|undo" -- '*.coax'`.
2. Read the shared module's header comment and its checks.
3. If two shared modules both seem to fit, use the one in the lower layer
   (`components/`, `aethyr/`, `coax/lib/`) over one inside an app.
4. If you find a hand-rolled copy while working, move it onto the shared
   module in the same change, or file an issue naming both.
