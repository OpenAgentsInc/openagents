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

That pane lets a contributor:

- connect an identity
- accept the governed external-beta contract
- see environment class and declared capabilities
- run the bounded benchmark pack
- submit a governed runtime disagreement receipt
- choose and run one bounded worker role
- inspect acceptance, quarantine, rejection, and review outcomes
- inspect pending contributor credit state and account linkage posture

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

- pending credit sats from accepted or review-routed contributions
- contributor credit-account identifier once identity is connected
- payment linkage posture

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
