# Labs: one platform for every Lab, on every surface

Status: proposed architecture and roadmap, 2026-10-16. It replaces the
Video-only first draft. [Video Lab's part](../video-lab/shared-primitive.md)
now covers only what is particular to video. Issues are linked in
[the roadmap](#roadmap-and-issues).

Owner request: make Video Lab a shared primitive that works equally well on
Coder in the terminal, the openagents.com web app, and the Coder Desktop and
iPhone apps in the Zeron fork, probably through the SDK. Then: "there may be
Film lab, Photo lab etc. … think bigger than just video lab". So this plan
designs one Labs platform. Video Lab is its first heavy kind, CAD Lab is the
proving kind, and later kinds plug in without new work on each surface.

## What a Lab is

Every Lab we have follows the same pattern, first set by
[Video Lab](../video-lab/README.md):

- **Typed documents.** Plain JSON with a schema.
- **Edits.** A model in any chat changes a document only through typed
  tools. The model's function choice is the route; no words are matched.
- **Checks.** The kind's own rules accept or refuse an edit in plain words
  before anything is saved.
- **Immutable versions.** Each has a summary, an author and a digest.
- **Outputs.** Work made from a version: renders, builds, playtests,
  simulations, exports.
- **A card.** It shows the result inline in the chat.
- **A deeper page.** A player, viewport or game page.
- **Notes** pinned to a place in a version.

| Lab | Documents | Checks | Outputs | Card view | Runs today |
| --- | --- | --- | --- | --- | --- |
| [Video](../video-lab/README.md) | `cut.v1`, `shot.v1`, `scene.v1` | Beat grid, animation beats, pauses, speech-safe cuts, safe area (`cut.py`); timeline (`verse-film`) | MP4 renders at preview, review and master quality; frames; music; transcripts | Video, compare | `scripts/film/lab.py`, loopback 8792, Mac only |
| [CAD](../cad-lab/README.md) | `part.v1`, `assembly.v1` | B-rep rebuild, mass properties, sketch freedom (`crates/cad`) | glTF, STEP, 3MF, STL, PDF and DXF drawings, hero image | 3D viewport with face picking | `crates/cad-lab` on loopback 8794, **and hosted on openagents.com** (#11653, `cad_web.rs`) |
| [Game](../game-lab/README.md) | `game.v1` | The bot plays it; still winnable (`icecream-rules`) | Playtest numbers, a WebGL build | Playable game, numbers | `crates/game-lab`, loopback |
| [Robot](../lab/README.md) | Robot, environment, experiment versions | Description import, dependencies (`crates/robot-lab`) | Simulation runs, replays, policy results | 3D or video replay | Loopback plus paired-host remote reads; the phone mounts Rust Native cards |
| [Agent](../agent-lab/README.md) | Agent drafts, task suites, experiment versions | SDK spec validation, compatible runtimes, task checks, finite limits | Existing SDK assignments, conversations, comparison reports | Report, compare | Custom-agent SDK and iOS creation/delegation implemented (#11902); Lab workbench proposed |
| Ship (#11651) | `spacecraft.v1` | Mass budget, thrust line, burn, latches (`verse_lagrange2::ship`) | A Verse zone file | Numbers, 3D | `crates/ship-lab`, loopback |
| Photo, Film, others | Not built | | | Image, compare, video | |

`crates/briefing-lab`, `layertree-tee-lab` and `verse-zone-lab` (Physics Lab
in Verse) are named Labs but are not chat Labs. They stay out of scope.

[Agent Lab](../agent-lab/README.md) extends the same workbench to creating,
trying, comparing, and improving agents. It reuses the account agent catalog
and existing session assignments; its Lab jobs coordinate finite experiments
rather than introducing another execution engine. Saving a tested candidate
for the mobile picker is an explicit action. Its initial Report card needs no
new standard view, and its hosted delivery shares the dependencies below.

### What is shared and what is not, today

- **Web chat:** one door for all chat Labs
  (`crates/openagents-web/src/labs.rs`, #11619). One Gemini turn sees every
  Lab's tools. The tool name's prefix (`lab_`, `cad_`, `game_`, `robot_`,
  `ship_`) picks the Lab, and a hard-coded `Kind` enum names each Lab's card
  and document argument.
- **Separate adapters for each Lab:** `lab.rs`, `cad_lab.rs`,
  `game_lab.rs`, `robot_lab.rs` and `ship_lab.rs` each draw their own card.
- **Separate local services:** each Lab runs its own loopback service,
  configured by its own variable (`OPENAGENTS_VIDEO_LAB`,
  `OPENAGENTS_CAD_LAB`, …). Each one exposes the same routes: `/healthz`,
  `/api/tools`, `POST /api/tool/{name}`, `/api/card`.
- **Three version stores:** Video's Python files, Robot Lab's `Store`
  (`crates/robot-lab/src/store.rs`, also used by CAD Lab locally), and the
  CAD files kept per account and project in the chat bucket
  (`cad_web.rs`).
- **Coder 1.0.0 and `openagents` 1.0.0 (frozen):** they carry copied
  clients for `VIDEO_LAB_URL`, `ROBOT_LAB_URL`, `CAD_LAB_URL` and
  `GAME_LAB_URL`. Every one calls `{base}/healthz` and
  `POST {base}/api/tool/{name}` with no credential.
- **History API:** it saves no cards. `artifacts` has `maxItems: 0`
  (`crates/openagents-web/src/history_openapi.rs`).
- **Desktop and phone apps** (`apps/openagents/`, imported from the Zeron
  fork on 2026-10-16): they read saved chats through `openagents-sdk` (by path
  from `apps/openagents/crates/coder-account`). Since 2026-10-16 the desktop
  has the Video Lab tools and a native Video Lab card (inline playback, live
  render state). The phone has neither yet.
  `crates/visual-gpu` has wgpu, and Swift paints the mobile layout.

**CAD Lab on openagents.com is the hosted precedent.** Signed-in people use
it in any web chat. The engine runs as a child process with a deadline and a
memory limit, one run at a time. Documents are scoped to the person or the
project, with viewer, editor and owner roles. Version files are written once.
`/cad/{doc}` pages exist, and the browser runs the wasm engine for exports.
The platform generalizes that design. It does not invent a second one.

## The model

Six common resources, one contract per kind, and a small set of standard
views.

### Common resources (identical for every kind)

| Resource | Meaning |
| --- | --- |
| **Space** | Where Labs live: a person's own space (`me`) or a project. Access follows project membership: viewers read, editors change, owners delete. This is CAD's `Scope` |
| **Document** | `{id, kind, schema, title, created, latest}`. `kind` is a registered Lab kind, and `schema` is one of that kind's document schemas |
| **Version** | Immutable: number, parent, author (person or chat turn), summary, content, content digest, the check report, and its asset manifest (the files it uses, each with a digest and an asset class). It is created only by the kind's `apply`, never written directly |
| **Artifact** | A file made from or attached to a version: media type, bytes digest, size, `role` (`render`, `poster`, `frame`, `export`, `build`, `replay`, `source`), asset class, `no_training`. Read with signed, short-lived Range reads |
| **Job** | Work that makes artifacts: kind-defined `type` (`render`, `rebuild`, `playtest`, `simulate`, `transcribe`, `export`), inputs (a version and parameters), requirements, placement, state (`queued`, `running`, `done`, `failed`, `cancelled`, `uncertain`), logs and outputs. Its idempotency key is the version digest plus the type plus the parameters |
| **Note** | Text by a person on a version, at a typed **locator**: `time` (seconds), `frame`, `step` (simulation), `ref` (a CAD face or edge reference), `region` (x, y, w, h on an image), or none |

Two more parts sit on top of these:

- **Events.** Each document has a stream of new versions, job state changes
  and notes.
- **References between kinds.** A version can cite another kind's version:
  a CAD part placed in Robot Lab or a Verse zone (`cad_web/handoff.rs`
  today), a game build cut into a film, a Ship zone file. A reference is
  always to an immutable version, and reading through it checks access to
  both.

### One contract per kind: `LabKind` in `lab-kit`

A kind is a crate that implements a pure trait, with no network or files:

```rust
pub trait LabKind {
    const KIND: &'static str;                         // "video", "cad", "game", ...
    fn manifest() -> KindManifest;                     // name, schemas, tools, job types, views, asset classes
    fn check(doc: &Document, ctx: &Measurements) -> Report;
    fn apply(base: &Version, change: Change) -> Result<Draft, Refusal>;
    fn jobs_for(version: &Version, want: Want) -> Vec<JobSpec>;
    fn card(version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card;
}
```

- **`Change`** is either a JSON Patch (RFC 6902; kinds can add addressing
  such as Video's `/shots/@name`) or a typed **intent** of that kind.
  Examples: Video timeline moves, CAD "round this edge" from a face pick,
  Game tuning sliders. Every edit, whether it comes from a chat tool, a
  timeline drag or a phone button, is one `apply`, and so is checked
  exactly once in one place.
- **`Refusal`** is typed and carries the plain-words reason. Every surface
  shows the same sentence.
- **`Measurements`** are facts that need media or heavy compute: Video's
  motion and pause measures, Game's bot runs. A job computes and caches
  them, so `check` stays pure.
- **Tools** are declared in the manifest. A chat tool is a thin, typed
  mapping onto the common operations: read, `apply`, request a job, add or
  list notes, compare. Kinds keep their own prefixes (`lab_`, `cad_`, …),
  so prompts and the 1.0 clients stay valid.

The trait compiles for wasm32 and for the apps' iOS and macOS targets. A
client can therefore check an edit, or preview a timeline drag, before
calling the server. The server still re-checks every change.

### Standard views: why new kinds cost nothing per surface

A card is an envelope (`lab.card.v1`) plus a kind-specific `detail`:

```text
{ kind, document, version, title, summary, state, refusal?,
  view: Video | Image | Compare | Model3d | Playable | Report | None,
  primary: artifact ref?, poster: artifact ref?, strip: [artifact refs],
  compare: { a, b, at }?, notes: [latest few], page: url, detail: {...} }
```

Each surface implements the **standard views once**:

| View | Web | Desktop (GPUI) | iPhone (UIKit) | Terminal |
| --- | --- | --- | --- | --- |
| Video | `<video>` with Range | `AVPlayerLayer` hosted in GPUI (macOS) | `AVPlayer` | Poster, state, link |
| Image, strip | `<img>` | GPUI image | `UIImageView` | Inline graphics where supported (after the freeze) |
| Compare | Side by side with one scrubber | Same | Swipe or split | Two posters and a link |
| Model3d (glTF) | The existing CAD WebGL viewer | wgpu (`crates/visual-gpu`) | SceneKit or Metal | Hero image and link |
| Playable | An iframe of the kind's page | Open the web page | Open the web page | Link |
| Report | Rendered fields | Same | Same | Text |

The next kind, a Photo Lab say, gets a working card everywhere as soon as it
declares `view: Image` or `Compare`. Its own `detail` is optional.
Unknown `detail` falls back to the envelope, and an unknown `view` falls
back to `Report` with a link. There is never a dead control.

## The API: `/v1/labs`

Served at `api.openagents.com/v1/labs` behind the same gate as Computers
while admission is limited. Specified in `docs/sdk/openapi.labs.json` and
added to the unified reference.

| Route | Does |
| --- | --- |
| `GET /labs/kinds` | Kinds this account may use, with manifests: schemas, tools, job types, views |
| `GET /labs/kinds/{kind}/tools` | The tool declarations for a model loop |
| `POST /labs/kinds/{kind}/tools/{name}` | Run one tool for a chat turn: `{space, chat?, arguments}` gives `{result, card?, refusal?}`. This is how model loops call Labs |
| `GET/POST /labs/documents?space=` | List or create (by kind and schema, or copy a version) |
| `GET/DELETE /labs/documents/{id}` | Read or delete (owner or editor) |
| `GET /labs/documents/{id}/versions` | Pages of versions |
| `POST /labs/documents/{id}/versions` | `{base, change, summary}` gives `201` with the version, `409` if `base` is stale, `422` with a refusal |
| `GET /labs/documents/{id}/versions/{n}` | Content, report, asset manifest |
| `GET /labs/documents/{id}/versions/{n}/card` | The card as of now |
| `GET/POST /labs/jobs`, `GET/DELETE /labs/jobs/{id}` | Request (idempotent), observe, cancel |
| `GET /labs/artifacts/{id}` | Metadata and a signed URL; the bytes support Range |
| `POST /labs/uploads` | Source files, through resumable project uploads (#11866) |
| `GET/POST /labs/documents/{id}/notes` | Notes with locators |
| `GET /labs/documents/{id}/events` | Server-sent events; `?since=` for polling |

It follows the [SDK contract rules](../sdk/README.md#contract-and-security-rules):

- Ownership comes from authentication. A key prefix never grants it.
- Accepting a job is not finishing it.
- One idempotency key covers each logical request.
- Payloads and pages are bounded.
- Errors are stable.
- Reading never starts a job.

Limits come from CAD's: arguments, document size, documents per space,
versions per document, runs per hour. Each kind declares them in its
manifest.

**Storage** (`lab-store`): the filesystem and SQLx PostgreSQL/Cloud Storage
backends, shared conformance checks, and reviewed CAD export copy command are
implemented in [`crates/lab-store`](../../crates/lab-store/README.md) (#11873).
API and CAD integration remain the next roadmap steps.


- **Postgres** for the index (documents, versions, jobs, notes,
  artifacts).
- **Cloud Storage** for content and artifacts, keyed by space. Version
  content is written once, so concurrent writers cannot both create
  version N.
- **A filesystem backend** with the same trait for loopback mode and tests
  (generalized from Robot Lab's `Store`).
- **CAD's existing chat-bucket layout** is read through an adapter until it
  is migrated. Nobody's models move without a checked copy.

**Who calls it.** Every product calls the API through the SDK, including
`openagents-web`'s Labs door, as #11806 does for Computers. The door keeps
choosing tools by the model's typed function choice. It builds its tool list
from `GET /labs/kinds`, takes the chat's current document from the last
card, and refuses a call that names another kind's document. The `Kind`
enum and prefix table become the registry.

## Jobs and placement

| Placement | What runs there | Used for |
| --- | --- | --- |
| Server child process | The kind's engine binary with a deadline and a memory limit, the way CAD runs today | Cheap, bounded jobs: CAD rebuild and export, Ship checks, short Game bot runs |
| Cloud worker | A Computers API computer (Boat, later SPACE) from a kind's worker image. Jobs are pulled, and the computer is deleted when idle | Video renders without private assets, long playtests, Robot simulation batches |
| Enrolled host | A worker on a person's own machine that **pulls** jobs over an outbound connection. Its device credential is limited to Lab jobs in named spaces | Anything that uses private local assets (MetaHuman, Fab packs, licensed fonts), the owner's GPU, a physical robot |
| Client | The kind's wasm or native engine on the device | Previews and exports the person makes locally (CAD's browser exports today) |

Placement is deterministic, from the job's requirements and the version's
asset manifest. Only an eligible placement may take a job. A job that needs
`private-local` assets goes only to an enrolled host that declares those
assets, and the worker uploads its outputs, never the inputs. With no
eligible host online, the job waits and says so. `no_training` is inherited
by every artifact made from a marked input, and dataset, trace and Gym
builders must refuse it. Usage and cost are recorded for each job and reuse
Computers usage (#11815) before access widens.

The enrolled host is a narrow job puller, not the general own-machine
computer (#11817). Robot Lab's paired-host physical controls (`remote.rs`,
with observe and operate permissions) stay their own protocol.

## Surfaces

| Surface | Reaches Labs through | Shows |
| --- | --- | --- |
| openagents.com chat | The SDK, from the server-side Labs door | Standard views in the card; `/labs/{kind}/{doc}` pages (CAD's `/cad/{doc}` stays and redirects later) |
| Saved history (all clients) | An additive `cards` field (`lab.card.v1`) on `/v1/history` messages, typed in the SDK | The same card, read-only, with fresh signed media |
| Desktop (`apps/openagents`) | `openagents::labs` from `coder-account`, by path | One Lab row type in the saved and live transcripts, the standard views, notes, compare. Kind editors come later (Video timeline first) |
| Phone (`apps/openagents`) | A small UniFFI facade over `openagents::labs` in `crates/mobile` | The same row type and views; notes by voice; chat edits |
| Coder 1.0.0 terminal (frozen) | A local `labs bridge`: one loopback listener with a path per kind, speaking the 1.0 routes and forwarding with the person's saved `openagents` login (`VIDEO_LAB_URL=http://127.0.0.1:8799/video`, `CAD_LAB_URL=…/cad`, `GAME_LAB_URL=…/game`, `ROBOT_LAB_URL=…/robot`) | Words and the page link, as today |
| Next Coder release | `openagents::labs` directly, tools from `/labs/kinds` | Inline posters and strips where the terminal supports graphics |

Live updates come from document events while a card is on screen. In saved
chats they come from the apps' existing two-second following. Readers never
start jobs. A job starts only from a tool call or a button the person
presses.

## Adding a kind

1. A crate implementing `LabKind`: schemas, `check`, `apply`, tools, job
   types, `card` with a standard view, and limits.
2. Job runners for its job types, with their placement requirements and a
   worker image if they leave the server.
3. Registration in the Labs registry (and a gate if it isn't for everyone).
4. Tests: refusal wording, idempotent jobs, the access matrix, the card on
   all four surfaces through the fixture renderer.
5. Optional: a `detail` renderer and a deeper page, without changing any
   surface for it.

A Photo Lab, for example, would be `image.v1` (source references plus an
ordered list of adjustments and generations), `render` jobs on a cloud
worker, and `view: Compare`. The current surfaces need nothing new.

## Roadmap and issues

Epic: #11871. Each step ships on its own. The owner's local loopback Labs
and Coder 1.0.0 keep working throughout.

| Step | Issue | Delivers | Depends on |
| --- | --- | --- | --- |
| L1 | #11872 (done) | `lab-kit`: common types, `LabKind`, locators, refusals, `lab.card.v1`, standard views, fixtures; builds for wasm32 and iOS | None |
| L2 | #11873 | `lab-store`: storage trait; filesystem, Postgres plus Cloud Storage, and CAD-layout adapter backends | L1 |
| L3 | #11874 | `/v1/labs` API, OpenAPI, access, limits, events, signed artifacts | L2 |
| L4 | #11875 | CAD as the first kind on the platform; `cad_web` moves onto it unchanged for users | L3 |
| L5 | #11876 | `openagents::labs` in the SDK, plus an external example and evidence | L3 (L4 for the CAD example) |
| L6 | #11877 | Lab cards in saved history (`cards` on `/v1/history`) and the Labs door on the registry | L4, L5 |
| L7 | #11878 | Jobs and placement: server, cloud worker, enrolled host, `no_training`, usage | L3 |
| V1 | #11879 | `video-lab` kind crate: rules out of `cut.py`, `lab.py` and `verse-film` (done) | L1 |
| V2 | #11880 | Video Lab on the platform: runners, enrolled-host renders, web pages, a loopback mode compatible with 1.0 | V1, L6, L7 |
| G1 | #11881 | Game Lab and Ship Lab as kinds | L4 |
| R1 | #11882 | Robot Lab documents on the platform (paired-host controls unchanged) | L4, L7 |
| D1 | #11883 | Coder Desktop: Lab row and standard views | L5, L6 |
| P1 | #11884 | iPhone: Lab row and standard views | D1 |
| T1 | #11885 | `labs bridge` for Coder 1.0.0, and the next-release entry | L5 |
| D2 | #11886 | Video timeline editor in Coder Desktop over `video-lab` | V2, D1 |
| Agent | [Proposed A1–A6 slices](../agent-lab/README.md#delivery-slices-and-exit-checks) | Agent drafts, bounded trials, comparison evidence, and explicit saving to the agent picker | Custom agents #11902; L2, L3, L5–L7 and shared views for hosted delivery |

## Owner decisions

1. **Private-asset renders in the cloud.** May a render (never the raw
   assets) that shows MetaHuman or Fab content be stored in the space's
   Cloud Storage and streamed to the phone? Recommendation: yes, private to
   the space and `no_training`.
2. **Access.** Recommendation: CAD keeps its current audience. Video, Robot
   and the new hosted paths start with the owner and dedicated test
   accounts behind the Computers gate. Cloud jobs widen only after usage
   and billing.
3. **Desktop video.** Recommendation: AVPlayer on macOS and iOS first.
4. **Coder 1.0 bridge.** Recommendation: for the owner and testers only,
   not advertised.

## Verification

Use the [SDK delivery rules](../sdk/delivery-plan.md#issue-handling-evidence-and-completion):

- Dedicated test accounts, never the owner's chats, documents or machines.
- Synthetic documents and media outside the owner's Mac.
- Affected crates' tests and formatting.
- Evidence JSON per step.
- Every test document, job, computer and upload cleaned up.

The platform core is a formal target, with Stateright on `lab-kit` plus the
store's version and job state machine:

- concurrent `apply` on one base;
- idempotent job creation under retry and lost responses;
- placement never sending `private-local` inputs off an enrolled host;
- access never crossing spaces through a cross-kind reference.

Every counterexample becomes a regression test.
