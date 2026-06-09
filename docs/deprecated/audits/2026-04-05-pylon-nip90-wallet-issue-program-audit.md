<!-- Historical note: this audit extends the March 7 archive audit with the current standalone `apps/pylon` and `apps/pylon-tui` surface. Current product authority still lives in `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/pylon/PYLON_PLAN.md`. -->

# Audit: Pylon NIP-90 And Wallet Issue Program

Date: 2026-04-05

## Scope

This audit answers four questions:

1. Which archived Pylon capabilities are still relevant to the new standalone `apps/pylon` boundary.
2. Which archived capabilities should stay dead.
3. What the current `apps/pylon` and `apps/pylon-tui` surface still lacks for a real NIP-90 compute-miner node.
4. How to decompose the remaining work into an issue program that fits the current repo boundary.

Primary sources reviewed:

- current repo:
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`
  - `docs/pylon/README.md`
  - `docs/pylon/PYLON_PLAN.md`
  - `docs/audits/2026-03-07-pylon-archive-and-transcript-audit.md`
  - `apps/pylon/src/lib.rs`
  - `apps/pylon-tui/src/lib.rs`
- archived Pylon:
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/README.md`
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/CLI.md`
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/PROVIDER_MODE.md`
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/CONFIGURATION.md`
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/docs/RELEASE-v0.1.md`
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/src/cli/mod.rs`
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/src/cli/wallet.rs`
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/src/cli/job.rs`
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/runtime/src/compute/providers/dvm.rs`
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/src/jobs/store.rs`
  - `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/src/db/earnings.rs`
  - `backroom/openagents-doc-archive/2026-02-25-doc-cleanup-pass/docs/transcripts/203-pylon-and-nexus.md`

## Current reading

The narrow standalone `apps/pylon` is the right retained boundary. It is a compute miner and provider connector. It should not grow back into the old sovereign-agent host, browser bridge, Codex shell, or RLM bundle.

The missing NIP-90 work is still substantial. Current `apps/pylon` is a local provider admin shell with backend detection, product derivation, inventory/status reporting, Gemma download support, and local Gemma chat in the TUI. It does not yet operate as a real NIP-90 node. It does not manage relay connectivity, publish handler presence, accept real NIP-90 requests, submit real NIP-90 buyer jobs, or expose wallet operations in the TUI.

The archived Pylon proves that those surfaces were previously treated as first-class:

- provider mode listened on relays and processed NIP-90 jobs
- buyer mode could submit jobs and wait for results
- wallet mode could inspect balance, create invoices, pay invoices, and list payment history
- the local runtime persisted jobs and earnings instead of treating them as ephemeral UI state

Those are the parts worth restoring. The old host-mode, agent-runner, Codex bridge, and RLM bundle are not required for the new Pylon boundary.

## What the current standalone Pylon already has

The current standalone path is not empty. It already has three useful foundations.

First, it has the right local-home model. `apps/pylon` owns `~/.openagents/pylon`, generates a local identity, persists provider snapshots, and exposes a clear headless control path. That matches the old "local node on your machine" idea without restoring the old daemon sprawl.

Second, it already has a TUI-first shell. `cargo pylon` opens a retained transcript interface with a full-width composer, local Gemma chat, streaming output, conversation memory, model downloads, and local capacity stats. That gives us the right place to put slash-command control without having to invent a second user-facing shell.

Third, it already uses current repo ownership correctly. Provider-domain truth stays in `openagents-provider-substrate`. Product and market truth stay in `openagents`. There is no need to pull archived runtime crates back in.

## What the current standalone Pylon still lacks

The missing work falls into six direct gaps.

### 1. No relay runtime

There is no current relay connection manager in `apps/pylon`. The archived Pylon had relay URLs, NIP-42 auth handling, relay subscriptions, job publishing, and result publishing. The new standalone Pylon currently has none of that. A compute miner without relay connectivity is still only a local status program.

### 2. No NIP-89 / provider announcement surface

The archived product expected provider discovery on relays. The current standalone Pylon does not yet publish or refresh provider presence, handler info, or supply-facing announcements. The node can detect local supply, but it does not yet make that supply network-visible over Nostr.

### 3. No provider-side NIP-90 job intake

The old provider path understood the request -> status -> result -> payment flow. The current standalone Pylon does not subscribe to job kinds, filter targeted jobs, mark lifecycle state, emit feedback, or publish results. Current `jobs`, `earnings`, and `receipts` surfaces are local provider-domain views, not live NIP-90 execution surfaces.

### 4. No buyer-side NIP-90 workflow

Archived Pylon had explicit buyer commands for submitting a job, waiting for results, auto-paying invoices, and listing local job history. The current TUI has only prompt chat to local Gemma and model downloads. It does not yet let the user submit a real NIP-90 request from the composer and track the result in the transcript.

### 5. No wallet management in the TUI

Archived Pylon exposed wallet balance, status, address, invoice creation, invoice payment, history, and funding. The current standalone Pylon has no wallet command surface at all. That is a direct blocker for making Pylon feel like the node software described in the old transcript. "Sell compute for Bitcoin" needs visible receive, pay, and history flows.

### 6. No persisted NIP-90-specific local ledger

Archived Pylon persisted buyer jobs and provider earnings in separate local SQLite stores. The new standalone Pylon has provider snapshot persistence, but it does not yet persist a full NIP-90 local ledger for:

- relay configuration and auth state
- handler announcements
- provider-side job intake and status events
- buyer-side job submissions and results
- invoice creation and payment records
- settlement and receipt linkage

Without that, every future TUI surface will be thin and fragile.

## What should stay dead

The old archive bundled unrelated surfaces that should not return under this issue program.

Do not restore:

- host mode
- agent registry and agent-runner supervision
- Codex bridge and local browser bridge
- RLM-specific swarm orchestration
- the old monolithic `crates/pylon`
- generic workstation-runtime ambitions

The current request is to make Pylon a real NIP-90 compute miner with wallet management and slash commands in the TUI. That is large enough already.

## Direct product reading

The current TUI is the right user-facing home for this work.

Plain prompts should stay what they are now: local Gemma chat when local weights are available.

Slash commands should become the operational control plane for everything that is not plain local chat. That keeps the shell simple. It also matches the old "agent-driven first, human-usable second" posture from the archive and transcript.

The practical rule should be:

- plain text = local chat
- slash commands = wallet, relays, NIP-90 jobs, status, history, lifecycle, and later market actions

## Recommended slash-command surface

The first real NIP-90 and wallet surface should cover these commands:

- `/help`
- `/online`
- `/offline`
- `/pause`
- `/resume`
- `/status`
- `/relays`
- `/relays add <url>`
- `/relays remove <url>`
- `/relays refresh`
- `/announce`
- `/wallet`
- `/wallet balance`
- `/wallet address`
- `/wallet invoice <sats> [memo]`
- `/wallet pay <bolt11>`
- `/wallet history`
- `/jobs`
- `/job submit <prompt or json>`
- `/job status <id>`
- `/job result <id>`
- `/earnings`
- `/receipts`

This is enough to make Pylon feel like a real node. It covers local machine readiness, network connectivity, provider lifecycle, wallet visibility, buyer actions, and economic history.

## Recommended issue program

The right issue program is not "restore old Pylon." It is "finish the retained standalone Pylon as a real NIP-90 node."

### Foundation

1. add a slash-command registry and parser for the Pylon TUI  
2. add a persisted local Pylon ledger for relay, job, wallet, and settlement state  
3. add relay configuration management and live connection state to Pylon  
4. add NIP-42 relay authentication and reconnect handling to Pylon  

### Provider-side NIP-90

5. add NIP-89 handler announcement publish and refresh flows to Pylon  
6. add provider-side NIP-90 request subscriptions and targeted-job filtering  
7. add provider-side NIP-90 request intake for the retained compute families  
8. add provider-side status and result publishing for Pylon jobs  
9. add provider-side payment-required and invoice-tag handling  
10. add provider-side settlement, earnings, and receipt persistence for NIP-90 jobs  

### Buyer-side NIP-90

11. add buyer-side NIP-90 job submission from Pylon slash commands  
12. add buyer-side result and feedback tracking in the TUI transcript  
13. add buyer-side invoice acceptance and auto-pay policy controls  
14. add buyer-side persisted job history and replay in Pylon  

### Wallet

15. add a Pylon wallet runtime and config surface on top of current Spark primitives  
16. add wallet slash commands for balance, address, invoice, pay, and history  
17. add payout and withdrawal surfaces for provider earnings inside the TUI  

### UX and verification

18. add transcript views for jobs, earnings, receipts, and relay activity  
19. add an end-to-end regtest harness for provider, buyer, relay, and payment roundtrips  
20. update standalone Pylon docs and verification gates for the real NIP-90 and wallet path  

## Sequencing

This should ship in four passes.

Pass one should add the slash-command framework, the local ledger, relay config, and wallet runtime. That makes the shell capable of real node control.

Pass two should add provider-side NIP-90 presence, request intake, result publishing, and earnings persistence. That makes Pylon a real supply node.

Pass three should add buyer-side submission, result tracking, and invoice-payment flows. That restores the old buyer path inside the new boundary.

Pass four should add the transcript-level operational polish, e2e harnesses, and documentation updates. That turns the feature set into a releaseable node program.

## Bottom line

The old Pylon archive already answered the strategic question. Pylon was meant to be a local node that could hold identity, connect to relays, sell compute, buy compute, and move Bitcoin. The old code also answered the architectural question. Trying to do all of that plus host mode, Codex bridge, browser bridge, and RLM in one crate was a mistake.

The current standalone `apps/pylon` and `apps/pylon-tui` boundary is the corrected version. It already has the right home path, the right provider substrate seam, and the right TUI shell. The missing work is to finish the NIP-90 node and wallet surfaces inside that corrected boundary.

That means restoring the old Pylon market features without restoring the old Pylon sprawl.
