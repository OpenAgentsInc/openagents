# @openagentsinc/product-spec

OpenAgents implementation of the [ProductSpec](https://github.com/gokulrajaram/ProductSpec)
open standard (format v0.1) for `.product-spec.md` intent artifacts — an Effect
Schema document model, parser, validator, and CLI. Adoption rationale and
design: `docs/fable/2026-07-08-productspec-adoption-analysis.md`; repo
conventions: `specs/CONVENTIONS.md`; tracking: #8593.

This is our own implementation. The upstream `@productspec/parser` is a
conformance reference only — never a runtime dependency. Compatibility is held
by the vendored fixtures in `fixtures/conformance/` (MIT, attributed), which
this package's tests run on every sweep alongside the OpenAgents extension
fixtures and every spec in the repo `specs/` tree.

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

Specs declare intent; they never enforce. Behavior contracts and Eval Suites
remain the oracles; the promise registry remains the sole authority for public
claims. A spec links registry IDs — it never duplicates their content.
