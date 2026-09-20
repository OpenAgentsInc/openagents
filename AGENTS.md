# OpenAgents agent contract

This repository is a Rust-only workspace. Do not add TypeScript. The one
non-Rust source tree is `swift/lev-bridge`, the helper that reaches Apple's
`FoundationModels` framework, which has no Rust binding; it is built by a
repo script and supervised as a child process.

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

Two skills are vendored under `.agents/skills/`. Read and apply them:

- `.agents/skills/google-developer-style/SKILL.md` — every piece of prose in
  this repository follows the Google Developer Documentation Style Guide:
  `README.md`, this file, every file under `docs/`, code comments and doc
  comments, commit messages, interface copy, error messages, and log lines.
  Read it before you write or review prose.
- `.agents/skills/typesafe-ai/SKILL.md` — read it before you write a Jev
  question set, a threshold, or a client call. The Rust SDK lives in
  `crates/jev`.

[`docs/glossary.md`](docs/glossary.md) defines the terms this repository
uses, and marks which are implemented and which are only specified.

## Crates

- `crates/atif` — the Agent Trajectory Interchange Format (`ATIF-v1.7`):
  a session as ordered steps, the append-only log a running session
  writes them to, and the document a reader renders from it. A decision
  call is a first-class `Call`, so a Jev, Kev, or Lev question and a
  shell command record the same way. Reimplemented from the Harbor
  trajectory RFC's reference implementation; `serde` and `sha2` only, no
  network. Read `docs/coder/traces.md`.
- `crates/gym` — the measurement and control plane for decision models:
  pinned suites, a receipt-chained result store, digested acceptance gates,
  and the terminal that reads them. It scores whatever answers
  `POST /v1/systemone` and knows nothing else about the door. Read
  `docs/gym/` before changing a schema or a gate.
- `crates/jev` — the Rust SDK for TypeSafe's System One API.
- `crates/kev` — the Rust port of the kev decision model: packed prefill,
  block-causal question isolation, pointer readout, and `kev-serve`, a
  TypeSafe-compatible `POST /v1/systemone` server. Documentation and
  conformance numbers live in `docs/kev/`; golden fixtures in
  `crates/kev/fixtures/`; weights stay in `~/work/kev-artifacts/` out of git.
- `crates/coder-terminal` — the Coder terminal: the amber intensity ladder,
  the framed composer, and the shell they draw. It also holds the terminal
  design system every other terminal here depends on — `Intensity`,
  `Ladder`, `frame`, and `rail`. Extend it rather than copying it. The
  rebuild plan lives in `docs/coder/`.
- `crates/coder` — the agent: `classify` routes each turn through Jev,
  `generate` answers through an Open Responses door, and the `coder`
  binary draws the conversation in the terminal or, with `-p`, runs one
  turn from a script. Both modes run the same turn, `coder::turn::run`;
  keep it that way. `docs/coder/headless.md` covers the headless flags and
  the exit codes. Every conversation records itself to
  `~/.openagents/traces/` as it runs; `docs/coder/traces.md` covers the
  location, the opt-out, and what a trace holds.
- `crates/lev` — the same contract answered by Apple's on-device foundation
  model, through the in-repo Swift helper in `swift/lev-bridge`. Build the
  helper with `./scripts/build-lev-bridge.sh`; the crate builds and tests
  without it. Read `docs/lev/` before changing an estimator or the door.
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
