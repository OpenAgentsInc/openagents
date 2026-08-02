# External multi-device controller teardown — 2026-07-31

Naming note: this document uses the external project's proper name only when
source attribution or an exact upstream identifier requires it. It is not an
OpenAgents or Omega product, surface, mode, route, or feature name. The shipped
application and its primary interface are **Omega**.

Architecture and product audit of the public `zeronsh/comet` repository at an
exact commit in the local reference clone `~/work/projects/repos/comet`,
compared to the Omega desktop client (the tracked Zed fork at `~/work/omega`,
**not** the retired Electron OpenAgents Desktop app). The original 2026-07-31
audit was read-only: nothing in either tree was modified, and it did not build
binaries, start the edge Worker, run a harness against a live Claude or Codex
CLI, open a Durable Object room, or package a release. Section 7.4.9 is a
separate 2026-08-01 implementation update that records the first landed
source-informed Omega presentation slices and their local verification. [limitation]

The reference project is a multi-device controller for coding agents: start Claude Code or
Codex on one machine, follow and drive the same session from another, and keep
the engine running as a daemon after the laptop lid closes. The audited tree is
a ground-up **Rust + GPUI native rewrite** of an earlier TypeScript/Electron
product. The product name, install domain (`comet.zeron.sh`), and architecture
docs still call the rewrite "comet-native" in places; the public repository is
`zeronsh/comet`. [source] [public]

The comparison target is **Omega** — OpenAgents' primary desktop IDE and
workroom, selected by the owner as the Zed-fork destination surface (see the
[2026-07-23 Omega disposition](./README.md#2026-07-23-omega-disposition) and
the [App for All Work thesis](../allwork/README.md)). This report does not
re-open the retired Electron desktop path.

UI pixels and the input bar are covered in the companion
[external reference UI deep dive](./2026-07-31-external-reference-ui-deep-dive.md). That
report's product judgment: **The reference project's composer is the better agent input bar**;
Omega should match its density while keeping MessageEditor and disposition law.

Section 7.4 below evaluates the stronger counterfactual: **make the reference project's UI the
presentation starting point for Omega and rebuild the Omega workbench inside
it**. That is feasible, but it is a product-shell rewrite with explicit adapter
boundaries, not a Cargo dependency swap or a theme change. [source] [inferred]

## Executive decision

**The reference project is the strongest open multi-device agent-controller reference in the
catalog for a headed/headless split, durable offline-tolerant command queue,
and CRDT session docs over a cloud room fabric.** It is not an IDE, not a
receipt system, and not a model for OpenAgents production infrastructure.

Omega should **not** adopt the reference project's application graph wholesale, replace its
native agent router with the reference project's engine, pin the reference project's third-party Zed/GPUI fork,
or move session authority onto Cloudflare Durable Objects and WorkOS. Those
choices collide with Omega's IDE substrate, signed-device authority, and
Google Cloud production contract. A presentation-only rebuild that source-
ports the reference project's shell into Omega is technically coherent, but it is a large,
separately admitted product rewrite; §7.4 specifies that path. [inferred]

Omega **should** harvest selected control laws:

1. **Detach is not kill.** A viewport (desktop window or TUI) may quit while
   the engine keeps sessions, docs, and device presence. Reattach is cheap
   because the engine was never torn down.
2. **Durable command ledger, host-only execution.** Send, steer, interrupt, and
   respond-to-input are append-only doc entries. Offline devices queue. Only
   the chat's host device executes. Processed-ledger idempotence and
   supersede/TTL rules are explicit.
3. **One pure view-derivation module shared by every surface.** Sidebar sort,
   staleness gating, grouping, and boot gates live in `comet_proto::view` so
   the GPUI app and the TUI cannot disagree on row order.
4. **Spaces as the unit of organization.** A space is a synced
   `(deviceId, folder path)` pair. The workspace index is small; transcripts
   live in per-chat session docs. The sidebar needs one subscription, not N
   tiny rooms.
5. **Incremental mirror projection.** Diff-driven application into typed
   structs, not full re-hydration on every CRDT change — the reference project's documented
   fix for the prior O(transcript) re-projection cost.

Those lessons strengthen Omega's multi-device and portable-session path. They
do not replace Omega's editor, Project graph, front-door router, grants,
receipts, or owned cloud edge.

```text
recommended harvest (laws, not dependencies)

Omega front door + native loop + ACP attachments
      |
      +-- keep: editor / Project / Git / language / Zero Base
      +-- keep: device enrollment, signed grants, receipts
      +-- keep: Google Cloud / OpenAgents authority planes
      |
      +-- adapt from the reference project:
            detach-not-kill engine lifetime
            durable host-only command ledger
            pure shared view derivations
            space = device + folder index
            incremental session projection

not recommended

Omega process -> the reference project engine as second authority
Omega sessions -> Cloudflare SessionRoom / DeviceRoom DOs
Omega identity -> WorkOS as sole product gate
Omega UI -> pin wingleeio/zed GPUI fork beside OpenAgentsInc/omega
Omega default -> unattended auto-approve as product policy
```

## Summary

The reference project makes one bet: **the coding agent session should outlive any single
viewport and any single machine.** The engine owns harnesses, journals,
terminals, worktrees, and Loro docs. The UI is a viewport over typed RPC —
in-process when convenient, over localhost when a daemon already owns the
data, over a DeviceRoom relay when the host is another device. The edge is a
TypeScript Cloudflare Worker plus Durable Objects that store CRDT rooms and
relay device frames. Auth is WorkOS. [source]

```text
comet (headed) ── in-proc or localhost RPC ── engine A
                                                    ║
                              DeviceRoom DO (byte relay + nudges)
                                                    ║
comet-tui / phone / laptop UI ── RPC ── engine B (or attach to A)
                                                    ║
                         SessionRoom DO (Loro session doc per chat)
                         Workspace DO  (spaces / chats / devices index)
                              Cloudflare Worker + R2 + WorkOS JWKS
```

Omega makes a different bet: **one native application for all work** — editor,
agents, review, identity, and evidence in one GPUI process graph derived from
Zed, with optional paired devices and external ACP executors, under explicit
authority and signed device grants. Multi-device control is present as a
pairing bridge and enrollment store, not as a cloud CRDT room fabric. [source]

The products overlap on "drive Claude/Codex from more than one place." They
diverge on almost every authority and packaging choice. The reference project is a thin
controller around foreign CLIs. Omega is a full IDE that also attaches agents.

## 1. Snapshot, provenance, and limits

### 1.1 Exact source identity

| Artifact              | Identity                                                                                         | What it establishes                        |
| --------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------ |
| Public repository     | `https://github.com/zeronsh/comet`                                                               | Public native-rewrite source               |
| Local clone           | `~/work/projects/repos/comet`                                                                    | Audited tree                               |
| Audited commit        | `e5d8e9fb4c2ffe2350e4114db3bfd89979a2136d`                                                       | Exact snapshot                             |
| Commit time           | `2026-07-31T22:03:44+00:00`                                                                      | Freshness of the tip                       |
| Commit subject        | `Scrub Unpeel references from TUI test fixture`                                                  | Latest audited change                      |
| Workspace version     | `0.1.5` (`Cargo.toml` workspace.package)                                                         | Pre-1.0 train                              |
| License               | MIT (`LICENSE`, copyright 2026 Wing)                                                             | Permissive                                 |
| First commit in clone | `2026-07-19`                                                                                     | About twelve days of history at audit time |
| History scale         | 151 commits, two shortlog identities (Wing ~112, wing-anara ~46)                                 | Solo-led, high velocity                    |
| Language scale        | ~79k lines of Rust under `crates/` + `apps/`; ~2.7k lines edge TypeScript; ~9.1k lines iOS Swift | Mid-size controller product                |
| Install surface       | `https://comet.zeron.sh/install.sh` (Linux systemd user unit); macOS points at DMG               | Headless Linux first                       |
| Omega local clone     | `~/work/omega`                                                                                   | Comparison tree                            |
| Omega audited commit  | `acd0f5324a570ef8de19b188f93c5e487abe760b`                                                       | Current local `main` tip at audit          |
| Omega product claim   | "Your last IDE"; tracked Zed fork; bootstrap phase                                               | Full IDE + workroom                        |

Selected content digests at the reference project pin:

| File                | SHA-256                                                            |
| ------------------- | ------------------------------------------------------------------ |
| `README.md`         | `2e22d67359d6352bb35fa957baaa1b2e64590cf1c18aaaa183051eb0d23da58e` |
| `ARCHITECTURE.md`   | `4695fd191f1dfb89c136d8dc433ceb92420c65b4e2b317f3e80a8e0a170f1b80` |
| `docs/PARITY.md`    | `ccd009276606a72220c1574ec76e58bf01ee0cc0c1087a9e23eb82d666d27858` |
| `Cargo.toml`        | `e5b953549bca85b59f0b7c3831b53154c75cbdc9705e394580e2af956cd48ba4` |
| `edge/package.json` | `5803233d553eaede5cfa54e7aacb49a0667999a6d95065caa41c537434886237` |

Selected Omega digests at the Omega pin:

| File                                                        | SHA-256                                                            |
| ----------------------------------------------------------- | ------------------------------------------------------------------ |
| `README.md`                                                 | `8c11f9ec59c28738ebf6ab9186f8dc8df0bd0fdf595aee6540a8efadc04e5d6f` |
| `crates/omega_device_bridge/src/omega_device_bridge.rs`     | `74c35bffc574ecb1899b3994810838c1ca0742e23f1759f1c59908a474635cb9` |
| `crates/omega_device_enrollment/omega_device_enrollment.rs` | `63db8d3a9a0ae0d9670c1e8fbef9665a16b308a52f9cc7cd353536f13a9cd359` |
| `crates/omega_front_door/src/omega_front_door.rs`           | `4ae5c14003911da32755fb40703538e445a952c01452de9c72160e5f3a389115` |

### 1.2 Evidence labels

- **`[source]`** — tracked source, docs, or manifests at the audited commit.
- **`[schema]`** — typed Rust, TypeScript, or wire contracts.
- **`[test]`** — an executable test, smoke script, or CI gate present in source.
- **`[history]`** — Git history at or before the audited commit.
- **`[public]`** — a linked public page or repository description.
- **`[implementation]`** — source landed on Omega `main` after the audit pin.
- **`[runtime]`** — a bounded local build or launch observation, not release
  or production evidence.
- **`[inferred]`** — reasoned from several observations.
- **`[limitation]`** — a boundary on what this audit can prove.

### 1.3 Audit limits

This was a source audit. It does not prove live Durable Object hibernation,
WorkOS production behavior, harness correctness against current Claude/Codex
CLIs, iOS App Store readiness, macOS notarization, or cryptographic security
of the room protocol under an adversarial network. The original TypeScript
comet tree that this rewrite replaces was not cloned. The reference project's PARITY matrix
claims live Claude CLI verification; this audit treats that as author
testimony encoded in docs, not as a re-run. [limitation]

## 2. What the reference project is

### 2.1 Product thesis

The README positions the reference project as: control Claude Code and Codex from any of your
devices. Every device runs a small engine that keeps sessions in sync. Install
the engine as a daemon on an always-on machine so agents keep working after
the laptop closes. Day-to-day CLI: `comet login`, `comet status`,
`comet update`, `comet tui`, `comet daemon …`. [source]

### 2.2 Architecture pillars

From `ARCHITECTURE.md` and the crate layout:

| Pillar         | Implementation at the pin                                                                         |
| -------------- | ------------------------------------------------------------------------------------------------- |
| Sync           | Loro CRDT docs (`loro` 1.13) through Cloudflare Durable Objects                                   |
| Edge language  | TypeScript Worker + DOs (deliberate; device side is Rust)                                         |
| UI             | GPUI pinned to a `wingleeio/zed` rev; **no** Zed GPL crates (`markdown`, `ui`, `theme`, `editor`) |
| Binary model   | One `comet` binary, headed or headless; separate `comet-tui`                                      |
| Feature parity | Against the prior Electron product, excluding token-usage display                                 |
| License        | MIT throughout the rewrite                                                                        |

[source] [schema]

### 2.3 Crate map

| Crate / path     | Role                                                                                                    |
| ---------------- | ------------------------------------------------------------------------------------------------------- |
| `crates/proto`   | Wire types + shared pure `view` derivations                                                             |
| `crates/doc`     | Session + workspace doc schemas, command ledger, parts privacy                                          |
| `crates/sync`    | Loro room client, DocsStore (SQLite snapshots + processed ledger)                                       |
| `crates/harness` | Harness trait; Claude stream-json; Codex app-server; mock                                               |
| `crates/engine`  | Sessions, doc host, repos/worktrees, terminals, auth, device-room host                                  |
| `crates/rpc`     | Typed UiRpc/ControlRpc over WS + in-memory duplex + device frames                                       |
| `crates/ui`      | GPUI shell, transcript, composer, terminal, changes, settings                                           |
| `crates/tui`     | ratatui viewport; never embeds an engine                                                                |
| `crates/update`  | Self-update support                                                                                     |
| `apps/comet`     | Headed default + `headless` subcommand + daemon install                                                 |
| `apps/tui`       | `comet-tui` binary                                                                                      |
| `apps/ios`       | Native SwiftUI controller (exists in tree; PARITY still lists mobile as deferred for the rewrite scope) |
| `edge/`          | Worker, SessionRoom DO, DeviceRoom DO, R2 attachments, WorkOS auth                                      |

Approximate Rust line counts at the pin: engine ~17k, ui ~34k, tui ~12k,
harness ~5k, doc ~3k, sync ~2.5k, rpc ~2.5k, proto ~1.5k. [source]

### 2.4 Milestone posture

ARCHITECTURE marks M0–M4 shipped and M5–M6 partial. `docs/PARITY.md` is the
finer checklist. Claimed live proofs include: two headless engines through a
real edge with mock harness (`scripts/e2e-smoke.sh`), and a real Claude CLI
2.1.215 run through the doc-queued host executor. Cursor harness is deferred.
E2EE of doc contents is deferred. macOS packaging config exists under
`dist/macos/` but packaging was not executed in-tree. [source] [test]

## 3. Control plane laws worth keeping

### 3.1 Headed, headless, and detach

- Headed `comet` attaches to an existing local engine if the IPC port answers;
  otherwise it runs the engine in-process **and** serves that engine on the
  IPC port so other viewports can attach without a daemon restart.
- `comet headless` is engine-only: sign-in URL on TTY, IPC on localhost, hosts
  its DeviceRoom for remote control.
- `comet-tui` **never** embeds an engine. It attaches to whatever answers, or
  spawns `comet headless` under `setsid`. Quitting the TUI detaches; SIGHUP
  hits the viewport, not the engine. [source]

This is the cleanest detach contract in the teardown catalog for a coding
agent controller. Omega's desktop process still tends to couple UI lifetime to
work lifetime for local runs. Portable and unattended work in Omega needs the
same lifetime split: a supervised engine (or effectd) that outlives any one
window, with viewports that reattach. [inferred]

### 3.2 Durable command ledger

`crates/doc` ports the prior session-doc command rules:

1. Append-only per-device command entries (`run`, `steer`, `interrupt`,
   `respondInput`).
2. Host-only outcomes (with a narrow cancel exception for the issuing
   composer).
3. Evaluation with processed-ledger dedupe, TTL, and supersede rules (newer
   steer/interrupt supersedes older pending ones; interrupt for a finished
   turn is moot).

Run/steer/interrupt are **not** fire-and-forget device RPCs. They ride the
doc so offline queues work and multi-device writers do not race the host
executor. The host marks processed **before** execute. [source] [schema]
[test]

Omega's send-during-turn / steer disposition work in `omega_front_door` is the
right product vocabulary for local UI law. The reference project shows how to make those
gestures **durable across devices and restarts** without inventing a second
queue. The harvest is the ledger shape and host rule, not Loro itself.
[inferred]

### 3.3 Session doc vs workspace doc

- **Session doc** (per chat): transcript + command queue. Message bodies are
  LoroText (measured oplog shape). Continuations split at 256KB. Render-only
  tool parts strip full Write/Edit inputs from the synced doc; full inputs
  stay in the host run journal.
- **Workspace doc** (per org): spaces, chats index, devices, session status
  rows, checkout-diff summary pointers. Small on purpose so one subscription
  feeds the sidebar. Presence uses Loro `EphemeralStore` instead of heartbeat
  writes into durable fields. [source] [schema]

### 3.4 Shared pure view module

`comet_proto::view` owns sort orders, staleness gating, sidebar grouping, and
the boot gate. Both frontends call it. Architecture text states the goal
explicitly: row order must never diverge per surface. [source]

Omega already fights multi-surface projection drift (desktop Zero Base,
mobile activity mirror, device bridge). A pure shared derivation crate for
thread list order, staleness, and attention is the same law. [inferred]

### 3.5 Spaces

A space is a synced device+folder pair. Git presence is stamped by the owning
device so branch pickers and the changes pane gate on a bool without an RPC.
`deleteSpace` tombstones the space and every chat/session row in one commit.
[source]

This is a useful product unit for multi-host work: not "project abstractly,"
but "this folder on this machine." Omega's Project graph is richer for an IDE.
The space concept is still the right **remote-control index** when the user
has many machines. [inferred]

## 4. Harness and containment

### 4.1 Adapters

| Harness     | Mechanism at the pin                                                                    | Status                              |
| ----------- | --------------------------------------------------------------------------------------- | ----------------------------------- |
| Claude Code | `claude` CLI stream-json; AskUserQuestion → requestInput; steering via persistent input | done; live-verified claim in PARITY |
| Codex       | `codex app-server` JSON-RPC; thread/start/resume; sandbox policy                        | done                                |
| Cursor      | deferred                                                                                | no settled CLI surface              |
| Mock        | scripted event replay                                                                   | powers tests and e2e smoke          |

[source] [test]

### 4.2 Containment posture

Codex comments and catalog mapping show workspace-write and danger-full-access
sandbox modes. Linked worktrees with `/` in the branch path force an automatic
escalation to `danger-full-access` because Codex's workspace-write sandbox
mangles those paths. Sessions are described as unattended: approval policy
auto-approves; the sandbox is treated as the containment boundary. [source]

That is a product choice Omega must not copy as a default. Omega's authority
model and OpenAgents own-capacity runbooks already treat full-access execution
as an explicit owner-local mode with receipts, not as silent controller
policy. Harvest the harness adapter patterns; reject silent escalation as
product law. [inferred]

### 4.3 Agent accounts

Credential-slot swap (macOS Keychain via `security-framework`, files
elsewhere), usage probes, paste-code and browser-poll OAuth, plan labels, and
a device switcher for which machine's logins are shown. Rate-limit meters are
kept; token-usage heatmaps are deliberately excluded from CRDT sync. [source]

## 5. Edge, identity, and privacy

### 5.1 Edge

The edge is Cloudflare Workers + Durable Objects + R2:

- SessionRoom DO: hibernatable WebSocket, update log, snapshot, two-level
  compaction, daily alarm checkpoint/trim/R2 backup, version-vector backfill.
- DeviceRoom DO: single host socket, byte-pipe frames, durable nudges with
  replay-on-join, sidecar slots.
- Attachments: content-addressed R2, hash-verified.
- Auth: WorkOS JWKS; workspace rooms authorize on org claim; claim-on-first-
  join ownership for session rooms; dev mode `user@org` bearers. [source]

OpenAgents production infrastructure authority is **Google Cloud only**.
Cloudflare Workers, Durable Objects, D1, R2, and Wrangler are retired for
OpenAgents product deploy paths. The reference project's edge is excellent evidence for
**room lifecycle laws** (hibernation hygiene, nudge durability, compaction,
disaster backup). It is not an admissible deploy target for Omega or
openagents.com. [source] [inferred]

### 5.2 Privacy gaps that matter

- **No E2EE** of doc contents. Transport is TLS + WorkOS bearers. Relays and
  operators who can read DO storage can read transcripts. PARITY lists E2EE as
  deferred. [source]
- **Render-parts stripping** is real and good: full tool inputs stay on the
  host journal. That is privacy relative to the synced doc, not end-to-end
  confidentiality. [source]
- **Dev auth** collapses to a configured user id when no key is present —
  useful for local development, catastrophic if shipped as a default. [source]

Omega's device bridge and enrollment path use signed proofs, pairing secrets,
capability grants, and local store fences. That is a different trust model:
pair devices cryptographically, project bounded surfaces, keep authority on
the enrolled host. The reference project optimizes for "sign into org, rooms just work."
Omega optimizes for "prove the device and the grant." Both are coherent.
Only the second matches the OpenAgents identity direction. [source]
[inferred]

## 6. Surfaces

| Surface         | Stack                                    | Notes                                                                                                                                                      |
| --------------- | ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Desktop         | GPUI app in `crates/ui`                  | Always-dark monochrome; Geist fonts; virtualized transcript; motion kit ported from prior product timings                                                  |
| TUI             | ratatui in `crates/tui`                  | ~12MB target vs ~60MB headed claim; no-tick coalescing event loop; fingerprinted transcript cache                                                          |
| Headless daemon | `comet headless` + systemd/launchd units | Linux install script first-class; macOS build-from-source / DMG path                                                                                       |
| iOS             | SwiftUI under `apps/ios`                 | Room client, workspace store, device relay client, composer, transcript; ~9k Swift lines present despite "mobile deferred" language in ARCHITECTURE/PARITY |

[source]

The reference project is **not** an editor. There is no Project service, no LSP, no multi-root
worktree IDE model, no built-in merge editor. The changes pane is a patch
viewer over checkout diffs. Terminals are session-scoped PTYs with replay.
That is enough for supervising a CLI coding agent. It is not enough for
"the application for all work." [source] [inferred]

## 7. Omega comparison

### 7.1 Product shape

| Dimension           | Reference project (pin above)                                    | Omega (pin above)                                                              |
| ------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------ |
| Category            | Multi-device coding-agent controller                 | Native IDE + workroom (Zed fork)                                               |
| Default surface     | Session list + transcript + composer                 | Zero Base thread + full editor substrate available                             |
| Primary runtime     | Thin Rust engine + foreign CLIs                      | Full GPUI app graph + native agent + ACP                                       |
| Multi-device fabric | Cloudflare DO rooms + Loro docs + WorkOS             | Device bridge WebSocket + enrollment grants + optional Nostr                   |
| Session durability  | CRDT session doc + SQLite snapshots + run journal    | Multiple durable stores; composition still the gap (see Omega/T3 gap analysis) |
| Authority           | Org membership + host device ownership of chats      | Front-door router owns no execution; grants, receipts, signed identity         |
| Containment default | Harness sandbox; unattended auto-approve posture     | Explicit lanes, permissions, and evidence work in progress                     |
| License             | MIT                                                  | GPL-3.0-or-later with Apache-2.0 components                                    |
| Production edge     | Cloudflare                                           | Google Cloud (OpenAgents contract)                                             |
| Mobile              | iOS tree present; product maturity unclear           | Activity mirror; stronger design than shipped control (T3 comparison)          |
| Scale               | ~11 crates + edge + iOS; ~2 weeks of rewrite history | ~225 crates; long Zed history + Omega overlays                                 |

[source] [inferred]

### 7.2 Multi-device specifically

The reference project's multi-device path is **cloud-room-first**:

1. Engines join DeviceRoom and SessionRoom through the edge.
2. A remote device writes a `QueueCommand` into the session doc.
3. A durable nudge wakes the host if the chat was cold.
4. The host executes and streams transcript back into the same doc.
5. Any viewport subscribed to the doc sees progress.

Omega's multi-device path is **pair-and-project**:

1. Host advertises pairing bootstrap (MagicDNS/Tailscale-oriented fields,
   host public key, pairing secret, generation, expiry).
2. Device enrollment stores grants, capabilities, revocation, and local
   credentials under a fenced account store.
3. Device bridge protocol `openagents.omega.device_bridge.v1` projects a
   bounded set of threads/runs/transcript bytes to the paired device with
   heartbeats and 100ms delta flush.
4. Control remains host-centered; the phone is not yet a complete controller
   in the T3 sense.

The reference project is ahead on **seamless multi-host session continuity and offline
command queuing**. Omega is ahead on **cryptographic device admission,
capability grants, and IDE-side work depth**. The product synthesis is not
"copy the reference project's cloud." It is "give Omega the reference project's lifetime and ledger laws on
Omega's trust rails." [source] [inferred]

### 7.3 GPUI note

Both products use GPUI. The relationship is not "the reference project uses Omega." the reference project pins
GPUI (and only GPUI/platform/tokio packages) from `wingleeio/zed` at a fixed
rev, deliberately avoiding Zed's GPL UI/editor crates. Omega **is** the
OpenAgents tracked Zed fork and owns the full application. Sharing a UI
toolkit does not share product authority, release identity, or license
posture. Do not treat the reference project as a lightweight Omega. Do not vendor the reference project's
GPUI pin into Omega. [source]

### 7.4 Counterfactual: rebuild Omega with the reference project's UI first

This section answers a deliberately stronger question than the harvest
recommendation above: **what would be required for the reference project's UI, rather than
Omega's current Workspace and AgentPanel presentation, to become the primary
desktop experience?**

The technically coherent interpretation is:

- the reference project owns the window composition, visual system, navigation density,
  session sidebar and tabs, transcript, composer chassis, pickers, motion,
  terminal/diff presentation, and settings vocabulary.
- Omega continues to own execution and product authority: Project, Editor,
  language services, Git state, tasks, terminals, native/ACP agent sessions,
  front-door routing, permissions, identity, devices, grants, receipts, and
  the Google Cloud production contract.
- A new presentation adapter projects Omega entities into the small view model
  the reference project's components expect and translates UI intents back into existing Omega
  actions. It does not create a second session store, router, engine, or RPC
  authority.

This is **source-informed presentation on Omega-first authority**. Forking the reference project as
the new application and then importing Omega's backend is the wrong direction:
Omega's IDE substrate is the larger and more interconnected graph, while
The reference project's presentation is the smaller replaceable layer. [source] [inferred]

#### 7.4.1 Why `comet-ui` cannot simply be added as a dependency

The shared GPUI ancestry makes the rewrite possible, but it does not make the
crates plug-compatible.

1. **GPUI type identity differs.** the reference project pins `gpui`, `gpui_platform`, and
   `gpui_tokio` from `wingleeio/zed` at one Git revision. Omega builds the
   in-tree GPUI crates in its tracked Zed fork. Two `Entity<T>`, `Window`,
   `App`, `Element`, and action types compiled from different GPUI packages are
   different Rust types and cannot share an element tree. The reference project UI source must
   be rebased onto Omega's in-tree GPUI API; Omega must not carry a second GPUI
   runtime. [source]
2. **The UI crate is backend-coupled.** `comet-ui` directly depends on
   `comet-proto`, `comet-doc`, `comet-engine`, `comet-rpc`, and `comet-update`.
   Its `AppState` owns `EngineHandle`, probes a daemon, speaks the reference project RPC, and
   projects the reference project `Space`, `Chat`, `Session`, and Loro message types. Those are
   not neutral presentation contracts. [source]
3. **The shell assumes a controller, not an IDE.** the reference project's center is one
   transcript; its right pane is changes; its bottom surface is a
   session-scoped terminal. Omega's `Workspace` owns pane groups, project
   items, editors, docks, toolbars, collaboration state, tasks, debug state,
   and multi-window persistence. Replacing `Workspace::render` removes the
   composition point through which most IDE surfaces appear. [source]
4. **The domain nouns do not line up.** A the reference project space is `(device, folder)`;
   an Omega project may have multiple worktrees, remote roots, language
   servers, buffers, tasks, and panes. A the reference project chat has one host harness;
   Omega's thread may route to native, ACP, Exo, or other executors with
   explicit send disposition and authority disclosure. A field rename cannot
   reconcile those semantics. [source]
5. **Several apparently reusable controls replace mature Omega substrates.**
   the reference project owns a hand-rolled input, Markdown renderer, terminal grid, diff
   viewer, menus, and settings. Omega already has Editor-backed composition,
   accessibility and keymaps, a mature terminal, Git UI, theme extensions,
   and settings infrastructure. Literal replacement creates feature debt that
   is invisible in a screenshot. [source] [inferred]

The required operation is therefore a **source port plus inversion of
dependencies**: preserve the reference project's presentation laws, replace its state root and
service calls, and compile every imported element against Omega's GPUI.

#### 7.4.2 Scope: what “all of the reference project UI” would mean

The reference project's core shell, state, composer, transcript, pickers, attachments, changes,
terminal, motion, and theme modules are roughly 21k Rust lines at the audited
pin, before the remaining Markdown, settings, popover, rail, loader, icon, and
support modules. The corresponding Omega presentation roots are not one
replaceable crate: `AgentPanel`, `ConversationView`, `ThreadView`,
`MessageEditor`, and `Workspace` alone are more than 77k lines, before Editor,
project panel, terminal, Git, search, debugger, settings, and titlebar UI.
[source]

| Reference-project surface                                         | Source-informed Omega treatment                                                       | What must remain Omega-owned                                                                                   |
| ----------------------------------------------------- | --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Window, frost shell, titlebar, sidebar, session tabs  | Port as the new outer composition and navigation language                         | Window persistence, multi-window Project identity, platform menus, release channel                             |
| Spaces and active-session list                        | Present projects/worktrees/devices through the reference project rows and attention sorting       | `Project`, `WorktreeStore`, remote projects, device grants; do not collapse them into the reference project docs               |
| Composer, picker row, attachments, question wizard    | Port the chassis, geometry, state motion, drafts, pickers, and wizard interaction | `MessageEditor` capabilities, mentions/context, voice, executor disclosure, `SendDisposition`, ACP elicitation |
| Transcript, Markdown, tool groups, message rail       | Port the row model, density, streaming veil, folding, and stick spring            | Omega thread/session entities, ACP part semantics, receipts, queue state, tool authorization                   |
| Changes pane                                          | Port the information architecture and visual treatment                            | Omega Git store, buffer diffs, staging, conflicts, worktree mutations                                          |
| Terminal panel                                        | Port panel geometry, tabs, drag/resize behavior, and visual treatment             | Omega terminal entities, task terminals, remote PTYs, shell integration                                        |
| Settings, accounts, devices, archived sessions        | Port layout and component vocabulary                                              | Omega settings schema, identity, providers, device enrollment, update and account authority                    |
| Theme, icons, popovers, loaders, motion               | Port into an Omega-owned presentation crate and token layer                       | Theme compatibility policy, accessibility settings, platform conventions                                       |
| the reference project `AppState`, engine bootstrap, RPC, auth, update | **Do not port as application authority**                                          | Replace with Omega projections and command adapters                                                            |

“All” should mean that the visible product grammar comes from the reference project, not that
every the reference project implementation is retained when Omega already has the stronger
primitive. The highest-value example is the composer: a source-informed product
can put Omega's Editor-backed `MessageEditor` inside the reference project composer
chassis. Porting `ComposerInput` literally would require rebuilding mentions,
project context, creases, keymap behavior, accessibility, voice, and rich paste
on top of the reference project's input before parity. The former preserves the reference project's UX; the
latter preserves more the reference project source but is a worse migration. [inferred]

The same rule applies to terminal and diff: retain Omega's data/control
entities and render them through source-informed containers before considering a
replacement of their low-level emulators or models.

#### 7.4.3 Target architecture

The port needs a narrow presentation model between the reference project-derived views and
Omega's large entity graph:

```text
omega binary and application initialization
                |
                v
OmegaShell (ported the reference project composition, Omega GPUI)
  sidebar | tabs | workbench slot | transcript | composer | panes | settings
                |
                v
OmegaPresentationState + typed UI intents
  ProjectSummary, WorkbenchTab, ThreadRow, MessageRow, RunState,
  ComposerState, DiffSummary, TerminalTab, IdentitySummary
                |
       +--------+---------+----------------+----------------+
       |                  |                |                |
       v                  v                v                v
 Workspace/Project   Agent/FrontDoor   Git/Terminal   Identity/Devices
 Editor/Pane graph   ACP/native runs   Tasks/Debug    Grants/Receipts
```

The new layer should have four explicit contracts:

1. **Shell host.** Read-only projections for projects, worktrees, devices,
   windows, tabs, attention state, and navigation; intents for open, close,
   archive, rename, switch, move, and create.
2. **Conversation host.** Stable message rows, streaming deltas, run state,
   queue/disposition, pending elicitation, attachments, and intents for send,
   steer, enqueue, stop, respond, retry, and approve. This is where the reference project's
   simple Send/Steer/Stop control is reconciled with Omega's total disposition
   law rather than silently overriding it.
3. **Workbench host.** A mount point for existing Omega `ItemHandle`/pane
   entities plus commands for editors, search, tasks, Git, terminal, debugger,
   and project panel. The reference project has no equivalent; this slot is the central new
   design work required to remain an IDE.
4. **Platform host.** Menus, keymaps, update status, auth/identity gates,
   settings, notifications, accessibility, and window lifecycle.

`OmegaPresentationState` must be a projection, not a second database. Existing
Omega entities remain canonical. Subscriptions calculate small immutable row
models and notify only the affected GPUI views. UI commands call existing
Omega actions/services and return typed success or failure to the shell. No
The reference project Loro doc, `EngineHandle`, or WebSocket loop is required for a local
Omega window. [inferred]

For upstream hygiene, the port should live in one obvious presentation area
(illustrative names: `omega_ui` plus `omega_ui_model`), retain MIT
provenance for imported the reference project files, and record the exact upstream commit.
The reference project updates should arrive as reviewed source-port commits, not as an
unbounded Git dependency. Once imported into Omega's GPL application, the
distributed combined work remains subject to Omega's license obligations.
[source] [inferred]

#### 7.4.4 Migration sequence

A flag-day replacement is not credible. The safe path is a strangler migration
with one window able to select the current or source-informed shell during the
transition.

| Phase                            | Work                                                                                                                                                                                  | Exit criterion                                                                                                                |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| 0. Contract and provenance spike | Pin the reference project source snapshot; inventory assets/licenses; define view models, intents, and non-negotiable Omega workflows; establish the feature flag and comparison scenes           | A written boundary test proves there will be one authority graph and one GPUI runtime                                         |
| 1. Presentation foundation       | Port theme tokens, fonts, icons, popovers, loaders, frost/opaque platform behavior, motion helpers, reduced-motion handling, and component states onto Omega GPUI                     | Component gallery passes macOS/Linux/Windows rendering and keyboard/focus checks                                              |
| 2. Shell skeleton                | Port window composition, sidebar, session tabs, navigation history, resize/collapse behavior, and settings route; mount a placeholder workbench slot                                  | Projects and threads can be navigated without mutating duplicate state                                                        |
| 3. Vertical agent slice          | Connect one real Omega thread to the reference project transcript and composer chassis; map optimistic sends, failures, live-run control, queue disposition, attachments, and common elicitations | A native and an ACP thread complete send/steer-or-enqueue/stop/question flows with honest executor disclosure                 |
| 4. IDE workbench                 | Mount Editor/pane entities; add project/worktree navigation, editor tabs, project panel, search, Git changes, terminal/tasks, and drag/drop; preserve command routing                 | A person can open a repository, edit, search, run a task, review a diff, and drive an agent without entering the legacy shell |
| 5. Secondary surfaces            | Rebuild settings, identity/onboarding, providers, devices, voice, debugger, notifications, command palette, remote projects, and remaining docks in the reference project vocabulary              | The parity ledger has no P0/P1 workflow that requires the old shell                                                           |
| 6. Hardening and cutover         | Performance, accessibility, IME, multi-window restore, crash/reopen, platform packaging, migration telemetry, visual regression, rollback drills                                      | source-informed is default for a release train; legacy shell remains a kill switch for at least one train                         |
| 7. Retirement                    | Remove duplicate presentation paths only after real-world parity and rollback confidence                                                                                              | Legacy UI state and visual fixtures are deleted without deleting canonical Omega services                                     |

The first meaningful proof is not a static shell screenshot. It is a single
vertical slice containing **one project, one real editor tab, one real agent
thread, the reference project composer/transcript, one terminal, and one diff**. That slice
forces focus routing, Project identity, run authority, and workbench embedding
to meet before the migration expands. [inferred]

#### 7.4.5 Hard problems and required decisions

| Problem                         | Required decision / mitigation                                                                                                                                                                              |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GPUI/API drift                  | Port onto Omega's in-tree GPUI and add compatibility helpers locally; never compile two GPUI revisions into the window graph                                                                                |
| Shell ownership                 | Decide that the reference project composition replaces `Workspace::render` only after the workbench host can mount existing pane/item entities; until then run behind a window-level flag                                   |
| Composer fidelity vs capability | Keep the reference project geometry and interaction reducer, but use MessageEditor as the text engine unless the project explicitly funds reimplementation of every rich-input feature                                      |
| Transcript model mismatch       | Define a stable presentation enum for user/assistant/tool/error/elicitation/receipt rows; preserve unknown ACP parts and authorization state rather than flattening them to Markdown                        |
| Actions and focus               | Build an action-routing map before visual work: global, shell, workbench, editor, terminal, and composer contexts need deterministic precedence                                                             |
| Theme policy                    | Decide whether “full reference-UI parity” means always-dark Geist or the reference project geometry under Omega themes. Supporting both materially expands token and contrast testing                                                   |
| Accessibility                   | Audit every imported custom control for semantic role, name, focus order, screen-reader updates, reduced motion, high contrast, IME, and keyboard-only operation; Omega cannot inherit parity by appearance |
| Platform behavior               | Specify titlebars, menus, shortcuts, blur/opaque surfaces, file dialogs, notifications, and packaging separately for macOS, Linux Wayland/X11, and Windows                                                  |
| State duplication               | Forbid UI-local canonical copies of threads, projects, permissions, or identity. Draft text and ephemeral animation state are local; durable product state is not                                           |
| Upstream synchronization        | Choose a finite policy: snapshot-and-own, or periodically port selected upstream UI commits. A permanent free-flowing fork will make every GPUI update a three-way merge                                    |
| Release rollback                | Keep the old shell selectable at process start and ensure both shells read the same stores; rollback must not require data migration                                                                        |

The most dangerous failure mode is a beautiful controller shell that quietly
narrows Omega into the reference project's product model. Every time a the reference project noun is simpler
than an Omega noun, the adapter must preserve the Omega capability and reveal
it progressively; it must not erase the capability to make the row model
cleaner. Consistency is an affordance, but false simplicity is lost authority.
[inferred]

#### 7.4.6 Verification and release gates

The migration needs a parity ledger, not subjective “looks like the reference project” review.
Minimum gates:

- **Visual:** reference scenes at narrow, default, and wide widths for empty,
  loading, long transcript, live run, elicitation, failure, diff, terminal,
  settings, onboarding, and multi-pane workbench; explicit opaque Linux/Windows
  baselines as well as macOS glass.
- **Interaction:** keyboard-only navigation, command palette, focus restoration,
  drag/drop, resize, fullscreen, multi-window, draft persistence, queue edits,
  Send/Steer/Stop behavior, and rollback to the legacy shell.
- **Input/accessibility:** IME composition, marked text, screen readers,
  semantic roles and names, contrast, high-DPI scaling, reduced motion, large
  text, clipboard images/files, and every MessageEditor mention/context path.
- **Authority:** the front door still owns no execution; executor selection and
  fallback remain disclosed; permissions, grants, receipts, and device fences
  are unchanged; no the reference project store becomes canonical.
- **Performance:** first paint, idle wakeups, long-transcript scrolling,
  streaming frame time, row invalidation, editor typing latency, memory with
  several projects, and resize behavior are measured against current Omega.
- **Durability:** window/project/thread restore, crash during a live run,
  reconnect, remote project loss, draft recovery, and downgrade to the legacy
  shell do not corrupt state.

#### 7.4.7 Planning range

These are planning ranges, not source facts. They assume experienced GPUI/Rust
engineers, reuse of Omega's service entities, and no simultaneous rewrite of
the execution or cloud planes.

| Milestone                                            | Approximate effort               | What it buys                                                                                               |
| ---------------------------------------------------- | -------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Boundary spike + visual shell prototype              | 6–10 engineer-weeks              | Validates GPUI rebase, adapters, workbench mount, licensing, and one comparison scene                      |
| Agent-first alpha                                    | 18–30 cumulative engineer-weeks  | reference shell + real Omega threads, composer, transcript, basic project/editor slot, terminal/diff           |
| Daily-driver beta                                    | 45–75 cumulative engineer-weeks  | Core IDE workflows, settings/identity, multi-window, accessibility, platform and persistence hardening     |
| Full Omega presentation parity and legacy retirement | 80–120 cumulative engineer-weeks | Long-tail panels, remote/debug/task/provider flows, release confidence, deletion of old presentation paths |

With a stable team of four to five engineers, the credible calendar range is
roughly **six to nine months for full daily-driver cutover**, followed by a
long-tail parity train. A one- or two-person effort can produce the agent-first
surface, but cannot honestly replace the full Omega UI on the same calendar.
The dominant cost is not repainting controls; it is preserving the IDE's
actions, focus, state, accessibility, and authority inside a shell designed for
a smaller controller. [inferred]

#### 7.4.8 Conditional recommendation

If the product decision is genuinely “the reference project UI first,” proceed as a
presentation fork **inside Omega**, not as an Omega backend transplanted into
the reference project application and not as a runtime dependency on the reference project's engine. Start
with the vertical slice above, require one GPUI runtime and one authority graph,
and keep rollback until parity is demonstrated.

This does not change the control-plane disposition: do not import the reference project's
Cloudflare rooms, WorkOS authority, Loro stores, or harness engine. It does
revise the blanket visual conclusion. Copying isolated styling is not the only
coherent option; a source-informed Omega presentation is possible and may produce
a calmer product, but it must be funded and governed as an **Omega UI rebuild**,
not described as a reskin. [inferred]

#### 7.4.9 Implementation update — 2026-08-01

The counterfactual is no longer only a planning exercise. Omega now has a
primary interface and the first agent-first vertical slice. The current
implementation is deliberately narrower than the
full migration described above: it ports the window grammar, conversation
surface, and interaction density while keeping Omega's existing entities,
router, editor-backed composer, thread store, and action system authoritative.
[implementation]

The implementation snapshot for this update is Omega `main` at
`989adfac0b35bbc85817b64f9348944ed9195d95`. The changes landed as a sequence
of small commits rather than one source import:

| Omega commit | Landed outcome                                                                                                   |
| ------------ | ---------------------------------------------------------------------------------------------------------------- |
| `ab0ce4916e` | Reworked the normal Omega agent composer toward the reference project's compact chassis and control density.                     |
| `31a229af1a` | Removed the new-thread/existing-thread composer size divergence and updated the visual fixtures.                 |
| `fdb19ff03a` | Added sealed primary Omega interface without making it the default Omega presentation.                          |
| `2949da292d` | Made clean Omega profiles wait for the real composer instead of exposing an intermediate empty state.            |
| `2d082a20bd` | Ported the reference project window composition, titlebar, sidebar, session tabs, and main conversation card.                |
| `77f1c87785` | Replaced the provisional gray shell with Omega's Khala blue theme vocabulary.                                    |
| `ffcfd42660` | Updated the structural delta contracts for the Omega shell and Khala styling.                                    |
| `ed42dfd346` | Matched the reference project composer more closely, including the integrated executor/model picker treatment.               |
| `d3f9a9abe4` | Added primary-interface message rows and grouped tool-call rendering while retaining Omega's font and thread semantics. |
| `ffec0f0124` | Added active-session-tab close behavior and platform key bindings (`Command-W` on macOS, `Ctrl-W` elsewhere).    |
| `5901977b17` | Replaced decorative Spaces with real Omega Projects and guaranteed an agent working directory before launch.     |
| `8a91d98c51` | Ported the reference project's settings-navigation shape around Omega's real API Keys and Voice settings pages.                  |
| `b153c6838a` | Made the titlebar Back and Forward controls walk browser-style session history.                                  |
| `989adfac0b` | Seeded navigation history from the restored active session on the first Omega render.                            |

[history] [implementation]

##### Launch boundary and zero-base baseline

The primary Omega interface is a one-way process-start selection inside the existing Zero Base
seal. `omega_zero_base::enable_primary_interface()` sets a process-global flag before
the first window opens; `is_active()` remains true, so the existing action
admission boundary still applies. The interface does not switch a live window
between two authority graphs and does not create another thread or project
store. [source] [implementation]

The launch contract is:

```sh
omega --primary-interface --user-data-dir <isolated-user-data-directory>
```

The local operator convenience alias is:

```sh
alias 'omega:primary-interface'='<omega release-fast binary> --primary-interface \
  --user-data-dir <isolated Omega primary-interface profile>'
```

The alias lives in the operator's shell configuration, not in either
repository. The isolated profile keeps the porting baseline separate from the
normal installed Omega process. The first version intentionally opened only a
blank conversation plane and the reference-style input. Later commits added one
surface at a time. [runtime] [implementation]

##### Reference-app investigation

The reference project source tree was built and launched in two forms during the port: the
small GPUI development demo and the full desktop application. The exercise
confirmed that the desktop presentation is GPUI, while its engine, RPC,
document, harness, update, and edge responsibilities remain separate crates.
On non-macOS systems the same GPUI element tree targets GPUI's Linux or Windows
platform backend; macOS blur/frost behavior becomes an opaque, platform-
appropriate surface where native translucency is unavailable. This is why the
Omega port treats frost as a platform token and never as a required authority
or data dependency. No live the reference project cloud-room or cross-device claim was
re-verified by this UI exercise. [source] [runtime] [limitation]

##### Window, shell, and navigation now landed

The primary-interface root is rendered by `AgentPanel::render_omega_shell` on Omega's
in-tree GPUI. It now includes:

- a single rounded main card inside the native window;
- macOS blurred window appearance, with theme/platform fallback elsewhere;
- traffic-light clearance, sidebar toggle, back/forward affordances, session
  tabs, and the new-session button on one titlebar row;
- a draggable titlebar surface with native double-click zoom behavior;
- no extra arbitrary band above the titlebar;
- an Omega-native sidebar with Projects, the current Omega project, recent
  sessions, attention state, and an Omega primary-interface identity footer;
- active and recent session tabs backed by Omega thread rows; and
- the existing Omega conversation entity mounted as the center content.

The first shell pass used the reference project-like neutral grays. The follow-up deliberately
rejected that color copy and remapped card, sidebar, selected, hover, border,
text, placeholder, and accent roles to Omega's Khala blue theme. The current
decision is therefore **reference geometry under Omega identity**, not the
external project's monochrome. The earlier planning question in §7.4.5 is
resolved for this interface in favor of Khala tokens. [implementation]

The project list remains an Omega projection rather than the reference project's full
`(device, folder)` Spaces model. Device ownership, remote roots, and
cross-device presence are still visible gaps rather than implied parity.
[source] [limitation]

##### Composer parity now landed

The composer work happened in two layers. First, the ordinary Omega agent
surface adopted reference-sized bar and then fixed the bug where a new thread
and an existing thread rendered at different heights. Second, the primary Omega interface
received the closer visual treatment: compact rounded chassis, placeholder,
bottom-aligned controls, integrated Omega Agent/model picker, microphone,
attachment/control affordances, and circular send/stop control. [implementation]

This remains a chassis port over Omega's mature input substrate:

- `MessageEditor` still owns text editing, mentions, rich context, paste, IME,
  and focus behavior;
- Omega's executor menu and model state remain canonical even when rendered as
  reference-style inline controls;
- Omega's front-door and `SendDisposition` laws still decide send, steer,
  enqueue, and refusal behavior; and
- clean profiles wait for the actual composer rather than rendering a false
  but attractive placeholder that cannot send.

The result matches the reference bar much more closely without importing
The reference project's `ComposerInput`, engine handle, drafts store, or picker authority.
Attachments, the paged question-wizard takeover, the exact flip/hysteresis
reducer, and the complete Send/Steer/Stop morph remain incomplete relative to
the full the reference project implementation. [source] [implementation] [limitation]

##### Transcript and tool-call presentation now landed

the primary Omega interface now changes the visual grammar of real Omega thread entries:

- user messages render as right-aligned rounded bubbles with a bounded width;
- assistant Markdown renders as calm full-width prose with reduced container
  chrome;
- consecutive groupable tool calls collapse into a reference-style command group;
- individual tool rows retain Omega tool semantics and status;
- canceled tool calls with no visible result are omitted; and
- Omega's existing agent font, Markdown renderer, ACP parts, thread entity,
  authorization, and context-menu behavior remain in place.

This is a presentation adapter inside `ThreadView`, not a transcript fork. It
does not yet implement the reference project's MessageRail, paint-only streaming veil,
direction-aware stick spring, or every specialized tool/result visualization.
[source] [implementation] [limitation]

##### Session-tab keyboard law now landed

`Command-N` on macOS continues to dispatch Omega's `agent::NewThread` action,
which opens a fresh message/session in the primary Omega interface. The new
`omega_workbench::CloseActiveSessionTab` action is bound only in the
`AgentPanel && OmegaInterface` key context:

- `Command-W` closes the active Omega session tab on macOS;
- `Ctrl-W` performs the same action on Linux and Windows;
- focus moves to the next open, non-refused session;
- closing the last open tab creates a fresh message session; and
- the closed conversation remains in the sidebar and can be reopened.

“Close tab” therefore does not delete or archive the durable Omega thread and
does not fall through to the generic pane/window close action. The closed-tab
set is presentation-local state, while the thread remains canonical in Omega's
store. [source] [implementation]

##### Projects and working-directory law now landed

The sidebar no longer presents a decorative Spaces row. It presents Omega
**Projects**, derived from real worktree candidates. Selecting a project opens
its actual root; the plus control opens the folder picker. Project selection is
also propagated into conversation drafts so agent work has a concrete working
directory before a request can launch. If no valid project root exists, Omega
opens project selection instead of allowing a tool run with an absent working
directory. [source] [implementation]

This closes the immediate tool-call failure that motivated the port without
adopting the reference project's device-room authority. The remaining project work is richer
multi-project switching, remote/device roots, worktree selection, and parity
for folder rename/remove and presence metadata. [implementation] [limitation]

##### Omega-adapted settings navigation now landed

The primary-interface identity footer now opens Omega's real settings surface. Omega
settings use a 256-pixel Omega-native navigation sidebar with selected and
hover states, accessible navigation rows, and a bottom Back action. Its entries
are generated from Omega's existing focused settings pages rather than copied
from the reference project's product schema. At this snapshot those focused pages expose:

- **API Keys**, including Omega's provider credentials route; and
- **Voice**, including the existing Sarah Voice configuration.

The setting values, validation, persistence, and page contents remain owned by
Omega. This is therefore a port of the reference project's settings information architecture
and navigation grammar, not an import of the reference project's Accounts, Devices, Shortcuts,
Archived Sessions, or update authority. Settings also still open in Omega's
separate `SettingsWindow`; they are not yet an embedded route inside the Omega
shell. [source] [implementation] [limitation]

##### Browser-style titlebar history now landed

Back and Forward are no longer decorative. `OmegaNavigationHistory` records
session selection in memory, deduplicates the current entry, walks backward and
forward like a browser, and truncates the forward branch after a new selection.
The controls derive disabled state from actually restorable targets and expose
tooltips, button roles, labels, and `aria-disabled`. Navigation skips missing
targets, reopens a presentation-closed tab when its durable thread still
exists, and seeds the restored active session on the first Omega render.
[source] [implementation]

The history is intentionally presentation-local. It does not alter Omega's
durable thread order, archive state, or router authority, and it is not yet a
persisted cross-window navigation ledger. [implementation] [limitation]

##### Verification receipt at the current snapshot

The combined snapshot was formatted, compiled, linted, tested, built, and
opened locally:

| Check                                                 | Result at `989adfac0b`                                                |
| ----------------------------------------------------- | --------------------------------------------------------------------- |
| `cargo check -p agent_ui -p settings_ui -p omega`     | Passed                                                                |
| `cargo test -p omega_deltas`                          | Passed, 308 tests                                                     |
| `cargo test -p settings_ui`                           | Passed, 57 tests                                                      |
| Focused `OmegaNavigationHistory` unit tests           | Passed, 2 tests                                                       |
| `./script/clippy` on the changed Omega crates         | Passed                                                                |
| `cargo build --profile release-fast`                  | Passed                                                                |
| Release-fast primary-interface launch with isolated profile  | Opened successfully alongside the untouched installed Omega process   |
| Live Back → Forward walk across two restored sessions | Returned to each expected session and updated control state correctly |

These receipts establish source integration and a bounded local macOS launch.
They are not signed package, multi-platform visual, accessibility, release, or
long-duration daily-driver evidence. [test] [runtime] [limitation]

##### Revised migration status

The current result completes the visual shell skeleton for the agent surface
and a meaningful part of the vertical conversation slice, but it does not
complete the “all of Omega UI” rebuild:

| Migration phase from §7.4.4         | 2026-08-01 status                                                                                                                                                                                                                                                                                            |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 0. Contract and provenance spike    | Partial: one GPUI runtime and one Omega authority graph are preserved; a formal import/provenance manifest for copied the reference project source is still unnecessary because the implementation currently re-creates the presentation in Omega code rather than importing files.                                          |
| 1. Presentation foundation          | Partial: Khala color roles, macOS blur, card/shell states, and core controls exist; a shared motion kit, component gallery, reduced-motion audit, and cross-platform visual baselines remain.                                                                                                                |
| 2. Shell skeleton                   | Agent-first slice landed: window composition, Projects, guaranteed working directories, sidebar, session tabs, titlebar drag, new/close session, browser-style history, settings entry, and conversation mount work; remote/device projects, resize/collapse parity, and restored multi-window state remain. |
| 3. Vertical agent slice             | Substantial visual slice landed: real Omega threads use the Omega-native composer, message rows, and grouped tools; full attachments, question takeover, disposition morph, optimistic failure return, stick spring, and specialized result parity remain.                                                   |
| 4. IDE workbench                    | Not started in the primary Omega interface beyond retaining the underlying Omega entities; editor panes, search, Git, terminal/tasks, diff, and drag/drop are not yet mounted in the primary interface.                                                                                                                      |
| 5. Secondary surfaces               | Partial: Omega-native settings navigation now exposes Omega's real API Keys and Voice pages. Identity/account, device, onboarding, command-palette, notification, debugger, and remaining settings surfaces are not ported.                                                                                  |
| 6–7. Hardening, cutover, retirement | Not started; the primary Omega interface is additive and the normal Omega presentation remains the default.                                                                                                                                                                                                                   |

This changes the planning posture in one important way: the high-risk premise
that the reference project's composition could coexist with Omega's GPUI and authority graph
has now been demonstrated for the agent surface. It does **not** collapse the
remaining estimate for a full IDE cutover. The difficult work identified in
§7.4.5—focus/action routing across editors and terminals, platform parity,
accessibility, durable window/workbench restore, and long-tail surfaces—still
sits ahead. [implementation] [inferred]

##### Entropy analysis as the first workbench slice

The 2026-08-02 owner direction narrows the first workbench proof again. Start
with a live entropy-only traversal of the pinned Coldcard repository, not the
full evidence console. The user edits one visible prompt, starts the run,
watches the file queue advance, inspects source-grounded entropy candidates,
and reruns the same source with a changed prompt. Omega issues
[#199](https://github.com/OpenAgentsInc/omega/issues/199) through
[#202](https://github.com/OpenAgentsInc/omega/issues/202) own the traversal,
prompt, dashboard, and 15-project campaign. [owner-direction] [inferred]

This slice preserves the normal project list, task tabs, history, and composer.
Its workbench needs only a repository/run header, visible prompt editor, stable
file-status list, selected candidate pane, summary counts, Cancel, and prompt
A/B changes. It should reuse Omega's existing project, task, agent, and source-
navigation authorities. It should not start with forensic tabs, publication,
model voting, product grades, or a new execution authority. [inferred]

The 15-project view is a later campaign over the same single-repository run.
Every row must declare its repository, pinned revision, and source eligibility.
Closed or incomplete source stays unavailable; it does not receive a clean or
unsafe color. [inferred]

Implementation receipt, 2026-08-02: Omega #199 through #202 closed in order at
`6fd2767b5d`, `4ebde0f20d`, `a18287b216`, and `15ffc050aa`. The resulting
Omega-native shell keeps the normal sidebar and adds a persistent
**Forensics** entry, one editable prompt, live file traversal and candidate
detail, immutable prompt comparisons, and the sequential 15-product campaign.
The implementation preserves this teardown's no-grade and source-availability
boundaries. [implementation]

##### Broader forensics after entropy

The 2026-08-02 Coldcard and Omega-thread audit identifies a smaller, higher-
value phase-4 proof than mounting a general editor, terminal, and Git surface
all at once: after the entropy slice, put one read-only forensic case inside the
Omega-native workbench.
The Coldcard fixture already exercises dense, linked evidence, typed
limitations, disputes, model-panel outcomes, reconciliation, and a publication
boundary. That makes it a strong test of whether the shell can carry real Omega
state without creating presentation-owned authority. [analysis] [inferred]

The first slice should preserve the normal project list, task tabs, history,
and composer. Its primary region should add a sticky case header; Evidence,
Claims, Limitations, Panel, and Publication tabs; a queue and claim inspector;
copyable exact refs; explicit loading, empty, incomplete, denied, stale, and
tool-contract-failed states; and a visibly blocked publication gate. Use
restrained semantic color and the existing typography/tokens. Do not add a
second workbench rail, a modal-only review path, nested-card chrome for every
row, or a presentation-owned evidence store. [inferred]

Live lifecycle controls are a later state of the same surface. Keep Prepare,
Launch, Cancel, and cleanup disabled until OpenAgents issues #9289 and #9290
have accepted live worker and source-delivery receipts. Issue #9300 remains open
and prevents a fixture-backed UI from being represented as an accepted live
forensic program. [runtime] [limitation]

##### Ordered remaining port plan

The next work should advance product capability rather than add more inert
shell replicas:

1. **Complete project semantics.** Add explicit multi-project switching,
   worktree selection, remote/device roots, and remove/rename flows while
   retaining Omega's project and worktree authorities.
2. **Mount one real workbench slice.** Start with the Coldcard entropy traversal,
   editable prompt, live file queue, and prompt comparison. Expand next into
   the read-only forensic case reader, then add editor/source navigation,
   terminal/task, and Git diff with correct focus, actions, drag/drop, and
   restore. This is the next decisive proof that the rebuild can carry more
   than chat.
3. **Finish conversation interaction parity.** Add staged attachments,
   question-wizard takeover, the complete Send/Steer/Stop transition,
   optimistic failure return-to-draft, streaming stick behavior, and the
   specialized result renderers that matter in daily use.
4. **Expand secondary surfaces.** Port account/identity, devices, onboarding,
   command palette, notifications, debugger, and the rest of Omega's settings
   into the same presentation grammar without replacing their authorities.
5. **Harden before any cutover.** Establish macOS, Linux, and Windows visual
   baselines; keyboard and IME coverage; accessibility and reduced-motion
   audits; persisted multi-window restore; performance budgets; and sustained
   daily-driver evidence.
6. **Cut over only after measured parity.** Keep normal Omega as the default and
   the primary Omega interface as an additive rollback path until the contract matrix is green;
   retire the old shell only after that evidence exists.

The immediate recommendation is therefore the entropy-only Coldcard dashboard,
in parallel with closing project semantics. Follow it with the broader forensic
reader, then editor, terminal, and diff integration. More shell polish has lower
architectural value until real Omega authority survives inside source-informed
composition. [inferred]

### 7.5 Relationship to other teardowns

| Prior teardown                                                                                                          | How the reference project sits relative to it                                                                                                                                                                                                       |
| ----------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [T3 Code](./2026-07-13-t3-code-teardown.md) + [Omega/T3 gap](./2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md) | T3 is the multi-client control-plane peer (Effect core, web/desktop/mobile). The reference project is the multi-device **daemon+CRDT** peer. T3 owns environment projection; the reference project owns detachable engine + doc command queue.                      |
| [Paseo](./2026-07-17-paseo-teardown.md)                                                                                 | Paseo is the cross-device agent-daemon delivery reference with typed WebSocket protocol. The reference project is closer to "cloud CRDT rooms + host executor."                                                                                     |
| [Amp](./2026-07-16-amp-code-teardown.md)                                                                                | Amp's thread fabric and remote control thesis; the reference project implements a concrete open session-doc + device-room version of that class of product.                                                                                         |
| [Goose / GDK](./2026-07-27-goose-gdk-omega-teardown.md)                                                                 | Goose is an ACP engine Omega may attach. The reference project is a multi-device host for Claude/Codex CLIs, not an ACP peer kit.                                                                                                                   |
| [Superlogical](./2026-07-29-superlogical-teardown.md) + [All Work](../allwork/README.md)                                | Superlogical is a durable-session thesis without public code. The reference project is shipped open code for durable multi-device agent sessions — useful evidence for the coordination layer, still not a substitute for Omega as the application. |

## 8. Disposition for Omega

### 8.1 Track

Track the reference project as a **high-relevance multi-device controller peer** and as
evidence for detachable engine lifetime, durable host-only command ledgers,
shared pure view derivation, and space-as-device-folder indexing.

### 8.2 Harvest (implementation-shaped, separately admitted)

| Lesson                      | Omega landing zone (suggested)                                                | Do not import                             |
| --------------------------- | ----------------------------------------------------------------------------- | ----------------------------------------- |
| Detach ≠ kill               | effectd / engine supervision; viewport reattach; TUI and mobile as attachers  | the reference project binary as sidecar authority         |
| Durable QueueCommand ledger | Sync/thread command log with host executor + processed ledger                 | Loro dependency by default                |
| Host-only execute           | Explicit chat/thread host device in multi-host map                            | Silent host races                         |
| Pure `view` module          | Shared projection crate for list order/staleness across desktop/mobile/bridge | Duplicated sort per surface               |
| Spaces index                | Multi-host folder registry separate from full Project graph                   | Replacing Project with spaces             |
| Render-parts privacy        | Public/synced transcript omits full tool payloads                             | Treating stripping as E2EE                |
| e2e two-engine smoke        | Conformance: queue on B, execute on A, project back                           | Cloudflare-specific harness as product CI |

### 8.3 Reject

- Cloudflare Workers / Durable Objects / R2 as Omega or OpenAgents production
  session fabric.
- WorkOS as the sole identity and org gate for Omega.
- Unattended auto-approve + silent sandbox escalation as default product
  policy.
- Pinning `wingleeio/zed` GPUI beside `OpenAgentsInc/omega`.
- Replacing Omega's native agent router or editor with the reference project's thin engine.
- Claiming E2EE, multi-tenant safety, or receipt-grade audit from the reference project's
  current doc path.
- Treating the reference project MIT code as a way around Omega's GPL obligations for forked
  Zed surfaces.

### 8.4 Decision sentence

**Use the reference project as control-plane evidence for multi-device coding-agent sessions.
Build the laws into Omega on Omega's identity, IDE, and Google Cloud rails.
Do not adopt the reference project's engine or application authority as the OpenAgents
desktop. If the product chooses source-informed presentation, source-port it inside
Omega behind the adapter and cutover plan in §7.4.**

## 9. Open questions for a later pass

1. How mature is `apps/ios` relative to the desktop/TUI path — shippable
   controller or scaffold?
2. Does production `comet.zeron.sh` match the open edge tree, and what is the
   operator threat model for DO-stored transcripts?
3. Wire compatibility of `loro-protocol` Rust ⇄ TS at the audited revs under
   partition and large-transcript load.
4. Whether Omega should store the durable command ledger in Khala Sync /
   effectd SQLite / a CRDT — product decision, not implied by this teardown.
5. Cursor harness shape once the reference project lands it — only then compare to Omega's
   custom ACP path.

## 10. Required reading

- This repository: [teardown catalog README](./README.md), [App for All Work](../allwork/README.md), [Omega/T3 gap analysis](./2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md), [remote-first portable sessions pathway](../sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md)
- the reference project tree: `ARCHITECTURE.md`, `docs/PARITY.md`, `docs/research/feature-inventory.md`, `crates/ui/src/{lib,state,shell,composer,transcript,pickers,changes}.rs`, `crates/doc/src/commands.rs`, `crates/harness/src/codex/mod.rs`, `edge/src/session-room.ts`, `edge/src/device-room.ts`
- Omega tree: `README.md`, `crates/workspace/src/workspace.rs`, `crates/agent_ui/src/{agent_panel,message_editor,conversation_view}.rs`, `crates/agent_ui/src/conversation_view/thread_view.rs`, `crates/omega_front_door`, `crates/omega_device_bridge`, `crates/omega_device_enrollment`, `crates/agent_servers`

---

_Evidence convention matches the catalog: labels in §1.2. This document is
design evidence, not Omega status, not a ProductSpec, and not release
authority._
