# Pylon LDK Wallet Telemetry

`wallet telemetry` is the v0.2 operator support surface for the built-in Pylon
LDK wallet:

```bash
cargo pylon-headless wallet telemetry --json
```

The payload is `pylon.wallet.telemetry.v1`. It is intentionally support-focused:
it shows whether the local wallet is healthy, payable, synced, backed up, and
liquid without exposing recovery material or raw Lightning state.

## Payload Shape

The telemetry payload includes:

- runtime kind, network, node ID, and runtime status
- chain source kind and redacted endpoint, plus RGS gossip source if configured
- sync state and latest LDK wallet/RGS sync timestamps
- on-chain, spendable on-chain, Lightning, anchor reserve, and total balances
- channel counts by usable, ready, pending, and inactive state
- inbound/outbound liquidity totals and buckets
- Lightning receive/send readiness and LSP configuration state
- backup status, stale state, artifact count, and last encrypted backup digest
- typed warning and error codes for operator action
- redaction policy metadata

The human render prints the same operational facts in line-oriented form for
local support sessions.

## Redaction Contract

Telemetry, status output, and wallet status receipts must not include:

- Pylon recovery phrases
- raw LDK node entropy
- private keys
- payment preimages
- bearer tokens, API keys, or endpoint query credentials
- raw channel monitor or channel-state bytes

Configured chain-source and RGS endpoints are redacted before publication. For
example, `https://user:pass@example/rgs?access_token=secret` becomes:

```text
https://[redacted]@example/rgs?[redacted]
```

The focused wallet-runtime tests cover both the structured telemetry payload and
the string redaction helpers.

The matching channel-first operator surface is
`docs/pylon/LDK_WALLET_CHANNEL_LIQUIDITY.md`.
