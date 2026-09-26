# Earn in the coder repository

Status: historical sibling-repository survey, 2026-09-19. Commands and service
claims below describe that reviewed Coder source, not this repository. For
current public implementation, use the [migration tracker](coder/migration-status.md),
[agent labor plan](agents/market-infrastructure.md), and
[free labor runtime](coder/runtime/free-labor.md).

A map of Coder's Earn functionality: the mode that offers a machine's spare
capacity to the inference mesh for credit on the owner's ledger. Surveyed
2026-09-19 in `~/work/coder` (`OpenAgentsInc/coder`).

## What Earn is

Earn turns a person's machine into a provider on Coder's mesh. The person
runs `coder earn on`, sets limits, and the machine then serves two kinds of
work inside those limits:

- **Inference.** The fleet coordinator dispatches other accounts' model
  requests to the node's worker.
- **Runs.** The service's `earn` claim lane offers confined agent runs —
  guest executions that clone a repository and drive coding turns — which
  settle at a per-hour rate.

Every finished unit of work produces a signed receipt, which the service
prices and credits to the owner's ledger in cents. The canonical design doc
is `docs/earn/README.md` in the coder repo; the epic is issue #745. Earn is
experimental as of the 0.5.0 release: the command, config, supervisor,
pairing, windows, drain, and board are implemented and tested, and one
provider machine has recorded a verified receipt through Ollama against a
local service and coordinator. No node has recorded a receipt on the
production mesh.

## The moving parts

```text
earn node (a person's machine)          coder-mesh coordinator
┌─────────────────────────────┐        ┌────────────────────────────┐
│ coder earn serve supervisor │        │ coder-fleet, one Cloud Run │
│  · reads earn.toml          │        │  instance                  │
│  · probes the machine       │ grant  │  · worker registry, queue  │
│  · picks a catalog row      │───────►│  · verification floor      │
│  · fetches + verifies       │ frames │  · signs a receipt per     │
│    the artifact             │        │    finished dispatch       │
│  · runs the worker:         │        │  · bounded receipt outbox  │
│    coder-inference-daemon   │        └──────────────┬─────────────┘
│    or an in-process link to │                       │ POST /v1/earn/receipts
│    a local Ollama           │                       ▼
└─────────────────────────────┘        coder service (coder-serve)
                                        · verifies Ed25519 receipts
                                        · prices out of prices.toml
                                        · credits the owner's ledger
                                        · serves /earn, /earn/ws,
                                          /earn/state, GET /v1/earn
```

## The command grammar

One grammar in `coder_tools::earn` serves every surface: `/earn` inside a
session, `coder earn` on the terminal binary, `coder-cli earn`, and the
`coder_cli` MCP tool.

| Command | What it does |
| --- | --- |
| `coder earn on` | Pairs the machine as a provider when it holds no pairing, writes `~/.openagents/earn/earn.toml` with probed defaults, installs and starts the service unit, prints status. |
| `coder earn off` | Stops the unit; keeps the pairing and config. `--forget` also removes the computer from the account and drops its key. |
| `coder earn status` | Prints the card the supervisor's last heartbeat wrote. |
| `coder earn doctor` | Reports the probe and confinement checks; exits nonzero when the machine can offer nothing. |
| `coder earn limit <field> <value>` | Sets one `earn.toml` field and signals the supervisor to reread it. |
| `coder earn serve` | The supervisor entry point the installed unit runs. |
| `coder earn board` | Draws the mesh board in the terminal from `/earn/state`. |

The service unit is `~/Library/LaunchAgents/com.openagents.earn.plist` on
macOS and `~/.config/systemd/user/coder-earn.service` on Linux. Both run
`coder earn serve`. Everything else Earn writes lives under
`~/.openagents/earn/`: `earn.toml` (mode `0600`), `status.json` (the
heartbeat, rewritten atomically), `earn.log`, the drain marker, and the
freshness-tracked grant file.

## The limits

`earn.toml` bounds what the node offers. A field left unset takes the probed
default:

| Field | Default |
| --- | --- |
| `memory` | Half of unified memory on a Mac; VRAM less 2 GiB on a Linux NVIDIA host |
| `disk` | `80g`, bounding `~/.openagents/models` |
| `hours` | `always`; windows such as `mon-fri 22:00-07:00,sat-sun` |
| `idle_only` | `off` |
| `sequences` | `1` |
| `runs` | The probed capacity under confinement, `0` without a container runtime, `CODER_SANDBOX_RUNTIME`, and `CODER_RUN_MEMORY` |
| `kinds` | `inference` and `runs` |
| `mesh` | `https://mesh.openagents.com` |
| `engine` | `ollama` when one answers the probe, else `daemon` |
| `ollama_host` | `http://127.0.0.1:11434` |
| `[ollama].models` | Every eligible local model; a list narrows the offer set |

## The supervisor loop

`coder earn serve` beats every 15 seconds. Each beat it rereads
`earn.toml`, probes the machine, picks the largest row of
`fleet/catalog.toml` whose `min_memory_bytes` fits the `memory` limit and
whose `backends` names the probed backend, and fetches the row's artifact
from its Hugging Face source — resumable, verified against the row's
`weight_hash`, refused past the `disk` limit. It then keeps a worker alive
inside the `hours` window, restarts an exited worker on a backoff from one
second to one minute, drains at a window's edge or on a spec change, and
rewrites `status.json` every beat. `SIGHUP` rereads the config between
beats; `SIGTERM` drains and writes a final `off` state.

The worker takes one of two shapes:

- `engine = "daemon"`: launches `coder-inference-daemon --engine local
  --backend cuda|metal --join-fleet` under the earn grant. The daemon is not
  distributed in the 0.5.0 terminal release (issue #782); a CoderOS host
  builds it from `os/pkgs/coder-inference-daemon.nix`, other machines build
  it from source.
- `engine = "ollama"`: runs the fleet adapter's `link::run` as an in-process
  task fronting the local Ollama. The supervisor offers every eligible model
  on Ollama's disk under its Ollama name and blob digest, pins offered models
  resident with `keep_alive = -1` while a window holds, and unloads what it
  pinned at the edge. It never pulls, creates, or deletes a model.
  `docs/earn/ollama.md` is the design.

The worker proves itself to the coordinator with an *earn grant*: a
kind-`27242` signed event minted by `POST /computers/{id}/earn-grant`,
implemented in `coder_auth::grant` and specified in
`crates/coder-auth/docs/events.md`. A coordinator started with
`--earn-issuer-key` admits a worker presenting one and attributes its work to
the account and the computer. Forgetting the computer lands its grant ids on
`GET /v1/earn/grants/revoked`, which the coordinator polls.

## The verification floor

An earn worker joins unproven and routes nothing until it clears the floor:

- The coordinator sends the pinned probes of `fleet/probes.toml`, keyed on
  the `(model, backend)` the offer advertises, at registration, on the
  `CODER_PROBE_INTERVAL` cadence while idle (fifteen minutes unset), and
  after a replicated request disagrees. `model = "*"` rows cover engines'
  open offers.
- `CODER_REPLICATE_PERCENT` sets the share of live dispatches that re-run on
  a second verified worker before their receipts post (the floor is two
  percent).
- An unverified worker routes nothing and its receipts post held. A pass
  restores it and re-posts the held receipts verified under the same ids. A
  third failed pass in a row disconnects it with the `unverified` error.

The floor's word — `verified`, `disputed`, `last_probe`, `held_receipts` —
rides the worker's row on `/v1/fleet/status`, the node's `status.json`, and
`/earn status`. The node's own read of the coordinator is
`GET /v1/fleet/worker/status` under its grant.

## Receipts and pricing

A coordinator started with `--receipt-key` signs one receipt per finished
dispatch — the attributed account and computer, the model, the request, the
token counts, and whether the floor checked the answer — and posts batches
to `POST /v1/earn/receipts` from a bounded in-process outbox that retries
until each id is answered `accepted`, `held`, or `refused`. The outbox's
depth is the loss bound on a coordinator restart; `docs/ops/mesh.md` covers
the operator side.

The service verifies the Ed25519 signature against `CODER_EARN_RECEIPT_KEY`,
stores the row once under its id, prices it out of `CODER_EARN_PRICES`
(the committed default is `earn/prices.toml`), and credits the owner's
ledger on the `earn` lane. The default table credits 25 cents per million
tokens for a model with no row, 6 cents per metered hour for a run, and
charges a mesh caller 50 cents per million tokens. A receipt naming a
computer that is not a provider on the account refuses, so a forgotten
computer's late deliveries never credit an ex-provider. `GET /v1/earn` and
the `list_earn_receipts` MCP tool read the account's receipts, the cents
credited, and the count still held.

## Runs on the node

While `kinds` admits `runs` and `runs` names a slot, the supervisor also
holds a claim on the service's `earn` lane under the paired computer's key.
An order placed with `placement: "earn"` (`/cloud earn <directive>` in the
terminal, `earn nodes` on the order form) waits in that queue, and the claim
offers the oldest waiting run another account ordered — a node never picks
up its own account's run unless the order set `own_ok`. The run lands in a
guest under the confinement the claim required, clones its repository with a
short-lived repository-scoped installation token the service mints into the
offer, and posts its turns to the service's door under the run's
`x-coder-run` header — so neither the orderer's Git token nor a door key
reaches the node. A finished run writes a receipt priced at the run rate.

## The board

`/earn` on the service draws the earn side of the mesh live, as a projection
of reports and nothing else: the book (one column a catalog model — supply
against demand, price, p50 wait to first token), the tape (priced receipts
and probes, newest first, held rows updating in place), the devices list
(each node's platform, model, state word, and fetched-bytes bar), and the
weight map of earn workers. The read is public and tiered: a signed-out
viewer sees shortened computer ids, an owner sees its own nodes by name, an
administrator sees full ids and refusal reasons. A node goes `gone` ninety
seconds after its last report.

`/earn/ws` streams the board with snapshot-then-events and socket resume;
`/earn/state` answers the projection for `coder earn board`, the terminal's
`/earn` card, and the `earn_board` MCP tool; `/earn?fixture=first-node`
steps through the recorded replay under
`crates/coder-ui-core/fixtures/earn/`.

## The spend side

The mesh lane is what earn nodes serve. `model: "mesh"` or
`model: "mesh/<model>"` on `/v1/responses`, `/lane mesh` in a session, and
`lane: "mesh"` on `order_run` all route there. Admission is a positive
balance rather than the free quota; charging comes from the `[charge]` table
of `earn/prices.toml` and posts as a `generation` charge on lane `mesh`.
Every call carries the caller's account to the coordinator as
`x-coder-account`, which is what the provider's receipt attributes against.
Prompts and answers run in plaintext on machines other people own; the
terminal says so once per session when the lane is first taken.

## Where the code lives

| Path | Role |
| --- | --- |
| `crates/coder-tools/src/earn.rs` | The shared grammar, `Config` for `earn.toml`, path constants, the probe, `DEFAULT_MESH`. |
| `crates/coder-tools/src/earn/{catalog,choose,fetch,launch,ollama,schedule,unit}.rs` | Catalog parsing, row selection, resumable verified artifact fetch, worker launch spec, the Ollama probe and steering, `hours` windows, and the launchd/systemd unit files. |
| `crates/coder-tools/src/earn_tests.rs` | The grammar and config tests. |
| `bins/coder-terminal/src/earn_cmd.rs` | The `coder earn` clap surface; the async halves — pairing and `--forget`. |
| `bins/coder-terminal/src/earn_serve.rs` | The supervisor: heartbeat, grant minting and renewal, worker lifecycle, drain, `status.json`. |
| `bins/coder-terminal/src/earn_board.rs` | The terminal's board read over `/earn/state`. |
| `bins/coder-cli/src/earn.rs` | `coder-cli earn`, including `earn board`. |
| `bins/coder-serve/src/earn.rs` | The service surface: `POST /v1/earn/receipts` intake under `CODER_EARN_INTAKE_KEY`, `GET /v1/earn/grants/revoked`, `GET /v1/earn`. |
| `bins/coder-serve/src/earn_board.rs`, `earn_board_project.rs`, `earn_board_stream.rs` | The `/earn` page, the projection that scrubs by viewer, and the `/earn/ws` socket. |
| `bins/coder-serve/src/mcp/earn.rs` | The `earn_board` MCP tool. |
| `bins/coder-serve/src/run/mod.rs` | `Placement::Earn` and the `own_ok` flag on run orders. |
| `bins/coder-serve/src/computers.rs` | Pairing, `POST /computers/{id}/earn-grant`, `POST /computers/seen` heartbeats. |
| `crates/coder-contract/src/earn.rs` | The board's wire types: `Board`, `Prices`, `CatalogRow`, the event log, `TAPE_BOUND`, `GONE_AFTER_MS`. |
| `crates/coder-auth/src/earn_receipt.rs` | The receipt type and its canonical signed payload. |
| `crates/coder-auth/src/grant.rs` | `mint_earn` and `verify_earn` for the kind-`27242` earn grant. |
| `crates/coder-ui-core/earn.rs` | The board renderer every surface shares; `earn_tests.rs` and `snapshots/earn-*.txt` pin its states. |
| `crates/coder-ui-core/fixtures/earn/first-node/` | The recorded first-node replay the fixture mode draws. |
| `bins/coder-fleet/` | The coordinator: earn-grant admission (`--earn-issuer-key`), the probe floor (`fleet/probes.toml`, `--probe-interval`, `--replicate-percent`), receipt signing (`--receipt-key`), the outbox, and the revocation poll (`--earn-service`). |
| `bins/coder-fleet-adapter/src/link.rs` | The adapter worker `earn serve` runs in-process for `engine = "ollama"`. |
| `bins/coder-inference-daemon/` | The native worker binary for `engine = "daemon"`. |
| `bins/coder-fleet/tests/fleet/earn.rs`, `bins/coder-serve/tests/earn_socket.rs` | The fleet-side and socket-level integration tests. |
| `earn/prices.toml` | The committed price table. |
| `fleet/catalog.toml` | The model rows a node can offer and the `[open]` engine list. |
| `fleet/probes.toml` | The verification floor's pinned probes. |

## The docs

| Doc | Role |
| --- | --- |
| `docs/earn/README.md` | The canonical design: grammar, limits, supervisor, grant, floor, receipts, board, spend side. |
| `docs/earn/operator.md` | The node runbook: reading status, unverified loops, the outbox, missing prices, leaving the program. |
| `docs/earn/nodes.md` | The verification record, one section a machine; holds the 2026-09-14 Mac/Ollama run and the pending `coderos-4080` row. |
| `docs/earn/ollama.md` | The Ollama engine design: routes used, open admission, steering, risks, out-of-scope. |
| `docs/earn/board.md` | The board test procedure: fixture replay, a local mesh, the socket, a real node, the three reads. |
| `docs/ops/mesh.md` | The `coder-mesh` coordinator runbook: what it holds, reading it, the floor, the secrets. |
| `docs/fleet/protocol-v1.md` | The worker protocol, including the verification-floor section. |
| `docs/data.md` | What leaves the machine on each earn socket. |
| `docs/releases/0.5.0.md` | The release note's experimental framing. |

## Status and cautions

- Earn is experimental in 0.5.0. The one recorded verified receipt ran
  through Ollama against a local service and coordinator on 2026-09-14; the
  production mesh was unreachable at record time, and `docs/earn/nodes.md`
  keeps the `coderos-4080` row pending. Issue #782 tracks distributing the
  daemon so a machine without Ollama can serve.
- An earn node reads the prompts, activations, outputs, and repositories
  that run on it. The design says so plainly; do not earn on a machine whose
  owner would not accept that.
- Receipt credit settles on the Coder ledger in cents. There is no separate
  payout system in this design.
- The coordinator holds its registry and receipt outbox in one process's
  memory; a roll or crash loses the unacknowledged outbox.
