<!-- Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `docs/MVP.md` and `docs/OWNERSHIP.md`. Archived code described here was intentionally pruned from the retained MVP repo and should not be restored wholesale without an explicit decision. -->

# Audit: Pylon Archive And Transcript

Date: 2026-03-07

## Scope

This audit answers four questions:

1. What Pylon was in the archived OpenAgents codebase.
2. How Pylon was framed relative to Nexus in the `203-pylon-and-nexus` episode transcript.
3. What the archived code actually implemented, as distinct from what the docs pitched.
4. What matters to the current MVP repo, given that `crates/pylon` was intentionally removed.

Primary sources reviewed:

- archived Pylon crate:
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/README.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/ARCHITECTURE.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/PROVIDER_MODE.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/HOST_MODE.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/CONFIGURATION.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/DATABASE.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/RELEASE-v0.1.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/src/*.rs`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/src/**/*.rs`
- adjacent runtime docs:
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/agent/README.md`
  - `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/compute/src/backends/mod.rs`
- transcript:
  - `/Users/christopherdavid/code/backroom/openagents-doc-archive/2026-02-25-doc-cleanup-pass/docs/transcripts/203-pylon-and-nexus.md`
- secondary Pylon-adjacent archive:
  - `/Users/christopherdavid/code/backroom/hyperion-pylon-codex-archive-20260116-150350/edited/docs/EFFECT_FRONTEND_INTEGRATION.md`
  - `/Users/christopherdavid/code/backroom/hyperion-pylon-codex-archive-20260116-150350/edited/resources/js/pages/settings/connections.tsx`
- current repo context:
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`
  - `docs/autopilot-earn/AUTOPILOT_EARN_BACKROOM_HARVEST_AUDIT.md`
  - `docs/autopilot-earn/AUTOPILOT_EARN_RUNTIME_PLACEMENT_DECISION.md`

## Executive Summary

Pylon was the archived local runtime for the old "sovereign agent" vision. It was explicitly positioned as a single binary that ran on your machine, held your identity and wallet, connected to relays, sold compute for sats, and could also host your own agents locally. Nexus was the hosted complement: the cloud relay/runtime fabric that traded sovereignty for convenience.

The important part is that Pylon was not just a thin provider daemon. By the time of the archive, it had become a broad local workstation runtime with at least seven concerns in one crate:

- daemon/process control
- provider mode
- buyer mode
- wallet tooling
- host mode
- RLM swarm orchestration
- local browser/Codex bridge

The archive shows real implementation in provider mode, buyer/wallet flows, RLM, and the local Codex bridge. It also shows a lot of product drift: docs and README frequently describe a larger, cleaner, more complete system than the code actually provides.

For the current MVP repo, Pylon is best read as historical precedent and a pattern library. It is not a good candidate for wholesale restoration. The current repo's earlier decisions to keep provider runtime ownership in `apps/autopilot-desktop` and keep `crates/pylon` archived are consistent with what this audit found.

## What Pylon Was Supposed To Be

### Local sovereign runtime

The core pitch is very consistent across the README and the transcript:

- Pylon is "the local runtime for sovereign AI agents" and a single binary with two modes: host your own agents and sell compute to the network (`crates/pylon/README.md:3-10`).
- In the episode transcript, Pylon is described as "the thing you run on your computer that lets you sell your compute for Bitcoin" and "the swarm compute node" (`203-pylon-and-nexus.md:24-34`).
- The agent crate README treats Pylon as the local runtime, with Nexus as the hosted runtime using the same underlying agent model (`crates/agent/README.md:13-19`, `184-205`).

### Paired with Nexus

The transcript and README line up on the Pylon/Nexus split:

- Pylon = your device, your keys, your hardware, your uptime risk.
- Nexus = OpenAgents-hosted cloud runtime/relay, convenience, higher uptime, less sovereignty.

That is stated directly in the README's `Pylon vs Nexus` table (`crates/pylon/README.md:311-327`) and almost verbatim in the transcript, where Nexus is described as the default hosted swarm relay at `nexus.openagents.com`, initially a Cloudflare Workers-based relay surface (`203-pylon-and-nexus.md:29-34`, `91-93`).

### Economic loop

The economic story had three parts:

- provider mode earns sats by serving NIP-90 jobs
- hosted agents spend sats for compute
- the same machine can both buy and sell, creating the README's "symbiotic loop" (`crates/pylon/README.md:329-352`)

The transcript makes the same argument in simpler language: go online, compete for jobs, do inference, get paid Bitcoin (`203-pylon-and-nexus.md:54-61`).

### RLM as demand engine

The transcript is especially clear that RLM was the intended demand-side unlock:

- a dedicated job kind `5940` was introduced for RLM subqueries (`203-pylon-and-nexus.md:95-103`)
- the speaker explicitly says earlier swarm compute lacked compelling buy-side demand and that RLM fan-out was the answer (`203-pylon-and-nexus.md:123-143`)

That was not just talk. The archive contains a full `pylon rlm` command, a separate `rlm.db`, and FRLM integration code (`crates/pylon/src/cli/rlm.rs:1-157`, `crates/pylon/src/db/rlm.rs:1-198`).

## What The Archived Code Actually Implemented

### 1. A real daemon shell with local state

Pylon had a genuine local runtime shell:

- `pylon start` loaded `identity.mnemonic`, opened SQLite, opened a Unix control socket, optionally launched provider mode and host mode, started a local bridge, and entered an event loop (`crates/pylon/src/cli/start.rs:62-360`).
- Daemon IPC was simple JSON over `~/.openagents/pylon/control.sock` with `Ping`, `Status`, and `Shutdown` commands (`crates/pylon/src/daemon/control.rs:10-144`).
- Persistence was SQLite with migrations for jobs, earnings, invoices, agents, and tick history (`crates/pylon/src/db/mod.rs:22-181`).

This part is concrete, not aspirational. Pylon really was designed as a resident local process rather than a one-shot CLI.

### 2. Provider mode was real and central

The provider path is the strongest implemented part of the crate:

- `PylonProvider` owns relay service, DVM service, wallet hookup, backend registries, and diagnostics (`crates/pylon/src/provider.rs:94-121`).
- It auto-detects local inference backends via `BackendRegistry::detect()`, which only probes Ollama, Apple Foundation Models, and llama.cpp (`crates/pylon/src/provider.rs:143-174`, `crates/compute/src/backends/mod.rs:237-281`).
- It separately registers Codex as an agent backend for Bazaar-style NIP-90 jobs when Codex is available (`crates/pylon/src/provider.rs:176-191`).
- It wires payment policy into `DvmService` via `min_price_msats`, `require_payment`, and `network` (`crates/pylon/src/provider.rs:276-320`).
- The daemon event loop persists provider events like `JobReceived`, `JobStarted`, `InvoiceCreated`, `PaymentReceived`, `JobCompleted`, and `JobFailed` back into SQLite (`crates/pylon/src/cli/start.rs:287-360`).

The release notes support this reading. Pylon v0.1 is framed as a NIP-90 DVM release with NIP-42 auth, multi-relay support, Spark payments, and regtest alpha constraints (`crates/pylon/docs/RELEASE-v0.1.md:8-47`, `85-97`).

### 3. Buyer mode and wallet flows were also present

Pylon was not only a provider:

- `pylon wallet` could show balance, addresses, invoices, payment history, and request regtest faucet funds (`crates/pylon/src/cli/wallet.rs:17-219`).
- `pylon job submit` could publish NIP-90 jobs, subscribe for feedback, auto-pay invoices, wait for results, and store buyer-side history in a local `jobs.db` (`crates/pylon/src/cli/job.rs:21-260`).
- `pylon api` exposed a local OpenAI-compatible-ish completions API over detected local backends (`crates/pylon/src/cli/api.rs:1-220`).

This matters because it means Pylon was trying to be the user's full local market node, not just a provider worker.

### 4. RLM was implemented deeply enough to be a first-class product lane

The transcript says RLM was strategic; the code agrees:

- `pylon rlm` builds fragments, fans out subqueries, supports local-only fallback, budgets, timeouts, and trace logging (`crates/pylon/src/cli/rlm.rs:32-157`, `159-260`).
- the code uses `KIND_JOB_RLM_SUBQUERY`, which resolves to `5940` in the archived Nostr core crate (`crates/pylon/src/cli/rlm.rs:22`, `crates/nostr/core/src/nip90.rs:175` from the archive search)
- `RlmStore` persists runs and trace events to `rlm.db` (`crates/pylon/src/db/rlm.rs:23-187`)

So the archive does not just mention RLM in docs. It has a dedicated data model and CLI surface for it.

### 5. Host mode existed, but much more thinly than the docs imply

The host-mode docs describe a rich sovereign-agent runtime: lifecycle states, funding-based wake/sleep behavior, scheduled ticks, action loops, and separate agent subprocesses (`crates/pylon/docs/HOST_MODE.md:3-260`).

The actual Pylon-side host implementation is much thinner:

- `AgentRunner` loads registry entries and spawns `agent-runner` subprocesses (`crates/pylon/src/host/runner.rs:62-190`)
- it uses `kill -0` to test liveness and `SIGKILL`-style process termination on shutdown (`crates/pylon/src/host/runner.rs:24-47`, `193-214`)
- it upserts a minimal agent record into SQLite but does not itself implement the rich tick lifecycle described in the docs (`crates/pylon/src/host/runner.rs:151-165`)

More importantly, in the inspected pruned snapshot I did not find a checked-in `agent-runner` binary target. The host runtime depends on an external or elsewhere-defined binary (`crates/pylon/src/host/runner.rs:67-81`), while the `agent` crate itself has no `[[bin]]` target in its Cargo manifest.

My read: host mode was conceptually important, but Pylon the crate was mostly acting as a launcher/supervisor, not the full agent runtime.

### 6. Pylon had become a local browser/Codex bridge, not just a compute node

This is the biggest thing the transcript does not fully capture.

The two largest files in the crate are:

- `src/local_bridge.rs` at 2,227 lines
- `src/codex_agent_backend.rs` at 1,360 lines

That is not accidental. A large share of Pylon's real complexity had shifted into local workstation integration:

- `local_bridge.rs` exposes `pylon.system` and `pylon.codex` channels over a local WebSocket/TLS bridge (`crates/pylon/src/local_bridge.rs:42-67`, `107-145`)
- it supports `client-pylon.discover`, `client-pylon.ping`, `client-codex.connect`, `client-codex.request`, and `client-codex.respond` event flows (`crates/pylon/src/local_bridge.rs:591-846`)
- `codex_agent_backend.rs` can clone repositories, spawn Codex app-server sessions, run patch generation, run code review, and execute sandbox commands (`crates/pylon/src/codex_agent_backend.rs:302-620`)

This is reinforced by the separate Hyperion archive, where the frontend has:

- Pylon bridge discovery
- local bridge settings UI
- Pylon/Codex status displays
- Effect-based stream adapters for `pylon.system` and `pylon.codex`

So by archive time, Pylon was becoming a local execution bridge for browser/web UX, not only a headless Bitcoin-paid inference node.

## Implementation Drift And Gaps

The archive docs are useful, but they are not a reliable single source of truth. I found repeated drift between README/docs and the inspected code.

### 1. The README advertises a `pylon connect` command that is not in the CLI

- README and CLI docs describe a `pylon connect --tunnel-url <url>` Codex tunnel command (`crates/pylon/README.md:198-203`)
- actual CLI commands do not include `Connect`; the enum only has `Init`, `Api`, `Start`, `Stop`, `Status`, `Doctor`, `Agent`, `Earnings`, `Infer`, `Compute`, `Wallet`, `Job`, `Rlm`, and `Gateway` (`crates/pylon/src/cli/mod.rs:29-60`)

That suggests the docs were ahead of, or lagging behind, the real implementation.

### 2. Codex configuration docs overstate what the code supports

The docs describe rich Codex settings like:

- `model = "gpt-5.2-codex"`
- isolation modes (`local | container | gvisor`)
- `max_workers`
- pricing tables

See `crates/pylon/docs/CONFIGURATION.md:132-191` and `crates/pylon/docs/PROVIDER_MODE.md:130-151`.

The actual `CodexConfig` only contains:

- `enabled`
- `model`
- `autonomy`
- approval / allow / block lists
- optional `max_cost_usd`
- optional `cwd`
- optional `executable_path`

(`crates/pylon/src/config.rs:79-137`)

The default Codex model is also empty in code (`default_codex_model() -> String::new()`) even though the docs present `gpt-5.2-codex` as the default (`crates/pylon/src/config.rs:127-133`, `crates/pylon/docs/CONFIGURATION.md:139-149`).

### 3. Docs claim more IPC surface than the daemon actually exposes

`docs/DIRECTORIES.md` describes Neobank-related control-socket commands and responses.

The actual daemon control enum only exposes:

- `Status`
- `Shutdown`
- `Ping`

(`crates/pylon/src/daemon/control.rs:10-37`)

So at least part of the docs had drifted toward adjacent runtime/neobank ideas that were not present in this crate snapshot.

### 4. Provider docs claim broader Bazaar support than the code clearly delivers

`docs/PROVIDER_MODE.md` lists Bazaar job kinds `5930`, `5931`, `5932`, and `5933`, including `RepoIndex` (`crates/pylon/docs/PROVIDER_MODE.md:63-70`).

The actual Codex agent backend capabilities report:

- `patch_gen: true`
- `code_review: true`
- `sandbox_run: true`
- `repo_index: false`

(`crates/pylon/src/codex_agent_backend.rs:326-343`)

So RepoIndex support was documented as if available, but the inspected implementation still advertised it as disabled.

### 5. Provider startup still assumes inference is mandatory even though agent backends exist

This is the most concrete behavior bug I found.

`PylonProvider::start()` allows startup when either inference backends or agent backends are present (`crates/pylon/src/provider.rs:364-384`).

But `pylon start` disables provider mode whenever `status.backends.is_empty()`, ignoring `status.agent_backends` (`crates/pylon/src/cli/start.rs:176-193`).

That means a Codex-only Bazaar provider could be supported by the provider object and still get turned off by the daemon bootstrap path.

### 6. Network/mainnet story was mid-transition and inconsistent

The transcript explicitly says regtest alpha was intentional and mainnet was expected "next week" (`203-pylon-and-nexus.md:54-81`, `98-148`).

The codebase shows that transition was incomplete:

- config supports `mainnet`, `testnet`, `signet`, and `regtest` (`crates/pylon/src/config.rs:26-40`, `crates/pylon/docs/CONFIGURATION.md:72-118`)
- provider wallet initialization maps those strings to Spark network variants (`crates/pylon/src/provider.rs:329-361`)
- but `pylon wallet`, `pylon job`, and `pylon rlm` each hardcode `Network::Regtest` in their local wallet creation paths (`crates/pylon/src/cli/wallet.rs:80-99`, `crates/pylon/src/cli/job.rs:112-129`, `crates/pylon/src/cli/rlm.rs:139-157`)
- release notes still list "Regtest only" as a known limitation (`crates/pylon/docs/RELEASE-v0.1.md:85-97`)

So the archive shows a codebase in the middle of a network migration, not a fully landed mainnet-ready runtime.

### 7. README claims OpenAI fallback for provider inference, but the inspected provider runtime does not auto-register it

The README says provider mode can use "OpenAI (fallback)" (`crates/pylon/README.md:118-122`).

The inspected provider inference backend registry only probes:

- Ollama
- Apple FM
- llama.cpp

(`crates/compute/src/backends/mod.rs:237-281`)

There is cloud-provider detection in `pylon compute`, but that is a diagnostic command, not provider registration (`crates/pylon/src/cli/compute.rs`, search results). I did not find matching provider-side OpenAI inference registration in the reviewed code.

### 8. Provider stats fields exist but appear unused inside `PylonProvider`

`ProviderStatus` exposes `jobs_processed` and `total_earnings_msats`, and `PylonProvider` stores those fields (`crates/pylon/src/provider.rs:45-61`, `94-121`).

In the inspected file, those fields are initialized and read back into status, but I did not find any internal mutation paths for them (`crates/pylon/src/provider.rs:203-216`, `421-458` plus archive-wide search). The daemon separately tracks stats from broadcast events in `start.rs`.

This is minor compared to the drift above, but it is another sign that some internal interfaces were only partially wired.

## What The Transcript Adds That The Code Alone Does Not

The transcript is valuable because it explains the intended emotional and market positioning:

- Pylon was supposed to be dead simple: start it, go online, compete for jobs, earn sats.
- It was expected to be agent-operated before it was human-polished.
- The team was explicitly using regtest alpha to gather real field feedback before mainnet.
- RLM was not side content. It was the answer to the swarm's earlier demand problem.
- Autopilot was already envisioned as the main buyer surface that would consume Pylon/Nexus compute and create a revenue-sharing loop (`203-pylon-and-nexus.md:108-148`).

In other words, the transcript turns the code into a product thesis:

> Pylon was the local node in a two-sided agent economy, and Autopilot was the future application that would make that economy worth using.

## What Matters For The Current MVP Repo

The current repo has already made the right scoping call: keep Pylon archived.

That aligns with three facts from this audit:

1. Pylon bundled too many concerns into one crate.
2. Its docs had already drifted from implementation in multiple places.
3. The portable value is in patterns, not in the archived topology.

The parts worth remembering are:

- explicit local identity and wallet authority
- truthful online/offline provider lifecycle
- persisted job/earnings state instead of inferred UI counters
- RLM trace and observability patterns
- Codex-backed Bazaar job shapes as a future extension
- local bridge/browser discovery as a pattern for workstation-connected web UX

The parts that should stay archived unless explicitly requested are:

- the monolithic `crates/pylon` crate
- the coupled daemon + provider + buyer + wallet + host + bridge + RLM bundle
- the older host-mode/runtime layering

This matches the existing repo decision to keep provider runtime ownership in `apps/autopilot-desktop` and not pull archived `pylon`/`compute` crates back into MVP scope (`docs/autopilot-earn/AUTOPILOT_EARN_BACKROOM_HARVEST_AUDIT.md`, `docs/autopilot-earn/AUTOPILOT_EARN_RUNTIME_PLACEMENT_DECISION.md`).

## Bottom Line

Pylon was the archived local-runtime ancestor of today's Autopilot earn/provider direction.

It was trying to be all of these at once:

- a sovereign-agent host
- a Bitcoin-paid NIP-90 provider
- a buyer client
- an RLM swarm orchestrator
- a Spark wallet shell
- a browser-discoverable local Codex bridge

That ambition is visible in the archive, and so is the lack of focus.

The code shows real substance, especially around provider mode, buyer flows, RLM, and the local bridge. It also shows enough drift and bundling that restoring it wholesale into this MVP repo would be a regression in scope control.

My read is: Pylon matters as history, vocabulary, and pattern source. It should not be treated as the retained architecture.
