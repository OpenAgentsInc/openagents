# Workroom Surface Projections v1

Issue #220 adds the first aggregate Omni workroom surface projection model.

The projection module composes private workroom state into safe surface-specific
views for:

- Public proof pages
- Customer order/status pages
- Team dashboards
- Agent APIs
- Operator HUDs

## Inputs

The aggregate projection can compose:

- Workroom record
- Evidence bundles
- Lifecycle decisions
- Accepted outcome economics
- Route scorecards

It builds on each record's existing public/customer/operator projection helper
instead of creating route-specific redaction code.

## Classification Gate

The workroom data classification and trust tier gate runs before any aggregate
projection is emitted. If the workroom classification cannot project to the
requested surface, projection fails.

Examples:

- Customer-classified workrooms cannot project to public proof.
- Payment-private workrooms cannot project to customer pages.
- Team-classified workrooms can project to team dashboards.
- Operator/private internals stay out of public and customer surfaces.

## Surface Behavior

Public surfaces include only public-safe proof material, public route summary,
public economics caveats, and no private source/task/provider details.

Customer surfaces include customer-safe artifacts, receipts, lifecycle refs, and
route decision refs without operator metadata or provider account material.

Team surfaces include more internal workroom context such as source refs, but
still avoid raw task packets and operator-only metadata.

Agent surfaces are customer-safe and machine-readable. They include enough refs
for agent follow-up without exposing private mechanics.

Operator surfaces include the full operator-safe projection, including private
metadata, route scorecard internals, and economics cost fields.

## Current Scope

This is a typed projection boundary, not a new route. Future HTTP/API handlers
should use this module rather than reimplementing projection logic per route.
