# @openagentsinc/product-spec

OpenAgents implementation of the [ProductSpec](https://github.com/gokulrajaram/ProductSpec)
open standard (format v0.1) for `.product-spec.md` intent artifacts — an Effect
Schema document model, parser, validator, and CLI. Adoption rationale and
design: `docs/fable/2026-07-08-productspec-adoption-analysis.md`; repo
conventions: `specs/CONVENTIONS.md`; tracking: #8593.

This is our own implementation. The upstream `@productspec/parser` is a
conformance reference only — never a runtime dependency. The vendored fixtures
in `fixtures/conformance/` (MIT, attributed) pin the exact upstream
compatibility target: parser `0.19.0` at commit `9ef2654` (PSEL-0/PSEL-1,
#8757). The `UPSTREAM_COMPATIBILITY` export states exactly which upstream
semantics the package supports. The package also implements the upstream
Decision Trace v0.1 JSON companion with an Effect Schema decoder and stable
diagnostics; the dependency graph resolver and MCP evidence checklist remain
out of scope. The
adopted boundary and migration plan are in
`docs/assurance/PRODUCTSPEC_EVIDENCE_LOOP.md`.

## Profiles

`validateProductSpec(markdown, { profile })` accepts two profiles:

- **`openagents`** (default) — the local profile. Legacy documents keep
  working: prose acceptance criteria are allowed (the executable `CW-AC-*`
  profile sits on top), and success metrics may use the OpenAgents shape
  (snake_case semantic ids plus `segment`/`source`). Every upstream structured
  construct — `applies_to`, `productspec-acceptance-criteria`,
  upstream-dialect evals/metrics, `productspec-related-artifacts` — is still
  validated strictly when a document uses it.
- **`upstream`** — the pinned upstream `0.19.0` semantics: structured
  `AC-<n>` criteria and `SM-<n>` metrics are mandatory,
  `target_status`/`target_owner` rules apply, and the OpenAgents-only metric
  fields are rejected.

There is no silent ID aliasing between profiles: the revision-6 MVP
ProductSpec (frozen with its exact digest as
`fixtures/openagents/legacy-rev6-mvp.product-spec.md`) validates under
`openagents`, and its exact `upstream` incompatibilities are recorded as typed
tests, not folklore. The `CW-AC-*` → `AC-*` migration is PSEL-2.

## Dual digests and evidence-attachment edits

Per `docs/assurance/ASSURANCE_SPEC.md` §4:

- `computeProductSpecDocumentDigest(markdown)` — SHA-256 over the exact
  authored UTF-8 bytes (provenance, race detection).
- `computeProductSpecIntentDigest(markdown | document)` — SHA-256 over the
  versioned canonical intent projection (`productSpecIntentProjection`,
  `INTENT_PROJECTION_VERSION`). The projection excludes only Related Artifact
  attachments that are not `product_spec` dependencies plus the
  `created_at`/`updated_at` provenance timestamps; everything else, including
  `tool_metadata`, is intent-bound by default.
- `planProductSpecEvidenceAttachmentEdit` /
  `applyProductSpecEvidenceAttachmentEdit` — the typed, owner-confirmed
  evidence-attachment-only edit path. It proves the intent projection is
  unchanged (same intent digest, same `spec_revision`, immutable
  `created_at`) and rechecks the exact file bytes against the reviewed
  document digest immediately before write. Anything else is intent drift and
  keeps the generic edit/revision rule.

## Usage

```ts
import { parseDecisionTrace, validateProductSpec, parseProductSpec, stripToolMetadata } from "@openagentsinc/product-spec"

const result = validateProductSpec(markdown)
if (result.valid) console.log(result.document.frontmatter.title)

const upstream = validateProductSpec(markdown, { profile: "upstream" })
const trace = parseDecisionTrace(decisionTraceJson)
```

CLI:

```sh
bun packages/product-spec/src/cli.ts validate specs/web/my-feature.product-spec.md
bun packages/product-spec/src/cli.ts validate --specs-root specs
bun packages/product-spec/src/cli.ts validate --profile upstream <file>
bun packages/product-spec/src/cli.ts validate-trace <file.decision-trace.json>
bun packages/product-spec/src/cli.ts digest <file>
bun packages/product-spec/src/cli.ts init specs/<area>/<name>.product-spec.md --title "My Feature"
```

`init` scaffolds the OpenAgents custom sections (`custom-owner-gates`,
`custom-receipts`, `custom-promise-links`) and a flat `tool_metadata` slot.
`stripToolMetadata` is the public-safe export helper — `tool_metadata` never
leaves the repo in a shared artifact.

Decision Trace validation follows the pinned upstream v0.1 schema exactly:
portable subjects, events, decisions, drift/source/result records, and links.
`validateDecisionTrace` returns stable error codes without throwing;
`parseDecisionTrace` returns the typed decoded trace or throws
`DecisionTraceValidationError` with the same diagnostics.

## Boundaries

Product Specs commit intent and, in current upstream ProductSpec, may index
external implementation/eval/outcome evidence with Related Artifacts. Those
links never enforce or verify anything. Behavior contracts and Eval Suites
remain the oracles; evidence systems retain the observations; the promise
registry remains the sole authority for public claims.

The separate Desktop ProductSpec workroom runtime owns accepted plans, work
packets, leases, evidence envelopes, distinct-verifier receipts, and owner
packet dispositions. This package supplies ProductSpec identity and criteria;
it does not own that runtime state.

## Proposed assurance companion

OpenAgents' proposed QA language lives in a separate **AssuranceSpec**
companion, not in ProductSpec fields, custom sections, or `tool_metadata`.
ProductSpec remains the product-intent and portable evidence-attachment layer.
The ProductSpec implementation does not interpret test techniques,
environments, falsifiers, proof rungs, evidence policy, freshness, adequacy, or
release policy. Assurance artifacts and receipts may be linked through
ProductSpec and the Desktop workroom by reference; their state does not move
into this package.

The proposal, including its independent revision law and generated immutable
Assurance Manifest, is documented in
`docs/assurance/ASSURANCE_SPEC.md`. No
AssuranceSpec parser or compiler is claimed to exist in this package.
