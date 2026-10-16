# Computers: one API over every place work can run (2026-10-15)

Status: thinking, building on what's built. Part of the
[OpenAgents SDK](README.md).

Sources:
- the [computers layer](../cloud/computers.md) (`oa-computers`, #11624);
- the [Factory plan](../research/factory/README.md) (one lifecycle layer,
  driver contract, conformance);
- the [SPACE assessment](../research/perplexity/2026-10-15-space-assessment.md)
  (a fast Firecracker driver, credentials injected at egress);
- the [computers-for-chats audit](../audits/2026-10-14-computers-for-chats-audit.md).

## The idea

A **computer** is a place an agent's work runs, with a disk, a shell and
files. Today there are three kinds, handled three ways:

| Kind | How it's handled today | Speed |
| --- | --- | --- |
| Our cloud: a GCE VM per computer | `oa-boat` (Boat-shaped API), driven by `oa-computers` | About 77 s to a first turn, 52 s to resume, disk only |
| Your own Mac or PC | Coder as a host (NIP-HOST enrollment), Mac jobs, own runs | Instant, free; it's your machine |
| A fast sandbox (SPACE / Firecracker) | Not built; a spike is planned (#11728) | SPACE reports 60 ms median creates with warm pools |

**The Computers API makes these one thing.** You ask for a computer with
what you need; we decide where it runs; you get one id and one set of
operations, whichever driver serves it.

## Two sides, two contracts

The most important design choice is keeping these apart:

```
   developers, our apps, agents
              │
              ▼   Computers API  (northbound, public)
   api.openagents.com/v1/computers ── oa-computers: intent, Postgres, reconcile
              │
              ▼   computer-driver  (southbound, provider contract)
   ┌──────────┼───────────────┬───────────────────┐
   oa-boat    own machine      SPACE (Firecracker)  later: Pylon, Kubernetes
   (GCE)      (Coder host)     over Boat API/HTTP
```

- **Northbound: the Computers API.** It's what people call: create, list,
  wake, stop, exec, files, snapshot, fork, delete. It speaks in *what you
  need* (an environment, a size, what may happen to it), never in driver
  names.
- **Southbound: `computer-driver`.** It's what a driver implements: the
  Core, Suspend, Snapshot and Fork profiles, declared capabilities, refusal
  before any state changes, and `computer-conformance` as its test.
  Out-of-process drivers speak the Boat API.

Today the northbound API is `oa-computers`'s own internal HTTP API. The
website calls it with a service token and re-serves part of it at
`/v1/computers` for chats. Making it public means:
- serving it on `api.openagents.com/v1/computers` with normal keys;
- per-account ownership on every row;
- documenting it.

## The resource

```
Computer
  id                 cmp_…
  owner              account (and project, if any)
  environment        an Environment version (immutable image, driver-neutral digest)
  size               small | medium | large | gpu (named sizes, not machine types)
  needs              { memory_suspend?, fork?, snapshot?, gpu?, max_start_seconds? }
  placement          auto | our-cloud | my-computer:<host> | fast-sandbox
  state              desired + observed (creating, running, stopping, stopped, failed, deleted)
  profiles           what its driver declared: core, suspend, snapshot, fork
  lifecycle          idle stop after N minutes, absolute end, keep disk?
  egress             a named profile (default-deny + allow lists), fixed at create
  cost               metered rate and so far (zero on your own machine)
```

**Operations** follow the intent pattern we already run:
- **Create:** `POST /v1/computers` with an `Idempotency-Key` returns the id
  at once; `GET` shows desired and observed state.
- **Commands:** `POST /v1/computers/{id}/commands` creates a command
  session, then you attach to stream it (SPACE's create-then-attach, so no
  output is lost).
- **Files:** `PUT`/`GET /v1/computers/{id}/files/{path}`.
- **Snapshot and fork:** `POST …/snapshots` and `POST …/fork`, refused
  unless the driver declared those profiles.
- **Stop, wake and delete:** `stop`, `wake` and `delete`. Delete is final
  and metered.

## Placement: who decides where

- **`placement: auto` is the default.** We pick the driver from the needs,
  the owner's policy, price and health. It's a typed decision through the
  Decisions API (Clef first), recorded with the computer. Never keyword
  matching.
- **Explicit placement** is always allowed, and refused with a reason if the
  driver can't meet the declared needs. Asking for `fork` on your own Mac
  says so before anything happens.
- **People see plain words:** "Our cloud", "Your Mac", "Fast sandbox".
  Driver and vendor names stay out of the UI and the main API. A
  `driver` field appears on the record only for debugging.

## Credentials

Per the SPACE assessment: a computer should never see a long-lived secret.

- **Now:** credentials passed per command, in memory, never on disk
  (#11766). The Vertex case waits for the next Coder release (#11771).
- **Next:** credentials *by reference*. The API takes a reference (a
  Secret Manager name, or a scoped token we mint), and an egress proxy on
  the computer's network swaps it in on matching hosts. The agent inside
  never holds the real key. SPACE has this built in; for `oa-boat` it's a
  proxy on the VPC path.
- **The SDK surface:**
  `computers.create(.., secrets: [Secret::reference("github", "projects/…/secrets/…")])`,
  never raw values.

## The SDK

- **Client (`openagents::computers`):** a typed client for the northbound
  API, with:
  - the operation pattern (wait for running, with a timeout the caller sets);
  - command streaming;
  - files;
  - idempotency keys added automatically.
- **Host (`openagents::host`):** run an agent session *on* a computer. Today
  `coder remote delegate --on boat|gce` and the website's chat computers do
  this in two different ways. The host layer should take a computer id and
  run a harness there, with the same hooks as a local session.
- **Provider (`computer-driver` + `computer-conformance`):** the trait and
  the suite. A third party who wants to offer computers (a customer's own
  cluster, a contributor's machine) implements the Boat API subset for
  their profiles and passes the suite.

## How we consume it vs developers

- **Us:**
  - **Chats:** they already get their computer through `oa-computers`.
  - **Solver and Environments:** they still call `oa-boat` directly, and
    should move to the Computers API so there's one list and one meter
    (Factory plan §1).
  - **Coder's `--on boat`:** it builds its own launch script and goes
    straight to a computer. The next Coder release should go through the
    API (it's a binary change, so it goes on
    [next-release.md](../coder/next-release.md)).
- **Developers:** "give my agent a computer" in one call, paid by credits
  or x402, with our cloud by default and their own machines through the
  same API once enrolled.
- **Agents:** the `openagents` CLI already lists computers (`/v1/computers`);
  add create, exec and delete there, plus MCP tools, so an agent can get
  itself a sandbox.

## Order of work

None of this needs a new Coder binary.

1. **Make the northbound API public:** account ownership on every row,
   `oak_`/`oa_sk_` auth, `api.openagents.com/v1/computers`, OpenAPI, docs.
   Keep the agent-work gate (admins and named agents) until billing is on.
2. **Commands and files** in the public API (today they're driver calls
   inside turns).
3. **Move Solver and Environments** onto it, so one list holds every
   computer.
4. **The SPACE spike as a driver** (#11728). It must pass
   `computer-conformance`; then `placement: fast-sandbox` exists.
5. **Credentials by reference,** before any computer runs someone else's
   money or repository.
6. **Your own machine as a driver** (Core only: attach and detach an
   enrolled host), so "Your Mac" sits in the same list.
7. **Billing and the warm pool**, then open it beyond the gate. Pricing is
   the owner's.

## Owner decisions

1. Opening the Computers API beyond admins and named agents, and its price
   (the Factory plan suggested metering at 40 msat per second as a starting
   point).
2. Whether third parties may bring drivers, and how they're paid.
3. Whether `placement: auto` may move work to a contributor's machine (a
   Pylon) for people who allow it.
