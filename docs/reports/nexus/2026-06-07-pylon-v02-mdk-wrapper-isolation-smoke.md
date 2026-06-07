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

## 2026-06-07 Release Candidate Rerun

The later v0.2 release candidate added `runtime.local_daemon_port` to wallet
status JSON so the two-home smoke can prove daemon-port isolation from CLI
output without inspecting private runtime internals.

Rerun summary:

```text
MDK wrapper smoke passed: home-a moneydevkit 39397, home-b moneydevkit 44854
```

The rerun validated:

- `wallet status --json` reports `runtime.runtime_kind=moneydevkit`;
- `wallet status --json` reports a numeric `runtime.local_daemon_port`;
- `wallet balance --json` returns a JSON object;
- `wallet invoice 21 --description ... --json` returns a BOLT11 receive
  artifact;
- `wallet offer --description ... --json` returns a BOLT12 receive artifact;
- `wallet history --limit 5 --json` returns a JSON object;
- two clean Pylon homes used different MDK daemon ports.

No mnemonic, preimage, full invoice, or full offer was committed. Temporary
smoke daemons under `/var/.../T/tmp...` and `/var/.../T/.tmp...` were stopped
after the run; only the real local `.secrets/pylon-v02-mdk-*` wallet daemons
remained.

## Release Status

This is still not a Pylon v0.2 release proof.

Current status as of the later 2026-06-07 release-gate update:

- `#4548` and the old native-LDK treasury refresh path are no longer default
  release blockers for the MDK-default Pylon v0.2 path.
- Omega has recorded a real 100-bitcoin-sat MDK checkout/payment proof.
- The live Artanis SHC bootstrap proof is recorded in
  `docs/reports/nexus/2026-06-07-pylon-v02-live-artanis-shc-bootstrap-proof.md`.
- Later release work published and verified `pylon-v0.2.2` as the stable public
  GitHub binary release. The remaining bootstrap gap is npm publication, which
  is blocked on npm one-time authorization for `@openagentsinc/pylon@0.2.2`.
  See `docs/reports/nexus/2026-06-07-pylon-v02-release-publication-proof.md`.
