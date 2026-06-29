# Pylon Wallet Telemetry Projection

Issue #352 / `OPENAGENTS-L-004` adds redacted Pylon wallet telemetry projections.

The implementation lives in `workers/api/src/pylon-wallet-telemetry.ts`.

## Purpose

This is an operator-diagnostic projection layer for wallet readiness. It keeps
enough safe telemetry refs to understand readiness while excluding all wallet
secret material and preserving read-only authority.

The telemetry surfaces are:

- `sync`;
- `channel`;
- `liquidity`;
- `lsp`;
- `backup`;
- `warning`.

Each surface records:

- state;
- freshness;
- severity;
- evidence refs;
- warning refs;
- caveat refs;
- blocker refs;
- operator action refs; and
- source refs.

## States, Freshness, And Severity

Telemetry state is:

- `ok`;
- `degraded`;
- `attention_required`;
- `blocked`;
- `unknown`.

Freshness is:

- `fresh`;
- `stale`;
- `expired`;
- `unknown`.

Severity is:

- `info`;
- `warning`;
- `critical`;
- `blocked`.

Stale, expired, degraded, or attention-required telemetry requires warning,
caveat, or operator action refs. Blocked telemetry requires blocker refs.
Critical or blocked telemetry requires operator action refs.

## Authority Boundary

`PYLON_WALLET_TELEMETRY_READ_ONLY_AUTHORITY` explicitly denies:

- wallet mutation;
- channel mutation;
- LSP mutation;
- backup mutation;
- live wallet spend;
- payout dispatch;
- settlement mutation.

The telemetry contract cannot rebalance channels, change an LSP, restore or
write backups, spend, dispatch payouts, or claim settlement.

## Redaction

Public, customer, team, and agent projections redact private provider refs,
wallet refs, channel refs, liquidity refs, LSP refs, backup refs, evidence
refs, source refs, warning refs, and operator action refs according to
audience.

All projections reject recovery phrases, raw entropy, private keys, preimages,
raw channel monitor state, raw telemetry payloads, raw backups, credentials,
provider secrets, customer data, and raw timestamps.

Operator projections can show safe internal refs such as `channel.private.*` or
`lsp.private.*`, but they still cannot contain wallet material or raw telemetry
payloads.

## Tests

`workers/api/src/pylon-wallet-telemetry.test.ts` covers:

- fixture decoding;
- required telemetry surfaces;
- read-only authority;
- freshness, severity, and blocked-state evidence requirements;
- public redaction of private provider, wallet, channel, liquidity, LSP,
  operator, and source refs; and
- rejection of recovery phrases, raw entropy, private keys, preimages, raw
  channel monitor state, credentials, provider secrets, and raw telemetry.
