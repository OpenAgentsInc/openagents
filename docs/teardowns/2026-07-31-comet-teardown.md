# Comet Teardown — 2026-07-31

Read-only architecture and product audit of the public `zeronsh/comet`
repository at an exact commit in the local reference clone
`~/work/projects/repos/comet`, compared to the current Omega desktop client
(the tracked Zed fork at `~/work/omega`, **not** the retired Electron
OpenAgents Desktop app). Nothing in either tree was modified. This audit did
not build binaries, start the edge Worker, run a harness against a live Claude
or Codex CLI, open a Durable Object room, or package a release. [limitation]

Comet is a multi-device controller for coding agents: start Claude Code or
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
[Comet vs Omega UI deep dive](./2026-07-31-comet-omega-ui-deep-dive.md). That
report's product judgment: **Comet's composer is the better agent input bar**;
Omega should match its density while keeping MessageEditor and disposition law.

## Executive decision

**Comet is the strongest open multi-device agent-controller reference in the
catalog for a headed/headless split, durable offline-tolerant command queue,
and CRDT session docs over a cloud room fabric.** It is not an IDE, not a
receipt system, and not a model for OpenAgents production infrastructure.

Omega should **not** adopt Comet as a product shell, replace its native agent
router with Comet's engine, pin Comet's third-party Zed/GPUI fork, or move
session authority onto Cloudflare Durable Objects and WorkOS. Those choices
collide with Omega's IDE substrate, signed-device authority, and Google Cloud
production contract.

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
   structs, not full re-hydration on every CRDT change — Comet's documented
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
      +-- adapt from Comet:
            detach-not-kill engine lifetime
            durable host-only command ledger
            pure shared view derivations
            space = device + folder index
            incremental session projection

not recommended

Omega process -> Comet engine as second authority
Omega sessions -> Cloudflare SessionRoom / DeviceRoom DOs
Omega identity -> WorkOS as sole product gate
Omega UI -> pin wingleeio/zed GPUI fork beside OpenAgentsInc/omega
Omega default -> unattended auto-approve as product policy
```

## Summary

Comet makes one bet: **the coding agent session should outlive any single
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
diverge on almost every authority and packaging choice. Comet is a thin
controller around foreign CLIs. Omega is a full IDE that also attaches agents.

## 1. Snapshot, provenance, and limits

### 1.1 Exact source identity

| Artifact | Identity | What it establishes |
| --- | --- | --- |
| Public repository | `https://github.com/zeronsh/comet` | Public native-rewrite source |
| Local clone | `~/work/projects/repos/comet` | Audited tree |
| Audited commit | `e5d8e9fb4c2ffe2350e4114db3bfd89979a2136d` | Exact snapshot |
| Commit time | `2026-07-31T22:03:44+00:00` | Freshness of the tip |
| Commit subject | `Scrub Unpeel references from TUI test fixture` | Latest audited change |
| Workspace version | `0.1.5` (`Cargo.toml` workspace.package) | Pre-1.0 train |
| License | MIT (`LICENSE`, copyright 2026 Wing) | Permissive |
| First commit in clone | `2026-07-19` | About twelve days of history at audit time |
| History scale | 151 commits, two shortlog identities (Wing ~112, wing-anara ~46) | Solo-led, high velocity |
| Language scale | ~79k lines of Rust under `crates/` + `apps/`; ~2.7k lines edge TypeScript; ~9.1k lines iOS Swift | Mid-size controller product |
| Install surface | `https://comet.zeron.sh/install.sh` (Linux systemd user unit); macOS points at DMG | Headless Linux first |
| Omega local clone | `~/work/omega` | Comparison tree |
| Omega audited commit | `acd0f5324a570ef8de19b188f93c5e487abe760b` | Current local `main` tip at audit |
| Omega product claim | "Your last IDE"; tracked Zed fork; bootstrap phase | Full IDE + workroom |

Selected content digests at the Comet pin:

| File | SHA-256 |
| --- | --- |
| `README.md` | `2e22d67359d6352bb35fa957baaa1b2e64590cf1c18aaaa183051eb0d23da58e` |
| `ARCHITECTURE.md` | `4695fd191f1dfb89c136d8dc433ceb92420c65b4e2b317f3e80a8e0a170f1b80` |
| `docs/PARITY.md` | `ccd009276606a72220c1574ec76e58bf01ee0cc0c1087a9e23eb82d666d27858` |
| `Cargo.toml` | `e5b953549bca85b59f0b7c3831b53154c75cbdc9705e394580e2af956cd48ba4` |
| `edge/package.json` | `5803233d553eaede5cfa54e7aacb49a0667999a6d95065caa41c537434886237` |

Selected Omega digests at the Omega pin:

| File | SHA-256 |
| --- | --- |
| `README.md` | `8c11f9ec59c28738ebf6ab9186f8dc8df0bd0fdf595aee6540a8efadc04e5d6f` |
| `crates/omega_device_bridge/src/omega_device_bridge.rs` | `74c35bffc574ecb1899b3994810838c1ca0742e23f1759f1c59908a474635cb9` |
| `crates/omega_device_enrollment/omega_device_enrollment.rs` | `63db8d3a9a0ae0d9670c1e8fbef9665a16b308a52f9cc7cd353536f13a9cd359` |
| `crates/omega_front_door/src/omega_front_door.rs` | `4ae5c14003911da32755fb40703538e445a952c01452de9c72160e5f3a389115` |

### 1.2 Evidence labels

- **`[source]`** — tracked source, docs, or manifests at the audited commit.
- **`[schema]`** — typed Rust, TypeScript, or wire contracts.
- **`[test]`** — an executable test, smoke script, or CI gate present in source.
- **`[history]`** — Git history at or before the audited commit.
- **`[public]`** — a linked public page or repository description.
- **`[inferred]`** — reasoned from several observations.
- **`[limitation]`** — a boundary on what this audit can prove.

### 1.3 Audit limits

This was a source audit. It does not prove live Durable Object hibernation,
WorkOS production behavior, harness correctness against current Claude/Codex
CLIs, iOS App Store readiness, macOS notarization, or cryptographic security
of the room protocol under an adversarial network. The original TypeScript
comet tree that this rewrite replaces was not cloned. Comet's PARITY matrix
claims live Claude CLI verification; this audit treats that as author
testimony encoded in docs, not as a re-run. [limitation]

## 2. What Comet is

### 2.1 Product thesis

The README positions Comet as: control Claude Code and Codex from any of your
devices. Every device runs a small engine that keeps sessions in sync. Install
the engine as a daemon on an always-on machine so agents keep working after
the laptop closes. Day-to-day CLI: `comet login`, `comet status`,
`comet update`, `comet tui`, `comet daemon …`. [source]

### 2.2 Architecture pillars

From `ARCHITECTURE.md` and the crate layout:

| Pillar | Implementation at the pin |
| --- | --- |
| Sync | Loro CRDT docs (`loro` 1.13) through Cloudflare Durable Objects |
| Edge language | TypeScript Worker + DOs (deliberate; device side is Rust) |
| UI | GPUI pinned to a `wingleeio/zed` rev; **no** Zed GPL crates (`markdown`, `ui`, `theme`, `editor`) |
| Binary model | One `comet` binary, headed or headless; separate `comet-tui` |
| Feature parity | Against the prior Electron product, excluding token-usage display |
| License | MIT throughout the rewrite |

[source] [schema]

### 2.3 Crate map

| Crate / path | Role |
| --- | --- |
| `crates/proto` | Wire types + shared pure `view` derivations |
| `crates/doc` | Session + workspace doc schemas, command ledger, parts privacy |
| `crates/sync` | Loro room client, DocsStore (SQLite snapshots + processed ledger) |
| `crates/harness` | Harness trait; Claude stream-json; Codex app-server; mock |
| `crates/engine` | Sessions, doc host, repos/worktrees, terminals, auth, device-room host |
| `crates/rpc` | Typed UiRpc/ControlRpc over WS + in-memory duplex + device frames |
| `crates/ui` | GPUI shell, transcript, composer, terminal, changes, settings |
| `crates/tui` | ratatui viewport; never embeds an engine |
| `crates/update` | Self-update support |
| `apps/comet` | Headed default + `headless` subcommand + daemon install |
| `apps/tui` | `comet-tui` binary |
| `apps/ios` | Native SwiftUI controller (exists in tree; PARITY still lists mobile as deferred for the rewrite scope) |
| `edge/` | Worker, SessionRoom DO, DeviceRoom DO, R2 attachments, WorkOS auth |

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
right product vocabulary for local UI law. Comet shows how to make those
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

| Harness | Mechanism at the pin | Status |
| --- | --- | --- |
| Claude Code | `claude` CLI stream-json; AskUserQuestion → requestInput; steering via persistent input | done; live-verified claim in PARITY |
| Codex | `codex app-server` JSON-RPC; thread/start/resume; sandbox policy | done |
| Cursor | deferred | no settled CLI surface |
| Mock | scripted event replay | powers tests and e2e smoke |

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
OpenAgents product deploy paths. Comet's edge is excellent evidence for
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
the enrolled host. Comet optimizes for "sign into org, rooms just work."
Omega optimizes for "prove the device and the grant." Both are coherent.
Only the second matches the OpenAgents identity direction. [source]
[inferred]

## 6. Surfaces

| Surface | Stack | Notes |
| --- | --- | --- |
| Desktop | GPUI app in `crates/ui` | Always-dark monochrome; Geist fonts; virtualized transcript; motion kit ported from prior product timings |
| TUI | ratatui in `crates/tui` | ~12MB target vs ~60MB headed claim; no-tick coalescing event loop; fingerprinted transcript cache |
| Headless daemon | `comet headless` + systemd/launchd units | Linux install script first-class; macOS build-from-source / DMG path |
| iOS | SwiftUI under `apps/ios` | Room client, workspace store, device relay client, composer, transcript; ~9k Swift lines present despite "mobile deferred" language in ARCHITECTURE/PARITY |

[source]

Comet is **not** an editor. There is no Project service, no LSP, no multi-root
worktree IDE model, no built-in merge editor. The changes pane is a patch
viewer over checkout diffs. Terminals are session-scoped PTYs with replay.
That is enough for supervising a CLI coding agent. It is not enough for
"the application for all work." [source] [inferred]

## 7. Omega comparison

### 7.1 Product shape

| Dimension | Comet (pin above) | Omega (pin above) |
| --- | --- | --- |
| Category | Multi-device coding-agent controller | Native IDE + workroom (Zed fork) |
| Default surface | Session list + transcript + composer | Zero Base thread + full editor substrate available |
| Primary runtime | Thin Rust engine + foreign CLIs | Full GPUI app graph + native agent + ACP |
| Multi-device fabric | Cloudflare DO rooms + Loro docs + WorkOS | Device bridge WebSocket + enrollment grants + optional Nostr |
| Session durability | CRDT session doc + SQLite snapshots + run journal | Multiple durable stores; composition still the gap (see Omega/T3 gap analysis) |
| Authority | Org membership + host device ownership of chats | Front-door router owns no execution; grants, receipts, signed identity |
| Containment default | Harness sandbox; unattended auto-approve posture | Explicit lanes, permissions, and evidence work in progress |
| License | MIT | GPL-3.0-or-later with Apache-2.0 components |
| Production edge | Cloudflare | Google Cloud (OpenAgents contract) |
| Mobile | iOS tree present; product maturity unclear | Activity mirror; stronger design than shipped control (T3 comparison) |
| Scale | ~11 crates + edge + iOS; ~2 weeks of rewrite history | ~225 crates; long Zed history + Omega overlays |

[source] [inferred]

### 7.2 Multi-device specifically

Comet's multi-device path is **cloud-room-first**:

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

Comet is ahead on **seamless multi-host session continuity and offline
command queuing**. Omega is ahead on **cryptographic device admission,
capability grants, and IDE-side work depth**. The product synthesis is not
"copy Comet's cloud." It is "give Omega Comet's lifetime and ledger laws on
Omega's trust rails." [source] [inferred]

### 7.3 GPUI note

Both products use GPUI. The relationship is not "Comet uses Omega." Comet pins
GPUI (and only GPUI/platform/tokio packages) from `wingleeio/zed` at a fixed
rev, deliberately avoiding Zed's GPL UI/editor crates. Omega **is** the
OpenAgents tracked Zed fork and owns the full application. Sharing a UI
toolkit does not share product authority, release identity, or license
posture. Do not treat Comet as a lightweight Omega. Do not vendor Comet's
GPUI pin into Omega. [source]

### 7.4 Relationship to other teardowns

| Prior teardown | How Comet sits relative to it |
| --- | --- |
| [T3 Code](./2026-07-13-t3-code-teardown.md) + [Omega/T3 gap](./2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md) | T3 is the multi-client control-plane peer (Effect core, web/desktop/mobile). Comet is the multi-device **daemon+CRDT** peer. T3 owns environment projection; Comet owns detachable engine + doc command queue. |
| [Paseo](./2026-07-17-paseo-teardown.md) | Paseo is the cross-device agent-daemon delivery reference with typed WebSocket protocol. Comet is closer to "cloud CRDT rooms + host executor." |
| [Amp](./2026-07-16-amp-code-teardown.md) | Amp's thread fabric and remote control thesis; Comet implements a concrete open session-doc + device-room version of that class of product. |
| [Goose / GDK](./2026-07-27-goose-gdk-omega-teardown.md) | Goose is an ACP engine Omega may attach. Comet is a multi-device host for Claude/Codex CLIs, not an ACP peer kit. |
| [Superlogical](./2026-07-29-superlogical-teardown.md) + [All Work](../allwork/README.md) | Superlogical is a durable-session thesis without public code. Comet is shipped open code for durable multi-device agent sessions — useful evidence for the coordination layer, still not a substitute for Omega as the application. |

## 8. Disposition for Omega

### 8.1 Track

Track Comet as a **high-relevance multi-device controller peer** and as
evidence for detachable engine lifetime, durable host-only command ledgers,
shared pure view derivation, and space-as-device-folder indexing.

### 8.2 Harvest (implementation-shaped, separately admitted)

| Lesson | Omega landing zone (suggested) | Do not import |
| --- | --- | --- |
| Detach ≠ kill | effectd / engine supervision; viewport reattach; TUI and mobile as attachers | Comet binary as sidecar authority |
| Durable QueueCommand ledger | Sync/thread command log with host executor + processed ledger | Loro dependency by default |
| Host-only execute | Explicit chat/thread host device in multi-host map | Silent host races |
| Pure `view` module | Shared projection crate for list order/staleness across desktop/mobile/bridge | Duplicated sort per surface |
| Spaces index | Multi-host folder registry separate from full Project graph | Replacing Project with spaces |
| Render-parts privacy | Public/synced transcript omits full tool payloads | Treating stripping as E2EE |
| e2e two-engine smoke | Conformance: queue on B, execute on A, project back | Cloudflare-specific harness as product CI |

### 8.3 Reject

- Cloudflare Workers / Durable Objects / R2 as Omega or OpenAgents production
  session fabric.
- WorkOS as the sole identity and org gate for Omega.
- Unattended auto-approve + silent sandbox escalation as default product
  policy.
- Pinning `wingleeio/zed` GPUI beside `OpenAgentsInc/omega`.
- Replacing Omega's native agent router or editor with Comet's thin engine.
- Claiming E2EE, multi-tenant safety, or receipt-grade audit from Comet's
  current doc path.
- Treating Comet MIT code as a way around Omega's GPL obligations for forked
  Zed surfaces.

### 8.4 Decision sentence

**Use Comet as control-plane evidence for multi-device coding-agent sessions.
Build the laws into Omega on Omega's identity, IDE, and Google Cloud rails.
Do not adopt Comet as the OpenAgents desktop.**

## 9. Open questions for a later pass

1. How mature is `apps/ios` relative to the desktop/TUI path — shippable
   controller or scaffold?
2. Does production `comet.zeron.sh` match the open edge tree, and what is the
   operator threat model for DO-stored transcripts?
3. Wire compatibility of `loro-protocol` Rust ⇄ TS at the audited revs under
   partition and large-transcript load.
4. Whether Omega should store the durable command ledger in Khala Sync /
   effectd SQLite / a CRDT — product decision, not implied by this teardown.
5. Cursor harness shape once Comet lands it — only then compare to Omega's
   custom ACP path.

## 10. Required reading

- This repository: [teardown catalog README](./README.md), [App for All Work](../allwork/README.md), [Omega/T3 gap analysis](./2026-07-27-omega-t3-code-desktop-mobile-gap-analysis.md), [remote-first portable sessions pathway](../sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md)
- Comet tree: `ARCHITECTURE.md`, `docs/PARITY.md`, `docs/research/feature-inventory.md`, `crates/doc/src/commands.rs`, `crates/harness/src/codex/mod.rs`, `edge/src/session-room.ts`, `edge/src/device-room.ts`
- Omega tree: `README.md`, `crates/omega_front_door`, `crates/omega_device_bridge`, `crates/omega_device_enrollment`, `crates/agent_servers`

---

*Evidence convention matches the catalog: labels in §1.2. This document is
design evidence, not Omega status, not a ProductSpec, and not release
authority.*
