# Reuse what exists

Owner rule, 2026-11-11: **never hand-roll something the repository already
has.** Before you write a text box, a button, an undo stack, a camera, a
controller, a parser, a hash or a model call, find the shared one in the
table below and use it. If it falls short, extend the shared one so every
app gets the fix. Never write a second copy, and never write a "small local
version for now".

If nothing fits, say so in your commit message and put the new code where the
next app can reuse it (`components/`, `metaverse-engine/`, `coax/lib/`, `everglade/`),
never inside one page.

Why: a hand-rolled copy is missing what the shared one already got right.
WorldBuilder's chat box was hand-rolled and lost Cmd+A, Cmd+C and Cmd+V; its
undo was hand-rolled and had no redo, while the OpenAgents Metaverse Engine's edit log had both.

## Where to look

| You need | Use | Where |
| --- | --- | --- |
| A text box: typing, selection, caret, undo, paste, IME | The edit buffer | `components/core/edit.coax` |
| Cmd/Ctrl+A, C, X, V and word deletion in a box on the website | The site's text keys | `apps/openagents-web/textkeys.coax` |
| Buttons, toggles, sliders, tabs, lists, panels, bars | The control kit, drawn in a skin | `components/kit/controls.coax`, `components/skin/` |
| A text surface on the web (a chat, a settings page, a document): buttons, fields, a composer, messages, tool calls, code blocks, notices, rows, menus | The frames set in the mono skin, drawn by the browser | `components/kit/frames.coax`, `components/skin/mono.coax`, `components/core/tree.coax`, `components/backend/dom.coax`, `coax/web/dom-host.js`; see them at `/components` |
| An icon | Apps SDK UI's 755, by name (`IC_COPY`), drawn from their SVG files; `k_icon` and `k_icon_button` in the frames set | `components/core/icons.coax`, `components/icons/apps-sdk/` |
| ARWES frames, easings, the animator, text effects, backgrounds | The ARWES ports | `components/core/arwes/` |
| A chat on the GPU (the desktop app): composer, transcript, Markdown, tool chips | The earlier chat set | `components/zeron/`, `components/core/markdown.coax` |
| Measuring and wrapping text, fonts | Layout | `components/layout/` |
| A game HUD: bars, hotbar, windows, numbers | The HUD sets | `components/kit/hud.coax`, `everglade/hud/`, `verse/hud.coax` |
| Drawing UI on the GPU or in a terminal | Backends | `components/backend/` |
| Editing a world: typed edits, undo and redo, revision-checked saves | The Metaverse Engine's edit log | `metaverse-engine/editor/edit.coax`, `metaverse-engine/editor/doc.coax` |
| Publishing or packing a realm | Metaverse Engine | `metaverse-engine/editor/publish.coax`, `metaverse-engine/editor/pack.coax` |
| The player character, walking, WASD, jump, touch stick | The Everglade zone controller | `everglade/zone.coax`, `everglade/zone-touch.coax`, `metaverse-engine/move/walk.coax`, `metaverse-engine/move/player.coax` |
| A follow, orbit or free camera | The Metaverse Engine's cameras | `metaverse-engine/cam/follow.coax`, `metaverse-engine/physics/orbit.coax`, `metaverse-engine/cam/camera.coax`, `metaverse-engine/cam/` |
| Characters, rigs, faces, poses, materials | The Metaverse Engine's character | `metaverse-engine/character/`, `metaverse-engine/character/character-appearance.coax` |
| Ground, terrain, heights, ground rays | The Metaverse Engine's terrain | `metaverse-engine/world/terrain.coax`, `metaverse-engine/world/terrain-tile.coax`, `metaverse-engine/physics/ground-query.coax` |
| Collision, contact, queries | Metaverse Engine | `metaverse-engine/physics/collide.coax`, `metaverse-engine/physics/contact.coax`, `metaverse-engine/physics/query.coax` |
| Scattering things, grass, props | Metaverse Engine | `metaverse-engine/world/scatter.coax`, `metaverse-engine/render/` |
| Rendering, lighting, shadows, quality tiers | The Metaverse Engine's renderer and governor | `metaverse-engine/render/`, `metaverse-engine/core/quality.coax`, `metaverse-engine/core/frame-pacing.coax` |
| Multiplayer: transport, replication, prediction, rewind | The Metaverse Engine's netcode | `metaverse-engine/net/` |
| Steering and pathfinding | The Metaverse Engine: the steering service, and the navigation cells built from colliders ([how](verse/navigation.md)) | `metaverse-engine/ai/`, `metaverse-engine/world/nav/`, `everglade/shared/nav.coax` |
| A world in cells: which cells load, simulate and draw round the players | The Metaverse Engine's cell controller, and Everglade's manifest ([how](verse/one-world.md)) | `metaverse-engine/world/cells.coax`, `everglade/world/zone.coax` |
| The Grid | The Grid and its session | `verse/grid.coax`, `verse/gridwalk.coax` |
| 3D models | The asset registry | `assets/registry/` (read its README) |
| Hashes, signatures, encryption | Coax crypto | `coax/lib/crypto/` |
| Nostr events, NIPs, NIP-98, zaps | Coax Nostr | `coax/lib/nostr/` |
| An app's login to the account: email code, a key, passkeys, access and refresh tokens, a request with `Authorization: Bearer` | The apps' login (iPhone and Mac) | `apps/shared/host/apple/AppAuth.swift` (`authorize` for any request) |
| A hardware key that ties an app's login to its device (Secure Enclave, Android Keystore), and its proof | The shared device key | `apps/shared/host/apple/DeviceKey.swift`, `apps/shared/host/android/device-key.c`, `coax/web/api/auth-device-crypto.coax` |
| Bitcoin and Lightning | Coax Bitcoin and Lightning | `coax/lib/bitcoin/`, `coax/lib/lightning/`, `coax/lib/lnaddr/` |
| A wire format: messages between apps, the API, rooms or devices; a file or log line a program stores | The protocol's schemas and the one codec file each shape has | `proto/` (read its README). Look the shape up in `proto/registry.json` and load its `codec` file; never write its JSON, frame or id by hand in a second file. For a new shape, add the schema and a registry row ([how](../proto/README.md#register-a-wire-shape)). `coax/web/tool.sh wire-check` fails on an unregistered shape |
| JSON, protobuf, base64, Unicode, regex | Coax formats | `coax/lib/json/`, `coax/lib/proto/`, `coax/lib/text/` |
| HTTP, sockets, processes, files, time | Coax system | `coax/lib/net/`, `coax/lib/sys/`, `coax/lib/clock.coax` |
| Browser storage | The host's byte storage | `coax/web/STORAGE.md` |
| A page's state layout: a field's offset, size and type by path; what a code reload keeps | The build's field table (`coax layout`), which `coax drive` reads and a dev module carries | `build/.drive/APP.layout`, `coax/web/drive.coax`, `coax/web/state-keep.js` |
| A model call (Gemini, embeddings) | The API's Vertex client | `coax/web/api/inference.c`, `coax/lib/systemone/` |
| A decision (classify, route, rank) | The Decisions API | `coax/lib/systemone/` |
| Agents: engines, transcripts, tiers | Coax agent | `coax/lib/agent/` |
| A question over a context far bigger than one prompt (a long file, a folder): the model programs over the context in a safe Python subset and calls sub-models on pieces | Recursive Language Models | `coax/lib/rlm/` (read its README) |
| A coding agent whose repository stays out of the prompt: the model reads, searches, edits and runs commands in a workspace through the RLM's environment | RLM code | `coax/lib/rlm/code.coax` (README "RLM code") |
| Tensors, autodiff, training, inference | Psionic | `psionic/lib/` (read `psionic/README.md` and its book) |
| QR codes, images, media | Coax | `coax/lib/qr/`, `coax/lib/image/`, `coax/lib/media/` |
| A terminal UI | Coax TUI | `coax/lib/tui/` |

## How to check before writing

1. Search for the thing by what it does, not by your name for it:
   `git grep -n -i "redo\|undo" -- '*.coax'`.
2. Read the shared module's header comment and its checks.
3. If two shared modules both seem to fit, use the one in the lower layer
   (`components/`, `metaverse-engine/`, `coax/lib/`) over one inside an app.
4. If you find a hand-rolled copy while working, move it onto the shared
   module in the same change, or file an issue naming both.
