# OpenAgents agent contract

Product code in this workspace is Rust. Do not add TypeScript. The product
exception is `swift/lev-bridge`, the helper that reaches Apple's
`FoundationModels` framework, which has no Rust binding; it is built by a
repo script and supervised as a child process. Retained Python training and
acceptance tooling and shell orchestration are infrastructure exceptions,
not permission to add another product implementation language.

Documentation-only changes do not require the Rust verification gate, including
before a push. Check links, paths, and retained artifacts for documentation
reorganizations. Comment edits and documentation path updates do not require
workspace-wide tests; if an embedded document's loading path changes, check only
the affected consumer.

For Rust behavior changes, use the pinned toolchain and
`./scripts/verify-rust.sh` for the manual gate. Read `docs/verification.md`
for verification scope, minimum compiler versions, feature coverage, and
external prerequisites. Use a separate Cargo target directory per worktree.
Keep workspace formatting changes separate from behavior changes.

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
  the available balance.
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
  reads and the `/dashboard` pages, and the optional `billing` document that
  mounts plans, checkout, signed provider webhooks, and owner-only
  subscription management over `tenancy::billing` — under which a
  decision call names a subscribed workspace whose plan covers the
  door. Read
  `docs/decision-models/service/gateway.md` before changing a refusal code, a
  bound, or the reservation lifecycle.
- `crates/discovery` — the public discovery surface every origin shares:
  the bundled documentation corpus the MCP documentation tools and the
  `/v1/docs` API read, the machine-readable document set under
  `docs/agents/` served at `/`, the well-known agent card and
  agent-skills index, the MCP server card shape, and the plugin
  manifests under `plugins/`. Nothing in it authenticates or answers a
  decision call.
- `crates/jev` — the Rust SDK for TypeSafe's System One API.
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
- `crates/coder-terminal` — the Coder terminal: the amber intensity ladder,
  the framed composer, and the shell they draw. It also holds the terminal
  design system every other terminal here depends on — `Intensity`,
  `Ladder`, `frame`, and `rail`. Extend it rather than copying it. The
  rebuild plan lives in `docs/coder/`.
- `crates/coder` — the agent: `classify` routes each turn through Jev,
  `generate` answers through an Open Responses door, and the `coder`
  binary draws the conversation in the terminal or, with `-p`, runs one
  turn from a script. Both modes run the same turn, `coder::turn::run`;
  keep it that way. `permit` is the host's answer to whether a turn runs
  commands at all, built from the route and the operator's setting before
  anything generates and narrowing from there; a reply becomes an
  executable plan only under a permit that runs one, so keep execution
  policy there rather than in what the model is told.
  `docs/coder/guides/headless.md` covers the headless flags and
  the exit codes. Every conversation records itself to
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
  `docs/programs.md` covers all five. The crate's second binary,
  `coder-worker`, is the other end of the relay door: it answers NIP-CJ
  job requests from a relay through an Open Responses door.
  `docs/coder/measurements/relay-transport.md` is the measured proof that the two ends
  meet, and it holds the per-transport latency and the refusal causes.
- `crates/capability` — the capability manifest contract `coder` and
  `coderbench` share. Reading a registry is inert; an executable probe
  runs only under an approval the operator recorded with the
  `capability-trust` binary, which pins the manifest's digest and the
  adapter's canonical path and contents in a store outside any checkout.
  Probes run bounded through `supervise`, and a probe that cannot answer
  cleanly is `unknown`, never `present`.
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
- `crates/voyager` — open-ended agent episodes in a Minecraft world: a
  supervised local server, a world-manifest registry under `worlds/`, and
  the bot side through the nightly-built helper in `mc-bridge/`, built by
  `./scripts/build-mc-bridge.sh` and driven as a child process speaking
  line-delimited JSON — the `swift/lev-bridge` precedent in Rust. The crate
  builds and tests without the helper. Read `docs/voyager/` before
  changing an episode, a world manifest, or the bridge protocol.
- `crates/nostr` — pure Nostr protocol and verification primitives
  (events, filters, signatures, NIP-19/NIP-44, replacement and deletion,
  Block NIP validators). No storage, no network, no third-party Nostr
  crate.
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
