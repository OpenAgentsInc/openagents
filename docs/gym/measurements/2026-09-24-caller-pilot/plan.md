# Caller pilot: frozen evaluation plan

Frozen before any door was asked. Everything the run measures is fixed
here; the report names this file as its declared selection.

## The caller

**acme-returns**, a fictional caller synthesized for this pilot. The
issue's first acceptance item asks for a willing caller with permitted
caller-owned data; this pilot stands a synthetic caller in that role to
exercise the whole measured-record flow — intake, suite, evaluation,
report, commitment, verification — on workload-shaped data. It
validates that the offering runs end to end and that its record is
checkable. It does **not** validate customer demand; no real caller
approved these labels, and the record says so.

## The application decision

Route an inbound returns-desk record to one of four dispositions:

- `refund` — money back; a billing error or an open-and-return policy.
- `replacement` — a new unit ships; the item failed under warranty
  terms but the claim is routine.
- `warranty_claim` — the manufacturer's claim process owns it.
- `policy_only` — no action; the customer is asking what the policy is.

## Error consequences

- `warranty_claim` routed as `refund` pays out money the manufacturer
  owed — the expensive direction.
- `policy_only` routed as an action queues work that does not exist.
- `refund`/`replacement` swaps cost one re-touch, the cheap direction.

## The data

40 synthesized records across two families — `consumer-returns` and
`pro-returns`, 20 each — generated deterministically by
`gen_records.py` in this directory. Label source `acme`, label rule
"the returns desk's disposition at close", licence "measurement use
only". No private examples are published; the records were authored
for this file and contain no real customer data.

`gym build` partitions 40/40/20 per family: 8 calibration, 8
development, 4 locked. The locked partition is never read by this
pilot.

## Doors

- `kev-0.6b` — the local Kev adapter served by `kev-serve`, unmetered.
- `constant` — `constant_door.py` in this directory: a deterministic
  door that answers every choice with the first option. The baseline a
  real model must beat to be worth anything.

## Comparison criteria

Declared before the run:

1. `kev-0.6b` accuracy on the development partition must beat
   `constant`'s, or the model reference adds nothing.
2. Per-family accuracy is reported; a family where the constant wins
   names where the model is not load-bearing.
3. Coverage must be **complete**: every expected item of every
   expected door recorded exactly once.
4. Calibration limits are reported, not judged — 16 development items
   per door cannot support a calibration claim.
5. Costs: both lanes are local and unmetered; the report states that
   rather than inventing a number.
