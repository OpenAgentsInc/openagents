# Nexus And Pylon Audit

Date: 2026-04-06

## Scope

This audit answers four questions:

1. What the current `Nexus` implementation in this repo actually does.
2. How `Nexus` was envisioned to relate to `Pylon`.
3. What is happening now in the current codebase and on the live `nexus.openagents.com` host.
4. What should happen next if we want the Nexus/Pylon story to be coherent.

## Sources reviewed

Repo implementation:

- `apps/nexus-relay/src/durable.rs`
- `apps/nexus-relay/src/main.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/economy.rs`
- `apps/pylon/src/lib.rs`
- `apps/pylon-tui/src/lib.rs`
- `apps/autopilot-desktop/src/starter_demand_client.rs`
- `apps/autopilot-desktop/src/sync_bootstrap.rs`

Repo docs:

- `docs/MVP.md`
- `docs/pylon/README.md`
- `docs/pylon/PYLON_PLAN.md`
- `docs/autopilot-earn/AUTOPILOT_EARN_MVP.md`
- `docs/plans/nexus-relay-migration-plan.md`
- `docs/reports/nexus/2026-03-06-production-cutover.md`
- `docs/audits/2026-03-05-autopilot-earn-mvp-full-implementation-gap-audit.md`
- `docs/audits/2026-03-07-pylon-archive-and-transcript-audit.md`
- `docs/audits/2026-04-05-pylon-nip90-wallet-issue-program-audit.md`

Live verification performed on 2026-04-06:

- `curl https://nexus.openagents.com/healthz`
- `curl https://nexus.openagents.com/api/stats`
- `curl -H 'Accept: application/nostr+json' https://nexus.openagents.com/`

## Executive summary

`Nexus` currently exists as one deployed public host that combines two roles:

- a durable Nostr relay shell in `apps/nexus-relay`
- an in-process OpenAgents authority/API surface in `apps/nexus-control`

That deployed host is real. The live `nexus.openagents.com` endpoint currently serves:

- durable relay health
- NIP-11 relay metadata
- websocket relay traffic
- desktop session bootstrap
- sync token minting
- starter-demand control APIs
- a large `nexus-control` stats payload

The important constraint is that this hosted Nexus control-plane behavior is currently wired for `Autopilot Desktop`, not for standalone `Pylon`.

The current standalone `Pylon` is a narrow provider connector. It defaults one relay to `wss://nexus.openagents.com/`, but it does not implement the hosted Nexus session bootstrap, sync token path, or starter-demand eligibility path. In the current repo, `Pylon` and `Nexus` meet at relay transport and public market visibility, not at a shared hosted-control-plane contract.

That means the story is partially implemented and partially aspirational:

- the old worldview, where `Pylon` is the local node and `Nexus` is the hosted counterpart, still shapes the docs and product language
- the new narrowed `Pylon` boundary is real
- the current hosted Nexus privileges are still effectively `Autopilot`-only

The missing decision is not whether Nexus exists. It does. The missing decision is whether standalone `Pylon` is meant to be a first-class hosted Nexus client, or just a sovereign/public Nostr node that happens to use Nexus as one default relay.

## 1. What Nexus actually is in the current repo

### 1.1 `apps/nexus-relay` is the public host shell

The current public Nexus service is not the old in-memory relay harness. The live service path in this repo is the durable shell in `apps/nexus-relay/src/durable.rs`.

That shell does five direct things:

- starts an upstream durable relay based on the imported `nostr-rs-relay`
- binds one public HTTP/websocket service
- serves `/healthz`
- serves relay traffic on `/` and `/ws`
- merges the `nexus-control` authority router into the same process

The route shape in code is explicit:

- `/` = homepage, websocket upgrade, or NIP-11 relay info depending on headers
- `/ws` = websocket alias
- `/metrics` = proxied upstream metrics
- `/healthz` = durable relay shell health
- merged `nexus-control` routes under `/api/*`, `/stats`, and `/v1/*`

The health payload says exactly what this process thinks it is:

- `relay_backend = durable-upstream`
- `authority_mode = in-process`
- `managed_groups_mode = deferred`

That is consistent with the repo plan and with the live host as of 2026-04-06.

### 1.2 `apps/nexus-control` is the authority and control API

`apps/nexus-control` is not a generic relay extension. It is a control-plane and authority service.

Its current route set includes:

- `/api/session/desktop`
- `/api/session/me`
- `/api/sync/token`
- `/api/starter-demand/poll`
- `/api/starter-demand/offers/{request_id}/ack`
- `/api/starter-demand/offers/{request_id}/heartbeat`
- `/api/starter-demand/offers/{request_id}/fail`
- `/api/starter-demand/offers/{request_id}/complete`
- `/api/stats`
- `/stats`
- a large family of `/v1/kernel/*` authority routes

Its current responsibilities include:

- minting desktop bearer sessions
- minting sync tokens
- tracking starter-demand offer state
- recording authority receipts
- persisting or replaying kernel state
- exposing a public stats snapshot with session, starter-demand, and compute-market counters

The `PublicStatsSnapshot` type in `apps/nexus-control/src/economy.rs` is broad. It already includes:

- hosted relay URL
- receipt persistence flags
- session counts
- sync-token counts
- starter-demand counters
- compute-market counters
- liquidity-market counters
- risk-market counters

So the implementation is not a thin auth stub. It is already shaped like a server-side product authority and observability surface.

### 1.3 Nexus is currently deployed as one stateful public service

The deployment docs and the live host line up on the same deployment model:

- one public host: `nexus.openagents.com`
- one stateful VM
- one persistent relay data directory
- one Rust service combining relay and authority/API behavior
- Cloudflare tunnel for public ingress

The live probes on 2026-04-06 confirmed:

- `/healthz` returns `service = nexus-relay`
- `/api/stats` returns `service = nexus-control`
- NIP-11 on `/` returns `name = OpenAgents Nexus`

That means the public service shape described in the March cutover docs is still real and still live.

## 2. How Nexus was envisioned to work with Pylon

### 2.1 The older worldview

The older Pylon/Nexus story was simple:

- `Pylon` was the local node you ran on your machine
- `Nexus` was the hosted counterpart

The archived transcript and archive audit both describe the split in basically those terms:

- `Pylon` = local sovereignty, local hardware, local keys, local uptime risk
- `Nexus` = hosted convenience, hosted relay/runtime presence, less sovereignty

That older story assumed one network and one market with local and hosted participation modes.

### 2.2 The current narrowed worldview

The current repo narrowed `Pylon` substantially.

`docs/pylon/PYLON_PLAN.md` defines the modern product split as:

- `Autopilot` = product surface
- `Pylon` = standalone supply connector
- `Nexus` = authority/control plane

This is materially different from the old archived runtime. The current plan explicitly says:

- do not restore the old monolithic `crates/pylon`
- keep `Pylon` focused on provider supply
- keep `Nexus` as network authority and control plane

This matters because the repo no longer wants a giant local sovereign runtime that also acts as buyer shell, host runtime, bridge, and wallet shell.

### 2.3 The implied intended contract

If you take the current docs together, the intended relation seems to be:

- `Pylon` publishes supply and does local execution
- `Nexus` provides hosted coordination, authority, and observability
- `Pylon` can still work over open Nostr relays
- the OpenAgents-hosted Nexus adds privileged hosted behavior on top

That is the implied direction.

The current repo does not fully implement that contract.

## 3. What happens now

### 3.1 Live Nexus behavior is real, but mostly desktop-oriented

The live `nexus.openagents.com` host on 2026-04-06 returned:

- `/healthz`
  - `relay_backend = durable-upstream`
  - `authority_mode = in-process`
  - `managed_groups_mode = deferred`
- `/api/stats`
  - `service = nexus-control`
  - `hosted_nexus_relay_url = wss://nexus.openagents.com/`
  - `sessions_active = 5`
  - `sync_tokens_active = 4`
  - `compute_products_active = 0`
  - `starter_offers_* = 0` right now
  - recent stored receipts for `desktop_session.created`, `sync_token.issued`, and starter-offer lifecycle events
- `/` with `Accept: application/nostr+json`
  - `name = OpenAgents Nexus`
  - NIP-11 relay info
  - `supported_nips` including `11` and `42`

That tells us three important things.

First, the public Nexus service is live and coherent.

Second, the control-plane/session path is actually being used by something today.

Third, the compute-market authority payload exists, but the live stats currently show zero active compute products and zero active compute inventory.

### 3.2 The hosted proof path is Autopilot-only today

The strongest code fact in this audit is the current starter-demand proof rule in `apps/nexus-control/src/lib.rs`.

`starter_demand_provider_proof_reason(...)` requires:

- `desktop_client_id` starting with `autopilot-desktop`
- a non-empty `bound_nostr_pubkey` on the hosted desktop session
- a provider Nostr pubkey in the request
- exact equality between the bound session pubkey and the provider pubkey

If any of that is missing, starter-demand eligibility is denied.

This is not a generic "OpenAgents provider" proof system. It is explicitly an `Autopilot Desktop` proof system.

### 3.3 Desktop has the hosted Nexus client path

`apps/autopilot-desktop` contains the client code for the hosted Nexus control plane:

- `sync_bootstrap.rs` calls `/api/session/desktop` and `/api/sync/token`
- `starter_demand_client.rs` calls `/api/starter-demand/*`
- the desktop state defaults the primary relay to `wss://nexus.openagents.com/`

So the current hosted Nexus integration story is concrete for `Autopilot Desktop`.

### 3.4 Standalone Pylon does not have that client path

The current standalone `Pylon` does not contain a Nexus control-plane client.

What it does have:

- a default relay list that starts with `wss://nexus.openagents.com`
- public relay defaults alongside Nexus
- local identity and local ledger management
- local provider lifecycle controls
- provider announcement publishing
- provider scan/run
- buyer job submit/watch/history
- wallet and payout commands

What it does not have:

- `/api/session/desktop`
- `/api/sync/token`
- `/api/starter-demand/*`
- any Pylon-native hosted session enrollment path
- any obvious `nexus-control` authority client

That means standalone `Pylon` currently uses `Nexus` primarily as:

- one default relay
- one default place to publish and observe Nostr market traffic

It does not currently use Nexus as a first-class hosted control plane.

### 3.5 Nexus and Pylon currently meet at the relay layer, not the authority layer

This is the cleanest summary of the current implementation:

- `Nexus` is a live durable relay plus hosted authority service
- `Autopilot Desktop` uses both parts
- standalone `Pylon` uses the relay part
- standalone `Pylon` does not use the hosted authority/session/starter-demand part

So the current implementation is not "Nexus and Pylon fully integrated."

It is:

- `Autopilot <-> Nexus` integrated at relay plus authority layers
- `Pylon <-> Nexus` integrated at relay layer only

## 4. Where the story and the implementation diverge

### 4.1 The repo says Nexus is the Pylon authority/control plane, but the client contract is missing

`docs/pylon/PYLON_PLAN.md` says `Nexus` remains the authority/control plane for the narrow standalone provider connector.

The current implementation does not yet make that true in a concrete client-contract sense.

There is no Pylon-native equivalent of:

- session bootstrap
- provider enrollment
- sync-token issuance
- hosted eligibility proof
- hosted lease management

So the current system has the server shape of a control plane, but not the standalone-Pylon client shape.

### 4.2 Hosted benefits are still bound to the Autopilot identity model

The current hosted proof rule is not neutral. It encodes product policy:

- the hosted session must look like `autopilot-desktop`
- starter-demand eligibility is defined around that session type

That is reasonable if the intended rule is "starter demand is Autopilot-only."

It is not reasonable if the intended rule is "standalone Pylon is also a first-class hosted Nexus participant."

Right now the code picks the first interpretation.

### 4.3 The live Nexus stats already expose compute-market authority fields, but nothing is driving them

The live `/api/stats` payload already carries compute-market counters, but the current live values are zero across the active compute-supply fields.

That strongly suggests one of two realities:

- the authority layer is ready before the supply clients are wired to it
- the authority fields are future-facing scaffolding rather than the current live loop

Either way, standalone `Pylon` is not yet feeding its supply truth into that hosted authority surface in a meaningful way.

### 4.4 The public docs still make it too easy to blur three different meanings of Nexus

The word `Nexus` currently gets used in three overlapping ways:

- public relay host
- hosted desktop/session bootstrap service
- future or partial compute-market authority system

The code can support all three inside one deployed binary.

The problem is product clarity, not process count. Right now the docs and language still make it too easy to talk as if all three are equally implemented for both `Autopilot` and `Pylon`.

They are not.

## 5. What I think should happen

### 5.1 First, make the product claim explicit

We should stop leaving the Pylon/Nexus relation ambiguous.

We need one explicit statement in the root docs and the product docs:

- either standalone `Pylon` is a first-class hosted Nexus client
- or it is not

The current implementation behaves as if it is not.

### 5.2 If we want standalone Pylon to be first-class on hosted Nexus, add a Pylon-native enrollment contract

This is the path I would recommend if the desired story is truly "local Pylons plus hosted Nexus."

That contract should be explicit and minimal:

1. Add a Pylon-native hosted session endpoint.
2. Bind that session to the Pylon node identity, not to `autopilot-desktop` naming.
3. Expose a hosted eligibility or lease path for managed demand.
4. Let hosted Nexus stats and compute authority reflect real Pylon supply.
5. Keep starter-demand policy separate from open-market NIP-90 relay participation.

In other words:

- do not force standalone Pylon to impersonate Autopilot Desktop
- do not hide the authority contract behind desktop-only session semantics

### 5.3 If we do not want standalone Pylon to be first-class on hosted Nexus, say so plainly

This is the other coherent option.

If the intended product truth is:

- `Autopilot` gets hosted-Nexus bootstrap, starter demand, and privileged control-plane flows
- standalone `Pylon` is a sovereign supply node that can still use the public relay

then we should document that directly and stop implying more.

That would mean:

- keep `Nexus` as one default relay in Pylon
- keep `Pylon` public-relay and NIP-90 first
- reserve hosted starter-demand and hosted proof for Autopilot
- remove or narrow language that says Nexus is already the active standalone-Pylon control plane

That is a valid product choice. It is just not what some of the current language implies.

### 5.4 Regardless of the choice, wire the compute authority surface to real supply or stop over-signaling it

The live `nexus-control` stats payload currently includes a large compute-market authority surface, but the live counters are zero.

We should do one of two things:

- connect real provider inventory, delivery proofs, and settlement evidence into it
- or reduce the public claims until that path is actually live

The current middle state is technically fine but narratively weak.

### 5.5 Keep one public Nexus host, but keep the contract boundaries explicit

I do not think we need two public products or two public hosts.

The current single-host deployment is fine.

The important part is keeping the internal contract honest:

- relay role
- hosted session/control role
- compute authority role

One binary is fine.
One host is fine.
One muddy product story is not fine.

## Recommended decision

My recommendation is:

1. Keep the single public Nexus host and the current durable relay plus in-process authority deployment shape.
2. Keep the current narrowed standalone `Pylon` boundary.
3. Add an explicit Pylon-native hosted Nexus enrollment path if we want `Pylon` to participate in hosted demand and authority-managed supply.
4. Until that path exists, document that hosted starter-demand and hosted proof are Autopilot-only.

That gives us a clean intermediate truth:

- `Nexus` is real and live now
- `Autopilot` is the current hosted client
- standalone `Pylon` is the current sovereign provider connector
- a true hosted Nexus/Pylon contract is still missing and should be implemented deliberately rather than implied loosely

## Bottom line

The current `Nexus` implementation is real. It is a durable public relay plus a real OpenAgents authority/control API running in the same deployed service.

The current standalone `Pylon` implementation is also real. It is a narrow provider connector with relay, job, wallet, and local-ledger behavior.

What is not fully real yet is the claim that `Nexus` and standalone `Pylon` already operate as one integrated hosted-plus-local control-plane system.

Today:

- `Autopilot Desktop` has the hosted Nexus client path
- `Pylon` has the relay path

If we want more than that, we need to build it explicitly.
