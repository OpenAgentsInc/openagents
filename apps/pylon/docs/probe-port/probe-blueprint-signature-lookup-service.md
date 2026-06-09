# Probe Blueprint Signature Lookup Service

Date: 2026-06-07

Status: implemented for Probe issue #174.

Probe now has a typed `BlueprintSignatureLookupService` layered on the registry
source client. It takes a normalized registry view plus preflight facts such as
backend kind, runner capability refs, risk ceiling, allowed surfaces, exact
Program Signature refs, exact Program Type refs, context pack ref, and maximum
tool count. It returns the selected Program Type ids, Program Signature ids,
Module Version ids, release gate refs, evidence refs, receipt refs, candidate
tool scopes, registry version, policy ref, and direct-effect posture.

The selector deliberately does not read user prompt text. Exact typed refs win
when present and valid. Otherwise, the service filters over typed registry
fields: Program family, risk ceiling, allowed surfaces, status, release gate
state, backend kind, runner capability refs, and max tool count. This preserves
the workspace rule against ad hoc keyword routing while still giving Apple FM
and future backends a preflight result they can use before creating a model
session.

The first implementation treats `draft` and `active` Program entries as
selectable because the local Probe mirror is still a seed registry. It refuses
unsafe projections, direct-mutation entries, blocked or rejected release gates,
missing context packs for context-capable signatures, unavailable backend
capabilities, risk-ceiling violations, and signatures with no tool scope on an
allowed surface.

The result always sets `actionSubmissionRequiredForDirectEffects: true`.
`directMutationAllowed` only becomes true if every selected entry and Program
Type explicitly allows it; the current safe registry validation rejects direct
mutation, so current Probe selections remain evidence-first. Later issues can
feed this result into the backend-independent tool menu planner and then into
Apple FM's upfront tool projection.
