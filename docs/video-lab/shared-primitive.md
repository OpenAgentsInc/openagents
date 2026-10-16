# Video Lab as a shared primitive: assessment and roadmap

Status: assessment and proposed roadmap, 2026-10-16. Owner request: make
Video Lab work equally well on every surface (Coder in the terminal, the
openagents.com web app, and the Coder Desktop and iPhone apps in the Zeron
fork), probably through the SDK. Nothing here is implemented yet. Issues come
after the owner accepts the plan.

Inputs: the [spec](README.md), the [MVP](mvp.md), the
[SDK overview](../sdk/README.md) and [delivery plan](../sdk/delivery-plan.md),
[saved chat reads](../sdk/chat-history.md), the [Coder Desktop plan](../coder-desktop/README.md),
the [phone plan](../mobile/2026-10-15-coder-mobile-from-zeron.md), the
[1.0 freeze](../coder/1.0-freeze.md), and issues #11581, #11617, #11619,
#11835, #11869, #11824–#11832, #11840–#11852 and #11800–#11823. Sources were
`origin/main` at `146bc23236` here and `57c13158` in
[OpenAgentsInc/coder-desktop](https://github.com/OpenAgentsInc/coder-desktop).

## Summary

Video Lab works well today, but only on the owner's Mac. It is a loopback
Python service with no accounts. Three local HTTP callers reach it: the
local web chat, Coder, and `openagents lab`. A browser page on port 8792 is
the one real editor. The production website, Coder Desktop and the iPhone app
have no part of it. When a Lab chat is read back through the SDK, the card is
gone, because the history API has no field for it.

Turning it into a shared primitive needs five things, in this order:

1. **One Rust core** for the document model, patches, checks and timeline,
   so every surface runs the same rules. Today they are split across
   `lab.py`, `cut.py` and `verse-film`.
2. **One account-scoped Lab API** at `api.openagents.com/v1/lab`, specified
   in OpenAPI, with documents, versions, notes, render jobs and media
   behind authorization.
3. **An `openagents::lab` module in `openagents-sdk`.** It is the only way
   any client reaches the Lab, including our own web server.
4. **Render placement that respects the private-asset rule.** Cloud render
   boxes go through the Computers API. Anything that uses MetaHuman or other
   private assets renders on an enrolled machine that pulls jobs from the
   API, so those assets never leave it.
5. **One typed card** (`video_lab.card.v1`) saved on the chat message and
   returned by the history API. Each surface draws it with its own player.

The terminal is the hard case because of the 1.0 freeze. Coder 1.0.0
already ships Lab tools that talk to any `VIDEO_LAB_URL`, so a local bridge
can bring account-hosted documents to 1.0 without a new binary. Inline
pictures and the SDK client wait for the next release.

## What exists today

| Piece | Where | Shape |
| --- | --- | --- |
| Lab service | `scripts/film/lab.py` (2,741 lines) | Python stdlib HTTP server, 127.0.0.1 only, no auth. 18 tools at `POST /api/tool/{name}`, plus `/api/tools`, `/api/docs`, `/api/doc`, `/api/card`, `/api/timeline`, `/api/note`, `/media/*` |
| Storage | `~/work/film-out/lab/<project>/<doc>/vN/` | Files outside git: version JSON, summary, author, digest. Sources, scores and logs live beside them |
| Checks | `scripts/film/cut.py check` (995 lines), `lab.py check()` | Beat grid, mid-animation cuts, pauses, speech-safe cuts, safe area, sources. Python |
| Rendering | `lab.py do_render` → `cut.py render` (ffmpeg), Blender `studio.py`, `verse --example film_shot`, whisper.cpp | One job at a time, cached by digest and quality. Mac only |
| Timeline | `crates/verse-film/src/timeline.rs` (474 lines) through `video-lab-timeline` and `timeline_bridge.py` | Rust owns edits, links, snapping and speed (#11835). It is the only Lab logic in Rust |
| Player and editor | `scripts/film/lab_player.html` (about 20 KB of vanilla JS) | Documents, versions, compare, notes, the timeline in the screenshot |
| Web chat | `crates/openagents-web/src/lab.rs` and `labs.rs` | Loopback-only `OPENAGENTS_VIDEO_LAB`. Reads the tool list from `/api/tools`, draws an inline `<video>` card (maud), proxies media. One Gemini door for all Labs (#11619) |
| Coder | `crates/coder-new/src/video_lab.rs` (373 lines, **in 1.0.0**) | Copied declarations of 14 tools. `POST {VIDEO_LAB_URL}/api/tool/{name}`, no credential. Removes the card and returns the player link |
| CLI | `crates/openagents-cli/src/lab.rs` (287 lines, **in 1.0.0**) | `openagents lab list|open|show|frame|edit|versions|render|compare|notes` against the same URL |
| Tests | `scripts/film/test_lab.py` (27 tests), Rust unit tests | Local only |

The 1.0 tool list is missing `lab_keep_captions`, `lab_new`, `lab_lines` and
`lab_voice`. The service and the web chat have them.

In the fork, `crates/coder-account` depends on `openagents-sdk` (pinned to
`5a3d1c14`) for sign-in, saved history and reading transcripts. Desktop
(GPUI) and iOS (UIKit over the UniFFI `crates/mobile`) both draw saved chats.
4091b7b5 added following of saved updates for #11869. Neither app has a video
player. The closest thing to a card is the Luau `show_visual` runtime
(`crates/visual`, `crates/ui/src/visuals.rs`, `VisualView.swift`). Saved
OpenAgents tool rows are generic, and unknown parts show "This part uses a
format this app doesn't support yet".

The saved history API returns `tool {name, summary, status, input, output}`
and declares `artifacts` with `maxItems: 0`
(`crates/openagents-web/src/history_openapi.rs`). A Lab card saved on a web
reply (`Meta::cards`) never reaches the SDK.

#11869 shows the current state well. One chat can now be followed live on
all four surfaces, but each one shows only words. The film was assembled in
the Mac's 8792 player.

## Gaps, surface by surface

| Need | Web (openagents.com) | Terminal (Coder) | Desktop (fork, GPUI) | iPhone (fork, UIKit) |
| --- | --- | --- | --- | --- |
| Reach the Lab | Loopback only. Production has none | Loopback, no credential, frozen | None | None |
| Document and version store | Mac files | Mac files | None | None |
| Rules (patch, checks, timeline) | Python service | Python service | None | None |
| Card in a live chat | `<video>` card, local stack only | Link only | No card type | No card type |
| Card in saved history | Lost at the API | Lost | Lost | Lost |
| Player | `lab_player.html` and the card | Opens the browser player | No video playback | No video playback |
| Timeline editing | `lab_player.html` only | None | None | None |
| Renders | Mac | Mac | Mac (through the API later) | None |
| Notes at a time | Card and player | `lab_notes` read | None | None |

## Design

### 1. `video-lab` core crate (pure Rust, no IO)

New `crates/video-lab`. Schemas (`cut.v1`, `shot.v1`, `scene.v1`, and
`video_lab.card.v1`), document and version types, digests, RFC 6902 patches
with `@shot` addressing, every `cut.py check` rule, the timeline from
`verse-film/src/timeline.rs`, caption timing (`lab_lines` pace rules), and
the tool manifest as data. No network, file, ffmpeg or Blender code. It must
compile for the fork's macOS, iOS (UniFFI) and wasm32 targets, so a client
can check an edit or move a clip before anything reaches a server.

Refusals keep today's plain wording ("s4-tray and the 15 shots after it
would start off the 120 BPM beat grid…"). They are typed values, and a
client may show them before sending anything.

`lab.py` calls the crate (through a bridge binary, as `timeline_bridge.py`
does now) until the service is ported. Behaviour stays the same and
`test_lab.py` keeps passing. The checks that need media (pauses, motion,
speech) split into a pure rule over measurements plus a measurement made
where the media is.

### 2. Lab API (account-scoped, OpenAPI)

`/v1/lab/*` on `api.openagents.com`, behind the same gate as Computers while
admission is limited. It belongs to a project, so access, sharing and
deletion follow project membership.

| Resource | Operations |
| --- | --- |
| `documents` | list, create (`lab_new`), get latest or a version, delete |
| `documents/{id}/versions` | list; `POST` with patch, summary and the `If-Match` base version. Refusals are `422` with the typed reason and nothing is saved |
| `documents/{id}/timeline` | view, and intents (move, link, speed, text) that become a version |
| `renders` | create with document, version and quality; get state; cancel. Idempotent per digest and quality |
| `media/{id}` | Range reads of renders, posters, frame strips, sources. Short-lived signed URLs so `<video>`, AVPlayer and terminals can stream |
| `frames`, `compare` | One still or a strip; a pair of versions at a time |
| `notes` | List or add a note at a time on a version |
| `sources` | Import through resumable project uploads (#11866), then probe, transcribe and find pauses as a job |
| `tools` | The tool manifest, generated from the core crate, so every model loop uses one list |
| `events` | Server-sent events per document: new version, render state, new note. Polling is the fallback |

It follows the [contract rules](../sdk/README.md#contract-and-security-rules):
ownership comes from authentication, an accepted job is not a finished
render, one idempotency key covers each logical request, and limits are
bounded. Storage holds documents and versions in Postgres and media in
Cloud Storage, keyed by account and project.

The service is the `lab.py` HTTP layer ported to Rust (axum), using the core
crate. Rendering moves to workers. **Keep a loopback mode** that serves the
1.0 routes (`/healthz`, `/api/tools`, `/api/tool/{name}`) unchanged, with no
accounts, so the owner's current workflow and frozen Coder keep working.

### 3. `openagents::lab` in the SDK

Typed `Lab` client in `openagents-sdk` next to `history` and `computers`:
documents, versions with the conflict and refusal results, renders with a
wait helper and cancellation, media URLs, notes, sources, tools and an event
stream. It re-exports the core crate's types, so a client checks an edit
locally with the same code the server runs. This follows the history reader
pattern: account-bound references, caller-owned cancellation, and explicit
`changed`/`missing` results.

Every product goes through it, including `openagents-web` (as #11806 does for
Computers). The web adapter stops posting to loopback and calls the SDK. The
Labs door keeps choosing tools by the model's function choice, never by
matching words. Desktop and iOS get it by moving their pin in
`coder-account`. The mobile FFI exposes a small facade, not the whole
client.

### 4. Rendering: three placements, one queue

| Placement | Runs | Used for |
| --- | --- | --- |
| Local loopback | Today's Mac service | Owner offline, and Coder 1.0 without the bridge |
| Enrolled render host | A worker on the owner's Mac pulls jobs from the API over an outbound connection, with a device credential limited to Lab jobs. It renders with local private assets and uploads only the result | Anything that uses MetaHuman, Draconia, Fab packs or Berkeley Mono |
| Cloud render box | A Computers API computer (Boat, later SPACE) from a render image with ffmpeg, Blender, whisper.cpp and Verse. Delete on completion | Documents without private assets, so web and phone work while the Mac is off |

The API decides the placement from the version's asset manifest (the digest
already lists files) using deterministic rules. A job needing private assets
is never given to a cloud box. If no eligible host is online, it waits and
says so. The `.no-training` marker becomes a field on the render and its
media, and anything that builds a dataset, trace or Gym run checks it. The
spec says renders of her are fine to watch and share, but the MVP has no
upload path. Storing those renders in the cloud is an owner decision (below).

The enrolled host is a narrow job puller, not a general own-machine computer,
so it does not wait for #11817.

### 5. The card and the player on each surface

`video_lab.card.v1` holds the document, version and summary; render state
and media references; poster; frame strip; compare pair and time; the
latest notes; and refusals. It is saved on the assistant message. The
history API gets an additive `cards` field (the frozen 1.0 contract allows
new fields), and the SDK returns it as a typed enum with an unknown-variant
fallback. The card stores only references. Each surface fetches media
through the SDK, so access is checked on every read.

A shared view-model in the core crate (`CardState`: poster, play position,
rendering, failed, compare, note draft) keeps the four renderers consistent:

| Surface | Card | Player and editor |
| --- | --- | --- |
| Web | The existing maud card, fed by the SDK and signed media URLs | `lab_player.html` becomes a page on openagents.com (`/lab/{project}/{doc}`), still vanilla JS, calling `/v1/lab` with the browser session. Timeline intents go to the API. Local checks can use the core crate as wasm when that helps |
| Desktop (GPUI) | A new typed row in `openagents_reader.rs` and the live transcript, with poster, state, play and Compare | GPUI has no video. Host an `AVPlayerLayer` on macOS, behind a small trait for other platforms. Timeline in GPUI over the core crate. This is the first native editor |
| iPhone | A new layout row in `crates/mobile` and a Swift cell | `AVPlayer` full screen; compare; "Note at this time" by voice; version list. Editing is limited to chat edits and trims at first. The full timeline waits until the desktop one is proven |
| Terminal | Words, render state and the player link today. After the freeze lifts: poster and frame strip inline with Kitty or iTerm2 graphics where the terminal supports them, with the link everywhere | The browser player. There is never a dead play key |

Live updates use the document event stream when a card is on screen, and the
same following the fork added for saved chats. Readers never start renders.
A render starts only from a tool call or a button the person presses.

### 6. Terminal and the 1.0 freeze

Coder 1.0.0 and `openagents` 1.0.0 cannot change. They already offer Lab
tools whenever `VIDEO_LAB_URL` is set, and they call
`POST {url}/api/tool/{name}` and `/healthz` with no credential.

- **Now, no binary:** `lab bridge` (a mode of the ported service) listens on
  loopback. It speaks exactly the 1.0 routes and forwards to `/v1/lab` with
  the person's saved `openagents` login. The 14 tools in 1.0 work against
  account documents, and the four newer tools stay web-only there. This is
  for the owner and testers who run the bridge. A plain 1.0 install without
  it sees no Lab tools, as today.
- **Next release** (add to [next-release.md](../coder/next-release.md)):
  Coder and `openagents lab` use `openagents::lab`, read `tools` from the
  API instead of a copied list, follow render events, and draw posters and
  strips inline.

## Roadmap

Each step ships on its own and keeps today's Mac workflow working.

| Step | Delivers | Depends on | Done when |
| --- | --- | --- | --- |
| VL0: core crate | `crates/video-lab`: schemas, patch, checks, timeline (moved from `verse-film`), tool manifest, card schema. `lab.py` calls it | None | `test_lab.py` and the Rust ports of its cases pass; Coder's copied list is checked against the manifest; builds for iOS and wasm32 |
| VL1: Rust service, loopback | axum port of `lab.py` routes and the player, render workers shelling out as now | VL0 | The 1.0 Coder, the CLI and the local web chat run the MVP script unchanged against it; Episode 290 re-renders to the same digest |
| VL2: Lab API and SDK | `/v1/lab` with Postgres and Cloud Storage, OpenAPI (`docs/sdk/openapi.lab.json`), `openagents::lab`, events, signed media | VL1; #11866 for source uploads | External SDK example: a dedicated test account creates, edits (one refused), renders, notes and deletes; a second account is refused everywhere; evidence JSON; cleanup |
| VL3: render placement | Enrolled render host on the Mac, and cloud render boxes on the Computers API | VL2; Computers client (#11805, done) | A private-asset document renders only on the Mac and its raw assets appear in no bucket (checked); a plain document renders on Boat with the Mac off; the box is deleted |
| VL4: production web | The Labs door calls the SDK; the card and player on openagents.com behind the gate; `cards` in the history API | VL2, VL3 | The four MVP prompts work on openagents.com with a test account; the card comes back through `/v1/history`; the 1.0 contract checks still pass |
| VL5: Coder Desktop | Pin bump, card row, AVPlayer view, compare, notes, then the GPUI timeline | VL4 (cards in history), VL2 | A web Lab chat opened on the desktop plays and compares; a timeline move on the desktop appears as a new version on the web |
| VL6: iPhone | FFI facade, card cell, player, compare, notes by voice, chat edits | VL5 | The same chat on the phone plays the latest render and adds a timed note that the web player shows |
| VL7: terminal | `lab bridge` now; SDK client and inline graphics in the next release | VL2 (bridge); freeze lifted (rest) | 1.0.0 with the bridge edits an account document and returns the web player link; the next-release entry is recorded |
| VL8: beyond video | Live Verse scene view (spec slice 6); Game, CAD, Robot and Ship Labs on the same store, events and card shape | VL4 | Separate plans |

VL0 and VL1 need no owner decision and change nothing for users. VL5 and
VL6 can start their card rows against fixture cards from VL0 while VL2 to
VL4 land. The order inside the fork follows its plan: pull upstream often,
and keep OpenAgents wiring in our own crates (`coder-account` and a new
`coder-lab` UI module) so merges stay small.

## Measures

The spec's measures hold on every surface: a 10-second preview plays within
60 s locally and within 3 minutes from a cold cloud box; a refused edit never
reaches a render; every version re-renders to the same pixels from its
digest; no private asset reaches a cloud bucket, dataset or trace. Add:

- The same refusal text for the same patch on web, desktop, phone and
  terminal, because they share one crate.
- A card saved in a web chat appears on the other three surfaces through
  `/v1/history` within the following interval (2 s in the fork today).
- Media starts playing within 2 s of pressing play on desktop and phone for
  a cached render. Range reads mean seeking doesn't download the whole file.

## Verification

Use the [SDK rules](../sdk/delivery-plan.md#issue-handling-evidence-and-completion):
dedicated test accounts, never the owner's chats or documents; synthetic
documents and media (no MetaHuman) for anything outside the owner's Mac; the
affected crates' tests and formatting; evidence JSON per step; every test
render, box and upload cleaned up. The Lab version store is a good formal
target: concurrent edits against one base version, idempotent render
creation, and placement never sending private assets to a cloud host. Model
those with Stateright on the core as for the Computers lifecycle, and turn
each counterexample into a test.

## Owner decisions

1. **Private-asset renders in the cloud.** May a render (never the raw
   assets) that shows MetaHuman or Fab content be stored in the account's
   Cloud Storage and streamed to the phone? The spec allows sharing renders,
   and the MVP has no upload path. Recommendation: yes, private to the
   project and marked `no-training`.
2. **Who gets it first.** Recommendation: the owner and dedicated test
   accounts behind the Computers gate. Wider access waits for billing,
   because cloud renders cost GPU time.
3. **Desktop video approach.** Recommendation: AVPlayer on macOS and iOS
   first. A cross-platform decoder can wait until Coder Desktop ships beyond
   macOS.
4. **Bridge for Coder 1.0.** Recommendation: build it in VL7 for the owner
   and testers, and don't advertise it to 1.0 users.

## Risks

- **Port size.** About 3,700 lines of Python carry rules the owner relies
  on. VL0 moves the rules first and checks them against the current tests.
  VL1 keeps the routes and output digests, so a regression shows before any
  user sees it.
- **Upstream churn in the fork.** Zeron ships almost daily. Keep Lab code in
  our own crates and add one card row in the transcript instead of changing
  the transcript painter.
- **GPUI video.** There is no player in GPUI. A native layer brings
  z-ordering and resizing work, and the timeline needs frame posters rather
  than live decoding at first.
- **Cost.** Renders are GPU time. Cloud boxes need the Computers usage and
  quota work (#11815) before anyone outside the gate can use them.
