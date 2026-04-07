# Pylon Standalone Operator Guide

`Pylon` is the standalone provider program for the OpenAgents Compute Market.

The default local repo entrypoint is the small terminal shell:

```bash
cargo pylon
```

On first launch, the TUI bootstraps its own local Pylon config and identity under the normal Pylon home path. It does not ask the user to run a manual init step first.

The current provider automation stays in the explicit headless CLI:

```bash
cargo pylon-headless <command>
```

It is still a narrow supply connector. It is not a buyer shell, not a labor runtime, and not a raw accelerator exchange.

## Install Paths

Prefer the npm bootstrap lane when the operator already has `npm` or `bun`:

```bash
npx @openagentsinc/pylon
bunx @openagentsinc/pylon
npm install -g @openagentsinc/pylon && pylon
bun install -g @openagentsinc/pylon && pylon
npx @openagentsinc/pylon --version 0.0.1-rc5
npx @openagentsinc/pylon --no-launch
npx @openagentsinc/pylon --download-curated-cache
```

That launcher checks GitHub for the latest tagged `pylon-v...` release on each default run, or resolves a specific tagged `Pylon` version when `--version` is provided. It then finds the matching release asset for the local machine, verifies the published SHA-256 checksum, caches the binaries locally, runs the `init` / `status --json` / `inventory --json` smoke path, and then drives `pylon gemma diagnose <model>`. It only prefetches the optional Hugging Face GGUF cache when `--download-curated-cache` is set, because the sellable lane still depends on the configured local runtime endpoint rather than the local GGUF cache alone.
The default no-argument path is the intended onboarding lane: it streams terminal
status updates during bootstrap and opens `pylon-tui` automatically when the
smoke path finishes. Use `--no-launch` when you want the same install and
bootstrap flow without handing the terminal to the TUI.
If the resolved release does not ship a prebuilt archive for the local
platform, the launcher now falls back to the exact tagged source checkout,
prompts before installing Rust if `cargo` and `rustc` are missing, and builds
`pylon` plus `pylon-tui` locally before continuing into the same smoke path.
The launcher only caches those standalone binaries under
`~/.openagents/pylon/bootstrap/versions/`. It does not copy or symlink them
into a shared global bin directory, so a global npm or bun install keeps the
package-managed `pylon` command as the stable entrypoint on `PATH`.
The npm bootstrap lane now also emits best-effort anonymous install telemetry
to `openagents.com` so the public stats page can show install starts,
completions, source-build fallbacks, Rust prompts, and smoke-test outcomes.
Set `OPENAGENTS_DISABLE_TELEMETRY=1` to disable that stream, or point
`OPENAGENTS_TELEMETRY_URL` at a non-production endpoint during local validation.
The bootstrap summary now ends with an explicit operator verdict:

- `fully online`
- `runtime ready`
- `installed but runtime missing`

That verdict is intentionally separate from local cache/download success. The
bootstrap does not auto-install or auto-mutate a local runtime; it tells the
operator exactly what is missing and how to finish the bring-up path.

Prefer an official release asset when one exists for the user's platform. Those archives ship the standalone `pylon` and `pylon-tui` binaries directly, so the operator does not need a Rust toolchain just to bring a node online.

Use a direct release asset install only when the operator explicitly does not want the npm bootstrap layer:

```bash
./pylon
./pylon init
./pylon status --json
./pylon inventory --json
./pylon config show
./pylon gemma diagnose gemma-4-e4b --max-output-tokens 96 --repeats 3
```

Bare `./pylon` now opens the terminal UI. Use `./pylon-tui` only when you want
to target the shell binary explicitly.

Use a source checkout only when:

- no matching official release asset exists for the machine
- the operator needs the retained Psionic benchmark path and wants to work from source
- the operator is modifying or validating the code itself

## Launch Truth

The market is still the **OpenAgents Compute Market**.

At launch, the first standalone `Pylon` sellable lane is:

- `psionic.local.inference.gemma.single_node`

The broader market direction still includes `inference`, `embeddings`, and later bounded execution. The current operator bring-up in this repo is narrower on purpose: get one honest local Gemma inference lane online first.

Do not describe launch as raw GPU or raw accelerator trading. Accelerator, memory, and platform facts remain capability-envelope qualifiers that refine supply rather than replace product identity.

Current planned-but-not-live surfaces:

- broader embeddings lanes
- pooled inference routing
- broad wallet-shell UX
- sandbox execution as a generally released family

## Prerequisites

Minimum local requirements:

- either an official `Pylon` release asset for the local platform, or a local source checkout plus Rust
- a writable local home/config path

Runtime-specific requirements:

- an Ollama-compatible local runtime endpoint at `local_gemma_base_url`
  (default `http://127.0.0.1:11434`) that answers `GET /api/tags` and
  `POST /api/chat`, with a Gemma 4 model loaded
- on macOS, the shortest supported runtime path today is:
  - `brew install ollama`
  - `brew services start ollama`
  - `ollama pull gemma4:e4b`
- preferred runtime model names are:
  - `gemma4:e2b`
  - `gemma4:e4b`
  - `gemma4:26b`
  - `gemma4:31b`
- the curated Hugging Face GGUF cache under `~/.openagents/pylon/models/huggingface/` is optional and does not make the sellable lane eligible by itself
- sibling `psionic` checkout only if the operator explicitly needs the retained benchmark and validation lane

If local Gemma supply is not available, `Pylon` should still install and run, but it should report `degraded` or `offline` truthfully rather than pretending healthy supply exists.

## Quick Start

Open the local terminal shell:

```bash
cargo pylon
```

If you installed from a release asset instead of a source checkout, run:

```bash
./pylon
```

The first cut is intentionally small. It renders one full-screen transcript shell with:

- whether a Gemma 4-serving path is visible to the node
- live host, CPU, memory, swap, uptime, disk, network, thermal, and power-source state
- a GPU summary and NVIDIA power telemetry when the host can report it
- a built-in Hugging Face Gemma GGUF catalog that shows which curated models are installed, missing, or actively downloading
- a retained transcript area for local shell activity
- a bottom textbox where plain text submits a prompt, `/help` shows the retained shell commands, and `/download <model>` pulls a curated Gemma GGUF into the local Pylon cache

The shell keeps submitted input in the transcript, streams the local Gemma reply back into the same view while it is generating, and carries prior user and assistant turns into the next prompt when local Gemma weights are available. The right column now shows a curated Hugging Face catalog for `gemma-4-e2b`, `gemma-4-e4b`, `gemma-4-26b-a4b`, and `gemma-4-31b`, with live per-model progress bars while downloads are active. Downloaded GGUFs land under `~/.openagents/pylon/models/huggingface/`. The current local chat path only accepts Gemma models visible through the configured local runtime endpoint. The system block is meant to show what the node can honestly report right now about local capacity and headroom. On Macs that includes power source and battery state. On NVIDIA hosts it can also show `power.draw / power.limit` from `nvidia-smi`. The current provider automation still lives in the explicit headless `cargo pylon-headless ...` flow below. `cargo run -p pylon-tui` remains the direct fallback if you want to bypass the alias.

Headless Gemma operator commands now exist too:

- `cargo pylon-headless gemma`
- `cargo pylon-headless gemma download remaining --transport curl`
- `cargo pylon-headless gemma diagnose gemma-4-e4b --max-output-tokens 96 --repeats 3`
- `cargo pylon-headless gemma benchmark all --download-missing --mode matrix`

Use the first, third, and fourth commands for normal onboarding. They inspect the optional curated cache, confirm a loaded runtime model is actually answering `/api/chat`, and persist a local first-run diagnostic report without requiring a sibling `psionic` checkout. Use `gemma download ...` only when you intentionally want the local GGUF cache too.

Important:

- `pylon gemma download ...` only downloads GGUF files into `~/.openagents/pylon/models/huggingface/`
- `pylon gemma download ... --transport curl` is the explicit fallback when the default Rust HTTP transport is unhappy in an SSH/VPN-constrained shell
- `pylon gemma diagnose ...` only benchmarks models that are already loaded in the configured local runtime
- the latest first-run diagnostic report is retained at `~/.openagents/pylon/diagnostics/gemma/latest.json`
- downloaded GGUFs alone do not make supply eligible
- `Pylon` still requires a local runtime endpoint at `local_gemma_base_url` (default `http://127.0.0.1:11434`) that answers `/api/tags` and has a Gemma 4 model loaded
- if `pylon online` reports `degraded` or `NO_ELIGIBLE_SUPPLY`, check that runtime first before falling back to a source build

Treat the full `gemma benchmark` matrix as a retained validation lane, not as required bring-up. That command shells into a sibling Psionic checkout for the real runtime benchmark. By default that means a local `../psionic` clone. Override it with `OPENAGENTS_PSIONIC_REPO=/absolute/path/to/psionic` when needed. If an existing sibling checkout is stale or missing the retained Gemma benchmark entrypoints, refresh it or clone a clean compatible `psionic` checkout and point `OPENAGENTS_PSIONIC_REPO` there.

Pylon now also keeps a focused local ledger at `~/.openagents/pylon/ledger.json`. That file is the retained standalone durability layer for relay state, NIP-90 jobs, invoices, payments, settlements, and local activity replay. It is intentionally narrower than the old archived Pylon database.

The retained relay controls are now exposed in both places:
- TUI: `/relay list`, `/relay add <wss://...>`, `/relay remove <wss://...>`, `/relay refresh`
- headless: `cargo pylon-headless relays`, `cargo pylon-headless relay add <wss://...>`, `cargo pylon-headless relay remove <wss://...>`, `cargo pylon-headless relay refresh`

Relay refresh now reuses the local Pylon node identity for NIP-42 `AUTH` challenges by default. If you need to disable that on a local node, use `cargo pylon-headless config set relay_auth_enabled false`.

The retained provider announcement controls now also exist in both places:
- TUI: `/announce`, `/announce publish`, `/announce refresh`
- headless: `cargo pylon-headless announce`, `cargo pylon-headless announce publish`, `cargo pylon-headless announce refresh`

The current retained announcement scope is one honest local text-generation handler for `kind:5050`. Pylon only publishes it when a local Gemma-backed text-generation path is actually eligible.
When `cargo pylon-headless serve` is running and the node is `online` with
eligible local Gemma supply, Pylon now auto-publishes or refreshes that handler
announcement. `announce publish` remains the explicit manual path when you want
to force the publish step yourself.

The retained provider intake controls also exist in both places:
- TUI: `/provider scan [--seconds <n>]`, `/provider run [--seconds <n>]`
- headless: `cargo pylon-headless provider scan [--seconds <n>]`, `cargo pylon-headless provider run [--seconds <n>]`

The current retained execution scope is narrow and honest. Pylon subscribes to retained inbound `kind:5050` requests on the configured relays, filters targeted jobs, and only accepts work when the provider is online and a local Gemma text-generation path is actually ready. `scan` records intake decisions without executing. `run` has two honest paths:

- for unpriced local work, it publishes a `kind:7000` processing update, executes accepted jobs locally, publishes the retained `kind:6050` result, and links those published event IDs back into the local ledger
- for explicit paid requests, it stops at `payment-required`, creates a local Bolt11 invoice through the retained Spark wallet path, publishes that invoice in a `kind:7000` feedback event, and persists the amount plus Bolt11 string in the local ledger

When that invoice is later marked paid in the local wallet, the next `provider run` picks the same job back up, records the settled payment, executes the work, publishes the retained result, and persists the settlement outcome. The retained `jobs`, `earnings`, `receipts`, and `activity` views now project that local NIP-90 provider settlement state directly from the Pylon ledger instead of forcing the operator to reconstruct it from relay logs.

If the local wallet cannot create an invoice, the provider path fails honestly instead of pretending the request is payable.

The retained wallet controls now also exist in both places:
- TUI: `/wallet`, `/wallet balance`, `/wallet address`, `/wallet invoice <sats> [--description <text>]`, `/wallet pay <bolt11> [--amount-sats <n>]`, `/wallet history [--limit <n>]`
- headless: `cargo pylon-headless wallet status|balance|address|invoice|pay|history`

The retained provider payout controls now also exist in both places:
- TUI: `/payout`, `/payout history [--limit <n>]`, `/payout withdraw <bolt11> [--amount-sats <n>]`
- headless: `cargo pylon-headless payout [--limit <n>]`, `cargo pylon-headless payout withdraw <bolt11> [--amount-sats <n>]`

That path projects retained provider earnings, current wallet balance, and prior withdrawal outcomes from the same local ledger. `payout withdraw` uses the retained wallet send path, persists the resulting withdrawal record locally, and appends a matching relay-activity fact so later transcript views can replay it honestly.

The retained transcript observability commands now also exist in the shell:
- TUI: `/jobs [--limit <n>]`, `/earnings`, `/receipts [--limit <n>]`, `/activity [--limit <n>]`
- headless: `cargo pylon-headless jobs [--limit <n>]`, `cargo pylon-headless earnings`, `cargo pylon-headless receipts [--limit <n>]`, `cargo pylon-headless activity [--limit <n>]`

Those views stay ledger-backed. They can still replay retained provider jobs, earnings, receipts, and relay activity even when there is no live provider service answering local HTTP routes.

The first retained buyer controls now also exist in both places:
- TUI: `/job submit [--bid-msats <n>] [--model <id>] [--provider <pubkey>] [--request-json <json>] <prompt>`, `/job watch [<request_event_id>] [--seconds <n>]`, `/job history [--limit <n>]`, `/job replay <request_event_id>`, `/job approve <request_event_id>`, `/job deny <request_event_id>`, `/job policy [show|auto|manual]`
- headless: `cargo pylon-headless job submit [--bid-msats <n>] [--model <id>] [--provider <pubkey>] [--output <mime>] [--request-json <json>] <prompt>`, `cargo pylon-headless job watch [<request_event_id>] [--seconds <n>]`, `cargo pylon-headless job history [--limit <n>]`, `cargo pylon-headless job replay <request_event_id>`, `cargo pylon-headless job approve <request_event_id>`, `cargo pylon-headless job deny <request_event_id>`, `cargo pylon-headless job policy [show|auto|manual]`

That path publishes a retained `kind:5050` buyer request to the configured relays and persists the outbound request locally in the Pylon ledger. It already supports plain prompt text and structured JSON payload mode. The watch path subscribes to retained `kind:7000` feedback and `kind:6050` results for local buyer jobs, streams those updates into the transcript, and persists the observed payment-required and result state back into the same retained ledger record. `job history` projects the retained buyer ledger back into a short local summary list. `job replay` expands one retained request back into its stored lifecycle, settlement state, and matching relay activity.

When a provider returns `payment-required`, Pylon now keeps the invoice amount, provider pubkey, Bolt11 string, and final payment outcome in the same retained buyer record. Manual buyer mode uses `/job approve <request_event_id>` or `/job deny <request_event_id>`. Auto-pay mode is explicit and off by default. Use `/job policy auto` or `cargo pylon-headless job policy auto` to enable it, and `/job policy manual` to turn it back off.

Initialize a standalone config and identity:

```bash
cargo pylon-headless init
```

With a release asset install, use the same commands through the shipped binary:

```bash
./pylon init
./pylon status --json
./pylon inventory --json
./pylon config show
./pylon online
```

If the shipped binary is current, `status`, `inventory`, `config show`, and `doctor` should all agree on the current standalone lane:

- backend naming should be `local_gemma`
- the launch product should be `psionic.local.inference.gemma.single_node`
- legacy `gpt_oss_*`, `ollama_*`, or Apple-FM-only product names should not be the surfaced launch truth for standalone Pylon onboarding

Inspect status:

```bash
cargo pylon-headless status
cargo pylon-headless status --json
cargo pylon-headless online
cargo pylon-headless announce
cargo pylon-headless announce publish
cargo pylon-headless provider scan --seconds 5
cargo pylon-headless provider run --seconds 5
cargo pylon-headless job submit --model gemma4:e4b --bid-msats 21000 "write a haiku about bitcoin"
cargo pylon-headless job watch --seconds 30
cargo pylon-headless gemma
cargo pylon-headless gemma download remaining --transport curl
cargo pylon-headless gemma diagnose gemma-4-e4b --max-output-tokens 96 --repeats 3
```

Run the retained Psionic benchmark lane only when the operator explicitly needs it:

```bash
cargo pylon-headless gemma benchmark all --download-missing --mode matrix --peer-base-url http://127.0.0.1:18080
```

Inspect provider truth:

```bash
cargo pylon-headless backends
cargo pylon-headless products
cargo pylon-headless sandbox
cargo pylon-headless inventory
cargo pylon-headless jobs
cargo pylon-headless earnings
cargo pylon-headless receipts
cargo pylon-headless activity
```

Inspect or operate the standalone Spark wallet:

```bash
cargo pylon-headless wallet status
cargo pylon-headless wallet balance
cargo pylon-headless wallet address
cargo pylon-headless wallet invoice 21 --description "pylon receive"
cargo pylon-headless wallet pay <bolt11> --amount-sats 21
cargo pylon-headless wallet history --limit 10
cargo pylon-headless payout --limit 10
cargo pylon-headless payout withdraw <bolt11> --amount-sats 21
```

Move the node through explicit lifecycle controls:

```bash
cargo pylon-headless online
cargo pylon-headless pause
cargo pylon-headless resume
cargo pylon-headless offline
```

Run the local admin/status loop:

```bash
cargo pylon-headless serve
```

Important:

- `pylon serve` does not implicitly force the node online.
- lifecycle is explicit; use `pylon online` / `offline` / `pause` / `resume`
- status should show `unconfigured`, `ready`, `online`, `paused`, `draining`, `degraded`, `offline`, or `error` truthfully
- when sandbox supply is declared, `status`, `backends`, `sandbox`, `jobs`, and `receipts` should surface execution classes, profile IDs, termination reasons, and failure reasons without inventing a separate sandbox-only provider model
- `cargo run -p pylon -- <command>` remains a direct fallback if you do not want the alias

## Config and Paths

Default home:

```text
$HOME/.openagents/pylon
```

Important overrides:

- `OPENAGENTS_PYLON_HOME`
- `OPENAGENTS_PYLON_CONFIG_PATH`

The generated config currently includes:

- node label
- payout destination
- identity path
- admin sqlite path
- admin listen address
- wallet network
- wallet API key env var
- wallet storage dir
- local Gemma runtime base URL (`local_gemma_base_url`; legacy `ollama_base_url` still loads on read)
- inventory-control toggles with `local_gemma_*` names; legacy `gpt_oss_*` and `ollama_*` names still load on read
- declared sandbox profiles

## Headless Service Guidance

`Pylon` is service-style. The simplest supported operational pattern is:

1. initialize once with `cargo pylon-headless init`
2. set desired mode explicitly with `cargo pylon-headless online` or `cargo pylon-headless offline`
3. run `cargo pylon-headless serve` under a local service manager
4. use `cargo pylon-headless status`, `backends`, `products`, `inventory`, `jobs`, `earnings`, `receipts`, and `activity` for observability
5. use `cargo pylon-headless sandbox` when you need the declared runtime/profile view for bounded `sandbox_execution`

### `systemd` example

```ini
[Unit]
Description=OpenAgents Pylon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/path/to/openagents
Environment=OPENAGENTS_PYLON_HOME=/var/lib/openagents/pylon
ExecStart=/usr/bin/env cargo pylon-headless serve
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### `launchd` / user-session guidance

On macOS, run the same `cargo pylon-headless serve` command under `launchd`, `tmux`, or another persistent user-session manager. The operational requirement is explicit lifecycle control plus a stable long-running `serve` process, not a specific packaging format.

The current binary-first distribution lane is GitHub Releases with per-platform archives. Source checkout plus Cargo remains the fallback for unsupported platforms and local development.

## Verification and Release Discipline

Do not treat Pylon as shipped because the binary compiles.

Before calling it launch-ready, use:

- [PYLON_VERIFICATION_MATRIX.md](./PYLON_VERIFICATION_MATRIX.md)
- [`scripts/pylon/verify_standalone.sh`](../../scripts/pylon/verify_standalone.sh)
- [`scripts/pylon/verify_nip90_wallet.sh`](../../scripts/pylon/verify_nip90_wallet.sh)

Those materials cover:

- backend detection
- launch-product derivation
- sandbox runtime/profile detection and declared execution classes
- lifecycle transitions
- restart and replay expectations
- local observability surfaces
- local relay and wallet roundtrip coverage for the retained NIP-90 lane
- receipt and earnings visibility, including sandbox failure and termination detail
- Autopilot parity checks
- rollout and launch-truth gates

The retained NIP-90 and wallet verification lane is local and explicit. `scripts/pylon/verify_nip90_wallet.sh` sets a fresh standalone Pylon home to `wallet_network=regtest`, checks the retained headless report commands, and then runs the focused local websocket-relay and wallet-hook tests that cover provider intake, buyer submit/watch/pay, payout persistence, and retained activity replay. It does not claim a live funded external Spark regtest backend.
