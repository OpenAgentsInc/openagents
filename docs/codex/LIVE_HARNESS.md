# Codex Live Harness

## Purpose

`codex-live-harness` is a programmatic integration probe for the Codex app-server used by `autopilot-desktop`.

It is designed to:

- simulate core chat-pane flows (`refresh threads`, `new chat`, `send`)
- exercise the broader app-server API surface from one command
- capture notifications/requests after each call so regressions are visible without GUI interaction

The harness runs against the user’s real local Codex installation and account/session state.

## Location

- Binary: `apps/autopilot-desktop/src/bin/codex_live_harness.rs`
- Target: `cargo run -p autopilot-desktop --bin codex-live-harness`

## Current Model Policy

The harness does **not** default to legacy hardcoded model IDs.

- Default behavior: resolve model from live `model/list` (`is_default == true`, else first model)
- Override behavior: pass `--model <id>`

This keeps probes aligned with currently available Codex models.

## API Coverage

The harness currently probes these app-server methods:

- Auth/account:
  - `account/read`
  - `account/rateLimits/read`
- Models/features:
  - `model/list`
  - `collaborationMode/list`
  - `experimentalFeature/list`
  - `mock/experimentalMethod`
- Config/external config:
  - `config/read`
  - `configRequirements/read`
  - `externalAgentConfig/detect`
  - optional: `externalAgentConfig/import` (`--include-writes`)
- MCP:
  - `mcpServerStatus/list`
  - optional: `config/mcpServer/reload` (`--include-writes`)
- Apps/skills:
  - `app/list`
  - `skills/list`
  - `skills/remote/list`
  - optional: `skills/remote/export` (`--include-writes`)
  - optional: `skills/config/write` (`--include-writes`)
- Threads/chat:
  - `thread/list`
  - `thread/loaded/list`
  - `thread/read`
  - `thread/start`
  - `turn/start` (if `--prompt`, optional skill attachment via `--skill`)
- Thread mutation probes (default on, disable with `--skip-thread-mutations`):
  - `thread/name/set`
  - `thread/backgroundTerminals/clean`
  - `thread/compact/start`
  - `thread/fork`
  - `thread/rollback` (if turns exist)
  - `thread/archive`
  - `thread/unarchive`
- Additional execution/review probes:
  - `command/exec`
  - `review/start`
- Optional live Blink swap probe (real network, no mocks):
  - `skills/blink/scripts/swap_quote.js`
  - `skills/blink/scripts/swap_execute.js` (when `--blink-swap-execute-live`)
- Optional StableSats+SA live 3-wallet scenario (real network, no mocks):
  - multi-wallet `balance.js` snapshots
  - cross-wallet BTC/USD transfers via invoice + fee probe + pay
  - wallet-native BTC<->USD swaps via `swap_execute.js`
  - before/after balance verification and effective fee/spread reporting
- Experimental probes (default on, disable with `--skip-experimental`):
  - `fuzzyFileSearch/sessionStart`
  - `fuzzyFileSearch/sessionUpdate`
  - `fuzzyFileSearch/sessionStop`
  - `thread/realtime/start`
  - `thread/realtime/appendText`
  - `thread/realtime/stop`
  - `windowsSandbox/setupStart`

## Important Runtime Findings (Observed Live)

From a live run on **February 28, 2026**:

- `thread/read` with `includeTurns=true` on a brand-new thread can fail until first user message:
  - error: `thread ... is not materialized yet; includeTurns is unavailable before first user message`
- Some app-server builds do not expose `thread/realtime/*` methods:
  - error: unknown variant for `thread/realtime/start|appendText|stop`
- `windowsSandbox/setupStart` validates `mode` strictly:
  - accepted values include `elevated` or `unelevated`

These are useful compatibility checks, not harness failures.

## Usage

Minimal run:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents
```

Run with prompt to test end-to-end thread materialization and turn flow:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --prompt "harness ping" \
  --max-events 8
```

Run with write probes enabled:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --prompt "harness ping" \
  --include-writes
```

Run a skill-attached turn (example: `blink`):

First install the skill into Codex user skills (one-time):

```bash
python3 /Users/christopherdavid/.codex/skills/.system/skill-installer/scripts/install-skill-from-github.py \
  --repo OpenAgentsInc/openagents \
  --path skills/blink
```

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --model gpt-5.2-codex \
  --include-writes \
  --skill blink \
  --prompt "Use the blink skill to summarize available payment operations and first command to check balance."
```

Run live Blink swap quote + execute probe:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --skip-experimental \
  --skip-thread-mutations \
  --blink-swap-live \
  --blink-swap-direction btc-to-usd \
  --blink-swap-amount 10 \
  --blink-swap-unit sats \
  --blink-swap-execute-live
```

Run StableSats+SA live 3-wallet scenario:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --skip-experimental \
  --skip-thread-mutations \
  --blink-stablesats-sa-live \
  --blink-stablesats-sa-rounds 1 \
  --blink-stablesats-sa-transfer-btc-sats 600 \
  --blink-stablesats-sa-transfer-usd-cents 75 \
  --blink-stablesats-sa-convert-btc-sats 450 \
  --blink-stablesats-sa-convert-usd-cents 90
```

Override model explicitly:

```bash
cargo run -p autopilot-desktop --bin codex-live-harness -- \
  --cwd /Users/christopherdavid/code/openagents \
  --model gpt-5.3-codex
```

## Spark Funding Flow (For Live Blink Payments)

Use `spark-wallet-cli` to verify Spark funds and to fund Spark before paying fresh Blink invoices.

Binary location:

- `apps/autopilot-desktop/src/bin/spark_wallet_cli.rs`
- `cargo run -p autopilot-desktop --bin spark-wallet-cli -- ...`

Check Spark balance and connectivity:

```bash
set -a; source .env.local; set +a
cargo run -p autopilot-desktop --bin spark-wallet-cli -- \
  --network mainnet status
```

Generate Spark funding targets:

```bash
# Spark transfer address
cargo run -p autopilot-desktop --bin spark-wallet-cli -- \
  --network mainnet spark-address

# Lightning-like Spark invoice for funding
cargo run -p autopilot-desktop --bin spark-wallet-cli -- \
  --network mainnet create-invoice 2000 --description "fund-spark-wallet" --expiry-seconds 3600

# On-chain BTC funding address
cargo run -p autopilot-desktop --bin spark-wallet-cli -- \
  --network mainnet bitcoin-address
```

Regenerate a fresh Blink BTC invoice and pay it from Spark:

```bash
set -a; source .env.local; set +a
node skills/blink/scripts/create_invoice.js 100 --no-subscribe "spark-funding-test" | tee /tmp/blink_btc_invoice.out
INVOICE=$(sed '/^Subscription skipped/d' /tmp/blink_btc_invoice.out | jq -r '.paymentRequest')

cargo run -p autopilot-desktop --bin spark-wallet-cli -- \
  --network mainnet pay-invoice "$INVOICE"
```

Check settlement status on Blink:

```bash
HASH=$(sed '/^Subscription skipped/d' /tmp/blink_btc_invoice.out | jq -r '.paymentHash')
node skills/blink/scripts/check_invoice.js "$HASH"
```

Notes:

- Blink USD invoices expire in about 5 minutes; regenerate right before payment.
- If Spark payment returns `insufficient funds`, fund Spark first using one of the targets above.

## StableSats+SA Wallet Topology

The live 3-wallet scenario uses:

- `operator`: `BLINK_API_KEY` (and optional `BLINK_API_URL`)
- `sa-alpha`: `BLINK_API_KEY_SA_ALPHA` (optional `BLINK_API_URL_SA_ALPHA`)
- `sa-beta`: `BLINK_API_KEY_SA_BETA` (optional `BLINK_API_URL_SA_BETA`)

Credential resolution order per variable:

1. Process environment variable.
2. macOS keychain entry (`com.openagents.autopilot.credentials`) using that variable name as account.

If a per-wallet URL variable is missing, the harness falls back to `BLINK_API_URL`.

Limitations:

- Each wallet must be a real Blink account with both BTC and USD wallets enabled.
- Scenario steps send real payments and perform real conversions; there is no stub path.
- Internal transfer fee probes are usually `0 sats`, but still measured and reported.

## StableSats+SA Fee And Spread Semantics

The scenario report includes:

- `transfer_fee_probe_total_sats`: sum of `fee_probe.js` estimates before payments.
- `transfer_effective_fee_total_sats|cents`: observed source-minus-destination transfer delta residual.
- `swap_effective_spread_total_sats|cents`: quote output minus observed settlement delta per direction.

Typical live behavior (as of March 2, 2026):

- Transfer fee probes are often `0 sats` for internal routes.
- Swap explicit fee fields are `0`, but settlement can differ by 1 unit (rounding spread).

## StableSats+SA Troubleshooting

- Missing wallet credentials:
  - Verify `BLINK_API_KEY`, `BLINK_API_KEY_SA_ALPHA`, and `BLINK_API_KEY_SA_BETA`.
- Insufficient balances:
  - Fund each participating wallet before running the scenario.
- Contract mismatch failures:
  - If `--blink-stablesats-sa-require-success` is on, the harness fails when script-reported deltas do not match observed post-balance deltas.
- Node/runtime issues:
  - Confirm `node` is installed and runnable from PATH.

## Flags

- `--cwd <path>`: working directory sent to app-server
- `--model <id>`: explicit model override (otherwise use live default model)
- `--prompt <text>`: sends a turn after `thread/start`
- `--skill <name>`: resolves and attaches a skill in `turn/start` as `UserInput::Skill`
  - Lookup order: local `skills/list` name/path match, then optional remote match/export if `--include-writes`
  - If found but disabled: harness enables it through `skills/config/write` when `--include-writes` is set
- `--list-limit <n>`: `thread/list` limit (default `20`)
- `--drain-ms <n>`: idle settle window for collecting events (default `700`)
- `--timeout-ms <n>`: max wait window for each event drain (default `4000`)
- `--max-events <n>`: print cap per phase for notifications/requests (default `24`)
- `--include-writes`: run write/mutation probes for config/skills/mcp/export paths
- `--skip-experimental`: skip experimental probe group
- `--skip-thread-mutations`: skip thread mutation probes
- `--allow-echo-replies`: disable harness failure when assistant echoes prompt exactly
- `--blink-swap-live`: run a live Blink quote probe from `skills/blink/scripts/swap_quote.js`
- `--blink-swap-direction <btc-to-usd|usd-to-btc>`: swap direction for live probe
- `--blink-swap-amount <n>`: probe amount (`>0`)
- `--blink-swap-unit <sats|cents>`: probe unit (default derives from direction)
- `--blink-swap-execute-live`: run real execute attempt from `skills/blink/scripts/swap_execute.js`
- `--blink-swap-require-success`: fail unless execute returns `SUCCESS`
- `--blink-swap-memo <text>`: optional memo for execute probe
- `--blink-stablesats-sa-live`: run real 3-wallet StableSats+SA transfer/swap scenario
- `--blink-stablesats-sa-rounds <n>`: number of scenario rounds (default `1`)
- `--blink-stablesats-sa-transfer-btc-sats <n>`: BTC transfer size per round
- `--blink-stablesats-sa-transfer-usd-cents <n>`: USD transfer size per round
- `--blink-stablesats-sa-convert-btc-sats <n>`: operator BTC->USD swap size per round
- `--blink-stablesats-sa-convert-usd-cents <n>`: SA alpha USD->BTC swap size per round
- `--blink-stablesats-sa-require-success`: fail on any non-`SUCCESS` step (default enabled)
- `--blink-stablesats-sa-memo-prefix <text>`: memo prefix for transfer/swap operations

## Output Format

Each probe prints:

- method name
- `status=ok` or `status=error`
- compact summary (counts, selected ids, etc.)
- `post-<method>` event drain summary:
  - notifications count
  - requests count
  - capped event lines

This allows deterministic diffing of behavior between app-server versions.
