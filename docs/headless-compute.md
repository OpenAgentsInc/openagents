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
