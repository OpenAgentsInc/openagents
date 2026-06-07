# Pylon v0.2 local MDK proof smoke

Date: 2026-06-07

## Result

The local `cs336-a1-hosted-starter` proof lane completed after the Pylon
MoneyDevKit wallet wrapper and closeout fixes.

Command:

```bash
OA_BIN="$PWD/target/debug/oa" \
  OA_PROOF_ARTIFACTS="$(mktemp -d /tmp/openagents-proof-smoke.XXXXXX)" \
  OA_PROOF_TIMEOUT=420 \
  scripts/nexus/ldk-accepted-work-proof-smoke.sh
```

Receipt:

```text
Status:    completed
Lane:      cs336-a1-hosted-starter
Detail:    window window.cs336.a1.starter.20260607024506.30871c23.0001 reconciled with 1 accepted contribution(s), closeout=rewarded, workers_healthy=2, validators_healthy=1
```

Public-safe facts from the receipt:

- Namespace: `proof.cs336-a1-hosted-starter.1780800271535`
- Training run: `run.cs336.a1.starter.20260607024506.30871c23`
- Reconciled window: `window.cs336.a1.starter.20260607024506.30871c23.0001`
- Accepted contribution count: `1`
- Latest closeout status: `rewarded`
- Workers healthy: `2`
- Validators healthy: `1`

During the run, the local authority stats showed one accepted closeout, one
payout-eligible closeout, and a simulated accepted-work payout of `120`
bitcoin sats.

## Fixes proven

- The proof smoke wrapper now calls the current `oa proof run <lane>
  --timeout-seconds <n> --json` CLI form.
- The smoke wrapper now separates JSON stdout from build/runtime logs, so Rust
  warnings on stderr no longer corrupt the receipt.
- Local proof authorities now set
  `NEXUS_CONTROL_CS336_HOMEWORK_LEASE_AUTO_LAUNCH_ENABLED=true`, matching the
  hosted-starter lane being tested.
- Pylon training intake now uses the provider-admin host telemetry context
  instead of deriving host capability from the mutable run directory.
- Successful stopped workers no longer mark a lease `released` or `drained`
  at `window_sealed`; they stay active until `reconcile_observed`, `accepted`,
  `paid`, or `terminal_failed`.

## Remaining production caveat

This was a local proof authority with simulated/regtest treasury behavior. It
does not prove a production real-bitcoin payout through the live Nexus
treasury. Production release readiness still requires a live run that pays a
real provider wallet and clears the current production treasury continuity
alert.
