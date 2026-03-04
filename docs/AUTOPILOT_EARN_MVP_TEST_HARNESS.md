# Autopilot Earn MVP Programmatic Test Harness

Date: 2026-03-04

## Purpose

Programmatically validate as much of the MVP earn loop as possible:

1. NIP-90 request publish
2. NIP-90 result receive/correlation
3. Job lifecycle progression
4. Wallet-confirmed payout gating
5. Earnings/reconciliation projection

Scope note: this harness validates the compute-provider lane only. Liquidity-solver lane tests are future scope.

## Added / Extended Harnesses

### 1) NIP-90 Relay Round-Trip Integration (`nostr-client`)

File: `crates/nostr/client/tests/dvm_submit_await_e2e.rs`

What it covers:
- Spins up a real local websocket relay mock.
- Verifies `submit_job_request_and_await_result` publishes request event and receives correlated result event.
- Verifies timeout path when no result arrives.

Primary assertions:
- result kind is in NIP-90 result range (`6050` in harness).
- result contains `e` tag referencing original request id.
- timeout errors are deterministic and request-correlated.

### 2) Desktop Earn Loop End-to-End State Harness (`autopilot-desktop`)

File: `apps/autopilot-desktop/src/app_state.rs` test `mission_control_earn_loop_wallet_confirmed_end_to_end`

What it covers:
- request accepted from inbox,
- active job transitions `accepted -> running -> delivered -> paid`,
- history receipt recorded from active job,
- wallet payment evidence injected,
- reconciliation confirms earned sats,
- earnings scoreboard reflects wallet-confirmed payout.

Primary assertions:
- history row remains `Succeeded` only with wallet-confirmed payment pointer,
- payout sats are non-zero and tied to the expected payment pointer,
- reconciliation earned delta matches payout amount,
- scoreboard jobs/sats update from authoritative sources.

## Existing Supporting Tests Used In This Pass

- `app_state::tests::job_history_rejects_unconfirmed_success_settlement_from_active_job`
- `state::earnings_gate::tests::accepts_wallet_backed_earnings_evidence`
- `state::wallet_reconciliation::tests::reconciliation_distinguishes_earn_vs_swap_and_fee`

These provide additional coverage for payout hard-gates and reconciliation semantics.

## Commands Executed

```bash
cargo test -p nostr-client --test dvm_submit_await_e2e
cargo test -p autopilot-desktop --bin autopilot-desktop mission_control_earn_loop_wallet_confirmed_end_to_end
cargo test -p autopilot-desktop --bin autopilot-desktop app_state::tests::job_history_rejects_unconfirmed_success_settlement_from_active_job
cargo test -p autopilot-desktop --bin autopilot-desktop state::earnings_gate::tests::accepts_wallet_backed_earnings_evidence
cargo test -p autopilot-desktop --bin autopilot-desktop state::wallet_reconciliation::tests::reconciliation_distinguishes_earn_vs_swap_and_fee
```

## Latest Run Result

All commands above passed on 2026-03-04.

## Coverage Boundary (What This Does Not Prove)

- Does not prove settlement against real external Lightning infrastructure.
- Does not prove production relay behavior across hostile/public relays.
- Does not prove full GUI pixel/layout behavior; this harness is state + protocol focused.

## Next Practical Extension

- Add a longer-running stress harness that loops the websocket relay round-trip at fixed cadence (3s target) and emits latency percentile snapshots.
