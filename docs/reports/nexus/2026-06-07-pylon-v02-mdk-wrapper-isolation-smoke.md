# Pylon v0.2 MDK wrapper isolation smoke

Date: 2026-06-07

## Purpose

Validate the new decision that Pylon v0.2 should wrap MoneyDevKit's
`@moneydevkit/agent-wallet` as the default self-custodial Lightning wallet
runtime, while keeping native LDK Node as an explicit lower-level path.

The MoneyDevKit agent-wallet docs specify:

- commands emit JSON on stdout;
- the wallet daemon runs a local HTTP server, defaulting to `localhost:3456`;
- config and history live under `~/.mdk-wallet/`;
- `MDK_WALLET_PORT` can override the daemon port;
- `init --network signet` selects signet for testnet-style operation;
- `send` supports BOLT11, BOLT12, LNURL, and Lightning addresses.

## Finding

The wrapper already initialized MDK with `init --network <mainnet|signet>`, but
it did not set `MDK_WALLET_PORT`. That meant two supposedly isolated Pylon homes
on one machine could collide on MDK's default `3456` daemon port.

This pass fixed that by deriving a stable Pylon-scoped MDK daemon port from the
Pylon wallet home and exporting it on every MDK command. The wrapper still runs
MDK with a Pylon-scoped `HOME`, so MDK state remains under the selected Pylon
wallet storage root rather than the operator's personal home wallet.

## Local Smoke

Command shape:

```bash
OPENAGENTS_PYLON_HOME=<clean-home-a> cargo run -q -p pylon --bin pylon -- wallet status --json
OPENAGENTS_PYLON_HOME=<clean-home-a> cargo run -q -p pylon --bin pylon -- wallet balance --json
OPENAGENTS_PYLON_HOME=<clean-home-a> cargo run -q -p pylon --bin pylon -- wallet invoice 21 --description "pylon v0.2 mdk two-home smoke" --json
OPENAGENTS_PYLON_HOME=<clean-home-a> cargo run -q -p pylon --bin pylon -- wallet offer --description "pylon v0.2 mdk two-home smoke" --json
OPENAGENTS_PYLON_HOME=<clean-home-a> cargo run -q -p pylon --bin pylon -- wallet history --json
OPENAGENTS_PYLON_HOME=<clean-home-b> cargo run -q -p pylon --bin pylon -- wallet status --json
OPENAGENTS_PYLON_HOME=<clean-home-b> cargo run -q -p pylon --bin pylon -- wallet offer --description "pylon v0.2 mdk second home smoke" --json
```

Observed redacted result:

```json
{
  "aRuntimeKind": "moneydevkit",
  "bRuntimeKind": "moneydevkit",
  "aNetwork": "bitcoin",
  "bNetwork": "bitcoin",
  "aTotalSats": 0,
  "aLightningSats": 0,
  "aInvoicePrefix": "lnbc210n1p4z",
  "aInvoiceHasPaymentHash": true,
  "aOfferPrefix": "lno1pgwhq7tv",
  "bOfferPrefix": "lno1pgs8q7tv",
  "offersAreDifferent": true,
  "aHistoryRows": 0
}
```

The full BOLT11 invoice and BOLT12 offers are intentionally not committed.

Temporary smoke daemons were stopped after the run. The isolated real-bitcoin
test wallet daemon under `.secrets/pylon-v02-mdk-real-wallet-home` remains
running on port `3462` so it can observe a live payment if the operator funds
the test wallet.

## Lifecycle Smoke

Issue `#4549` added Pylon-native wallet lifecycle commands so agents and
operators do not need to call MDK directly for daemon control:

```bash
cargo run -q -p pylon --bin pylon -- wallet start --json
cargo run -q -p pylon --bin pylon -- wallet status --json
cargo run -q -p pylon --bin pylon -- wallet restart --json
cargo run -q -p pylon --bin pylon -- wallet stop --json
```

Observed redacted result:

```json
{
  "startAction": "start",
  "startRuntime": "moneydevkit",
  "startStatus": "started",
  "statusRuntime": "moneydevkit",
  "statusState": "connected",
  "restartAction": "restart",
  "restartStatus": "started",
  "stopAction": "stop",
  "stopStatus": "stopped"
}
```

The first restart attempt exposed an MDK teardown race: `stop` can return before
the daemon's listening port is reusable, causing immediate `restart` to see
`EADDRINUSE` or a start timeout. The wrapper now waits after stop and retries
restart once. The final smoke confirmed no `agent-wallet` process remained
under the temporary Pylon home after `wallet stop` returned.

## Tests

Passed:

```bash
cargo test -p pylon moneydevkit_
cargo test -p pylon parse_wallet_command_supports_balance_and_history
cargo test -p pylon mock_runtime_returns_deterministic_wallet_reports
cargo check -p pylon
cargo test -p pylon-tui footer_hints_focus_on_passive_homework_earning
cargo test -p pylon-tui wallet_card_surfaces_balance_and_clear_actions
```

The Pylon tests cover stable MDK daemon port derivation and wallet-owned
MoneyDevKit provider-registration metadata, plus parser and deterministic mock
coverage for the new lifecycle commands. The TUI tests cover the updated
MDK-default operator copy.

## Release Status

This is still not a Pylon v0.2 release proof.

Remaining blockers before creating `pylon-v0.2.0`:

- deploy the Nexus tracked-payout reconciliation fix from issue `#4548`;
- run admin treasury refresh on production;
- confirm `/v1/treasury/status` no longer reports
  `continuity_alert:confirmations_stalled`;
- fund the isolated MDK real-bitcoin test wallet or otherwise provide a live
  wallet with bitcoin available for the proof;
- run a production accepted-work payout proof with real bitcoin movement;
- record the production proof receipt and only then create the GitHub release.
