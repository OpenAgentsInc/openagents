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

## Launch Truth

The market is still the **OpenAgents Compute Market**.

At launch, the first live compute product families are:

- `inference`
- `embeddings`

The first backend-specific launch products are:

- `ollama.text_generation`
- `ollama.embeddings`
- `apple_foundation_models.text_generation`

Do not describe launch as raw GPU or raw accelerator trading. Accelerator, memory, and platform facts remain capability-envelope qualifiers that refine supply rather than replace product identity.

Current planned-but-not-live surfaces:

- raw hardware spot or futures markets
- Apple Foundation Models embeddings
- buyer mode in `Pylon`
- broad wallet-shell UX
- sandbox execution as a generally released family

## Prerequisites

Minimum local requirements:

- Rust toolchain installed (`cargo`, `rustc`)
- repo checkout available locally
- a writable local home/config path

Backend-specific requirements:

- `Ollama` for launch inference and embeddings supply
- `Apple Foundation Models` bridge for Apple FM inference supply

If neither backend is available, `Pylon` should still install and run, but it should report `degraded` or `offline` truthfully rather than pretending healthy supply exists.

## Quick Start

Open the local terminal shell:

```bash
cargo pylon
```

The first cut is intentionally small. It renders one full-screen transcript shell with:

- whether a Gemma 4-serving path is visible to the node
- live host, CPU, memory, swap, uptime, disk, network, thermal, and power-source state
- a GPU summary and NVIDIA power telemetry when the host can report it
- a built-in Hugging Face Gemma GGUF catalog that shows which curated models are installed, missing, or actively downloading
- a retained transcript area for local shell activity
- a bottom textbox where plain text submits a prompt, `/help` shows the retained shell commands, and `/download <model>` pulls a curated Gemma GGUF into the local Pylon cache

The shell keeps submitted input in the transcript, streams the local Gemma reply back into the same view while it is generating, and carries prior user and assistant turns into the next prompt when local Gemma weights are available. The right column now also shows a small curated Hugging Face catalog for `gemma-3-1b`, `gemma-3n-e4b`, `gemma-3-4b`, `gemma-3-12b`, and `gemma-3-27b`, with live per-model progress bars while downloads are active. Downloaded GGUFs land under `~/.openagents/pylon/models/huggingface/`. The current local chat path still uses backend-visible Gemma models from the existing serving seam. The system block is meant to show what the node can honestly report right now about local capacity and headroom. On Macs that includes power source and battery state but not direct watt draw. On NVIDIA hosts it can also show `power.draw / power.limit` from `nvidia-smi`. The current provider automation still lives in the explicit headless `cargo pylon-headless ...` flow below. `cargo run -p pylon-tui` remains the direct fallback if you want to bypass the alias.

Pylon now also keeps a focused local ledger at `~/.openagents/pylon/ledger.json`. That file is the retained standalone durability layer for relay state, NIP-90 jobs, invoices, payments, settlements, and local activity replay. It is intentionally narrower than the old archived Pylon database.

The retained relay controls are now exposed in both places:
- TUI: `/relay list`, `/relay add <wss://...>`, `/relay remove <wss://...>`, `/relay refresh`
- headless: `cargo pylon-headless relays`, `cargo pylon-headless relay add <wss://...>`, `cargo pylon-headless relay remove <wss://...>`, `cargo pylon-headless relay refresh`

Relay refresh now reuses the local Pylon node identity for NIP-42 `AUTH` challenges by default. If you need to disable that on a local node, use `cargo pylon-headless config set relay_auth_enabled false`.

The retained wallet controls now also exist in both places:
- TUI: `/wallet`, `/wallet balance`, `/wallet address`, `/wallet invoice <sats> [--description <text>]`, `/wallet pay <bolt11> [--amount-sats <n>]`, `/wallet history [--limit <n>]`
- headless: `cargo pylon-headless wallet status|balance|address|invoice|pay|history`

Initialize a standalone config and identity:

```bash
cargo pylon-headless init
```

Inspect status:

```bash
cargo pylon-headless status
cargo pylon-headless status --json
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
```

Inspect or operate the standalone Spark wallet:

```bash
cargo pylon-headless wallet status
cargo pylon-headless wallet balance
cargo pylon-headless wallet address
cargo pylon-headless wallet invoice 21 --description "pylon receive"
cargo pylon-headless wallet pay <bolt11> --amount-sats 21
cargo pylon-headless wallet history --limit 10
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
- Ollama base URL
- Apple FM base URL
- inventory-control toggles
- declared sandbox profiles

## Headless Service Guidance

`Pylon` is service-style. The simplest supported operational pattern is:

1. initialize once with `cargo pylon-headless init`
2. set desired mode explicitly with `cargo pylon-headless online` or `cargo pylon-headless offline`
3. run `cargo pylon-headless serve` under a local service manager
4. use `cargo pylon-headless status`, `backends`, `products`, `inventory`, `jobs`, `earnings`, and `receipts` for observability
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

## Verification and Release Discipline

Do not treat Pylon as shipped because the binary compiles.

Before calling it launch-ready, use:

- [PYLON_VERIFICATION_MATRIX.md](./PYLON_VERIFICATION_MATRIX.md)
- [`scripts/pylon/verify_standalone.sh`](../../scripts/pylon/verify_standalone.sh)

Those materials cover:

- backend detection
- launch-product derivation
- sandbox runtime/profile detection and declared execution classes
- lifecycle transitions
- restart and replay expectations
- local observability surfaces
- receipt and earnings visibility, including sandbox failure and termination detail
- Autopilot parity checks
- rollout and launch-truth gates
