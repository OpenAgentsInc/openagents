# OpenAgents

OpenAgents builds one vertically integrated system for machine work.

Use this frame:

- `Autopilot` is the product shell.
- `Probe` is the coding runtime.
- `Forge` is the software factory.
- `Pylon` and `Nexus` form the compute network.
- `Psionic` is the execution substrate underneath parts of that stack.

This repo is not the entire OpenAgents system. It does own several of the key
shipped surfaces:

- the `Autopilot` desktop product
- the planned Tauri `Autopilot` shell at `apps/autopilot`
- the `Pylon` provider connector
- the `Nexus` hosted coordination and authority slice
- the `wgpui` product UI stack
- starter kernel and proto authority surfaces
- the current secondary Data Market slice

The current product authority is [docs/MVP.md](docs/MVP.md).
Ownership boundaries are defined in [docs/OWNERSHIP.md](docs/OWNERSHIP.md).
Docs are indexed in [docs/README.md](docs/README.md).
For the current release cut and honest shipped-vs-planned scope, see
[docs/v01.md](docs/v01.md).

## Product System

### `Autopilot`

`Autopilot` is the primary product in this repo.

It runs on the user's machine. It owns the app shell, the wallet-visible earn
loop, the chat and thread UX, provider controls, operator controls, and the
desktop surfaces where OpenAgents becomes legible as one product.

The MVP remains compute-first. A user opens the desktop app, clicks
`Go Online`, executes paid work locally, sees sats land in the built-in wallet,
and withdraws over Lightning.

`Autopilot` is also the shell that can host multiple capabilities over time:

- the current local coding shell
- the provider earn loop
- the secondary Data Market surfaces
- later richer workflow, labor, and business-system surfaces

The current refactor plan keeps the next Tauri implementation in this monorepo:

```text
apps/autopilot
```

Do not call that app `autopilot-tauri`. The product name is `Autopilot`; Tauri
is the desktop shell implementation.

### `Probe`

`Probe` is the coding runtime.

It lives in the sibling `probe` repo. `openagents` should consume it through a
stable runtime boundary rather than absorbing its runtime truth into the app.

Today `Autopilot` still carries the app-owned coding shell with Codex-backed
paths. The planned shape is explicit in `docs/MVP.md`: `Probe` becomes the
owned coding runtime underneath the same `Autopilot` shell.

### `Forge`

`Forge` is the software factory.

It lives in the sibling `forge` repo, with the current web shell living in the
sibling `openagents.com` repo. `Forge` owns lifecycle truth above the coding
runtime: work orders, runs, evidence, verification, delivery, and durable team
workflow.

`Autopilot` can host or project parts of that experience locally. `Forge`
should still own the factory lifecycle itself.

### `Pylon`

`Pylon` is the standalone provider connector in this repo.

It exposes local machine capacity into the network. In MVP terms, it is one of
the concrete ways the Compute Market becomes real instead of staying a spec.

### `Nexus`

`Nexus` is the hosted coordination and authority slice in this repo.

It handles relay, public stats, starter-job coordination, and backend mutation
surfaces that the desktop product depends on. It is the default network and
authority surface for the current MVP path.

### `Psionic`

`Psionic` is the execution substrate underneath parts of this stack.

It lives in the sibling `psionic` repo and is consumed here through pinned
dependencies. It provides execution/runtime machinery that supports current and
future compute-product expansion.

### `openagents.com`

`openagents.com` is the public web and domain-entry surface.

It lives in the sibling `openagents.com` repo. It is not the same thing as
`Autopilot`. Today it also hosts the current web shell for parts of `Forge`.

## Flywheel

OpenAgents compounds through one connected loop:

- make `Probe` good enough that developers want to use it every day
- turn that usage into durable workflow demand through `Forge`
- satisfy more of that demand on OpenAgents-controlled compute and models
  through `Pylon`, `Nexus`, and `Psionic`
- feed those gains back into `Probe` and `Forge`
- package the whole system coherently through `Autopilot`

That is the current system shape:

- `Probe` wins developers
- `Forge` wins teams and workflows
- `Pylon` / `Nexus` improve economics and model control
- `Autopilot` turns that stack into one product people can actually use

## Markets

The five markets still matter. They should not be treated as five disconnected
peer products.

They are economic functions that plug into the product system above.

### Compute Market

This is the primary shipped loop in this repo.

`Autopilot` and `Pylon` expose local machine capacity. `Nexus` coordinates
network presence and starter authority flows. The first live compute product
families are `inference` and `embeddings`, with the retained MVP still
inference-led today.

### Data Market

This is a real secondary slice in this repo.

The current implementation includes:

- `Data Seller`
- `Data Market`
- `Data Buyer`
- `autopilotctl data-market ...`
- `autopilot_headless_data_market`

The Data Market plugs into the same product shell and authority stack. It lets
users sell permissioned access to packaged local data, artifacts, and context.

### Labor Market

This market handles machine-delivered work.

It depends on the coding/runtime and workflow surfaces above it. `Forge` is the
durable workflow system for that path. Starter authority flows also exist in
the kernel slices in this repo.

### Liquidity Market

This market handles value movement, routing, and settlement support.

It plugs into wallet flows, payment routing, reserves, and later solver lanes.
It is not the first thing a new user sees, but it matters underneath the
machine-work economy.

### Risk Market

This market handles uncertainty, coverage, and policy signals.

It plugs into verification depth, coverage pricing, claims, and autonomy
throttles. It matters because machine-work output must be trusted before it is
settled.

## What Ships In This Repo Today

This repo is intentionally pruned for MVP work.

The shipped emphasis is narrower than the full system vision:

- compute-first `Autopilot Earn`
- a desktop-owned coding shell
- real `Pylon` and `Nexus` entry points
- app-owned desktop control and verification flows
- a real but secondary Data Market slice
- starter kernel and proto authority surfaces for broader market expansion

That means the near-term job here is not to treat every future market at equal
weight. The near-term job is to make the current loops reliable and legible.

## Where To Read Next

- [docs/MVP.md](docs/MVP.md)
  - current product authority for the repo
- [docs/OWNERSHIP.md](docs/OWNERSHIP.md)
  - current crate and subsystem boundary rules
- [docs/kernel/README.md](docs/kernel/README.md)
  - high-level kernel and market overview
- [docs/kernel/markets/README.md](docs/kernel/markets/README.md)
  - per-market implementation status
- [docs/autopilot-earn/README.md](docs/autopilot-earn/README.md)
  - current earn-loop setup and behavior
- [docs/headless-data-market.md](docs/headless-data-market.md)
  - current Data Market CLI and headless runbook

## Agent Install Instructions

If you are using a terminal coding agent, you can paste this prompt:

```text
Clone https://github.com/OpenAgentsInc/openagents.git. Ensure the Rust toolchain is installed so `cargo` and `rustc` are available.

If you are on macOS and using the Apple FM release path, test the bridge first. From the repo root: (1) build the bridge: `cd swift/foundation-bridge && ./build.sh` (requires Swift: Xcode or `xcode-select --install`). (2) Run it: `./bin/foundation-bridge` (default port 11435). (3) Verify: `curl -s http://127.0.0.1:11435/health` — confirm a JSON response. (4) Then start the desktop app: `cargo install --path .` and `cargo autopilot`. If the alias is unavailable, run `cargo run -p autopilot-desktop --bin autopilot-desktop --` instead.

If you are on a supported Linux NVIDIA host and using GPT-OSS, set `OPENAGENTS_GPT_OSS_BACKEND=cuda` and `OPENAGENTS_GPT_OSS_MODEL_PATH=/path/to/gpt-oss-20b-mxfp4.gguf`, then start the app. Mission Control now auto-warms the configured GPT-OSS model on startup. Verify with `autopilotctl local-runtime status` and `autopilotctl wait local-runtime-ready`.

See AGENTS.md for the Apple bridge rule and [docs/headless-compute.md](docs/headless-compute.md) for the current local-runtime runbooks.
```

## Run Locally

Requires the Rust toolchain (`cargo`/`rustc`).

Normal repo builds use a vendored `protoc` resolver through
[.cargo/config.toml](.cargo/config.toml), so you do not need to install a
machine-local Protocol Buffers compiler just to build `openagents`.

**Install build prerequisites on Debian/Ubuntu:**
```bash
sudo apt-get install -y pkg-config libssl-dev
```

**Run:**
```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo autopilot
```

`cargo autopilot` is defined in `.cargo/config.toml` as a local Cargo alias for `autopilot-desktop`.

### Psionic Checkout For Cross-Repo Dev

Normal `cargo autopilot` and `cargo check` flows fetch Psionic through pinned
git dependencies automatically. You only need a local Psionic checkout when you
are:

- editing Psionic alongside `openagents`
- running retained cross-repo release/validation scripts
- invoking Psionic-owned tests or binaries directly

The retained scripts assume a sibling checkout at `../psionic` by default:

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
git clone https://github.com/OpenAgentsInc/psionic.git
cd openagents
```

If you keep Psionic somewhere else, set
`OPENAGENTS_PSIONIC_REPO=/absolute/path/to/psionic`.

**Run on Linux with GPT-OSS/CUDA:**
```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
export OPENAGENTS_GPT_OSS_BACKEND=cuda
export OPENAGENTS_GPT_OSS_MODEL_PATH=/absolute/path/to/gpt-oss-20b-mxfp4.gguf
cargo autopilot
```

If `OPENAGENTS_GPT_OSS_MODEL_PATH` is unset, the runtime defaults to `~/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`.

### Apple FM bridge (macOS, for Go Online)

On macOS, going **Go Online** in the desktop app uses **Apple Foundation Models** via a small Swift HTTP bridge. You need the bridge built and (for the system model to be ready) **Apple Intelligence** enabled: System Settings → Apple Intelligence → turn on.

- **Build the bridge once** (from repo root): `cd swift/foundation-bridge && ./build.sh`. This produces `bin/foundation-bridge`. Building requires the Swift compiler (Xcode from the App Store, or `xcode-select --install` for Command Line Tools only).
- **Test the bridge**: run `./bin/foundation-bridge`, then `curl -s http://127.0.0.1:11435/health` — you should get a JSON response. The desktop app can also start the bridge automatically when you open Mission Control.
- **Shipping the app** so users don’t build on their machine: build the bridge once, then include `bin/foundation-bridge` in your app bundle (e.g. `YourApp.app/Contents/MacOS/foundation-bridge` or `Contents/Resources/foundation-bridge`). See [swift/foundation-bridge/README.md](swift/foundation-bridge/README.md) for steps and [OpenAgentsInc/psionic bridge considerations](https://github.com/OpenAgentsInc/psionic/blob/main/docs/FM_BRIDGE_CONSIDERATIONS.md) for full bridge considerations (architecture, discovery, shipping, user requirements).

### GPT-OSS local runtime (supported Linux NVIDIA/CUDA hosts)

On supported Linux NVIDIA hosts, Mission Control and `autopilotctl` use the
same app-owned local-runtime contract for GPT-OSS.

- Export `OPENAGENTS_GPT_OSS_BACKEND=cuda`
- Export `OPENAGENTS_GPT_OSS_MODEL_PATH=/absolute/path/to/gpt-oss-20b-mxfp4.gguf`
- If the model-path env var is unset, the runtime looks for `~/models/gpt-oss/gpt-oss-20b-mxfp4.gguf`
- Launch the app with `cargo autopilot`
- Mission Control should auto-warm the configured GPT-OSS model on startup and clear the preflight blocker once the model is ready
- Verify with:

```bash
autopilotctl local-runtime status
autopilotctl wait local-runtime-ready
autopilotctl wait gpt-oss-ready
autopilotctl provider online
```

If you want to force a new runtime probe after changing env vars or swapping GGUFs:

```bash
autopilotctl local-runtime refresh
autopilotctl wait local-runtime-ready
```

Repeatable scripted form:

```bash
scripts/release/check-gpt-oss-nvidia-mission-control.sh
```

`Go Online` currently unlocks sell-compute on the GPT-OSS lane only when the
backend is `cuda` and the configured GGUF model is loaded. Retained Metal/CPU
GPT-OSS backends can still appear in Mission Control and desktop-control status,
but they remain runtime bring-up/debug paths instead of supported sell-compute
hosts.

## Programmatic Control And Verification

The repo now includes an app-owned control plane for the running desktop app:

- implementation: [apps/autopilot-desktop/src/desktop_control.rs](apps/autopilot-desktop/src/desktop_control.rs)
- CLI client: [apps/autopilot-desktop/src/bin/autopilotctl.rs](apps/autopilot-desktop/src/bin/autopilotctl.rs)
- documentation: [docs/headless-compute.md](docs/headless-compute.md)

This is UI-synced control, not a separate fake harness. `autopilotctl` drives
the same Mission Control state the GUI renders, and the runtime persists:

- `desktop-control.json`
- `latest.jsonl`
- per-session JSONL logs

For the strongest packaged end-to-end check, use:

```bash
scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh
```

That script builds the bundled app, launches a bundled app plus a runtime app,
drives both through `autopilotctl`, verifies NIP-28 chat, and proves buyer and
seller Spark settlement through the real desktop shell. Full details and
related headless flows are documented in [docs/headless-compute.md](docs/headless-compute.md).
