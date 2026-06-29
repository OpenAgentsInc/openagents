# Investor Demo Bundle Export

Status: implemented for issue #368 / `OPENAGENTS-LATE-008`.

## Purpose

The investor demo bundle export is a read-only projection that packages the
public/investor-safe pieces of an Omni workstream into a reviewable bundle.

It does not create a downloadable file, publish an investor room, mutate proof
records, mutate settlement, spend a wallet, or upgrade a public claim. It only
summarizes existing projections and emits missing-evidence items where the
proof is incomplete.

Implementation:

- `workers/api/src/omni-investor-demo-bundle-export.ts`
- `workers/api/src/omni-investor-demo-bundle-export.test.ts`

## Inputs

The bundle is assembled from already-redacted projection surfaces:

- public proof bundle summaries;
- route scorecard summaries;
- investor outcome economics metrics;
- Pylon capacity funnel accounting; and
- accepted outcomes per power productivity metrics.

The export adds a bundle title, source refs, caveat refs, target audience, and
generated time. Raw source data remains outside the bundle.

## Readiness

Each export receives a readiness label:

- `ready`: enough public-safe evidence exists for the current bundle;
- `needs_evidence`: the bundle can be shown internally but has missing
  evidence; or
- `blocked`: a proof bundle or economics projection contains an explicit
  blocker.

The bundle also emits section states for:

- proof bundles;
- route scorecards;
- outcome economics;
- capacity funnel; and
- power productivity.

Sections can be `complete`, `partial`, `missing`, or `blocked`.

## Missing Evidence

Missing evidence is explicit. The bundle does not overstate proof when a
projection is modeled, stale, unpaid, unsettled, or not ready.

Current missing-evidence kinds are:

- accepted revenue;
- capacity funnel rows;
- fresh capacity rows;
- measured power evidence;
- power-linked settlement receipt;
- ready public proof bundle;
- provider settlement receipt;
- successful route scorecard; and
- visible capacity settlement receipt.

Each item includes a public-safe reason ref and a required-for ref so the next
operator, agent, or workroom can understand what must be fixed before sharing.

## Settlement And Claim Boundaries

The export keeps settlement labels separate:

- economics revenue state;
- provider settlement state;
- refund state;
- power settlement state; and
- visible capacity settlement receipt counts.

The presence of a settlement label or ref never grants settlement mutation
authority. The bundle authority requires:

- no download-route mutation;
- no investor-share mutation;
- no live wallet spend;
- no public claim upgrade;
- no raw data copy; and
- no settlement mutation.

## Redaction

The export rejects or redacts:

- private customer, provider, wallet, payment, payout, invoice, trading, and
  secret-shaped material;
- raw export, raw market, raw energy, raw meter, raw telemetry, raw runner, raw
  prompt, raw source archive, and raw webhook refs;
- private repository refs;
- raw timestamps; and
- audience-inappropriate private refs.

Public and investor projections are safe to review because private refs are
removed before projection, and a final serialized bundle safety check rejects
anything that survived accidentally.

## Review Rule

The bundle should be reviewed before investor or public sharing when:

- readiness is not `ready`;
- the bundle still carries a no-settlement-implication proof caveat; or
- a human operator has not inspected the source changes, receipts, and caveats.

This keeps the demo useful while preserving the distinction between a strong
proof bundle and a final legal/financial disclosure.

## Tests

Coverage includes:

- ready bundle assembly;
- missing evidence instead of overclaiming;
- public/investor redaction;
- blocker, claim-state, and settlement gap visibility; and
- unsafe source, route, proof, payment, wallet, raw data, secret, timestamp, and
  title rejection.
