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
- live CPU and memory state
- a basic GPU summary when the host can report one
- a retained transcript area for local shell activity
- a bottom textbox that accepts one submitted input at a time

The shell now keeps submitted input in the transcript instead of showing only a static status panel. The local Gemma chat-stream adapter is already in `Pylon`; the next step is wiring the `/chat` command into this shell. The current provider automation still lives in the explicit headless `cargo pylon-headless ...` flow below. `cargo run -p pylon-tui` remains the direct fallback if you want to bypass the alias.

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
