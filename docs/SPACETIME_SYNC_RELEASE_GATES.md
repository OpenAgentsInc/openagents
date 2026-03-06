# Spacetime Sync Release Gates

This document defines required parity/chaos gates for Spacetime sync rollout and release signoff.

## Required Gates

Run:

```bash
scripts/spacetime/parity-chaos-gate.sh
```

The gate is green only when all checks pass:

- `parity_replay_resume`
  - validates replay/resume parity across shared stream consumers.
- `stale_cursor_recovery`
  - validates stale-cursor detection + deterministic rebootstrap behavior.
- `duplicate_delivery`
  - validates duplicate delivery/idempotent apply handling.
- `reconnect_backoff_churn`
  - validates reconnect storm behavior and bounded backoff progression.

CI entrypoint:

```bash
scripts/spacetime/parity-chaos-ci.sh
```

## Evidence Artifacts

Each run must produce artifacts under:

- `output/spacetime/parity-chaos/<timestamp>/`

Required evidence files:

- `SUMMARY.md` (human-readable pass/fail summary)
- `summary.json` (machine-readable gate status)
- `<gate_id>.log` for each gate

## Release Checklist

Before promoting Spacetime sync changes:

- [ ] Spacetime module contract check passed (`scripts/spacetime/verify-autopilot-sync-contract.sh`)
- [ ] Parity/chaos gate suite passed (`scripts/spacetime/parity-chaos-gate.sh`)
- [ ] Gate artifacts attached to release evidence
- [ ] Any gate failure has root-cause notes + remediation commit references

## Failure Diagnostics Policy

- Failures must report gate id + path to the failing log file.
- Gate logs must include raw `cargo test` output (`--nocapture`) for immediate triage.
- Re-run commands must be copy/paste-able from this document and script output.
