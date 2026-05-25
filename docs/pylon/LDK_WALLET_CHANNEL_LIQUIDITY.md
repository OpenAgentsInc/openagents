# Pylon LDK Wallet Channel and Liquidity Readiness

Pylon v0.2 surfaces channel and liquidity readiness directly through the local
LDK wallet:

```bash
cargo pylon-headless wallet channels --json
cargo pylon-headless wallet status --json
cargo pylon-headless wallet telemetry --json
```

The JSON payloads intentionally separate four balances that operators otherwise
mix together:

- `spendable_onchain_sats`: Bitcoin that the wallet can currently spend on
  chain after reserves and pending state.
- `anchor_reserve_sats`: Bitcoin kept available for channel safety and fee
  obligations.
- `outbound_liquidity_sats`: Lightning channel balance that can be sent.
- `inbound_liquidity_sats`: remote channel balance that can receive Lightning
  payments.

`total_sats` is not the same thing as "ready to receive Lightning." A Pylon can
hold on-chain funds, generate on-chain receive addresses, and still have
`can_receive_lightning=false` until a usable channel with inbound liquidity is
present.

## Readiness States

`wallet channels --json` and `wallet status --json` include
`lightning_readiness`:

- `onchain_only_no_channels`: on-chain receive works, but no Lightning channel
  is visible.
- `channel_pending`: a channel exists, but it is waiting for confirmations or
  readiness.
- `peer_disconnected`: a channel exists, but no usable peer connection is
  visible.
- `needs_inbound_liquidity`: a usable channel exists, but Lightning receive
  needs inbound capacity.
- `receive_ready_send_limited`: Lightning receive is viable, while sends need
  more outbound liquidity.
- `lightning_ready`: the wallet has a usable connected channel with inbound and
  outbound liquidity.

Typed warning codes are stable support handles:

- `lightning_receive_unavailable_no_channels`
- `lightning_receive_pending_channel`
- `lightning_receive_peer_disconnected`
- `lightning_receive_needs_inbound_liquidity`
- `lightning_send_needs_outbound_liquidity`

## LSP Posture

The linked LDK Node stack exposes LSPS1 and LSPS2 integration hooks, including
LSPS2 just-in-time inbound liquidity. Pylon v0.2 records this as
`supported_protocols=["lsps1","lsps2"]`, but reports `lsp_state=not_configured`
until operator LSP credentials, policy, and network routing defaults are
configured. That keeps the user-facing wallet honest: Pylon can explain why
Lightning receive is not viable without pretending an LSP has been selected.

## Operator Guidance

Use `wallet address --json` to receive on-chain Bitcoin when
`can_receive_onchain=true` and `can_receive_lightning=false`.

Use `wallet channels --json` to inspect the channel count, pending count,
peer connectivity, inbound liquidity, and outbound liquidity before debugging
failed receives or sends.

Use `wallet telemetry --json` for support bundles. Telemetry carries the same
readiness and LSP state, while redacting endpoint credentials and excluding raw
channel state, mnemonic material, entropy, private keys, and preimages.
