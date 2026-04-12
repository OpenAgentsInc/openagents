# Transcript 222 Rehearsal Gates

Date: 2026-04-12  
Repo: `openagents`

This report retains the first passing Transcript 222 launch-hardening gate set
after the widened participant-threshold rehearsal was implemented in
`nexus-control`.

It is intentionally precise about what was proved:

- the small-cohort Transcript 222 canary now has a canonical release script
- the widened threshold rehearsal now has a canonical release script
- both run against the current `Psionic`, `Pylon`, `Nexus`, treasury, and
  public-stats paths already in this repo
- the widened threshold gate preserves the distinction between progress-only
  counters and settlement counters instead of flattening them

It is also intentionally precise about what this is not:

- not a production `nexus.openagents.com` deploy receipt
- not a public internet fleet rehearsal
- not a replacement for a later documented real-node demo run

## Canonical Commands

Small-cohort canary:

```bash
scripts/release/check-pylon-transcript-222-canary.sh
```

Widened crowd-threshold rehearsal:

```bash
scripts/release/check-pylon-transcript-222-crowd-threshold.sh
```

## Retained Output Bundles

Crowd-threshold gate bundle:

```text
target/pylon-transcript-222-crowd-threshold/20260412T161348Z-5625/
```

Nested canary bundle from that same run:

```text
target/pylon-transcript-222-crowd-threshold/20260412T161348Z-5625/canary/
```

Those directories contain:

- `SUMMARY.md`
- `steps.tsv`
- one log per retained step

## Small-Cohort Canary Result

The canary gate passed with the current sibling `psionic` checkout and covered:

- strong-node automatic actual-lane manifest build
- weak-device validation replay proof emission
- `Pylon` assignment claim, ack, materialization, and runtime launch
- `Pylon` terminal sync upload and retained publication behavior
- weak-device accepted-work payout dispatch
- strong-lane accepted-work payout dispatch
- `/api/stats`, `/api/training/summary`, and `/api/homepage` projection from
  the same authority truth

The passing canary summary is retained at:

```text
target/pylon-transcript-222-crowd-threshold/20260412T161348Z-5625/canary/SUMMARY.md
```

## Crowd-Threshold Result

The widened rehearsal passed after adding one retained `nexus-control` test:

```text
transcript_222_crowd_threshold_projects_public_truth_and_payouts
```

That test now proves one threshold-scale authority snapshot with:

- `80` online and admitted nodes
- `71` assigned contributors
- `71` accepted contributors
- `60` weak-device assigned contributors
- `60` weak-device accepted contributors
- `11` model-progress contributors
- `71` accepted settlement closeouts
- `71` payout-eligible settlement closeouts
- `60` weak-device-bearing settlement closeouts
- `11` progress-only payout-eligible closeouts
- `1491 sats` total accepted-work payout dispatched
- `1260 sats` weak-device accepted-work payout dispatched
- `231 sats` strong-lane accepted-work payout dispatched

The important semantic split is preserved:

- `training_payout_eligible_closeouts` remains a progress-only public counter
- settlement closeouts in `/api/training/summary` still include accepted
  weak-device work

That is the truthful contract frozen in
`docs/plans/transcript-222-launch-truth-contract.md`, and the widened rehearsal
now proves that the code still follows it over the >70 participant threshold.

## Source Files

The retained implementation for these gates lives in:

- `scripts/release/check-pylon-transcript-222-canary.sh`
- `scripts/release/check-pylon-transcript-222-crowd-threshold.sh`
- `apps/nexus-control/src/lib.rs`

## Residual Risk

These passing gates close the code-level hard blockers for Transcript 222
launch-hardening, but they do not remove the need for:

- a separately documented live-node demo run
- rollout discipline on the public fleet
- public comms that keep progress counters separate from settlement counters
