# Data Trace Marketplace Gate

Date: 2026-06-08

Issue: [#563](https://github.com/OpenAgentsInc/openagents/issues/563)

## Launch Decision

Pylon data revenue is blocked until OpenAgents product surface can project one public-safe settled
data sale smoke. A submitted local trace, a redaction receipt, a valuation, a
purchase, and an entitlement are all separate states. None of them may be
collapsed into payout or settlement.

The launch predicate is:

1. Public-safe trace submission refs exist.
2. Public-safe redaction receipt refs exist.
3. A typed semantic selector or structured query planner ref exists.
4. Public-safe valuation refs exist.
5. Public-safe purchase receipt refs exist.
6. Public-safe buyer entitlement refs exist.
7. Public-safe payout contract refs exist.
8. Public-safe settlement receipt refs exist.

Only after all eight predicates pass may public copy describe Pylon data revenue
as live.

## State Model

The gate projects the data trace marketplace into these states:

- `blocked`: missing trace evidence or keyword-routing attempt.
- `submitted`: trace submission exists, but redaction and downstream market
  evidence are incomplete.
- `redacted`: redaction exists, but planner and valuation evidence are
  incomplete.
- `valued`: valuation exists, but purchase evidence is missing.
- `purchased`: purchase exists, but entitlement evidence is missing.
- `entitled`: buyer entitlement exists, but payout contract evidence is
  missing.
- `payable`: payout contract exists, but settlement receipt evidence is
  missing.
- `settled`: the full public-safe sale smoke has settlement receipt evidence.

`settled` is the only state that permits `dataRevenueCopyAllowed`.

## Guards

Public data marketplace refs must reject:

- raw traces and trace payloads
- raw prompts and full prompt material
- private repository or private source content
- provider payloads, grants, accounts, credentials, secrets, and tokens
- customer data and customer email material
- wallet material, payment material, payout targets, invoices, and preimages
- raw timestamps

The gate also blocks `keyword_route` planner mode. Routing must use
`typed_semantic_selector` or `structured_query_planner`, matching the workspace
semantic routing invariant.

## Caveats

The public projection always carries these caveats:

- Trace material requires redaction before any public marketplace projection.
- Valuation is not payout.
- Purchase is not settlement.
- A semantic planner or structured query planner is required.

## Coverage

Regression coverage lives in
`workers/api/src/data-trace-marketplace-gate.test.ts`.

The tests cover:

- submitted trace evidence blocked before redaction and planner refs
- keyword routing blocked even with other refs present
- valuation separated from payout
- purchase and entitlement separated from settlement
- settled public-safe sale smoke enabling data revenue copy
- rejection fixtures for raw traces, prompts, private repos, provider payloads,
  customer data, wallet material, and timestamps
- settled projection scan for private material

## Current Gap

OpenAgents product surface now has the launch gate and regression tests. It still does not have a
live data marketplace that stores redacted local traces, prices them, sells
them, entitles buyers, pays contributors, and settles Bitcoin. Until the
settled public-safe sale smoke exists with receipt refs, Pylon data revenue
remains blocked.
