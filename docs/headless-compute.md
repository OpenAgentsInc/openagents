# Headless Compute

`autopilot-headless-compute` provides three headless runtime surfaces for the current NIP-90 + Spark flow:

- `relay`: tiny local websocket relay for deterministic buyer/provider smoke runs
- `provider`: headless provider mode with a separate Nostr/Spark identity path
- `buyer`: headless Buy Mode loop using the current wallet by default

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
