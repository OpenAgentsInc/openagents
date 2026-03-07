# Pylon Standalone Operator Guide

`Pylon` is the standalone provider binary for the OpenAgents Compute Market.

It is a narrow, headless supply connector. It is not a buyer shell, not a labor runtime, and not a raw accelerator exchange.

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

Initialize a standalone config and identity:

```bash
cargo run -p pylon -- init
```

Inspect status:

```bash
cargo run -p pylon -- status
cargo run -p pylon -- status --json
```

Inspect provider truth:

```bash
cargo run -p pylon -- backends
cargo run -p pylon -- products
cargo run -p pylon -- inventory
cargo run -p pylon -- jobs
cargo run -p pylon -- earnings
cargo run -p pylon -- receipts
```

Move the node through explicit lifecycle controls:

```bash
cargo run -p pylon -- online
cargo run -p pylon -- pause
cargo run -p pylon -- resume
cargo run -p pylon -- offline
```

Run the local admin/status loop:

```bash
cargo run -p pylon -- serve
```

Important:

- `pylon serve` does not implicitly force the node online.
- lifecycle is explicit; use `pylon online` / `offline` / `pause` / `resume`
- status should show `unconfigured`, `ready`, `online`, `paused`, `draining`, `degraded`, `offline`, or `error` truthfully

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

1. initialize once with `pylon init`
2. set desired mode explicitly with `pylon online` or `pylon offline`
3. run `pylon serve` under a local service manager
4. use `pylon status`, `backends`, `products`, `inventory`, `jobs`, `earnings`, and `receipts` for observability

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
ExecStart=/usr/bin/env cargo run -p pylon -- serve
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### `launchd` / user-session guidance

On macOS, run the same `pylon serve` command under `launchd`, `tmux`, or another persistent user-session manager. The operational requirement is explicit lifecycle control plus a stable long-running `serve` process, not a specific packaging format.

## Verification and Release Discipline

Do not treat Pylon as shipped because the binary compiles.

Before calling it launch-ready, use:

- [PYLON_VERIFICATION_MATRIX.md](./PYLON_VERIFICATION_MATRIX.md)
- [`scripts/pylon/verify_standalone.sh`](../../scripts/pylon/verify_standalone.sh)

Those materials cover:

- backend detection
- launch-product derivation
- lifecycle transitions
- restart and replay expectations
- local observability surfaces
- receipt and earnings visibility
- Autopilot parity checks
- rollout and launch-truth gates
