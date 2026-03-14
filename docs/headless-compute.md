# Headless Compute

`autopilot-headless-compute` provides three headless runtime surfaces for the current NIP-90 + Spark flow:

- `relay`: tiny local websocket relay for deterministic buyer/provider smoke runs
- `provider`: headless provider mode with a separate Nostr/Spark identity path
- `buyer`: headless Buy Mode loop using the current wallet by default

The same repo now also has an app-owned desktop control plane for the running GUI:

- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/compute_mcp.rs`
- `apps/autopilot-desktop/src/bin/autopilot_compute_mcp.rs`

That control plane is intentionally UI-synced. `autopilotctl` drives the running
desktop app through the same split-shell truth model the user sees on screen:
hotbar shell, Provider Control, Buy Mode, Wallet, Log Stream, and the mirrored
runtime status surfaces all come from the same app-owned snapshot instead of a
separate headless-only state machine.

## Desktop control and `autopilotctl`

`autopilotctl` is the thin CLI client for the running desktop-control runtime.
It can:

- fetch the current desktop-control snapshot
- stream desktop-control event batches
- inspect advanced provider inventory truth for local, clustered, and sandbox
  compute surfaces, including projection source, open quantities, and section
  blockers
- inspect buyer procurement truth for spot and forward RFQs, quote selection,
  accepted orders, and the topology/proof/environment posture of quoted compute
- inspect cluster, sandbox, proof, and challenge status through the same
  app-owned snapshot the desktop uses
- list, open, focus, close, and inspect panes in the running desktop shell
- inspect the active local-runtime truth model (`local_runtime`) and raw GPT-OSS runtime state
- refresh the active local runtime and wallet state
- refresh, warm, unload, and wait on GPT-OSS directly
- bring the provider online or offline
- inspect active-job and buy-mode state
- create, upload, start, wait on, and inspect desktop-owned sandbox jobs
- download sandbox workspace files and declared sandbox artifacts through the
  control plane
- inspect the current tunnel-status surface (`tunnels`) that will eventually
  reflect Psionic-backed service exposure when the desktop app starts owning
  those flows directly
- select the managed NIP-28 main channel
- list NIP-28 groups, channels, and recent messages
- send or retry NIP-28 chat messages
- start and stop Buy Mode against the same in-app state used by the GUI

`autopilot-compute-mcp` is the model-facing companion surface for the same
desktop-control contract. It speaks MCP over stdio and intentionally sits on
top of the running app's manifest, auth token, and action schema instead of
creating a second hidden compute RPC path.

The current MCP tool surface exposes:

- full compute snapshots and inventory summaries
- provider online/offline requests
- cluster status and topology inspection
- sandbox create/get/upload/start/wait/download operations
- proof and challenge inspection

It does not bypass desktop-control policy or kernel authority. If the desktop
would reject an operation through `autopilotctl`, the MCP layer returns the same
failure through the corresponding tool call.

The desktop control runtime writes and exposes:

- `desktop-control.json` manifest
- `latest.jsonl` session-log alias
- per-session JSONL logs

Those files are the source of truth for programmatic verification because they
prove the UI, the control plane, and the runtime logs stayed in sync.

For NIP-90 payment history, those JSONL files are audit and backfill inputs,
not the primary product read model. The desktop imports recoverable payment
facts from session logs into the app-owned
`~/.openagents/autopilot-nip90-payment-facts-v1.json` ledger with degraded
`log-backfill` provenance, and panes query that ledger instead of reparsing raw
logs on demand. During live desktop operation, that background import
intentionally defers hot session logs and caps the imported byte budget so UI
redraws do not block on large or actively growing JSONL files.

Useful `autopilotctl` starting points:

```bash
autopilotctl status
autopilotctl local-runtime status
autopilotctl local-runtime refresh
autopilotctl gpt-oss status
autopilotctl gpt-oss warm --wait
autopilotctl wait gpt-oss-ready
autopilotctl provider online
autopilotctl chat status
autopilotctl chat messages --tail 20
autopilotctl buy-mode status
autopilotctl tunnels status
autopilotctl cluster status
autopilotctl sandbox status
autopilotctl proof status
autopilotctl challenge status
autopilotctl logs --tail 50
autopilotctl pane list
autopilotctl pane status provider_control
autopilotctl pane close provider_control
autopilotctl pane open provider_control
```

`autopilotctl status` now prints the same app-owned inventory projection summary
the Provider Control pane uses, including the projection source, kernel snapshot
ID when present, per-section product and open-quantity counts, and any current
blocker reason for the local, cluster, or sandbox inventory sections.

It also prints the app-owned buyer procurement summary for compute RFQs and
quotes, including the active quote mode, selected quote IDs, and the quoted
backend, topology, proof posture, environment ref, and sandbox profile where
those fields are present.

Useful MCP starting point:

```bash
cargo run -p autopilot-desktop --bin autopilot-compute-mcp -- --manifest \
  ~/.openagents/autopilot/desktop-control.json
```

Typical MCP clients should launch that stdio server after the desktop app is
already running and the desktop-control manifest exists.

Apple-specific bridge flows still exist for the shipped macOS release path:

```bash
autopilotctl apple-fm refresh --wait
autopilotctl apple-fm smoke-test
```

Sandbox lifecycle examples:

```bash
autopilotctl sandbox status
autopilotctl sandbox create pythonexec-profile job-1 /tmp/openagents-sandbox \
  --entrypoint-type workspace-file \
  --entrypoint scripts/job.py \
  --expected-output result.txt
autopilotctl sandbox upload job-1 scripts/job.py ./job.py
autopilotctl sandbox start job-1
autopilotctl sandbox wait job-1 --timeout-ms 30000
autopilotctl sandbox job job-1
autopilotctl sandbox download-artifact job-1 result.txt --output /tmp/result.txt
```

## Supported local-runtime hosts

The app now exposes one app-owned `local_runtime` contract across the split
desktop shell, desktop control, and `autopilotctl`, but the supported host
stories are still lane-specific:

- macOS Apple Silicon: Apple FM via `foundation-bridge`
- supported non-macOS NVIDIA hosts: GPT-OSS via the in-process Psionic CUDA lane
- retained GPT-OSS Metal/CPU backends can still appear in status/readiness
  views, but `Go Online` currently unlocks sell-compute only for CUDA

Provider Control now renders the active local-runtime lane inline. On supported
NVIDIA/CUDA hosts that means the local-runtime area can show GPT-OSS readiness,
artifact state, load state, model path, and `REFRESH` / `WARM` / `UNLOAD`
actions directly, while the separate GPT-OSS workbench remains the prompt
playground and detailed model-management pane.

## GPT-OSS host bring-up

Use this on a supported non-macOS NVIDIA/CUDA host.

The GPT-OSS runtime reads:

- `OPENAGENTS_GPT_OSS_BACKEND=auto|cuda|metal|cpu`
- `OPENAGENTS_GPT_OSS_MODEL_PATH=/path/to/gpt-oss-20b-mxfp4.gguf`

If `OPENAGENTS_GPT_OSS_MODEL_PATH` is unset, the runtime defaults to:

```text
~/models/gpt-oss/gpt-oss-20b-mxfp4.gguf
```

Recommended bring-up flow:

```bash
export OPENAGENTS_GPT_OSS_BACKEND=cuda
export OPENAGENTS_GPT_OSS_MODEL_PATH=/absolute/path/to/gpt-oss-20b-mxfp4.gguf

cargo install --path .
cargo autopilot

autopilotctl local-runtime status
autopilotctl local-runtime refresh
autopilotctl gpt-oss status
autopilotctl gpt-oss warm --wait
autopilotctl wait local-runtime-ready
autopilotctl wait gpt-oss-ready
autopilotctl provider online
```

Useful follow-up checks:

```bash
autopilotctl gpt-oss unload --wait
autopilotctl logs --tail 100
```

Repeatable scripted form:

```bash
scripts/release/check-gpt-oss-nvidia-mission-control.sh
```

Operational notes:

- `local-runtime refresh` always targets the active Provider Control lane, but
  on GPT-OSS it does not load the GGUF by itself
- `gpt-oss warm` and `gpt-oss unload` act directly on the configured GGUF model
- `provider online` will still block if the backend is not `cuda`, the GGUF is
  missing, or the configured model is not loaded
- on retained Metal/CPU GPT-OSS hosts, Provider Control and `autopilotctl` stay
  truthful about runtime state but currently point you back to the GPT-OSS
  workbench instead of unlocking sell-compute

## Local smoke run

This uses the current default buyer wallet and creates a fresh provider account under `target/headless-compute-smoke/provider`:

```bash
scripts/autopilot/headless-compute-smoke.sh
```

Useful env overrides:

- `OPENAGENTS_HEADLESS_PROVIDER_BACKEND=auto|apple-fm|canned`
- `OPENAGENTS_HEADLESS_MAX_REQUESTS=1`
- `OPENAGENTS_HEADLESS_BUDGET_SATS=2`
- `OPENAGENTS_HEADLESS_BUYER_HOME=/path/to/funded-home`
- `OPENAGENTS_HEADLESS_RUN_DIR=/path/to/run-dir`
- `OPENAGENTS_HEADLESS_PROVIDER_HOME=/path/to/provider-home`
- `OPENAGENTS_SPARK_NETWORK=mainnet|regtest`

The smoke script now performs a funding preflight with `spark-wallet-cli`
before it boots the relay/provider pair. If the default buyer wallet does not
have at least the requested budget on the selected Spark network, the script
fails early and tells you which `HOME` it inspected.

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
- `OPENAGENTS_HEADLESS_BUYER_HOME=/path/to/funded-home`
- `OPENAGENTS_HEADLESS_RUN_DIR=/path/to/run-dir`
- `OPENAGENTS_SPARK_NETWORK=mainnet|regtest`

The roundtrip smoke script defaults to the deterministic `canned` backend so the payment path stays
stable even on machines without Apple Foundation Models. It still uses real NIP-90 requests,
real Spark invoices, and real Lightning settlement.

The forward leg spends from the buyer wallet selected by `OPENAGENTS_HEADLESS_BUYER_HOME`
(or the current shell `HOME` if unset). The script now checks that wallet up front and
fails early if it cannot cover the requested forward leg on the selected Spark network.

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
- `OPENAGENTS_SPARK_NETWORK=mainnet|regtest`

The packaged smoke script is intentionally app-owned verification, not a library-only harness.
It proves the production shell, desktop control runtime, and file-backed logs stay in sync
through the v0.1 paid compute loop.

Before generating funding invoices, the script now checks the configured funder wallet
and fails early if `OPENAGENTS_PACKAGED_FUNDER_HOME` / `OPENAGENTS_PACKAGED_FUNDER_IDENTITY_PATH`
do not point at a wallet that can cover the buyer seed amount on the selected Spark network.

The packaged `.app` verification flow is still the macOS Apple FM release path.
There is not yet a separate packaged GPT-OSS bundle check in this repo. For the
supported NVIDIA/CUDA lane, current operator verification is the running desktop
app plus `autopilotctl`, desktop-control snapshots, and the session JSONL logs.

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
  - `shell_mode=hotbar`
  - `dev_mode_enabled=false`

Useful env overrides:

- `OPENAGENTS_AUTOPILOTCTL_RUN_DIR=/path/to/run-dir`
- `OPENAGENTS_AUTOPILOTCTL_FUNDER_HOME=/path/to/funded-home`
- `OPENAGENTS_AUTOPILOTCTL_FUNDER_IDENTITY_PATH=/path/to/funded/identity.mnemonic`
- `OPENAGENTS_AUTOPILOTCTL_FUND_SATS=100`
- `OPENAGENTS_AUTOPILOTCTL_BUDGET_SATS=2`
- `OPENAGENTS_AUTOPILOTCTL_SKIP_BUILD=1`
- `OPENAGENTS_SPARK_NETWORK=mainnet|regtest`

This is the closest thing in the repo to a production-shell end-to-end test.
Use it when the question is not "does the library path work?" but "does the
actual app bundle work when steered programmatically?"

The roundtrip script also preflights the configured funder wallet and will stop
before app launch if it cannot cover the bundle/runtime seed amounts. That keeps
funding failures distinct from split-shell or desktop-control regressions.

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
