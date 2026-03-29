# Compiled-Agent Contributor Beta

This document describes the first external contributor beta for the narrow
compiled-agent family owned by `openagents` and trained through the governed
`psionic` loop.

## Scope

The beta is intentionally narrow.

It only covers the admitted compiled-agent family already used by the retained
route and grounded-answer learning loop. It does not widen into general
autonomy, broad marketplace participation, or outside promotion authority.

Current outside-facing contribution types are:

- benchmark-pack runs for the retained compiled-agent family
- governed runtime disagreement receipts from the same family
- bounded worker output for:
  - replay generation
  - ranking and labeling
  - validator scoring
  - bounded module training

## Product Surface

The first product surface is the `Contributor Beta` pane in
`apps/autopilot-desktop`.

Phase 5 makes that pane Tailnet-first.

The first real outside-compatible beta is still our own Tailnet, with this M5
device acting as the governed coordinator and the `archlinux` NVIDIA machine
acting as the first bounded worker node. The contributor surface now treats
that dual-node run as the first external proof instead of pretending a broad
public market already exists.

That pane lets a contributor:

- connect an identity
- accept the governed external-beta contract
- see environment class and declared capabilities
- see the Tailnet pilot roster, governed-run digest, and retained XTRAIN digest
- run the bounded benchmark pack in the Tailnet-first beta posture
- submit a governed runtime disagreement receipt
- choose and run one bounded worker role
- inspect acceptance, quarantine, rejection, and review outcomes
- inspect review-queue depth and the current manual review SLA
- inspect confirmed vs provisional contributor credit state and account linkage posture

The pane is intentionally opinionated. It is not a generic upload screen.

## Governance Rules

The contributor beta preserves the same boundary as the internal bounded
learning loop:

- evidence is not authority
- outside contributors do not get promotion authority
- outside contributors do not get live runtime authority
- raw contributed logs do not flow directly into training
- accepted, quarantined, rejected, and review-routed outcomes remain explicit

Each submission is tracked with:

- contributor identity
- contract version
- environment class
- capability summary
- admitted family
- digest
- outcome
- optional runtime receipt lineage
- optional authority path and confidence band
- optional worker role and review reason

## Trust And Accounting

The pane keeps trust and accounting narrow and operational.

- `pending` trust means the contributor has not yet built accepted lineage
- `governed` trust means the contributor has accepted bounded lineage in the
  current beta family
- `caution` trust means rejected or quarantined rows exist and the contributor
  should not be treated as a routine source without review

The pane also keeps a simple contributor credit surface:

- confirmed credit sats from accepted contributions
- provisional credit sats from accepted or review-routed contributions
- contributor credit-account identifier once identity is connected
- payment linkage posture
- a narrow rulebook:
  - accepted submissions earn credit
  - review-routed submissions remain provisional
  - rejected and quarantined submissions earn zero

This is intentionally not a marketplace or token system. It is the minimum
credit and accounting shape needed for the bounded beta.

## Runtime Receipt Flow

Runtime disagreement receipts are captured from the same compiled-agent slice
used by the product.

The current pane flow records:

- the source receipt id
- authority-path posture
- confidence-band posture
- the contributor correction note
- the retained failure class for review

That keeps the outside runtime evidence in the same contract family as the
internal replay and validator loop.

## Headless Observability

The contributor beta is visible through the existing pane-control contract:

```bash
autopilotctl pane open contributor_beta
autopilotctl pane status contributor_beta --json
```

The pane-status snapshot surfaces the same product truth the UI uses:

- identity and contract posture
- worker-role posture
- trust and accounting posture
- Tailnet roster for the M5 coordinator and NVIDIA worker pilot
- the retained Tailnet governed-run digest and latest XTRAIN digest
- review queue depth and SLA posture
- recent submission outcomes
- latest runtime receipt lineage

## Non-Goals

This beta explicitly does not do the following:

- open arbitrary training families
- grant outside promotion rights
- accept raw logs as training truth
- collapse learned-lane and stronger-evidence lanes together
- require Tassadar to participate

The goal is simpler: make bounded decentralized contribution real without
weakening validator discipline, rollback discipline, or evidence lineage.
