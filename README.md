# OpenAgents

OpenAgents is building **general agent infrastructure**: typed decisions,
programs, extensions, evidence, permissions, execution, and coordination that
can serve many kinds of agents. **Coder**, the terminal coding agent, is the
first specialization. It supplies concrete repository, shell, test, and code
review workflows that exercise those shared foundations.

This repository contains the agent, decision-model services and SDKs, relay,
and public protocols. Product implementation is Rust, with the documented Swift
bridge for Apple's on-device model. The [general agent architecture](docs/agents/README.md)
separates reusable contracts from domain adapters and records what remains
before other specializations can claim support.

## Coder One on Terminal-Bench 4.0

Coder One wraps a frontier coding agent in typed Jev judgments: it probes the
task environment, packs a briefing, checks the result, and repairs what a
check shows is wrong. On the 26 Terminal-Bench 4.0 tasks with a valid trial
on both sides, on the same host and the same model (Claude Opus 5.5), Coder
One passed **11 tasks for $32.87** where Claude Code alone passed **9 for
$59.27**: as many or more passes for 45% less money and 38% less time.
One attempt per task, so the cost result is strong and the accuracy
difference is small.

Read the [assessment](docs/terminal-bench/2026-09-23-coder-one-vs-claude-code-tb4.md),
the [Terminal-Bench status](docs/terminal-bench/README.md), and the
[tunable Coder design](docs/optimization/coder-components.md).

## Programming and improving agents

The target architecture separates semantic task contracts, replaceable AI
implementations, bounded optimization, and host enforcement. DSPy and GEPA can
help author and search implementations; Gym and domain evaluators measure
whether they improve complete tasks. Typed decisions and generation are useful
building blocks, with permissions, privacy, and effect control owned by code.

Read the [AI programming and optimization design](docs/optimization/README.md),
[standalone Nostr specifications](nips/openagents/README.md), and
[proposed integration issues](docs/optimization/proposed-issues.md). The design
applies across agent domains; implementation of this integration is deferred.

## What Coder is

A terminal agent with a deliberate two-stage mind:

- **System One — Jev.** Every turn starts with typed judgments, not
  vibes. A structured state object (task, transcript, repo vocabulary)
  goes to Jev, which returns a choice and its probabilities: `respond`
  vs `clarify` vs `end`. A routing table — plain Rust, reviewable — turns
  the judgment into the next step. The same machinery judges each round
  of shell output: `pass` / `retry` / `stop`.
  Typed outputs constrain answer shape; correctness still requires evaluation.
- **System Two — a model door.** The generative side is a trait
  (`Generate`), not a vendor. Own-key Open Responses endpoints, a remote
  worker over the relay, or a stub for tests — the loop doesn't care
  which serves it.

Between them sits **the shell loop**: the model can answer in prose or
emit a plan — a JSON array of `{command, why}` proposals. The terminal
runs them bounded (timeouts, output caps, a deny list for
machine-enders), Jev judges the round, the outcomes fold back into the
transcript, and the model plans again or answers. That's how Coder looks
around a repository instead of guessing at it.

And a **repo context** layer rides every turn: a bounded card of the
workspace (members, docs, top level) plus a `git grep` sniff of the
draft's own terms, so project questions land on project knowledge.

An **about this application** block rides alongside it: Coder's version,
the executable, the working directory it was launched from and keeps, the
workspace repository or the fact that there is none, and what the terminal
shows — the bottom-right rail is `input/output` generation tokens for the
latest completed turn, not a session total, and classifier calls are not
in it. Questions about Coder itself are answered from that block without
searching the workspace, which on an installed machine is the user's
project and not Coder's source; the block says when the source is
unavailable.

## Why Nostr

Coder's backend is a relay, not a service.

- **Your `npub` is the account.** NIP-42 authenticates the WebSocket
  itself; every event is already signed. No signup, no GitHub OAuth, no
  bearer tokens to leak — a fresh keypair on first run is a working
  anonymous identity, and an allowance ledger keys off the public key.
- **The wire protocol is public and inspectable.** NIP-CJ (`nips/openagents/`)
  defines the job traffic: ephemeral, NIP-44-encrypted request events to
  the worker, `judgment`/`partial`/`status`/`result` events back. The
  relay stores nothing — it fans ciphertext out to subscribers and
  forgets it. You can watch your own job flow with `nak`.
- **The fulfillment worker is just another relay client.** It holds the
  provider keys as deployment secrets; the terminal never does.
  Quotas, judgment, and streaming all live worker-side, expressed as
  events — application logic as NIPs, the pattern the Block lane uses
  for Buzz. `cargo run -p coder --bin coder-worker` is one;
  `docs/coder/measurements/relay-transport.md` measures what it costs to route a turn
  through it.
- **One relay serves everything — and it's yours if you want it.**
  `wss://relay.openagents.com` runs the same `nostr-relay` binary this
  repo ships: Postgres-backed, NIP-42 auth, NIP-29/45/50/59/65/70/86/98,
  Blossom media. Point `CODER_RELAY` at any relay that speaks the
  protocol — ours, someone else's, or one you run yourself. The whole
  stack self-hosts: `cargo run -p nostr-relay` against your own Postgres
  is the production software, and the terminal, the protocol, and the
  job traffic are all in this repo. No part of the path requires
  OpenAgents infrastructure.

## Why Jev

Most agents route on an LLM's mood. Coder routes on a typed judgment:
the state is structured, the questions are named, the answers come back
as probabilities with confidences, and the routing table is a function
you can read. When the loop decides whether a failed `cargo test`
deserves a retry or a stop, that decision is a Jev verdict you can log,
tune, and test — not a paragraph you hope a bigger model got right.

## Crates

| Crate | What it is |
| --- | --- |
| `crates/atif` | The Agent Trajectory Interchange Format: a session as ordered steps, and the append-only log one writes as it runs. |
| `crates/coder` | The agent: classify-then-generate turns, the shell loop, the relay client, and the binary that runs a turn in a terminal or headlessly with `-p`. Every conversation records itself to `~/.openagents/traces/`. `coder-worker`, the second binary, answers NIP-CJ jobs from the other side of the relay. |
| `crates/coder-terminal` | The amber terminal: composer, editor, spinner, intensity ladder, ratatui rendering. |
| `crates/coder-boundary` | The filesystem write boundary a delegated executor runs under (macOS sandbox or Linux bubblewrap) and independent workspace snapshots. Unsupported enforcement is refused. |
| `crates/coderbench` | The benchmark harness: runs a Coder task against a pinned repository, observes the trace and the workspace, and grades against a golden. |
| `crates/capability` | The NIP-CAP capability manifest contract and the `capability-trust` approval store that lets a probe or executor run. |
| `crates/jev` | The TypeSafe Jev SDK — typed questions, `Choice`/`Noul`/`Score` answers. |
| `crates/kev` | The Rust port of the kev decision model and `kev-serve`, a TypeSafe-compatible `POST /v1/systemone` door. |
| `crates/lev` | The same door answered by Apple's on-device foundation model through the `swift/lev-bridge` helper. |
| `crates/gym` | Measurement for decision models: pinned suites, receipt-chained results, digested acceptance gates, and the terminal that reads them. |
| `crates/supervise` | One subprocess supervisor: a job owns its process tree until cleanup finishes, and its output is bounded while it is read. Unix only, and it says so. |
| `crates/nostr` | Pure protocol primitives: events, filters, NIP-19/42/44/98, signers. No third-party Nostr crate. |
| `crates/nostr-relay` | The `nostr-relay` binary: WebSocket gateway + Postgres store. Deployed at `wss://relay.openagents.com`. |

## Protocol

`nips/` holds three lanes: `official/` (synced from nostr-protocol),
`block/` (synced from block/buzz — the app-logic-as-NIPs model), and
`openagents/` (authored here — the shared agent protocols). `manifest.json`
pins the synced lanes; `scripts/sync-nips.sh` checks parity. The
[protocol overview](nips/openagents/README.md) explains how the contracts fit
together. [General architecture](docs/agents/README.md) covers the shared
infrastructure; [Coder design](docs/coder/design/README.md) covers the first
specialization.

## Run it

```bash
cargo run -p coder                                  # the terminal
cargo run -p coder -- -p "count the crates"         # one turn, from a script
cargo run -p coder -- -p --json --trace one.jsonl "count the crates"
```

To run the terminal from another project's directory, use the launcher:

```bash
alias coderdev=~/work/openagents/scripts/coderdev   # then `coderdev` anywhere
CODERDEV_ENV_FILE=~/.config/openagents/coder.env coderdev -p "count the crates"
```

`scripts/coderdev` builds `coder` with the pinned toolchain first, so an
edit in the source tree is in the binary that runs and a build that fails
stops the launch instead of running the previous executable. It keeps the
directory you launched from, forwards arguments unchanged, honors
`CARGO_TARGET_DIR`, and writes one line to standard error naming the source
revision, whether the tree was dirty, and the executable it ran. Keys in
`CODERDEV_ENV_FILE` load into the child process only. Reload your shell,
or `source` the file that defines the alias, after changing it.
`./scripts/test-coderdev.sh` exercises the launcher against a stub Cargo.

`-p` runs one turn without a terminal, writes the reply to standard
output, and exits 0 for an answer, 2 for a declined turn, and 1 for one
that did not finish. `docs/coder/guides/headless.md` has the flags and the JSON
report.

[Coder as a Decision Router consumer](docs/coder/design/coder-as-decision-router-consumer.md)
defines the flagship product vision, the existing implementation, and the
issues that connect typed decisions to bounded programs and verified work.

The [TypeSafe-native Coder analysis](docs/coder/design/typesafe-agent-analysis.md)
and [delivery roadmap](docs/coder/design/typesafe-agent-roadmap.md) apply the
TypeSafe founder's coding-agent proposal to Coder: shared evidence,
task-specific context, native coding operations, progressive tools, and
bounded parallel/background work. The
[project snapshot](docs/coder/design/2026-09-21-project-roadmap-snapshot.md) maps
that plan to the consumer and Decision Router work in flight.

The [program and extension specification](docs/extensions/README.md) defines
how reusable workflows, bounded Wasm plugins, scoped skills, progressive
discovery, and packages fit that architecture.

[Delegate work to Devin with Coder](docs/coder/guides/devin-delegation-runbook.md)
is the operator runbook for local setup, six-session batches, remote workers,
result checks, and supervised integration.

Keys in the environment decide the door: `TYPESAFE_API_KEY` turns
classify on; `CODER_DOOR_KEY`/`CODER_DOOR_URL`/`CODER_MODEL` take an
own-key door; `CODER_WORKER` + `CODER_RELAY` route the turn through the
relay; none of it falls back to the stub. Naming two doors at once is
refused rather than resolved. `CODER_MODEL` takes a lane — `gemini` for
`google/gemini-3.8-flash`, `glm` for `zai/glm-5.3-flash` — or any model
id the gateway serves. To answer those jobs from the other side, run
`cargo run -p coder --bin coder-worker`; `docs/coder/measurements/relay-transport.md`
has the walkthrough and what the round trip costs. To run a relay
locally, `docs/deployment/runbook-local-dev.md` has the walkthrough.

## Verify

```bash
./scripts/verify-rust.sh
```

Read [the Rust verification policy](docs/verification.md) for the toolchain,
minimum versions, feature matrix, external prerequisites, and explicit skips.

Read [the dependency policy](docs/dependencies.md) for the advisory exception,
source and license checks, and the unresolved repository license conflict.

CC0-1.0. The repository contract is `AGENTS.md`.
