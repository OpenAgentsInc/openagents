# Blueprint Signature Contribution Draft v1

Date: 2026-06-06

Status: implemented for issue #278.

## Purpose

Signature Contribution Draft models future marketplace or community proposals
for Program Signatures and Module Versions without granting runtime authority.

The model is generic enough for continuation, review, routing, and context
Program Signatures, while remaining scoped to the current Blueprint promotion
path.

## Record Shape

`BlueprintSignatureContributionDraft` records:

- contributor refs;
- source refs;
- capability summary ref;
- intended Program family;
- risk class;
- proposed Program Type, Program Signature, and Module Version refs;
- required fixture refs;
- release gate refs;
- review status;
- rejection ref;
- promotion ref;
- explicit no-runtime-authority block.

## Lifecycle

Supported statuses:

- `draft`;
- `submitted`;
- `in_review`;
- `needs_changes`;
- `rejected`;
- `approved_for_release_gate`;
- `promoted`;
- `archived`.

Supported review states:

- `not_requested`;
- `pending`;
- `changes_requested`;
- `approved`;
- `rejected`.

## Authority Boundary

Contribution drafts cannot:

- execute;
- mutate state;
- deploy;
- spend;
- send email;
- change public claims.

`BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY` records these denied effects.
`blueprintSignatureContributionDraftHasRuntimeAuthority` and
`blueprintSignatureContributionDraftBlockerRefs` make authority violations
explicit.

## Promotion Path

`blueprintSignatureContributionDraftCanEnterReleaseGate` returns true only
when:

- the draft has no runtime authority;
- status is `approved_for_release_gate`;
- review status is `approved`;
- no rejection or promotion ref is already present;
- fixture refs are present;
- release gate refs are present;
- a proposed Program Signature or Module Version ref is present.

Even then, the draft still cannot run. It can only enter the release-gate path
modeled by `evaluateBlueprintContinuationReleaseGate` and later operator
promotion APIs.

## Projection

`projectBlueprintSignatureContributionDraft` drops raw private refs, raw logs,
raw email material, credentials, provider payloads, tokens, and raw timestamps.

This record should support future marketplace discovery and attribution without
turning "proposal submitted" into "production authority granted."
