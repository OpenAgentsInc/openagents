# Pylon DVM + Wallet Integration Plan (Autopilot Desktop)

## Goal
Bring Commander's DVM/wallet functionality into the WGPUI desktop app by **re-using Pylon** and the existing Rust Nostr/Spark/DVM crates. The app must **spawn Pylon on-demand** (never auto-start), surface status, and provide panes for wallet, DVM provider, DVM history, and NIP-90 consumer flows. The hotbar should expose these panes just like Commander.

## Canonical inputs (code wins)
- `docs/autopilot-migration-plan.md` (phase mapping + modal -> pane mapping)
- `crates/pylon/` + `crates/pylon/docs/*` (daemon, CLI, config, runtime dirs)
- `crates/autopilot-core/src/pylon_integration.rs` (existing start/status/identity helpers)
- `docs/nostr/NOSTR_AUDIT.md` (NIP-90 inventory and wiring)
- `docs/nostr/SPARK_AUDIT.md` (Spark + UnifiedIdentity)
- Commander reference:
  - `~/code/commander/docs/misc/SELLING_COMPUTE.md`
  - `~/code/commander/docs/misc/NIP90.md`
  - `~/code/commander/docs/systems/nip90-data-vending-machines.md`
  - `~/code/commander/src/components/sell-compute/SellComputePane.tsx`
  - `~/code/commander/src/components/dvm/DvmJobHistoryPane.tsx`
  - `~/code/commander/src/components/nip90/Nip90Dashboard.tsx`
  - `~/code/commander/src/components/hud/Hotbar.tsx`

## What we already have in OpenAgents
- **Pylon daemon + CLI** (`crates/pylon`) with control socket + start/stop/status.
- **Nostr + NIP-90** (core + client) and **DVM provider** in `crates/compute`.
- **Spark wallet** in `crates/spark` and unified Nostr+Spark identity in
  `crates/compute/src/domain/identity.rs` (re-exported by runtime).
- Legacy **Autopilot modals** for wallet/pylon/nostr in `crates/autopilot/` (usable as reference only).

## Commander behavior we are matching
- **Sell Compute** pane: wallet + Ollama status, online/offline toggle, settings dialog.
- **DVM job history** pane: jobs + stats, pagination.
- **NIP-90 dashboard**: create requests + view results/feedback.
- **Hotbar**: quick-access slots for these panes.

## Proposed architecture (Rust-first)

### 1) App-side Pylon client (autopilot_app)
Create a small Pylon client wrapper to abstract:
- **Identity**: `pylon init` (creates `~/.openagents/pylon/identity.mnemonic`).
- **Daemon control**: `pylon start`, `pylon stop`, `pylon status --json` or
  `pylon::daemon::ControlClient`.
- **Runtime paths**: use `pylon::PylonConfig::pylon_dir()` for config/db/socket.

This wrapper should live under `crates/autopilot_app` so WGPUI panes can subscribe
via `AppEvent` and dispatch `UserAction` commands.

### 2) Pane system integration (autopilot_ui + wgpui)
Add pane types and map them into the pane registry:
- **Pylon Control Pane** (start/stop/status/identity)
- **Wallet Pane** (Spark balance + address + activity)
- **Sell Compute Pane** (DVM provider on/off + settings)
- **DVM Job History Pane** (stats + history; read from Pylon DB)
- **NIP-90 Dashboard Pane** (consumer requests + results)

### 3) On-demand Pylon spawn policy
- Pylon **must not start automatically**. The user explicitly presses the
  hotbar button (or pane action) to start it.
- If `identity.mnemonic` is missing, prompt to **init identity** before start.
- Use the **control socket** for status and to detect already-running daemon.

### 4) Compute backends (start with Ollama)
- **Ollama is already supported in Rust** via `crates/compute`:
  - Backend registry auto-detects Ollama at `http://localhost:11434`.
  - `crates/compute/src/backends/ollama.rs` implements inference requests.
  - `DvmService` routes kind 5050 jobs to the `BackendRegistry`.
- Pylon provider uses this registry (`crates/pylon/src/provider.rs`) so DVM
  provider mode works with Ollama out of the box once the daemon (or in-process
  provider) is running.

## Data + config sources
- **Pylon config**: `~/.openagents/pylon/config.toml` (relays, payments, network).
- **Identity**: `~/.openagents/pylon/identity.mnemonic` (NIP-06 BIP39).
- **Control socket**: `~/.openagents/pylon/control.sock`.
- **Pylon DB**: `~/.openagents/pylon/pylon.db` (jobs/earnings).
- **Spark storage**: `~/.openagents/pylon/spark/`.

## Work plan

### Phase 0 - Inventory + contracts
- Define `PylonClient` in `autopilot_app` using `autopilot-core` helpers or
  direct `pylon` crate APIs.
- Establish `AppEvent` messages for:
  - PylonStatus
  - PylonIdentity
  - WalletSnapshot
  - DvmProviderStatus
  - DvmJobHistory
- Define `UserAction` commands for:
  - InitIdentity
  - StartPylon / StopPylon
  - RefreshStatus / RefreshWallet / RefreshJobs

### Phase 1 - Pylon Control Pane (start/stop)
- UI shows current daemon status, PID, uptime, earnings.
- Buttons: Init identity, Start, Stop.
- Display Pylon config path + runtime dirs for debugging.

### Phase 2 - Wallet Pane (Spark)
- Read wallet state from Pylon (preferred) or from `crates/spark` directly.
- Show balance, deposit address, last payments.
- Link to "Seed phrase / identity" info (read-only).

### Phase 3 - Sell Compute (DVM Provider)
- Use Pylon provider settings (relays, min price, required payment).
- Mirror Commander's **Go Online/Go Offline** flow.
- Add settings UI for DVM identity + pricing + relays (persist to Pylon config).

### Phase 4 - Job History + Earnings
- Read Pylon DB (`pylon.db`) for job history/earnings.
- Present stats and recent jobs (Commander's DvmJobHistoryPane parity).
- Add refresh + pagination.

### Phase 5 - NIP-90 Dashboard (Consumer)
- Provide request form + event list; route through Nostr client or runtime DVM
  consumer.
- Support selecting target DVM pubkey and relay list.
- If payment required, use Spark wallet to pay invoice.

### Phase 6 - Hotbar + Pane wiring
- Add hotbar slots for Pylon, Wallet, Sell Compute, DVM History, NIP-90.
- Map these to `PaneStore` open/toggle actions.
- Keep hotkeys consistent with Commander's HUD patterns.

### Phase 7 - Tests + verification
- Unit tests for `PylonClient` (status parse, identity existence).
- Integration tests for Pylon lifecycle (start/stop/status).
- UI smoke tests for pane open/close + hotbar mapping.

## Open questions / decisions
- Should we **embed** Pylon in-process (library) or **always spawn** the daemon?
  Prefer **in-process** so Pylon only runs while the app is open. Keep daemon
  support as a fallback for CLI parity or if we cannot safely share the core yet.
- Where should DVM settings live: **Pylon config** vs **app-local** settings?
  Prefer Pylon config to keep CLI + UI in sync.
- Do we want to read job history from Pylon DB directly or via a Pylon IPC API?
  (DB read is simplest but tighter coupling).

## Acceptance criteria
- Pylon runs **only while the app is open** (in-process by default).
- If daemon mode is used, the app starts it on pane-open and stops it on exit.
- Wallet + DVM provider + job history + NIP-90 dashboard accessible as panes.
- Hotbar entries open/close panes with no UI regressions.
- No automatic daemon spawning on app boot.

## Work log
- Reviewed OpenAgents Pylon, Nostr, Spark, and migration docs.
- Reviewed Commander Sell Compute, NIP-90, and DVM architecture docs and components.
