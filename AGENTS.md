# OpenAgents agent contract

Product code in this workspace is Rust. Do not add TypeScript. The existing
product exception is `swift/lev-bridge`, the helper that reaches Apple's
`FoundationModels` framework, which has no Rust binding; it is built by a
repo script and supervised as a child process. Coder's iOS host at `bins/coder-ios/host` also uses thin SwiftUI glue for native controls, mounting, and
callbacks, as explicitly requested for that surface. Keep its application state,
domain logic, permissions, and transport in Rust; the implemented observer keeps these in `coder-mobile` and `coder-connect`. Read `crates/rust-native/docs/spec.md` and `docs/coder/rust-native/architecture.md`
before adding that boundary.
The OpenAgents iOS host at `bins/openagents-ios/host` follows the same thin
SwiftUI boundary; its application state lives in `crates/openagents-mobile`.
The OpenAgents Android host at `bins/openagents-android/host` is thin Kotlin
over the same crate, through its JNI surface (`src/android.rs`).
The Android host at `bins/coder-android/host` uses the equivalent thin Kotlin
boundary for Android framework widgets, `SurfaceView`, camera, sensors, and
Keystore access. Keep domain state, Nostr, authorization, cache, and world
behavior in the same Rust mobile library; do not import the private Android
backend or authentication implementation.
Retained Python training and
acceptance tooling and shell orchestration are infrastructure exceptions,
not permission to add another product implementation language.
The Nix and shell under `os/` (CoderOS) are infrastructure in the same sense:
they configure a machine and launch Rust programs, and product behavior
belongs in Rust.

To ship the OpenAgents iOS app to TestFlight (for example when asked from the
phone), run `scripts/release/testflight.sh start` (add `--validate-only` for
a dry run that archives and validates without uploading), then run
`scripts/release/testflight.sh wait` again and again until it exits 0 (done)
or 1 (failed; the reason is its last line); each `wait` returns within four
minutes. Before a real upload, raise `CURRENT_PROJECT_VERSION` in
`bins/openagents-ios/host/project.yml` to the build being shipped, add that
build's entry at the top of `CHANGELOG` in
`crates/openagents-mobile/src/account.rs`, commit, and push to `main`; the
script refuses a dirty checkout or a build number App Store Connect already
has. Report the build number and the script's last line.

## User-facing copy (owner, 2026-10-08)

Never put **machine talk** in anything a user sees: web pages, native views,
served docs, onboarding, notices, errors, or CLI messages meant for end
users. Machine talk narrates the system's internals instead of telling the
person what happened or what they can do: internal words (retained,
projection, superseded, canonical, admitted, provenance, epoch, reconcile,
digest, lane, journal, original bytes, ...), narration of internal steps,
reassurance about guarantees nobody asked about, and hedged legalistic
phrasing. The test: would a normal person using a chat app say this sentence
out loud? If not, rewrite it in plain words or say nothing. Precise terms stay
in docs, logs, protocols, and code. Each surface's tests run the `oa-copy`
guard (`oa_copy::violations`) over the text users see; extend its lexicon
rather than weakening a surface's allowlist. See #11031.

Also: no agent-written walls of text in the product (design the flow as UI
instead), and comment out any control that doesn't work yet rather than
showing a placeholder.

## Velocity (owner, 2026-10-01)

Ship small changes fast. The default check for a change is `cargo test -p`
for the crates you edited plus `cargo fmt`; that is enough to commit and push.
Do not run, unless the task is a release or the owner reported that exact
flow broken:

- Clippy, the release gate (`scripts/release/acceptance.sh`), the phone
  suite, live runs against real engines, or other crates' tests.
- New `INVARIANTS.md` rows, design notes, or long docs. Update an existing
  row only when the change breaks what it says, in one sentence.

Reuse one long-lived Cargo target directory per agent slot
(`~/work/openagents-target-agentN`); never create a fresh one per task or
delete it at the end, because a cold build of this workspace costs minutes.

Run every heavy Cargo command (`build`, `test`, `check`, `clippy`, `run`)
through the machine's build lease, so builds take turns instead of
oversubscribing the cores: `openagents lease build --keep-target-dir --
cargo test -p CRATE`. `--keep-target-dir` keeps your long-lived
`CARGO_TARGET_DIR`; without it the lease picks a shared target slot. Run a
soak, benchmark, or other latency-sensitive job under the quiet lease, which
waits for running builds and holds new ones: `openagents lease quiet
--receipt FILE -- CMD`. `openagents lease list` shows who holds what. If the
installed `openagents` has no `lease` command, build it once with `cargo build
-p openagents-cli --bin openagents` and run it from your target directory.
Read `docs/coder/runtime/leases.md`. Run release gates and Terminal-Bench
with a class so placement can send them to another computer: `openagents
lease run --class release-gate -- CMD` or `--class bench`; soaks use
`openagents lease quiet --class soak -- CMD` and stay here
(`docs/coder/runtime/placement.md`).

Prefer offscreen captures (`verse --capture FILE.png` and other offscreen
paths). When the owner asks for a visible window or desktop test, run it
directly; no separate screen lease or grant is required.

Keep scratch files (captures, scripts, notes) in the directory `openagents
scratch` prints (`$OPENAGENTS_SCRATCH` under a lease or a Coder delegation),
not in `/tmp`, which a reboot clears; `docs/coder/guides/scratch.md`.

Verify in a browser through `openagents browser run -- CMD`, which gives each
check its own Chrome profile and port (`OPENAGENTS_CHROME_PORT`), never a
fixed port or a shared profile; `docs/coder/guides/browser.md`.

When a test fails only because a checked-in generated file is stale, run its
regenerate command and commit the result; don't investigate further.

Documentation-only changes do not require the Rust verification gate, including
before a push. Check links, paths, and retained artifacts for documentation
reorganizations. Comment edits and documentation path updates do not require
workspace-wide tests; if an embedded document's loading path changes, check only
the affected consumer.

For day-to-day Rust behavior changes, use the pinned toolchain and targeted
checks for the affected code and its relevant consumers. A bare
`./scripts/verify-rust.sh` runs changed-package formatting, Clippy, and tests;
use `--crates` or `--phases` to choose the needed coverage. Direct focused
Cargo commands are also valid. Record what ran and any remaining limitations.

Claim an issue before working it and release it if you stop: `openagents issue
claim N` / `release N` (comment marker, you as assignee, project Status), the
same record Coder's flows and `coder-project` write and honour; leave an issue
another claim holds (`openagents issue status N`) alone.

Before you launch subagents, run `openagents capacity check claude` (exit 1
means the login is out of its limit; don't launch), and when an agent stops on
a usage limit, run `openagents capacity record PROVIDER --reset TIME` with the
reset it printed (`docs/coder/runtime/capacity.md`).

Close an issue as soon as its work is code-complete: merged to `main`, with its
own checks passing and any host deploy it needs done. Never hold an issue open
waiting on the owner — a real-money payment, a device run, a key only the owner
can create, a store release, or any other owner-only verification. Put those
steps in the workspace `NEEDS_OWNER.md`, say in the closing comment what the
owner still has to do, and close the issue. If the owner's step later finds a
defect, open a new issue for it.

Keep the OpenAgents project board
(<https://github.com/orgs/OpenAgentsInc/projects/19>, `docs/project-board.md`)
current: when you start an issue, claim it and run
`scripts/project-status.sh N in-progress`; when it is blocked, run
`scripts/project-status.sh N blocked --blocked-by "B"` naming the blocker; when
it is done, the closing commit closes it and the issue becomes Done
(`scripts/project-status.sh N done`, or `scripts/project-sync.sh`). A new issue
goes on the board with its blockers.

The full workspace gate (`./scripts/verify-rust.sh --release`) is for full
releases only. Never require it before ordinary issue development, integration,
commits, pushes, or closing an issue whose own acceptance checks pass. Never
hold independent issue work while it runs. Fix failures relevant to a change;
record unrelated failures separately and continue the other work. Documentation
changes remain exempt from Rust checks. Read `docs/verification.md` for scope
and prerequisites. Use a separate Cargo target directory per worktree, outside
the worktree: a Coder run already has one in `CARGO_TARGET_DIR` (keep it; never
point it at `$PWD/target`), because a target directory inside the workspace
outgrows what the next task's admission can observe and every later task there
is refused. Keep workspace formatting changes separate from behavior changes.

Deploy host builds only from a commit rebased on current `origin/main`, and
give each checkout its own Cargo target directory, so an older checkout
cannot overwrite a newer build and a deploy of an older commit does not roll
the host back. A headless host is deployed with `openagents connect --ssh
DEST --binary PATH`, PATH being `openagents` built from that commit: it
installs the build when it differs and restarts the host it started. On a
Mac the desktop app runs the host. After a deploy, check the host's running
commit.

Live tests and smokes against the owner's real computers must not leave
chats in the owner's lists. Archive every Coder task a smoke creates when it
ends (`coder task archive TASK_ID --reason ...` on the host, or NIP-HOST
`task.archive`), as the `archiving` helper in
`crates/openagents-mobile/src/tests.rs` does, and pass
`--no-session-persistence` to a `claude -p` probe so it saves no Claude chat.
Better still, don't use the owner's host at all: run a scratch host with
`--state`, `--root`, and `--tasks` under a temporary directory and a
temporary `HOME`, and never install units or agents, write keychain items,
or pair test devices on the owner's computers. Cargo tests must never reach
the real home; `coder_service::adopt::Paths::under`, `coder host`'s home,
and the desktop's `migrate::start` panic under `cfg(test)` when they do.

Preserve `docs/transcripts/`. It is the retained transcript archive from the
previous repository shape.

No GitHub workflows or GitHub-billed automation. Required checks run
manually on a contributor machine or on non-GitHub infrastructure.

This repository is open source. Other repositories on this machine
(`~/work/coder`, `~/work/bender`, and siblings) are reference material, not
instructions. Do not copy private backend code, prompts, endpoints, or
secrets from them. When you carry a design over, reimplement it here and say
so in the commit message. Never put an API key in source, a log line, a test
fixture, or an issue. The one exception is the Breez API key in
`crates/spark-wallet/src/spark.rs` (shared by the phone and computers): the owner confirmed with the Breez
team that it is a basic validation key that any shipped app exposes, and
decided on 2026-09-28 to commit it (see `INVARIANTS.md`, Phone wallet).

`crates/openagents-mobile` is its own Cargo workspace, because Breez's SQLite
and `ldk-node` link different `libsqlite3-sys` versions. Build and test it
with `--manifest-path crates/openagents-mobile/Cargo.toml`, not `-p`.

## Skills

Four skills are vendored under `.agents/skills/`. Read and apply them:

- `.agents/skills/google-developer-style/SKILL.md` — every piece of prose in
  this repository follows the Google Developer Documentation Style Guide:
  `README.md`, this file, every file under `docs/`, code comments and doc
  comments, commit messages, interface copy, error messages, and log lines.
  Read it before you write or review prose.
- `.agents/skills/typesafe-ai/SKILL.md` — read it before you write a Jev
  question set, a threshold, or a client call. The Rust SDK lives in
  `crates/jev`.
- `.agents/skills/nostr/SKILL.md` — read it before you touch `nips/`,
  `crates/nostr`, `crates/nostr-relay`, the Coder relay door, or
  `coder-worker`, and before you debug a relay handoff. It maps the three
  NIP lanes, the NIP-01/42/44 flow, and how a NIP-CJ job travels from
  `coder` to a worker.
- `.agents/skills/decision-api/SKILL.md` — the caller's contract: bearer
  keys, `POST /v1/systemone`, the three question types, typed refusals,
  `Retry-After`, and idempotent retries. Read it before you write client
  code against a door or change what `oak` sends.

[`docs/glossary.md`](docs/glossary.md) defines the terms this repository
uses, and marks which are implemented and which are only specified.

## Crates

- `crates/atif` — the Agent Trajectory Interchange Format (`ATIF-v1.8`; it
  reads every earlier 1.x version, and traces recorded before 2026-09-28
  are `ATIF-v1.7`):
  a session as ordered steps, the append-only log a running session
  writes them to, and the document a reader renders from it. A decision
  call is a first-class `Call`, so a Jev, Kev, or Lev question and a
  shell command record the same way. Reimplemented from the Harbor
  trajectory RFC's reference implementation; `serde` and `sha2` only, no
  network. Read `docs/coder/runtime/traces.md`.
- `crates/gym` — the measurement and control plane for decision models:
  pinned suites, a receipt-chained result store, digested acceptance gates,
  and the terminal that reads them. It scores whatever answers
  `POST /v1/systemone` and knows nothing else about the door. Read
  `docs/gym/` before changing a schema or a gate.
- `crates/gym-bridge` — the separately granted Verse Gym connection: a portable
  encrypted Nostr client and an optional local observation/recipe host. Source
  directories and executable revisions are admitted explicitly; entry starts
  observation, and launches require confirmation. Read `docs/verse/gym.md` and
  the crate README before changing its authority, source readers, or retry rules.
- `crates/gym-leaderboard` — the Gym's published benchmark results: a typed,
  versioned leaderboard (`openagents.gym.leaderboard.v1`) and scrubbed,
  bounded trace bundles, generated from committed Terminal-Bench evidence
  into `bench/terminal-bench/published/` by `gym-leaderboard build`. Each
  adapter recomputes its study's verdicts and refuses to build on
  disagreement. A new study needs only a `study.json` descriptor and
  `openagents.gym.attempt-row.v1` rows. Regenerate after adding evidence;
  read `docs/verse/gym-leaderboard.md` before changing the contract. Apps
  depend on it with `default-features = false, features = ["client"]` for
  the fetch-verify-cache client, which never links the generator.
- `crates/tenancy` — the tenant-to-artifact registry: a versioned,
  self-digested manifest binding a tenant identity to the door names it may
  reach, each bound to an artifact digest and its execution configuration.
  Authorization returns an admission snapshot — an update cannot relabel a
  call in flight — and every revision stays archived under its digest so an
  earlier answer can always be explained. `tenancy::keys` is the credential
  half: `oak_<id>.<secret>` bearer keys, stored as digests only, issued and
  rotated through the `tenant-keys` binary. `tenancy::quota` is the
  durable budget: an append-only reservation ledger beside the registry,
  retry-safe by `(request, attempt)`, with crash recovery that orphans
  unsettled holds as `unknown` rather than freeing them; `tenant-usage`
  is the operator's view of it. `tenancy::accounts` is the account and
  workspace half — accounts, principals, personal and organization
  workspaces, the owner/admin/member matrix, single-use invitations and
  recovery tokens — and `tenancy::sessions` the persisted session
  store: `sess_<hex>` tokens, the funded anonymous lane's budgets, and
  a bounded access history, all digests and references. `tenancy::billing`
  is the billing book: operator-declared versioned plans, subscriptions
  that pin the plan version they bought, checkout sessions, invoices,
  and a deduplicated provider-event journal whose effects post to the
  money ledger under stable `billing:*` sources — a crash between the
  ledger append and the billing seal replays the identical mutation,
  and a refund or dispute clawback debits the lesser of its amount and
  the available balance. `tenancy::skills` is the skill-directory book:
  versioned `SKILL.md` submissions with recorded static, decision, and
  reasoning review stages, deduplication and supersession, withdrawal
  and moderation, and a persisted audit trail — the `skills-moderate`
  binary is the operator's takedown, reinstate, admit, and evidence
  path over it. `tenancy::training` is the tenant-training book: a
  four-partition corpus (training beside the Gym suite's three) with
  per-item provenance, group boundaries, and cross-partition leakage
  refusals, a headroom assessment that causes every baseline failure
  before a recipe may freeze, frozen seed/budget/metric recipes, an
  append-only trials ledger, sealed candidates whose signature is the
  `artifact_signature` an admission record binds, and retention
  tombstones — the `tenant-train` binary is the operator's path over
  it, and sealing serves nothing.
- `crates/receipts` — versioned receipts a decision call leaves behind.
  `receipts::execution` is the shared HTTP/relay shape: request and attempt
  identity, tenant and registry references, requested and served artifact
  identities, a typed outcome, timing, and digests of the request and
  result rather than their content. An attributable claim, never remote
  attestation.
- `crates/gateway` — the keyed HTTP front of the serving half: one
  admission path for `POST /v1/systemone` — authenticate the bearer key,
  authorize the door against `tenancy`'s registry, bound the door's
  declared capacity and the process's forward count, reserve quota
  durably, verify the backend's published model card against the bound
  identity before a byte is forwarded, then settle and leave a sealed
  receipt in `receipts.jsonl`. `GET /v1/models` is the caller's view of
  its own doors; `GET /healthz` is process liveness only. The
  `gateway` binary reads one `gateway.json` — listen address, registry
  directory, each door's backend endpoint, the optional `accounts`
  document that mounts the self-serve account, session, workspace, and
  key-management surface plus the member-scoped `/v1/workspaces/{id}/usage*`
  reads, the `/dashboard` pages, and the `/playground` decision demo
  (real inference under the session bearer, a labeled simulated lane,
  and a capped tool-backed chat), and the optional `billing` document that
  mounts plans, checkout, signed provider webhooks, and owner-only
  subscription management over `tenancy::billing` — under which a
  decision call names a subscribed workspace whose plan covers the
  door — plus the optional `skills` document that mounts the versioned
  skill directory's submission, review, and publication surface over
  `tenancy::skills`. Read
  `docs/decision-models/service/gateway.md` before changing a refusal code, a
  bound, or the reservation lifecycle.
- `crates/discovery` — the public discovery surface every origin shares:
  the bundled documentation corpus the MCP documentation tools and the
  `/v1/docs` API read, the machine-readable document set under
  `docs/agents/` served at `/`, the well-known agent card and
  agent-skills index, the MCP server card shape, and the plugin
  manifests under `plugins/`. Nothing in it authenticates or answers a
  decision call.
- `crates/jev` — the Rust SDK for TypeSafe's System One API and the
  gateway's caller routes: `classify`, the durable-job lifecycle, and
  the account reads, all through the one transport's retry and error
  contract. `docs/decision-models/guides/clients.md` is the matrix.
- `crates/oak` — the caller's CLI for the decision API: `oak ask` sends a
  state and a questions file through `POST /v1/systemone`, `--input
  lines|ndjson` runs a bounded batch with ordered NDJSON output, and
  `oak models` lists the doors a credential can reach. Retries are oak's
  own loop — the service settles quota by `(request, attempt)`, so each
  retry keeps the `Idempotency-Key` and bumps `x-attempt`. Credentials
  come from `OPENAGENTS_API_KEY` or a `0600` config file, never a flag.
  `docs/decision-models/guides/caller.md` is the caller's guide.
- `crates/kev` — the Rust port of the kev decision model: packed prefill,
  block-causal question isolation, pointer readout, and `kev-serve`, a
  TypeSafe-compatible `POST /v1/systemone` server. Documentation and
  conformance numbers live in `docs/kev/`; golden fixtures in
  `crates/kev/fixtures/`; weights stay in `~/work/kev-artifacts/` out of git.
- `crates/laya` — the Rust port of the Laya decision model: a ModernBERT
  encoder (English ModernBERT-large, multilingual mmBERT-base, and the
  typed-decisions checkpoint) with a marker-scoring decision head and
  act head, served through `laya-serve` as a TypeSafe-compatible
  `POST /v1/systemone` door with per-variant admission and explicit
  `model`-field checkpoint selection. Documentation and conformance
  numbers live in `docs/laya/`; golden fixtures in
  `crates/laya/fixtures/`; weights stay in `~/work/laya-artifacts/` out
  of git.
- `crates/rust-native` — the experimental shared UI foundation: validated
  serializable semantic views, typed application intents, deterministic style
  composition, and generic colors. Current primitives include `Stack`, `List`,
  `Text`, `Button`, and a locally registered `Surface`. Bounded viewports and
  active/disposed frame timing are generic; native adapters remain separate. The core has no
  product palette or application dependency. It owns no
  task execution, network transport, credentials, or platform objects. Read
  `crates/rust-native/docs/` before extending a shared UI contract. Put reusable
  component semantics here and keep platform implementations in adapters.
- `crates/coder-ui` — Coder application presentation values. It owns the Coder Noir
  palette and backgrounds; `coder-terminal` preserves its public
  imports through re-exports. Keep Coder components and product defaults here,
  never in the reusable `rust-native` framework.
- `crates/coder-history` — read-only retained Codex and Claude history adapters;
  explicit roots, bounded raw record pages, and source-bound cursors. The host
  feature opens files; portable DTOs and projection helpers serve mobile.
- `crates/coder-connect` — explicitly paired retained-history observation over
  NIP-42 and encrypted private Nostr artifacts. This cannot control an engine.
  Read its README and the NIP-SESS observer profile before changing authority.
- `crates/chat-load-bench` — the phone chat-loading benchmark: drives the real
  `coder-connect` client, observer host, relay and direct transports, and
  transcript layout phase by phase, and times the basic Coder's NIP-CJ legs.
  Its fixture run needs no network. Results and the ranked bottlenecks are in
  `docs/coder/runtime/chat-load-benchmark.md`.
- `crates/coder-access` — NIP-HOST host-wide device enrollment: single-use
  `coder-host:` invitations, reverse enrollment approved by code, host-signed
  device grants with seven closed rights and revocation epochs, and delegation
  that can only narrow. The client feature builds without host code. Read its
  README and `nips/openagents/NIP-HOST.md` before changing rights or admission.
- `crates/coder-reach` — NIP-REACH: the owner host directory, host presence,
  reachability hints that never offer loopback to another machine, the
  encrypted direct-channel handshake, and placement. Grant checks go through a
  trait. Read `nips/openagents/NIP-REACH.md` before changing the handshake.
- `crates/coder-link` — one connection supervisor per host for Coder clients:
  a deterministic state machine and registry with an injected `Connector` and
  clock, separate transport health and data freshness, and no network,
  storage, or UI dependency. Read its README before changing retry or blocked
  behavior.
- `crates/coder-pty` — NIP-TERM terminal sessions: Unix host PTYs owned as
  process groups, bounded replay with explicit gaps, idle expiry, and shutdown
  cleanup, plus portable client state. Rights and frame delivery are traits the
  resident host wires. Read `nips/openagents/NIP-TERM.md` before changing framing.
- `crates/coder-vt` — a small terminal emulator: the VT100/xterm subset a
  shell and common full-screen programs use, applied to a character grid, with
  xterm key and paste encoding. `vte` parses; the grid, modes, and replies are
  its own. The mobile terminal screen in `coder-computers::terminal` draws it.
  Read its README before adding a sequence or a reply.
- `crates/coder-ssh` — installs, starts or adopts, and reaches a Coder host over
  the system `ssh` binary with a fixed POSIX `sh` script and SHA-256-pinned
  archives. Only an explicit remove, or a launch with a changed release or
  runner, stops a managed host; client exit and tunnel loss never do. Read its
  README and the NIP-ENV SSH-launched hosts section.
- `crates/coder-service` — the Coder host as a launchd agent or systemd user
  unit, with trial updates against state snapshots, rollback, and the host
  descriptor. `scripts/coder-host.py` still stages bundles. Read
  `docs/coder/runtime/host-service.md` before changing a state transition.
- `crates/coder-desk` — the desk protocol (`coder_desk::protocol`, one
  versioned JSON line per request on `CODER_DESK_SOCKET`) and both of its
  halves: the client, with a native and a Hyprland backend, and the `serve`
  module a desk answers with. `crates/coder-desk-cli` is the `coder-desk`
  command the CoderOS scripts run instead of `hyprctl`, and
  `os/pkgs/coder-desk.nix` builds it. `crates/coder-binds` is the one table
  of desktop chords and window rules the Hyprland configuration renders
  from and the Coder compositor reads. A launcher or window rule for a
  module in a private host flake goes through `coderos.desktop.extraBinds`
  and `extraWindowRules`, never a row of the table. Read the crate READMEs
  before changing a verb or the generation.
- `crates/coder-compositor` — the optional Coder Wayland compositor for
  CoderOS, on Smithay 0.7, with a nested and a hardware (DRM, `libinput`,
  `libseat`) backend, Xwayland, and the desk protocol through
  `coder_desk::serve`. `crates/coder-wm` is its dwindle layout, and
  `os/pkgs/coder-compositor.nix` builds it; `nix develop ./os#compositor`
  has the system libraries its build needs. Hand tracking is a stub in
  `src/hands.rs` until `coder-hands` moves. Never run it on a seat someone
  is using; its tests need no display. Read its README first.
- `crates/coder-computers` — Coder's Computers screens (host status, add a
  computer, access, first run, activity) as Rust Native projections with typed
  intents and one shared authority check. Its `live` service owns the client's
  grants, owner-directory reads under the NIP-REACH owner-authority rule, and,
  with the `ssh` feature, SSH host setup through `coder-ssh`. Read its README
  before changing owner-key handling.
- `crates/playtest` — the playtest program's records: private reports
  sealed with NIP-17 to the triage key, the playtest log (on by default,
  off with the release-mode build switch) whose types
  hold no text, and triage (exact-identity deduplication, issue drafts, and
  the append-only triage log). No network or storage; `openagents playtest`
  is the triage inbox. Read `docs/game/playtesting.md` and
  `docs/game/playtest-triage.md` before changing what a report or the log
  may carry.
- `crates/openagents-cli` — the `openagents` command: one `--json`-first
  program over the existing crates for pairing and computers (`coder-host`,
  `coder-computers`), durable tasks (`coder`), headless Verse presence and
  the Lagrange zone (`verse`, `verse-lagrange`), relays and keys, and
  fail-closed NIP-SOV profile handling. It adds no protocol logic of its
  own; put behavior in the owning crate and expose it here. Read
  `docs/cli/README.md` before adding a command group.
- `crates/wallet` — `openagents-wallet`, the x402 Lightning rail on an
  exclusively held node key: the `LightningWallet` trait (exact invoices
  with a request-hash description hash, fee-capped payment that returns the
  preimage, lookup) and its `ldk-node` implementation behind the `ldk`
  feature, with Esplora and SQLite under `~/.openagents/wallet`. Read
  `nips/openagents/NIP-X402.md` before changing what an invoice or proof
  carries.
- `crates/x402` — `openagents-x402`, x402 v2 `exact` Lightning over HTTP
  (`http:1`): the restart-durable replay store whose insert is one exclusive
  file create per `network:payment_hash`, the embedded facilitator that
  verifies terms, invoice, and preimage through `crates/nostr::x402` and maps
  refusals to the upstream `errorReason` vocabulary, the base64
  `PAYMENT-REQUIRED`, `PAYMENT-SIGNATURE`, and `PAYMENT-RESPONSE` codecs, and
  the transport-free paid resource handler that `openagents x402 serve`
  carries. Nothing in it pays or advertises.
- `crates/coder-host` — the resident Coder host (`coder host serve`) and its
  client. It composes `coder-access`, `coder-reach`, `coder-pty`, and the task
  inbox behind one host key and rechecks the grant on every channel message.
  It also answers NIP-HOST over CJ execution through the same admission and
  publishes its `host-access` capability.
  `task.create` is an inert inbox submission and grants no execution authority,
  except under the owner's local auto-start policy (`coder host autostart`,
  off by default, in `coder::task::autostart`), which starts eligible tasks
  within its workspace, concurrency, and engine bounds and records each start.
  Read its README, `docs/coder/runtime/host-serve.md`, and
  `docs/coder/runtime/host-autostart.md` before changing a binding or its
  authority.
- Connecting devices: the OpenAgents desktop app and `openagents connect`
  (`crates/openagents-cli/src/connect.rs`, `--ssh` for a headless computer)
  pair a phone or computer with a host over its same-user control socket.
  Tailscale stays an optional route (`coder host serve --listen-websocket`,
  `--tailnet-admission`). Read `docs/coder/guides/link-devices.md`.
- `crates/push-gateway` — the NIP-PL push gateway: holds APNs and FCM
  credentials for the relay's PL executor, resolves relay-presented delivery
  grants to sealed device tokens, and sends only the registered wake constants.
  Its `client` feature is the device side `coder-mobile` uses to register a
  token and manage its lease. Read `docs/deployment/push-gateway.md` before
  changing a route, a result code, or token custody.
  Never commit `bins/coder-android/host/app/google-services.json`, and keep
  `CODER_IOS_PUSH` unset for TestFlight archives until the App ID has Push
  Notifications; both switch on native push tokens.
- `crates/coder-mobile` — Rust-owned iOS/Android reader state, encrypted cache, paging,
  synchronization, and C ABI, plus a separate main-thread Verse render handle
  using the shared `verse::runtime::WorldRuntime`. SwiftUI and Android widgets
  mount Rust Native views and native GPU surfaces. Platform adapters own native
  controls, Keychain/Keystore, camera, and sensors. Keep
  Verse identity and lifecycle separate from history-observer authority. Read
  `docs/coder/guides/mobile-readonly.md` and `docs/verse/mobile.md`.
- `crates/coder-terminal` — the Coder terminal: the Coder Noir intensity ladder,
  the framed composer, and the shell they draw. It also holds the terminal
  design system every other terminal here depends on — the re-exported
  `coder_ui::theme::Intensity`, `Ladder`, `frame`, and `rail`. Extend these
  terminal facilities rather than copying them; shared platform-independent
  UI contracts belong in Rust Native. The
  rebuild plan lives in `docs/coder/`.
- `crates/coder` — the agent: `classify` routes each turn through Jev,
  `generate` answers through an Open Responses door, and the `coder`
  binary draws the conversation in the terminal or, with `-p`, runs one
  turn from a script. Both modes run the same turn, `coder::turn::run`;
  keep it that way. `permit` is the host's answer to whether a turn runs
  commands at all, built from the route and the operator's setting before
  anything generates and narrowing from there; a reply becomes an
  executable plan only under a permit that runs one, so keep execution
  policy there rather than in what the model is told. `delegate_door`
  answers a turn through Microcoder's loop in process, on the first
  connected provider with capacity in the capacity book (the Codex login,
  then Claude Code's login, then, always last, Vertex through the
  OpenAgents cloud, which needs no token on the host; read
  `docs/coder/runtime/cloud-fallback.md`), failing over mid-turn when one refuses for a
  usage or rate limit; else through `coder-delegate`'s probes, Jev's judgments, a
  briefing, and Claude Code or Codex when one is installed, signed in, and
  has capacity, with the Open Responses door as the fallback. The permit
  maps to its `coder-boundary` boundary, and `coder-worker` never reaches
  it. A turn that asks to work a GitHub issue runs the issue flow on
  Microcoder, ending in a draft pull request, when the operator's permit
  runs commands. Read `docs/coder/runtime/delegate-door.md` before changing it.
  `scripts/install-coder.sh` installs the binary as `coder`, and
  `coder doctor` says which door a turn uses and why.
  `docs/coder/guides/headless.md` covers the headless flags and
  the exit codes. `coder task` is the opt-in durable local inbox in
  `coder::task`: submit, inspect, list, and cancel queued requests with
  exact-byte command retries and private atomic storage. It runs no agent
  and grants no execution authority. `task::owner` admits explicit bounded
  commands through the shared supervisor and filesystem boundary, records
  uncertain effects without replaying them, and binds paged ATIF views,
  retained artifacts, corrections, and independent checks to one task.
  Read `docs/coder/runtime/task-owner.md` and `docs/coder/guides/tasks.md`
  before changing its schema, persistence, or command semantics, and use
  `docs/coder/migration-status.md` for suite implementation status.
  Every conversation records itself to
  `~/.openagents/traces/` as it runs; `docs/coder/runtime/traces.md` covers the
  location, the opt-out, and what a trace holds. While a turn runs or a
  delegation has not reported, the process also writes
  `~/.openagents/activity/<pid>.json`, which `coder activity` reads and
  `os/bin/coder-close` asks before it closes a window; the mark is taken in
  `coder::turn::run`, and `docs/coder/guides/activity.md` covers it. `delegate` hands a
  bounded task to an executor and runs a fan-out of them under a stated
  bound; read `docs/coder/runtime/delegate.md` before changing it, and do not
  offer delegation to the model as a tool it may elect. `capability`
  re-exports the shared `crates/capability` contract. These readers load
  `capabilities/`, `programs/`, `questions/`, and `sources/`, so what a
  machine can reach is probed
  rather than hardcoded, and `runtime` runs a program's steps from the
  program. The turn reaches it by asking which program a request wants, or
  **none**, which is nearly every turn and leaves the turn unchanged;
  `docs/decision-models/measurements/2026-09-19-program-selection.md` is that question's
  baseline, headroom, and error rates, counted apart because a missed
  program costs a retry and a spurious one runs a program nobody asked for.
  `docs/programs.md` covers all seven. The crate's second binary,
  `coder-worker`, is the other end of the relay door: it answers NIP-CJ
  job requests from a relay through an Open Responses door.
  `docs/coder/measurements/relay-transport.md` is the measured proof that the two ends
  meet, and it holds the per-transport latency and the refusal causes.
- `crates/microcoder-loop` — the Microcoder loop, the simple loop the
  Luna pivot runs: Jev judges the state, one structured model call returns
  the next commands, and the host runs them. It holds the loop, the model
  and Jev calls, where commands run (with a `coder-boundary` environment
  for Coder's delegate door), the provider capacity book
  (`capacity.json`, which `coder::task::capacity` re-exports), and
  failover to the next provider with capacity. It depends on nothing from
  `crates/coder`, so both `coder` and `microcoder` run it. Keep it the one
  loop; stronger-model routing stays off by default. Read
  `docs/coder/guides/microcoder.md` before changing it.
- `crates/microcoder` — the `microcoder` binary over that loop: the task
  owner's repository adapter (`microcoder repository`), Terminal-Bench 4
  runs, and the knowledge network (NIP-KB, NIP-XP). It re-exports the
  loop's modules under their old paths. Read
  `docs/coder/runtime/microcoder-repository.md` before changing the
  adapter.
- `crates/codex-transport` — the operator's Codex login and the ChatGPT
  Codex Responses transport: one request and one reply, the typed
  `usage_limit_reached` refusal, list-price cost, the one-tool-call
  `oneshot`, and a scripted fake for tests. It only reads
  `~/.codex/auth.json` and never refreshes it. The Microcoder loop, Coder,
  and the knowledge base use it. Read `docs/coder/design/microluna.md`
  (its origin) before changing the transport.
- `crates/acp-client` — the Agent Client Protocol (ACP) as Coder speaks
  it to a local coding agent over stdio: typed frames and `session/update`
  payloads, one client with a silence limit and `session/cancel`, the
  agent as a process group of its own with an explicit environment, and
  a replaying stand-in for recorded fixtures. `acp_client::devin` holds
  the Devin CLI's specifics (`devin acp`), `acp_client::opencode` holds
  OpenCode's (`opencode acp`), `acp_client::grok` holds Grok Build's
  (`grok agent stdio`), and `acp_client::cursor` holds the Cursor agent's
  (`cursor-agent acp`). Read `docs/coder/runtime/devin.md`,
  `docs/coder/runtime/grok.md`, and `docs/coder/runtime/cursor.md` before
  changing it.
- `crates/microluna` — deprecated on 2026-09-28; Microcoder replaced it.
  Microluna ran short GPT-6 Luna sessions with five native function tools
  (run a command, read a file region, apply a patch, write a file, and
  finish) under `coder-boundary` and `supervise`. It stays in the
  workspace only for Coder One's retained Terminal-Bench policies
  (`coder_one::micro`, the `microluna-*` policies) and the `microluna`
  binary, so recorded evidence stays reproducible. Don't build new work
  on it; Coder's terminal no longer runs it.
- `crates/coder-delegate` — what Coder's terminal turn runs, split out
  of Coder One so Coder and Verse build without Microluna (#9889): the
  Claude Code and Codex adapters, the probe battery and Jev's judge, the
  briefing, the policy sections a turn reads, and the terminal turn
  (`coder_delegate::terminal`), with an `Engine` hook an in-process loop
  answers through, and the issue flow (`coder_delegate::issue`), which runs
  on any `Worker`: Microcoder in Coder's door, Microluna in Coder One. Coder One re-exports each module under its old path.
  It must not depend on `crates/microluna`; `cargo tree -p coder -i
  microluna` finds no package.
- `crates/coder-one` — Coder One, a minimal standalone agent that turns a
  GitHub issue into a pull request. Each step asks Jev for typed
  judgments over the state, puts them in the prompt, generates one
  action through `openagents.com/v1/responses`, and runs it. It does not
  depend on `crates/coder`. Issue #9531 holds the design and the
  evaluation. `--delegate always|auto` (`CODER_ONE_DELEGATE` in an
  episode) lets the loop explore, then hands the task to Claude Code, or
  to Codex CLI with `--delegate-agent codex`, with a briefing code builds
  from Jev's evidence; delegation is a host
  decision, never a tool the model sees. Issue #9532 and
  `docs/terminal-bench/coder-one-delegate-runbook.md` cover it.
  `coder-one ask` answers a question about runs by reading the Gym,
  inside a read-only boundary, with an allowlisted `read` tool and code
  that checks every citation; `docs/coder/guides/coder-one-ask.md` covers
  it, and issue #9574 holds the design.
- `crates/capability` — the capability manifest contract `coder` and
  `coderbench` share. Reading a registry is inert; an executable probe
  runs only under an approval the operator recorded with the
  `capability-trust` binary, which pins the manifest's digest and the
  adapter's canonical path and contents in a store outside any checkout.
  Probes run bounded through `supervise`, and a probe that cannot answer
  cleanly is `unknown`, never `present`.
- `crates/coder-control` — scoped Nostr access to the durable local task owner:
  owner-installed task mappings, independent observe/steer/cancel grants, exact
  encrypted input artifacts, retained command dispositions, and finite evidence
  views. The default `host` feature owns the private store; the client-only
  build has no dependency on Coder execution. Read
  `docs/coder/runtime/nostr-task-control.md` before changing authority, expiry,
  replay, or disclosure. Pairing cannot start execution or authorize spending.
- `crates/coder-labor` — the free-only labor host: pinned buyer/provider
  agreement, separately granted bounded execution, retained delivery and buyer
  verification, explicit acceptance, and conservative restart recovery. Read
  `docs/coder/runtime/free-labor.md`; paid settlement, resolver execution, and
  nonzero rework are unsupported.
- `crates/coder-mobile-probe` — the Rust-first platform feasibility prototype:
  shared synthetic task evidence, native iOS/Android controls, and Rust-rendered
  HTML. Read `docs/coder/design/rust-mobile-feasibility.md`. Simulator and
  emulator observations do not establish physical-device release acceptance.
- `crates/coder-boundary` — a filesystem write boundary (`sandbox-exec` on
  macOS, `bwrap` on Linux) and independent
  Unix workspace snapshots. Unsupported enforcement is refused; incomplete
  snapshots are unverifiable. Read
  `docs/coder/verification/2026-09-20-execution-boundary.md` before changing it.
- `crates/coder-lease` — the host resource broker: a file-backed lease
  table under `~/.openagents/leases/` (`OPENAGENTS_LEASE_ROOT`) with
  exclusive (`quiet`, `screen`, `gpu`, ...) and counted (`build`, `memory`,
  `disk`) leases, `flock` holder locks that free a dead holder's lease, a
  priority queue with aging, receipts, and the screen grant. No dependency on
  `crates/coder`; `openagents lease` is its command. Read
  `docs/coder/runtime/leases.md` before changing admission or the quiet rule.
  Its `artifact` module is the queue for single-digest artifacts listed in
  `artifacts/` (the Everglade pack, the Grid pack fingerprint): land a
  change to one with `openagents artifact submit NAME`, never a direct repin
  push; `docs/coder/runtime/artifact-queue.md`.
- `crates/supervise` — the one subprocess supervisor `coder` and
  `coderbench` run other programs through. A job runs in a process group of
  its own, a deadline or a cancelled caller terminates that group and reaps
  the direct child before the job reports, and stdout and stderr are held
  to their caps as they are read. Unix only, stated rather than assumed.
  Read `docs/coder/runtime/subprocesses.md` before changing a deadline, a cap, or a
  call site that spawns a process.
- `crates/lev` — the same contract answered by Apple's on-device foundation
  model, through the in-repo Swift helper in `swift/lev-bridge`. Build the
  helper with `./scripts/build-lev-bridge.sh`; the crate builds and tests
  without it. Read `docs/lev/` before changing an estimator or the door.
- `crates/voyager` — open-ended agent episodes in a Minecraft world, after
  arXiv:2305.16291: a supervised local server, a world-manifest registry
  under `worlds/`, and the bot side through the nightly-built helper in
  `mc-bridge/`, built by `./scripts/build-mc-bridge.sh` and driven as a
  child process speaking line-delimited JSON — the `swift/lev-bridge`
  precedent in Rust. A curriculum proposes tasks, a bounded Lua
  interpreter runs them as code-as-action, a mechanical or `noul` critic
  checks each attempt, and passing programs bank into a digested skill
  store; `voyager evidence` renders a run's coverage matrix and metrics.
  The crate builds and tests without the helper. Read `docs/voyager/`
  before changing an episode, a world manifest, or the bridge protocol.
- `crates/verse` — the shared Verse desktop/iOS world: a Tron-style city drawn in
  amber lines on the terminal's near-black field, and a third-person
  character with WoW-style movement and mouselook. The stack follows Ruins
  of Atlantis (`wgpu`, `winit`, `glam`, a custom renderer); the controller
  is reimplemented from its `client_core`, not copied. The global plaza uses
  `coder_ui::theme::Intensity`; a palette test protects its amber geometry.
  Separately loaded zones may have their own validated colors and atmosphere.
  Plaza portals lead to local zones: Lagrange 1 is a generated Sun–Earth L1
  construction station driven by `verse-lagrange`; the Physics Lab runs the
  `physics` crate's mechanisms live with HUD knobs (`docs/verse/physics-lab.md`);
  Everglade loads a pinned asset pack only on entry. Combat
  everywhere follows `docs/verse/combat-model.md`: real time, MMO style, with
  SRD dice rolled behind the scenes. Read `docs/verse/zones.md` (including its guide
  to building and registering a zone) and `docs/verse/zone-rules.md` before
  changing asset admission, transitions, portals, or rules.
  Keep product colors and world behavior out of Rust Native. Desktop features
  retain model chat, XP, and file-backed replays; mobile disables those host
  dependencies and injects identity. Both use the shared simulation, renderer,
  and Rust Native surface lifetime. `verse
  --capture <file.png>` renders the spawn view without a window. Players
  share the world over Nostr with NIP-MV (`nips/openagents/NIP-MV.md`):
  pose frames, entity states, and gestures through a relay, which
  `scripts/verse-relay.sh` runs locally. Chat follows Horse Isle 1 over
  NIP-C7, NIP-29, and NIP-17 (`docs/verse/chat.md`). Read `docs/verse/`
  before changing the controller, the palette, the world, chat, or the wire
  format.
- `crates/verse-gfx` — Verse's drawing foundations, split out of `verse`
  (#10631): the palette, the glyph atlas and UI batch, the overlay panel,
  GLES shader variants, frame profiling, and the follow camera. `verse`
  re-exports each module under its old path; no zone code belongs here.
- `crates/verse-net` — Verse's Nostr session plumbing, split out of `verse`
  (#10631): the relay link, identities, NIP-MV frames, chat, the chat feed,
  and the XP client (`xp` feature, which `verse`'s `xp-host` enables).
  `verse` re-exports each module under its old path.
- `crates/verse-pbr` — Verse's physical renderer, split out of `verse`
  (#10631): the PBR pipelines, baking, sky, and textured scenes (`pbr`), the
  world mesh (`mesh`), streamed content residency, and the fog distances.
  `verse` re-exports each module under its old path.
- `crates/verse-bake` — Verse's offline lighting baker (#10905): the
  `verse-bake` binary bakes a textured scene's per-vertex sky light over
  several bounces, per-vertex sun visibility for each configured sun
  direction, and the probe grid into a versioned `VBAK` products file keyed
  by a SHA-256 `bake_key`, with a JSON receipt. The CPU backend walks the
  bake's `Bvh` on every core; the GPU backend (`gpu` feature, off by
  default) traces through `wgpu` ray queries, and both share every sample
  so they agree within `GPU_TOLERANCE`. A rebake of an unchanged scene
  reproduces the products' digest. Nothing loads the products yet.
- `crates/verse-core` — what Verse's zones share, under the world runtime
  (#10631): the static `World` and the plaza's layout and `GymSite`, zone
  controls and atmosphere (`zone`), the avatar, the companion agent, the
  crowd of other players, particle effects, tooltips, scene labels, and the
  replay's places. `verse` re-exports each module under its old path.
- `crates/verse-zone-lagrange`, `crates/verse-zone-lab`, `crates/verse-zone-everglade`
  (Everglade with its studio, demolition yard, and pinned pack), and
  `crates/verse-zone-grove` — one crate a zone, over `verse-core`, so an
  edit to a zone recompiles that crate and what depends on it. `verse`
  re-exports each as `zones::<name>`; the tests that walk through the world
  runtime stay in `verse` as `zones/<name>_tests.rs`. Put a zone's code in
  its crate and anything two zones share in `verse-core`.
- `crates/verse-imported` — the chamber's imported content and the RITUAL
  client (#10631), re-exported as `verse::imported` and `verse::ritual`.
- `crates/verse-gym` — the Grid's Gym boards, hall, notes, and results
  panel (#10631), re-exported as `verse::gym*`.
- `crates/xp-ledger` — the NIP-XP ledger a reader derives
  (`nips/openagents/NIP-XP.md`): trust lists, the per-award re-checks, and
  the knowledge-entry parser the `kb-transfer` rule needs. `knowledge`
  re-exports it as `knowledge::xp` and `knowledge::Entry`. It depends on
  `nostr`, `serde`, and `sha2` only, so the phones read XP through Verse's
  `xp-host` feature without linking the knowledge base's model and
  embedding clients; keep it that way.
- `crates/memory-stream` — generative-agent memory scoring with no
  dependencies: `0.99^hours` recency, min-max normalization, the
  equal-weight combination, and a small bounded stream. Alice's briefing
  (`coder::task::agent_recall`) and Everglade's townsfolk share it; keep it
  free of I/O so it builds for `wasm32`.
- `crates/world-tree` — Verse's world tree
  (`openagents.verse-world-tree.v1`): districts, buildings, rooms, and
  objects with stable IDs, standing points, and affordances; object
  states; per-agent known subgraphs; and the place choice a plan grounds
  in. Serde and SHA-256 only, for `coder` and `wasm32`. Everglade's tree
  is generated in `verse-zone-everglade` (`zones::everglade::world_tree`)
  and checked in as `data/everglade.json`; regenerate it with
  `WORLD_TREE_WRITE=1 cargo test -p verse-zone-everglade world_tree_snapshot`
  after a layout change.
- `crates/townsfolk` — Everglade's villagers as data: definitions
  (`openagents.verse-npc.v1`), the roster (`openagents.verse-town.v1`)
  that admits them by digest under budgets with hard ceilings, validation
  against the world tree with typed problems, deterministic routines from
  the town clock and a seed, co-location queries, and the propose/admit
  files flow; rumors (`openagents.verse-rumor.v1`) with their seeded
  diffusion, and talk planning over each villager's bounded memory of the
  player. The data lives in `crates/verse-zone-everglade/townsfolk/`;
  `openagents verse town` is its command, and only the owner admits. Read
  "Authoring townsfolk" in `docs/verse/generative-agents.md` first.
- `crates/everglade-web` — Everglade in a browser: a `cdylib` over Verse's
  `web` feature (`wasm32-unknown-unknown`) that fetches the pinned pack from
  the same origin and draws the zone with WebGPU or WebGL2.
  `scripts/build-everglade-web.sh` builds it; its README holds the page
  contract (canvas ID, output files, and pack URL).
- `crates/physics` — shared, zone-agnostic rigid-body physics for Verse:
  bodies, fixed stepping, restorable world state, and replay traces, growing
  through the Genesis port roadmap (`docs/physics/2026-09-27-genesis-port-roadmap.md`,
  tracking issue #9788). Put generic mechanisms here, not in a zone crate;
  zones supply their fields, controls, and rules. No renderer, I/O, or zone
  types.
- `crates/verse-lagrange` — Sun–Earth L1 rules for the Lagrange 1 zone:
  the circular restricted three-body orbit with RK4 and unstable-mode
  station-keeping, the L1 tidal field, and the EVA construction sandbox built
  on `crates/physics`. No renderer or I/O. Keep named constants and the approximations listed in
  `docs/verse/lagrange-1.md` accurate; convert physics regressions into tests.
- `crates/pylon` — the NIP-PYLON compute provider and its client: beacons
  (`30200`), free NIP-CJ conversation jobs run on a local Psionic model
  server, buyer receipts (`3201`), pool aggregates (`30201`), and
  `RelayField`, the Pylon Field's relay source. The records themselves are
  `nostr::pylon`. `openagents pylon` is its command;
  `scripts/pylon-psionic.sh` runs a pylon. Read `docs/compute/pylon.md`
  before changing admission or what a beacon carries.
- `crates/psionic` — Psionic's serving path (30 crates), imported from
  OpenAgentsInc/psionic, as its own Cargo workspace excluded from the
  root. Build it with `--manifest-path crates/psionic/Cargo.toml`, and
  CUDA builds only on a machine with the toolkit. Read
  `docs/psionic/README.md`.
- `crates/eval-runner` — the hosted eval runner: a NIP-CJ execution
  worker (`nostr::eval_ext::hosted`) that runs extension test sets for the
  phone's chat on our computers through `crates/ext-eval`, for catalog
  tools (`crates/plugin-*` as extensions) and chat-made tools only, with
  the fixed read-and-sandbox-write grant, a per-trainer daily quota, and a
  turn ceiling. It seals each report to the trainer and publishes only on
  the trainer's publish request. It runs on `coderos-4080`; read
  `docs/deployment/eval-runner.md` before changing admission, the quota,
  or what a result carries.
- `crates/nostr` — pure Nostr protocol and verification primitives
  (events, filters, signatures, NIP-19/NIP-44, replacement and deletion,
  Block NIP validators). No storage, no network, no third-party Nostr
  crate.
- `crates/nostr-transport` — bounded authenticated WebSocket transport and exact
  private-artifact delivery. It shares NIP-42 connection handling between
  callers without depending on the Coder executor. Callers still establish
  relay, recipient, operation, and disclosure authority; delivery is not a grant.
- `crates/nostr-relay` — the relay: one binary and one Postgres database,
  extracted from the public CC0 `immortal-relay`. Serves
  `relay.openagents.com`. Deploy assets live in `deploy/` and `Dockerfile`;
  migrations in `migrations/`; protocol fixtures in `tests/fixtures/`.

## Protocol references

- `nips/` — pinned copies of the official Nostr NIPs and the Block/Buzz
  extension NIPs. `nips/manifest.json` records the exact upstream commits.
  `./scripts/sync-nips.sh` refreshes them; a sync never changes the
  implementation without review and a fixture update.
- The Block lane is the model for application behavior: the relay is the
  workspace, and application logic is expressed as event kinds plus relay
  policy rather than a private backend.
- `capabilities/`, `programs/`, `questions/`, and `sources/` hold the local
  registry: one NIP-CAP `kind:30180` manifest per file, one NIP-PRG
  `kind:30182` program per file, one question set per file, and one task
  source per file. They are files before they are events, and a host reads
  them from disk until the relay serves them. A program never carries a
  question's wording; it names a set in `questions/`, which is digested on
  its own so two runs are comparable. A program never carries a command
  either; a `query` step names a source in `sources/`, and what that source
  reads is the machine's business rather than the program's.
- `methods/` holds the well-known method registry that
  `verify.method_conformance` in `crates/coder-one/src/checks/conformance/`
  reads: one digested file per method, with its standard definition and
  citations, how to call a Python implementation, executable property
  checks derived from the definition, its provenance (the tasks it was
  learned from, which never count as its evidence), and its admission
  record. An entry cites a textbook or reference, never a benchmark task.

## Cursor Cloud specific instructions

Cloud Agents already have the pinned toolchain (Rust 1.97.1, rustfmt, and
Clippy), `protoc`, OpenSSL, SQLite, libclang, and `bubblewrap`. The root
workspace crates are fetched. Put each checkout's Cargo output in
`~/work/openagents-target-agentN`, as the Velocity section describes. This
machine has 4 cores and about 16 GB of memory. Set `CARGO_BUILD_JOBS=2`
when you compile `openagents` or `verse`, so a cold build stays within
that memory.

Build `openagents` with `cargo build -p openagents-cli --bin openagents`
before you use `openagents lease`. The day-to-day check is still
`cargo test -p` for the crates you edit, plus `cargo fmt`.

`cargo fetch --locked --manifest-path crates/openagents-mobile/Cargo.toml`
stops because Cargo wants to update that lock file. Leave the phone
workspace until the lock file resolves. Relay release checks need
PostgreSQL (`initdb`, `pg_ctl`, `createdb`). Psionic stays its own
workspace, and this setup does not fetch it.
