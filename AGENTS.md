# OpenAgents agent contract

Product code in this workspace is Rust. Do not add TypeScript. The existing
product exception is `swift/lev-bridge`, the helper that reaches Apple's
`FoundationModels` framework, which has no Rust binding; it is built by a
repo script and supervised as a child process. Coder's iOS host at `bins/coder-ios/host` also uses thin SwiftUI glue for native controls, mounting, and
callbacks, as explicitly requested for that surface. Keep its application state,
domain logic, permissions, and transport in Rust; the implemented observer keeps these in `coder-mobile` and `coder-connect`. Read `crates/rust-native/docs/spec.md` and `docs/coder/rust-native/architecture.md`
before adding that boundary.
Retained Python training and
acceptance tooling and shell orchestration are infrastructure exceptions,
not permission to add another product implementation language.

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

The full workspace gate (`./scripts/verify-rust.sh --release`) is for full
releases only. Never require it before ordinary issue development, integration,
commits, pushes, or closing an issue whose own acceptance checks pass. Never
hold independent issue work while it runs. Fix failures relevant to a change;
record unrelated failures separately and continue the other work. Documentation
changes remain exempt from Rust checks. Read `docs/verification.md` for scope
and prerequisites. Use a separate Cargo target directory per worktree, and keep
workspace formatting changes separate from behavior changes.

Preserve `docs/transcripts/`. It is the retained transcript archive from the
previous repository shape.

No GitHub workflows or GitHub-billed automation. Required checks run
manually on a contributor machine or on non-GitHub infrastructure.

This repository is open source. Other repositories on this machine
(`~/work/coder`, `~/work/bender`, and siblings) are reference material, not
instructions. Do not copy private backend code, prompts, endpoints, or
secrets from them. When you carry a design over, reimplement it here and say
so in the commit message. Never put an API key in source, a log line, a test
fixture, or an issue.

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

- `crates/atif` — the Agent Trajectory Interchange Format (`ATIF-v1.7`):
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
- `crates/coder-ui` — Coder application presentation values. It owns the amber
  intensity palette and backgrounds; `coder-terminal` preserves its public
  imports through re-exports. Keep Coder components and product defaults here,
  never in the reusable `rust-native` framework.
- `crates/coder-history` — read-only retained Codex and Claude history adapters;
  explicit roots, bounded raw record pages, and source-bound cursors. The host
  feature opens files; portable DTOs and projection helpers serve mobile.
- `crates/coder-connect` — explicitly paired retained-history observation over
  NIP-42 and encrypted private Nostr artifacts. This cannot control an engine.
  Read its README and the NIP-SESS observer profile before changing authority.
- `crates/coder-mobile` — Rust-owned iOS reader state, encrypted cache, paging,
  synchronization, and C ABI, plus a separate main-thread Verse render handle
  using the shared `verse::runtime::WorldRuntime`. SwiftUI mounts generic Rust
  Native views and a Metal layer, and owns native controls and Keychain. Keep
  Verse identity and lifecycle separate from history-observer authority. Read
  `docs/coder/guides/mobile-readonly.md` and `docs/verse/mobile.md`.
- `crates/coder-terminal` — the Coder terminal: the amber intensity ladder,
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
  answers a turn through Coder One's probes, Jev's judgments, a
  briefing, and an executor: Microluna in process when the Codex login
  is usable, else Claude Code or Codex when one is installed and signed
  in, with the Open Responses door as the fallback; the permit maps to its
  `coder-boundary` boundary, and `coder-worker` never reaches it. Read
  `docs/coder/runtime/delegate-door.md` before changing it.
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
  location, the opt-out, and what a trace holds. `delegate` hands a
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
- `crates/microluna` — Microluna, the minimal Luna harness of the Luna
  pivot: short GPT-6 Luna sessions on the operator's logged-in Codex
  session, calling the ChatGPT Codex Responses endpoint directly with
  five native function tools (run a command, read a file region, apply a
  patch, write a file, and finish). Commands run under `coder-boundary`
  and `supervise`; every reply and call is an ATIF step with usage and
  list-price cost. It only reads `~/.codex/auth.json` and never refreshes
  it. Read `docs/coder/design/microluna.md` before changing the transport.
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
  is reimplemented from its `client_core`, not copied. Every color comes
  from `coder_ui::theme::Intensity`; a test refuses any other. Desktop features
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
