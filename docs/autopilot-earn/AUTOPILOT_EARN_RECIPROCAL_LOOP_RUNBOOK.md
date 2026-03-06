# Autopilot Earn Reciprocal Loop Runbook

Date: 2026-03-04  
Owner: `apps/autopilot-desktop` (compute-lane MVP)

## Goal

Run and verify a bilateral 10-sat reciprocal loop between two operator identities:

1. identity `A` dispatches paid NIP-90 jobs to identity `B`
2. identity `B` dispatches paid NIP-90 jobs to identity `A`
3. both wallets show authoritative receive evidence
4. loop controls (`Start`/`Stop`/`Reset`) behave deterministically

This runbook is for local and staging verification of compute-lane earn behavior.

## Preconditions

- Two independent Spark wallets with spendable sats.
- Two independent Nostr identities.
- One relay reachable by both instances.
- OpenAgents Desktop build/binary available.

## Environment Setup (Two Keys, Two Wallets)

Use isolated home directories so identity, settings, and credentials do not collide.

```bash
mkdir -p "$HOME/tmp/openagents-loop/a"
mkdir -p "$HOME/tmp/openagents-loop/b"
```

### Step 1: Boot instance A once and record pubkey

```bash
HOME="$HOME/tmp/openagents-loop/a" \
OPENAGENTS_SPARK_NETWORK=mainnet \
OPENAGENTS_SPARK_API_KEY="<SPARK_API_KEY_A>" \
OPENAGENTS_RECIPROCAL_LOOP_AUTOSTART=0 \
target/debug/autopilot-desktop
```

In the app:

1. Open `Identity Keys` pane.
2. Copy `public_key_hex` for A (`PUBKEY_A`).
3. Open `Spark Wallet` pane and verify `connected`.
4. Open `Relay Connections` and add the shared relay URL.

Close instance A.

### Step 2: Boot instance B once and record pubkey

```bash
HOME="$HOME/tmp/openagents-loop/b" \
OPENAGENTS_SPARK_NETWORK=mainnet \
OPENAGENTS_SPARK_API_KEY="<SPARK_API_KEY_B>" \
OPENAGENTS_RECIPROCAL_LOOP_AUTOSTART=0 \
target/debug/autopilot-desktop
```

In the app:

1. Open `Identity Keys` pane.
2. Copy `public_key_hex` for B (`PUBKEY_B`).
3. Open `Spark Wallet` pane and verify `connected`.
4. Open `Relay Connections` and add the same shared relay URL.

Close instance B.

### Step 3: Relaunch both instances with peer wiring

Instance A:

```bash
HOME="$HOME/tmp/openagents-loop/a" \
OPENAGENTS_SPARK_NETWORK=mainnet \
OPENAGENTS_SPARK_API_KEY="<SPARK_API_KEY_A>" \
OPENAGENTS_RECIPROCAL_LOOP_PEER_PUBKEY="<PUBKEY_B>" \
OPENAGENTS_RECIPROCAL_LOOP_AUTOSTART=0 \
target/debug/autopilot-desktop
```

Instance B:

```bash
HOME="$HOME/tmp/openagents-loop/b" \
OPENAGENTS_SPARK_NETWORK=mainnet \
OPENAGENTS_SPARK_API_KEY="<SPARK_API_KEY_B>" \
OPENAGENTS_RECIPROCAL_LOOP_PEER_PUBKEY="<PUBKEY_A>" \
OPENAGENTS_RECIPROCAL_LOOP_AUTOSTART=0 \
target/debug/autopilot-desktop
```

## Pane Procedure

Perform on both instances unless noted.

1. Open `Reciprocal Loop` pane (`pane.reciprocal_loop`).
2. Confirm:
   - `Local pubkey` and `Peer pubkey` are populated and different.
   - relay/wallet health is not degraded.
3. Click `Start` on both instances.
4. Wait 30-90 seconds; verify metrics move in both directions.

Expected reciprocal-loop pane signals:

- `A->B paid` increments on the sender side.
- `B->A paid` increments on the receiver side.
- `Sats sent/received` increases in 10-sat increments.
- `Failure class` remains `none / none` during healthy operation.
- `Retry attempts` stays low (0 in steady state).

## Stop/Reset Verification

1. Click `Stop` on instance A.
2. Verify:
   - `Kill switch` shows `engaged`.
   - no new `A->B dispatched` increments after stop.
3. Click `Start` on instance A; verify loop resumes.
4. Click `Reset` on either instance when needed; verify counters clear.

## Pass/Fail Checklist (10-sat Bilateral Loop)

Pass when all are true:

- Both instances show non-empty local/peer pubkeys.
- Both wallets are connected and relay health is not offline.
- At least 2 successful `A->B` paid cycles and 2 successful `B->A` paid cycles occur.
- Both sides show non-zero `sats sent` and `sats received`.
- Payment pointers in activity/history are wallet-authoritative (`wallet:*`), not synthetic.
- `Stop` on one side prevents post-stop dispatch until explicit `Start`.

Fail when any are true:

- one direction never reaches `paid`,
- payment pointers are missing/synthetic,
- relay stays disconnected/degraded,
- wallet stays disconnected/error,
- post-stop dispatch still occurs.

## Troubleshooting

### Relay Failures

Symptoms:

- reciprocal loop `Failure class` shows dispatch/recoverable repeatedly
- relay health shows `offline` or `degraded`
- no new NIP-90 rows in activity feed

Actions:

1. Open `Relay Connections`; verify same relay URL on both instances.
2. Remove/re-add relay, then retry connection.
3. Confirm TLS-capable build and network reachability.
4. Check `Activity Feed` (filter `nip90`) for inbound/outbound event movement.

### Wallet Failures

Symptoms:

- wallet health `degraded`
- payment failures in network/job lifecycle
- no `wallet:*` pointer progression

Actions:

1. Open `Spark Wallet` and run `Refresh`.
2. Verify `OPENAGENTS_SPARK_API_KEY` and `OPENAGENTS_SPARK_NETWORK`.
3. Ensure wallet has spendable balance for outbound 10-sat payments.
4. If needed, restart with corrected Spark credentials.

### Payment Correlation Failures

Symptoms:

- request/job appears complete but not `Paid`
- history row exists without matching wallet receive evidence
- earnings counters do not increment

Actions:

1. Check `Job History` row `payment_pointer`.
2. Check `Spark Wallet` recent payments for matching pointer.
3. Check `Activity Feed` `nip90` details for matching request/result ids.
4. Treat mismatches as payout-integrity incident; stop loop and capture logs/state.

## Deterministic Programmatic Harness (CI/Local)

Use the bilateral harness test for deterministic non-UI verification:

```bash
cargo test -p autopilot-desktop --bin autopilot-desktop \
  app_state::tests::reciprocal_loop_two_identity_relay_harness_runs_bidirectional_paid_cycles
```

Earn regression gate includes this test:

```bash
./scripts/lint/autopilot-earnings-epic-test-gate.sh
```
