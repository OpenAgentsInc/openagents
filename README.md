# OpenAgents

OpenAgents is building **Coder**: a coding agent that lives in your
terminal, thinks in two stages, runs shell commands on your machine, and
talks to its backend over a public Nostr relay — no account, no bearer
token, no HTTP API. This repository is its home: the agent, the relay,
the protocol, and the SDKs, all in Rust, all public.

## What Coder is

A terminal agent with a deliberate two-stage mind:

- **System One — Jev.** Every turn starts with typed judgments, not
  vibes. A structured state object (task, transcript, repo vocabulary)
  goes to Jev, which returns probabilities and scores: `respond` vs
  `clarify` vs `end`, confidence, risk, progress, whether code is
  involved. A routing table — plain Rust, reviewable thresholds — turns
  the judgment into the next step. The same machinery judges each round
  of shell output: `pass` / `retry` / `stop`, `useful`, `damage`.
  Small, fast, cannot fabricate.
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
  `docs/coder/relay-transport.md` measures what it costs to route a turn
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
| `crates/jev` | The TypeSafe Jev SDK — typed questions, `Choice`/`Noul`/`Score` answers. |
| `crates/supervise` | One subprocess supervisor: a job owns its process tree until cleanup finishes, and its output is bounded while it is read. Unix only, and it says so. |
| `crates/nostr` | Pure protocol primitives: events, filters, NIP-19/42/44/98, signers. No third-party Nostr crate. |
| `crates/nostr-relay` | The `nostr-relay` binary: WebSocket gateway + Postgres store. Deployed at `wss://relay.openagents.com`. |

## Protocol

`nips/` holds three lanes: `official/` (synced from nostr-protocol),
`block/` (synced from block/buzz — the app-logic-as-NIPs model), and
`coder/` (authored here — NIP-CJ and whatever follows). `manifest.json`
pins the synced lanes; `scripts/sync-nips.sh` checks parity. Design docs
live in `docs/coder/`: the service spec, the relay backend plan, and
`shell-loop.md` for the command loop.

## Run it

```bash
cargo run -p coder                                  # the terminal
cargo run -p coder -- -p "count the crates"         # one turn, from a script
cargo run -p coder -- -p --json --trace one.jsonl "count the crates"
```

`-p` runs one turn without a terminal, writes the reply to standard
output, and exits 0 for an answer, 2 for a declined turn, and 1 for one
that did not finish. `docs/coder/headless.md` has the flags and the JSON
report.

Keys in the environment decide the door: `TYPESAFE_API_KEY` turns
classify on; `CODER_DOOR_KEY`/`CODER_DOOR_URL`/`CODER_MODEL` take an
own-key door; `CODER_WORKER` + `CODER_RELAY` route the turn through the
relay; none of it falls back to the stub. Naming two doors at once is
refused rather than resolved. `CODER_MODEL` takes a lane — `gemini` for
`google/gemini-3.8-flash`, `glm` for `zai/glm-5.3-flash` — or any model
id the gateway serves. To answer those jobs from the other side, run
`cargo run -p coder --bin coder-worker`; `docs/coder/relay-transport.md`
has the walkthrough and what the round trip costs. To run a relay
locally, `docs/deployment/runbook-local-dev.md` has the walkthrough.

## Verify

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/test-postgres.sh
./scripts/check-dependencies.sh
```

Read [the dependency policy](docs/dependencies.md) for the advisory exception,
source and license checks, and the unresolved repository license conflict.

CC0-1.0. The repository contract is `AGENTS.md`.
