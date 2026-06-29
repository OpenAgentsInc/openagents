# Workroom Kind Templates v1

Issue #217 adds the first Omni workroom kind template model.

The template layer is static product policy. It does not create workrooms by
itself. Instead, it gives Autopilot, Adjutant, future Blueprint Program
Signatures, and operator surfaces a typed way to decide what evidence,
artifacts, reviews, public projection, privacy constraints, and closeout steps a
workroom kind requires.

## Covered Kinds

The initial template set covers:

- Sites
- Coding
- CRM
- Investor ops
- Project ops
- Support
- Finance ops
- Meeting
- Document
- Legal review

Each template maps onto the existing accepted-outcome work kind model. For
example, Sites maps to `site`, coding maps to `coding`, legal review maps to
`legal_sensitive`, and business operations map to `business`.

## Policy Fields

Each template defines:

- Required evidence entry kinds.
- Required accepted-outcome artifact kinds.
- Review policy.
- Proof policy.
- Allowed public projection policy.
- Closeout requirements.
- Privacy constraint.

This makes it possible to share the same workroom lifecycle while keeping
different acceptance requirements for a public Site launch, a private PR, a CRM
operation, a finance operation, or a legal review.

## Guardrails

The validation helpers reject:

- Legal-private templates that allow public projection.
- Public-safe proof templates that require private-only evidence.
- Workroom evidence sets that are missing required evidence for the selected
  kind.

This keeps public proof, customer-safe summaries, and private workroom material
separate while later routing and Blueprint systems become more automatic.

## Next Steps

The next implementation layers should consume these templates when:

- Creating accepted-outcome contracts from an order or Blueprint Program
  Signature.
- Checking closeout readiness.
- Rendering customer, team, operator, and public projections.
- Deciding which proof bundle shape is legal to export.
