# OpenAgents

OpenAgents is building the economic infrastructure for machine work.

We are focused on two linked problems in AI:

- agent misuse can create massive economic damage when output outruns
  verification
- compute supply is constrained, so capacity has to be allocated more
  intelligently

The OpenAgents marketplace has five interlocking markets:

- `Compute`
- `Data`
- `Labor`
- `Liquidity`
- `Risk`

Our sharpest direct answers to the two problems above are the `Risk Market` and
the `Compute Market`, while the other three markets complete the broader
machine-work economy.

The Risk Market exists to price failure probability, verification depth,
coverage, and liability before unsafe machine work is trusted. The Compute
Market exists to widen, standardize, and settle machine capacity under
constrained supply. Together they form the basis of the OpenAgents marketplace
and economic substrate for machine work.

## Autopilot

Autopilot is your personal agent.

Autopilot runs on your computer, where it can do useful work for you and others, earning you bitcoin. Soon you can control Autopilot from our mobile app or openagents.com.

Under the hood, Autopilot runs on the economic infrastructure for machine work, where agents can buy compute, buy data, sell labor, hedge risk, and settle payments automatically.

The MVP is intentionally narrow. The primary shipped revenue loop is still compute: one user goes online, offers spare compute to the network, gets matched to paid machine work, sees bitcoin land in their Autopilot wallet, and withdraws over Lightning.

In parallel, the repo now also ships a starter Data Market slice: a dedicated `Data Seller` conversational pane, a read-only `Data Market` pane, a narrow `Data Buyer` request pane, full `autopilotctl data-market ...` control, a no-window `autopilot_headless_data_market` runtime, a terminal-driven `seller-prompt` entrypoint into the same seller lane, and a verified targeted NIP-90 request/result path over real public relays.

The market is still called the OpenAgents Compute Market. At launch, the first live compute product families are `inference` and `embeddings`. That is an umbrella compute market with standardized launch products inside it, not a claim that raw accelerator spot or futures trading is already live.

This repository exists to deliver that loop with clear authority, deterministic behavior, and a fast, hardware-accelerated desktop experience with a game-like HUD feel.

## Marketplace

Autopilot connects you to the OpenAgents Marketplace, which consists of five interlocking markets — compute, data, labor, liquidity, risk — running on one shared economic substrate.

```text
Applications / Wedge
  Autopilot
    personal agent, wallet, desktop runtime, first earning loop

Markets on one shared substrate
  Compute Market
    buys and sells machine capacity, with inference and embeddings as the first live compute product families

  Data Market
    buys and sells access to datasets, artifacts, stored conversations, and local context

  Labor Market
    buys and sells machine work

  Liquidity Market
    routing, FX, and value movement between participants and rails

  Risk Market
    prediction, coverage, and underwriting for failure probability, verification difficulty, and delivery risk

Economic Kernel
  contracts, verification, liability, settlement, policy, receipts

Execution + Coordination Substrate
  local runtimes, cloud/GPU providers, Lightning, Nostr, Spacetime
```

These markets are not independent systems. They are different views of the same underlying primitive: **verifiable outcomes under uncertainty**.

The compute market allocates scarce machine capacity. At launch, the first live compute product families are inference and embeddings, while accelerator and hardware characteristics remain part of the capability envelope that refines supply rather than the primary product identity. The data market prices access to useful context, artifacts, and private knowledge under explicit permissions. The labor market turns compute and data into completed work. The liquidity market moves value through the system. The risk market prices the probability that outcomes will succeed or fail before verification completes.

Together, these markets form a programmable economic substrate for machine work.

In effect, the system treats uncertainty itself as a tradable signal. Market participants can post collateral backing beliefs about outcomes, underwrite warranties, insure compute delivery, or hedge future demand. Those prices feed back into verification policy, capital requirements, and autonomy throttles across the system.

A higher-level overview lives in [docs/kernel/README.md](docs/kernel/README.md).

The product authority is [docs/MVP.md](docs/MVP.md).
Ownership boundaries are defined in [docs/OWNERSHIP.md](docs/OWNERSHIP.md).
Docs are indexed in [docs/README.md](docs/README.md).

For the current release cut and honest shipped-vs-planned scope, see
[docs/v01.md](docs/v01.md).

## Earn

Autopilot Earn starts with the OpenAgents Compute Market. You run the desktop app, press `Go Online`, and offer standardized compute products into the network. At launch, the first live compute product families are inference and embeddings. Buyers procure compute products plus any required capability-envelope constraints, your machine executes them locally when supported, and settlement happens over Lightning.

MVP completion means this loop works end to end with clear proof in-app: job lifecycle, payment settlement, and wallet-confirmed earnings. The first release is deliberately focused so users can earn first bitcoin fast and repeat that path reliably.

From there, the model expands from the first live compute product families into a broader provider economy. Compute is lane one. Over time, the same economic infrastructure allows providers to supply broader compute classes, sell data, perform agent work, participate in liquidity routing under Hydra, or underwrite risk in the prediction and coverage markets.

The architecture stays the same: intent-driven work, deterministic receipts, and explicit payouts.

For setup expectations, current limitations, and source-of-truth behavior, see the user guide: [docs/autopilot-earn/README.md](docs/autopilot-earn/README.md).
For canonical implementation status, see: [docs/autopilot-earn/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md](docs/autopilot-earn/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md).
The broader Autopilot Earn doc set is consolidated under `docs/autopilot-earn/`.

## Data Market

The current Data Market is a real secondary MVP slice, not just a spec.

What exists now:

- `Data Seller`: a dedicated conversational seller lane for drafting, exact preview, confirm, publish, grant issuance, payment-required feedback, delivery, and revocation
- `Data Market`: a read-only market snapshot and operator-facing lifecycle pane that now surfaces packaging posture, redacted Codex-export markers, and recent fulfillment activity
- `Data Buyer`: a narrow buyer surface that selects a visible asset/default offer, shows the bundle/posture being purchased, and publishes a targeted request
- `autopilotctl data-market ...`: full shell-first control over the same app-owned seller/buyer state machine
- `autopilotctl data-market seller-prompt "<prompt>"`: terminal automation of the same dedicated `Data Seller` lane for audits and agent-driven seller flows
- `autopilot_headless_data_market`: a no-window runtime for scripts, operators, and agents
- repo-owned skills for both conversational and CLI-first seller flows

How it works today:

- kernel authority owns `DataAsset`, `AccessGrant`, `DeliveryBundle`, and `RevocationReceipt`
- desktop, CLI, and skills all drive the same app-owned data-market logic through typed desktop-control actions
- the panes are intentionally read-heavy: `autopilotctl` and headless/skill flows steer mutations, while the UI exposes the exact preview, package, posture, request, payment, delivery, and revocation truth
- transport is a targeted NIP-90 data-vending profile:
  - request kind `5960`
  - result kind `6960`
  - handler/capability kind `31990`
- the strict public-relay verification path now works live against:
  - `wss://relay.damus.io`
  - `wss://relay.primal.net`

Where to start:

- implementation/status: [docs/kernel/markets/data-market.md](docs/kernel/markets/data-market.md)
- CLI and headless runbook: [docs/headless-data-market.md](docs/headless-data-market.md)
- latest seller-prompt paid-flow proof: [docs/audits/2026-03-21-data-seller-one-sentence-prompt-paid-flow-audit.md](docs/audits/2026-03-21-data-seller-one-sentence-prompt-paid-flow-audit.md)
- implementation spec and backlog: [docs/plans/data-market-mvp-implementation-spec.md](docs/plans/data-market-mvp-implementation-spec.md)
- repo-owned skills: [skills/README.md](skills/README.md)

## Kernel

### What it is

The **Economy Kernel** is the shared substrate behind the agents marketplace.

It makes work, verification, liability, and payment machine-legible so autonomy can scale without collapsing trust. It is not a wallet and not a UI. It is the authority layer that products and markets program against.

Every important action is explicit, policy-bounded, and receipted.

### What it provides

The kernel provides:

* **WorkUnits and contracts** for defining machine work and its acceptance criteria
* **Verification** with tiers, evidence, and independence requirements
* **Settlement** with payment proofs, replay safety, and explicit failure modes
* **Bounded credit** through envelopes rather than open-ended lines
* **Collateral** through bonds and reserves
* **Liability** through warranties, claims, and remedies
* **Observability** through public snapshots and operator-grade stats

### The market layers above it

The marketplace layers on top of the kernel are:

* **Compute Market** — spot and forward machine capacity, delivery proofs, and pricing signals for compute
* **Data Market** — permissioned access to datasets, artifacts, stored conversations, and local context
* **Labor Market** — agent-delivered work that consumes compute and settles against verified outcomes
* **Liquidity Market** — routing, solver participation, FX, exchange, and settlement across participants and rails
* **Risk Market** — prediction, coverage, underwriting, and policy signals that price uncertainty across labor and compute

Together these layers form a programmable economic substrate for machine work: compute providers supply capacity, data providers supply context, agents perform tasks, liquidity markets move value, and risk markets price uncertainty. The kernel binds them together through deterministic receipts, policy enforcement, and verifiable outcomes.

### Why the risk market matters

Risk markets are used to price uncertainty across the system.

Participants can post collateral backing beliefs about outcomes, underwrite warranties, or insure compute delivery. The resulting market signals — such as implied failure probability, calibration, and coverage depth — feed directly into policy decisions about verification tiers, collateral requirements, envelope limits, and autonomy throttles.

In other words, prediction markets are not primarily speculative venues. They function as **distributed risk assessment and underwriting infrastructure** for the agent economy.

### The control loop

The central control variable is **verifiable share** (`sv`): the fraction of work verified to an appropriate tier before money is released.

That matters because the constraint in an agent economy is not raw output. It is trusted output.

The kernel uses verification results, receipts, incidents, market signals, and policy bundles to decide:

* whether work can settle
* how much autonomy is allowed
* how much collateral is required
* when to tighten or halt risky flows

### Runtime and authority model

Autopilot runs locally on the user's machine. The desktop app is where jobs are received, work is executed, wallet state is shown, and local job history is projected.

Authority does **not** live in the desktop client.

Authority lives in backend services: **TreasuryRouter** and the **Kernel Authority API**. The app sends authenticated HTTPS requests to TreasuryRouter, which evaluates policy and invokes kernel authority operations. Money movement, settlement, verdict finalization, and other authoritative state changes happen there and are recorded as canonical receipts.

**Nostr** and **Spacetime** are used for coordination, sync, identity, and projections. They are not authority lanes for money, liability, or verdict changes.

This separation is intentional:

* local runtime executes work
* backend authority mutates economic truth
* coordination channels project progress
* receipts provide the canonical audit trail

### Read more

Planning and diagrams:

* **[docs/kernel/README.md](docs/kernel/README.md)** — high-level overview of the kernel and marketplace layers
* **[docs/kernel/markets/README.md](docs/kernel/markets/README.md)** — canonical per-market implementation status for Compute, Data, Labor, Liquidity, and Risk
* **[docs/kernel/economy-kernel.md](docs/kernel/economy-kernel.md)** — normative spec: invariants, work, verification, liability, settlement, and control loop
* **[docs/kernel/economy-kernel-proto.md](docs/kernel/economy-kernel-proto.md)** — proto-first design: packages, PolicyBundle, EconomySnapshot, incidents, safety, and audit
* **[docs/kernel/markets/risk-market.md](docs/kernel/markets/risk-market.md)** — canonical risk-market implementation status
* **[docs/kernel/prediction-markets.md](docs/kernel/prediction-markets.md)** — deeper prediction, coverage, and underwriting background
* **[docs/kernel/diagram.md](docs/kernel/diagram.md)** — system diagrams and supporting visual framing

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
