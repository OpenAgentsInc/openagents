# Headless Compute

`autopilot-headless-compute` provides three headless runtime surfaces for the current NIP-90 + Spark flow:

- `relay`: tiny local websocket relay for deterministic buyer/provider smoke runs
- `provider`: headless provider mode with a separate Nostr/Spark identity path
- `buyer`: headless Buy Mode loop using the current wallet by default

The same repo now also has an app-owned desktop control plane for the running GUI:

- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`

That control plane is intentionally UI-synced. `autopilotctl` drives the running
desktop app through the same Mission Control truth model the user sees on
screen, instead of mutating a separate headless-only state machine.

## Desktop control and `autopilotctl`

`autopilotctl` is the thin CLI client for the running desktop-control runtime.
It can:

- fetch the current Mission Control snapshot
- stream desktop-control event batches
- refresh Apple FM and wallet state
- bring the provider online or offline
- inspect active-job and buy-mode state
- select the managed NIP-28 main channel
- list NIP-28 groups, channels, and recent messages
- send or retry NIP-28 chat messages
- start and stop Buy Mode against the same in-app state used by the GUI

The desktop control runtime writes and exposes:

- `desktop-control.json` manifest
- `latest.jsonl` session-log alias
- per-session JSONL logs

Those files are the source of truth for programmatic verification because they
prove the UI, the control plane, and the runtime logs stayed in sync.

Useful `autopilotctl` starting points:

```bash
autopilotctl status
autopilotctl provider online
autopilotctl chat status
autopilotctl chat messages --tail 20
autopilotctl buy-mode status
autopilotctl logs --tail 50
```

## Local smoke run

This uses the current default buyer wallet and creates a fresh provider account under `target/headless-compute-smoke/provider`:

```bash
scripts/autopilot/headless-compute-smoke.sh
```

Useful env overrides:

- `OPENAGENTS_HEADLESS_PROVIDER_BACKEND=auto|apple-fm|canned`
- `OPENAGENTS_HEADLESS_MAX_REQUESTS=1`
- `OPENAGENTS_HEADLESS_RUN_DIR=/path/to/run-dir`
- `OPENAGENTS_HEADLESS_PROVIDER_HOME=/path/to/provider-home`

## Multi-payment roundtrip

This runs multiple paid requests from the default wallet into a fresh provider wallet,
then flips the roles and spends the earned sats back the other way:

```bash
scripts/autopilot/headless-compute-roundtrip.sh
```

Useful env overrides:

- `OPENAGENTS_HEADLESS_FORWARD_COUNT=6`
- `OPENAGENTS_HEADLESS_REVERSE_COUNT=3`
- `OPENAGENTS_HEADLESS_INTERVAL_SECONDS=8`
- `OPENAGENTS_HEADLESS_TIMEOUT_SECONDS=75`
- `OPENAGENTS_HEADLESS_PROVIDER_BACKEND=canned|auto|apple-fm`
- `OPENAGENTS_HEADLESS_RUN_DIR=/path/to/run-dir`

The roundtrip smoke script defaults to the deterministic `canned` backend so the payment path stays
stable even on machines without Apple Foundation Models. It still uses real NIP-90 requests,
real Spark invoices, and real Lightning settlement.

`OPENAGENTS_HEADLESS_REVERSE_COUNT` is treated as a ceiling, not a guarantee. After the forward leg,
the script measures the actual sats burned per send from the default wallet and trims the reverse leg
to what the fresh secondary wallet can really afford under the current Lightning fee conditions.

The script emits:

- `summary.txt` human summary
- `summary.json` machine-readable request/payment report
- requested vs executed reverse job counts
- per-phase buyer/provider logs
- Spark status snapshots before, between, and after the two phases

## Packaged app smoke run

This launches the real bundled `Autopilot.app`, points it at a deterministic local relay,
drives it through `autopilotctl`, and verifies the production shell completes the provider
side of the paid loop all the way through settlement:

```bash
scripts/release/check-v01-packaged-compute.sh
```

What it does:

- builds `Autopilot.app`, `autopilotctl`, `autopilot-headless-compute`, and `spark-wallet-cli`
- bundles `foundation-bridge` into the app
- launches the packaged app executable with isolated `HOME` and `OPENAGENTS_AUTOPILOT_LOG_DIR`
- configures the bundle against a local deterministic relay via its settings file
- brings the provider online through `autopilotctl`
- starts a controlled headless buyer targeted to the packaged provider
- asserts on the bundled app's `latest.jsonl` and per-session JSONL logs:
  - request accepted
  - request running
  - request delivered
  - `provider.result_published`
  - `provider.payment_requested`
  - `provider.settlement_confirmed`

Useful env overrides:

- `OPENAGENTS_PACKAGED_RUN_DIR=/path/to/run-dir`
- `OPENAGENTS_PACKAGED_FUNDER_HOME=/path/to/funded-home`
- `OPENAGENTS_PACKAGED_FUNDER_IDENTITY_PATH=/path/to/funded/identity.mnemonic`
- `OPENAGENTS_PACKAGED_BUYER_FUNDING_SATS=50`
- `OPENAGENTS_PACKAGED_BUDGET_SATS=2`
- `OPENAGENTS_PACKAGED_SKIP_BUILD=1`

The packaged smoke script is intentionally app-owned verification, not a library-only harness.
It proves the production shell, desktop control runtime, and file-backed logs stay in sync
through the v0.1 paid compute loop.

## Packaged app buyer + seller + chat roundtrip

This is the stronger packaged verification path for the current release cut:

```bash
scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh
```

What it does:

- builds the bundled `Autopilot.app`
- launches both a bundled app and a separate runtime app
- drives both apps entirely through `autopilotctl`
- selects the managed NIP-28 main channel in both apps
- verifies bidirectional NIP-28 chat
- brings both providers online
- funds Spark wallets as needed
- runs paid buyer/seller flows in both directions
- asserts on desktop-control snapshots plus `latest.jsonl` / session logs for:
  - NIP-28 presence and message delivery
  - targeted NIP-90 request dispatch
  - buyer payment settlement
  - provider settlement confirmation

This is the closest thing in the repo to a production-shell end-to-end test.
Use it when the question is not "does the library path work?" but "does the
actual app bundle work when steered programmatically?"

## Separate processes

Run a local relay:

```bash
cargo run -p autopilot-desktop --bin autopilot-headless-compute -- relay --listen 127.0.0.1:18490
```

Run a provider on a separate identity/wallet:

```bash
cargo run -p autopilot-desktop --bin autopilot-headless-compute -- provider \
  --relay ws://127.0.0.1:18490 \
  --identity-path ~/.openagents/headless-provider/identity.mnemonic \
  --backend auto
```

Run a buyer with the current default wallet:

```bash
cargo run -p autopilot-desktop --bin autopilot-headless-compute -- buyer \
  --relay ws://127.0.0.1:18490 \
  --max-settled-requests 1 \
  --fail-fast
```

Targeting a specific provider on a shared relay:

```bash
cargo run -p autopilot-desktop --bin autopilot-headless-compute -- buyer \
  --relay wss://your-relay.example \
  --target-provider-pubkey <provider-npub-or-hex>
```
