# MDK Restore Send-Readiness Preflight

Date: 2026-06-10

Status: Pylon-side honest re-scope for
`blocker.product_promises.mdk_send_readiness_not_proven_for_restore`.

## Summary

The current evidence supports wallet-home preservation as the safe operator
rule. Mnemonic-only restore can be receive-capable and show a positive balance,
but that is not send-readiness evidence.

Pylon now exposes a send-readiness preflight in `pylon wallet status`. The
preflight classifies the wallet as send-ready only when all of these are true:

- MDK reports `send_ready`;
- the wallet balance is known;
- outbound capacity is known and positive;
- the wallet is not a mnemonic-only restore; and
- `MDK_WALLET_PORT` is explicitly set so the daemon cannot silently cross-talk
  with the default port.

No raw wallet material, mnemonics, invoices, payment hashes, preimages,
wallet-home paths, ports, or daemon logs are recorded here.

## Side-By-Side Evidence

The source evidence is the public-safe incident report at
`apps/openagents.com/docs/nexus/2026-06-08-mdk-agent-wallet-outbound-capacity-restore-report.md`.
No new funded reproduction was run for this issue because small-sats funding
and wallet-home access are operator steps.

| Mode | Balance state | Receive-ready state | Send-ready state | Result |
| --- | --- | --- | --- | --- |
| Mnemonic-only restore into a fresh wallet home | Positive balance observed | Receive-capable | Not send-ready | Small BOLT11 send failed with insufficient outbound capacity. |
| Original funded wallet home | Positive balance observed | Receive-capable | Send-capable | Two accepted-work payouts settled for 1 sat each. |

The safe conclusion is operational rather than theoretical: until MDK clarifies
or fixes restore semantics, OpenAgents paid Pylon smokes must preserve the
original funded agent-wallet home for payer sends. A mnemonic-only restore is
not accepted as send-ready evidence even when balance is positive.

## `pylon wallet status` Preflight

The `sendReadinessPreflight` object reports:

- `mode`: `original-wallet-home`, `mnemonic-only-restore`, or `unknown`;
- `portConfigured`: whether `MDK_WALLET_PORT` was explicitly set;
- `portIsolationRef`: `mdk.port.configured` or
  `mdk.port.default_possible_crosstalk`;
- `balanceKnown`;
- `outboundCapacityKnown`;
- `outboundCapacityPositive`;
- `sendReady`; and
- `blockerRefs`.

Relevant blocker refs:

- `blocker.wallet.mdk_port_unset`
- `blocker.wallet.balance_unknown`
- `blocker.wallet.outbound_capacity_unknown`
- `blocker.wallet.outbound_capacity_zero`
- `blocker.wallet.mnemonic_only_restore_not_send_ready`
- `blocker.wallet.send_readiness_unproven`

## Upstream Report

Filed upstream:

- `https://github.com/moneydevkit/mdk-checkout/issues/96`

Requested clarification:

- If mnemonic-only restore should preserve outbound send readiness, treat the
  observed state as a restore/liquidity bug.
- If outbound capacity depends on wallet home, wallet id, VSS/channel state, or
  another non-mnemonic artifact, document that mnemonic-only restore is not
  send-ready.
- Expose public-safe preflight fields such as `send_ready`,
  `outbound_capacity_sats`, or `max_sendable_sats`.
- Add a stable error code for insufficient outbound capacity.

## Blocker Transition Proposal

Use the honest re-scope path, not proof. The blocker should clear only after
the product promise records that live paid Pylon sends require original
wallet-home preservation until MDK exposes a restore path that proves outbound
capacity. The Pylon classifier enforces this locally by refusing to classify a
mnemonic-only restore, zero/unknown outbound capacity, or unpinned MDK daemon
port as send-ready.
