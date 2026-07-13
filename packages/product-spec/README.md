# @openagentsinc/product-spec

OpenAgents implementation of the [ProductSpec](https://github.com/gokulrajaram/ProductSpec)
open standard (format v0.1) for `.product-spec.md` intent artifacts — an Effect
Schema document model, parser, validator, and CLI. Adoption rationale and
design: `docs/fable/2026-07-08-productspec-adoption-analysis.md`; repo
conventions: `specs/CONVENTIONS.md`; tracking: #8593.

This is our own implementation. The upstream `@productspec/parser` is a
conformance reference only — never a runtime dependency. The vendored fixtures
in `fixtures/conformance/` (MIT, attributed) preserve the earlier snapshot this
package implemented.

That snapshot is no longer current. Upstream `0.19.0` at `9ef2654` added
mandatory structured `AC-*`/`SM-*` items, Related Artifacts, evidence
checklists, spec sessions, and stricter validator semantics. This package does
not yet implement those features, and the current MVP ProductSpec is not valid
under the official `0.19.0` parser. Do not describe this package as
upstream-current until the catch-up and conformance fixtures land. The adopted
boundary and migration plan are in
`docs/assurance/PRODUCTSPEC_EVIDENCE_LOOP.md`.

## Usage

```ts
import { validateProductSpec, parseProductSpec, stripToolMetadata } from "@openagentsinc/product-spec"

const result = validateProductSpec(markdown)
if (result.valid) console.log(result.document.frontmatter.title)
```

CLI:

```sh
bun packages/product-spec/src/cli.ts validate specs/web/my-feature.product-spec.md
bun packages/product-spec/src/cli.ts validate --specs-root specs
bun packages/product-spec/src/cli.ts init specs/<area>/<name>.product-spec.md --title "My Feature"
```

`init` scaffolds the OpenAgents custom sections (`custom-owner-gates`,
`custom-receipts`, `custom-promise-links`) and a flat `tool_metadata` slot.
`stripToolMetadata` is the public-safe export helper — `tool_metadata` never
leaves the repo in a shared artifact.

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
