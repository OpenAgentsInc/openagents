# NIP-SA / NIP-SKL / NIP-AC Test Matrix and Recovery Runbook

## 1. Purpose

This runbook defines:

- the required automated coverage for SA/SKL/AC MVP rollout,
- deterministic failure expectations,
- release-gate criteria tied to concrete test files.

It is the execution companion to `docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md` (Phases 6 and 9).

## 2. Automated Test Matrix

### 2.1 Protocol unit suites (crate-local)

- SA core:
  - `crates/nostr/core/tests/nip_sa_exports.rs`
  - `crates/nostr/core/tests/nip_sa_e2e.rs`
- SKL core:
  - `crates/nostr/core/src/nip_skl/manifest.rs` (module tests)
  - `crates/nostr/core/src/nip_skl/discovery.rs` (module tests)
  - `crates/nostr/core/src/nip_skl/trust.rs` (module tests)
  - `crates/nostr/core/src/nip_skl/revocation.rs` (module tests)
  - `crates/nostr/core/src/nip_skl/yaml_derivation.rs` (module tests)
- AC core:
  - `crates/nostr/core/src/nip_ac/intent.rs` (module tests)
  - `crates/nostr/core/src/nip_ac/offer.rs` (module tests)
  - `crates/nostr/core/src/nip_ac/envelope.rs` (module tests)
  - `crates/nostr/core/src/nip_ac/spend.rs` (module tests)
  - `crates/nostr/core/src/nip_ac/settlement.rs` (module tests)
  - `crates/nostr/core/src/nip_ac/default_notice.rs` (module tests)
  - `crates/nostr/core/src/nip_ac/scope_hash.rs` (module tests)
  - `crates/nostr/core/src/nip_ac/reputation.rs` (module tests)

### 2.2 Integration suites

- Backroom-ported and adapted:
  - `crates/nostr/core/tests/nip44_integration.rs`
  - `crates/nostr/core/tests/nip90_integration.rs`
- SA/SKL/AC matrix + hardening:
  - `crates/nostr/core/tests/nip_sa_skl_ac_integration_matrix.rs`
  - `crates/nostr/core/tests/nip_ac_skill_linkage.rs`

### 2.3 Desktop flow checks

- `apps/autopilot-desktop/src/app_state.rs` test module:
  - sync stale-cursor rebootstrap
  - relay-loss resubscribe behavior
  - wallet-error scoreboard degradation
- `apps/autopilot-desktop/src/input.rs` test module:
  - command/submit parity and payment/invoice validation paths
- `apps/autopilot-desktop/src/pane_system.rs` test module:
  - pane action hit-target ordering for SA/SKL/AC panes

## 3. Failure-Mode Expectations

### 3.1 Relay loss

Expected behavior:

- relay health transitions to disconnected/error,
- sync subscription state reports `resubscribing`,
- no duplicate receipts or cursor rewind side effects.

Checks:

- `sync_health_marks_resubscribing_when_relays_are_lost` (desktop state test)

### 3.2 Stale cursor

Expected behavior:

- sync enters `reconnecting` once stale threshold exceeded,
- `rebootstrap` transitions to `replaying` and resets stale-age counter.

Checks:

- `sync_health_detects_stale_cursor_and_rebootstrap` (desktop state test)

### 3.3 Wallet errors

Expected behavior:

- earnings surface enters error state,
- failure is explicit and not masked as successful payout.

Checks:

- `earnings_scoreboard_surfaces_wallet_errors` (desktop state test)

### 3.4 Trust/revocation failures

Expected behavior:

- trust gate denies untrusted or revoked manifests,
- denial reason remains explicit and deterministic.

Checks:

- `test_trust_gate_denies_revoked_manifest` (SA/SKL/AC matrix integration)

### 3.5 Envelope/spend failures and replay

Expected behavior:

- stale envelope events are ignored,
- repeated envelope event application is idempotent,
- invalid state transitions fail,
- overspend/expired spends fail deterministically.

Checks:

- `test_envelope_replay_is_idempotent_and_stale_events_are_ignored`
- `test_scope_and_spend_failure_modes_are_deterministic`

## 4. Release Gate

Release is blocked unless all are true:

1. `cargo test -p nostr --tests` passes.
2. SA/SKL/AC integration matrix tests pass:
   - `nip_sa_skl_ac_integration_matrix`
   - `nip_ac_skill_linkage`
3. NIP-44 and NIP-90 integration suites pass.
4. `cargo check -p autopilot-desktop --tests` passes.
5. Desktop failure-mode tests listed above pass.
6. No test relies on non-deterministic timing/order assertions without explicit guards.

## 5. Incident Triage Order

1. Verify trust gate and revocation state (SKL) before accepting new fulfillment.
2. Verify envelope head and spend authorization validity (AC).
3. Verify NIP-90 request/result correlation and settlement linkage.
4. Verify desktop sync lane state (`subscription_state`, stale cursor age, replay count).
5. Verify wallet lane health before marking payout outcomes as complete.
