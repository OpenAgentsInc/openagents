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

### 3) Desktop Relay -> Execute -> Publish -> Wallet Confirm Harness (`autopilot-desktop`)

File: `apps/autopilot-desktop/src/provider_nip90_lane.rs` test `desktop_earn_harness_relay_execute_publish_wallet_confirm_end_to_end`

What it covers:
- spins up a websocket relay mock,
- ingests a live NIP-90 request through desktop provider lane worker,
- executes local active-job lifecycle transitions,
- publishes canonical feedback + result events back to relay and verifies publish outcomes,
- records paid receipt and confirms mission-control scoreboard from wallet-reconciled receive evidence.

Primary assertions:
- relay ingress produces a selectable inbox request,
- feedback/result publish outcomes are relay-accepted and events are observed by relay,
- active job requires result authority before `delivered` and wallet pointer before `paid`,
- earnings scoreboard `today/total/jobs` reflect reconciled wallet payout evidence only.

### 4) Starter-Demand Generator Controls Harness (`autopilot-desktop`)

File: `apps/autopilot-desktop/src/app_state.rs` tests prefixed with `starter_demand_`

What it covers:
- deterministic starter-demand dispatch sequence,
- dispatch interval gating,
- strict budget cap enforcement,
- kill-switch hard stop behavior,
- rollback semantics that reclaim reserved budget after dispatch failure.

Primary assertions:
- generator never allocates sats beyond configured cap,
- dispatch does not fire before the configured cadence,
- kill switch blocks all automatic starter dispatches,
- rollback removes queued quest and restores budget accounting.

### 5) Starter-Demand Provenance Tagging Harness (`autopilot-desktop`)

File: `apps/autopilot-desktop/src/app_state.rs` test `starter_provenance_propagates_from_inbox_to_history_receipt`

What it covers:
- creates a starter-demand request in inbox state,
- verifies demand source is preserved in active-job state after acceptance,
- verifies terminal history receipt retains the same source tag.

Primary assertions:
- starter-demand requests are explicitly tagged `starter-demand`,
- source tag propagation is stable across inbox -> active job -> history receipt,
- receipt provenance cannot silently downgrade into open-network labeling.

### 6) Mission Control Aggregate Counters Harness (`autopilot-desktop`)

File: `apps/autopilot-desktop/src/app_state.rs` tests prefixed with `network_aggregate_counters_`

What it covers:
- refreshes a dedicated aggregate-counters service state from authoritative wallet-reconciled payout rows,
- verifies Mission Control global counters (`providers online`, `jobs completed`, `sats paid`) derive from that aggregate pipeline rather than ad-hoc render-time math,
- verifies degraded/ignore paths for unreconciled receipts and wallet source failures.

Primary assertions:
- `jobs_completed` and `sats_paid` count only wallet-reconciled payout evidence,
- unreconciled receipt rows do not inflate global counters,
- wallet source failures set aggregate counter service state to degraded/error.

### 7) Relay Connectivity Truth Harness (`autopilot-desktop`)

Files:
- `apps/autopilot-desktop/src/provider_nip90_lane.rs` test `worker_ingests_live_relay_request`
- `apps/autopilot-desktop/src/app_state.rs` test `relay_connections_add_retry_remove_flow`

What it covers:
- verifies provider-lane snapshots carry live relay health rows sourced from transport state,
- verifies relay pane retry actions no longer stamp synthetic `connected/96ms` local success,
- verifies relay retry path remains a reconnect attempt (`connecting`) pending lane authority.

Primary assertions:
- relay health rows are present in live lane snapshots while ingress is active,
- retrying a relay keeps status in `connecting` until transport confirms connection state,
- relay remove/add/retry flows remain deterministic and replay-safe.

### 8) Epic Tracker Reconciliation Integrity Check (`scripts/lint`)

File: `scripts/lint/autopilot-earn-doc-reconciliation-check.sh`

What it covers:
- validates that the epic tracker explicitly separates historical stream (`#2814`-`#2876`) from reconciliation stream (`#2877`-`#2890`),
- validates that reconciliation issues (`#2886`-`#2890`) are present with expected state entries,
- validates implementation log wording avoids stale “complete” claims without reconciliation context.

Primary assertions:
- reconciliation sections exist in tracker/log docs,
- tracker reflects latest closed/open reconciliation issue states,
- implementation log includes evidence-ledger + reconciliation-stream declarations.

### 9) Failure Taxonomy + Diagnostics Classifier (`autopilot-desktop`)

File: `apps/autopilot-desktop/src/input/actions.rs` test `provider_failure_taxonomy_classifies_relay_execution_payment_and_reconciliation`

What it covers:
- classifies provider failures into one canonical class with deterministic precedence,
- verifies concise diagnostics prefixes for relay, execution, payment, and reconciliation classes.

Primary assertions:
- relay failures classify as `relay`,
- execution failures classify as `execution`,
- wallet failures classify as `payment`,
- succeeded-vs-reconciled payout mismatch classifies as `reconciliation`.

### 10) Loop Integrity SLO Metrics + Alerts (`autopilot-desktop`)

Files:
- `apps/autopilot-desktop/src/app_state.rs` test `earnings_scoreboard_tracks_loop_integrity_slo_metrics`
- `apps/autopilot-desktop/src/input/actions.rs` test `loop_integrity_alert_specs_flags_expected_slo_breaches`

What it covers:
- tracks first-job latency, completion ratio, payout success ratio, and wallet confirmation latency in Earnings Scoreboard state,
- evaluates SLO alert thresholds for those metrics so degraded loop integrity raises deterministic alerts.

Primary assertions:
- first-job latency tracks pending and completed latency from provider-online session timing,
- completion and payout-success ratios compute from authoritative history + wallet-reconciled payout evidence,
- wallet confirmation latency computes from reconciled payout receive timestamps,
- degraded metric samples activate all expected SLO alerts while healthy samples clear them.

### 11) Simulation Path Isolation (`autopilot-desktop`)

Files:
- `apps/autopilot-desktop/src/pane_registry.rs` test `simulation_panes_respect_runtime_gate`
- `apps/autopilot-desktop/src/render.rs` test `command_registry_hides_simulation_commands_when_disabled`
- `apps/autopilot-desktop/src/input/tool_bridge.rs` test `resolve_pane_kind_gates_simulation_references`

What it covers:
- gates simulation-only panes behind an explicit runtime flag (`OPENAGENTS_ENABLE_SIMULATION_PANES`),
- removes simulation pane commands from default command-palette routes,
- blocks tool-bridge pane resolution for simulation panes when the runtime gate is disabled.

Primary assertions:
- simulation pane kinds are denied when runtime simulation gate is off and allowed when on,
- default command registry excludes simulation pane commands,
- tool-bridge pane resolution rejects simulation pane references unless runtime simulation gate is enabled.

## Existing Supporting Tests Used In This Pass

- `app_state::tests::job_history_rejects_unconfirmed_success_settlement_from_active_job`
- `state::earnings_gate::tests::accepts_wallet_backed_earnings_evidence`
- `state::wallet_reconciliation::tests::reconciliation_distinguishes_earn_vs_swap_and_fee`

These provide additional coverage for payout hard-gates and reconciliation semantics.

## Commands Executed

```bash
cargo test -p nostr-client --test dvm_submit_await_e2e
cargo test -p autopilot-desktop --bin autopilot-desktop mission_control_earn_loop_wallet_confirmed_end_to_end
cargo test -p autopilot-desktop --bin autopilot-desktop provider_nip90_lane::tests::desktop_earn_harness_relay_execute_publish_wallet_confirm_end_to_end
cargo test -p autopilot-desktop --bin autopilot-desktop provider_nip90_lane::tests::worker_ingests_live_relay_request
cargo test -p autopilot-desktop --bin autopilot-desktop app_state::tests::relay_connections_add_retry_remove_flow
./scripts/lint/autopilot-earn-doc-reconciliation-check.sh
cargo test -p autopilot-desktop --bin autopilot-desktop app_state::tests::starter_demand_
cargo test -p autopilot-desktop --bin autopilot-desktop app_state::tests::starter_provenance_propagates_from_inbox_to_history_receipt
cargo test -p autopilot-desktop --bin autopilot-desktop app_state::tests::network_aggregate_counters_
cargo test -p autopilot-desktop --bin autopilot-desktop input::actions::tests::provider_failure_taxonomy_classifies_relay_execution_payment_and_reconciliation
cargo test -p autopilot-desktop --bin autopilot-desktop app_state::tests::earnings_scoreboard_tracks_loop_integrity_slo_metrics
cargo test -p autopilot-desktop --bin autopilot-desktop input::actions::tests::loop_integrity_alert_specs_flags_expected_slo_breaches
cargo test -p autopilot-desktop --bin autopilot-desktop pane_registry::tests::simulation_panes_respect_runtime_gate
cargo test -p autopilot-desktop --bin autopilot-desktop render::tests::command_registry_hides_simulation_commands_when_disabled
cargo test -p autopilot-desktop --bin autopilot-desktop input::tool_bridge::tests::resolve_pane_kind_gates_simulation_references
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
