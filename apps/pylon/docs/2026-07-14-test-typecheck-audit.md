# Pylon test typecheck audit

Date: 2026-07-14
Issue: [OpenAgentsInc/openagents#8792](https://github.com/OpenAgentsInc/openagents/issues/8792)

## Result

`pnpm run typecheck` in `apps/pylon` now runs both the existing production
`NodeNext` typecheck and a strict, no-emit test program. The test program uses
bundler module resolution because the Bun test suite intentionally imports
extensionless TypeScript modules and `.ts` modules directly; production source
keeps its existing `NodeNext` resolution contract.

The gate discovers every `*.test.{ts,tsx,mts,cts}` and
`*.spec.{ts,tsx,mts,cts}` file below `apps/pylon`, then proves every discovered
file is a root input to `tsconfig.tests.json`. At introduction this covers 317
TypeScript test files across the root tests, colocated source tests, and the
nested runtime package.

## Existing debt contract

The first strict program exposed 1,034 existing diagnostics, including stale
fixtures/test doubles and transitive source diagnostics. They are recorded by
exact file, location, TypeScript code, and message in
`typecheck-tests-baseline.json`.

This is a shrink-only baseline:

- a new diagnostic fails `pnpm run typecheck`;
- a resolved diagnostic also fails until its stale baseline entry is removed;
- the updater refuses to add a diagnostic to an existing baseline;
- deleting a test file fails until the test-file count change is explicitly
  reviewed;
- adding a test file fails until the count is reviewed, and the new test is
  compiled before any baseline update can pass.

Use `pnpm run typecheck:tests:update-baseline` only after fixing existing
diagnostics. It can rewrite the baseline when the diagnostic set shrinks; it
cannot bless new debt.

The Node test in `scripts/typecheck-tests.test.mjs` creates a valid required
fixture, records a zero-error baseline, breaks the required property, and
proves the typecheck gate rejects the new `TS2741` diagnostic. It also proves a
fixed diagnostic must be removed from the baseline and that the updater can
only shrink it.

## Workspace test-root sweep

A TypeScript-config root-input audit covered top-level `apps/*` and
`packages/*` projects with TypeScript test files. Apart from Pylon, it found
three uncovered groups:

| Surface                    | TypeScript tests | Disposition                                                                                                                    |
| -------------------------- | ---------------: | ------------------------------------------------------------------------------------------------------------------------------ |
| `packages/probe`           |               74 | No TypeScript project currently covers the tests; tracked by [#8801](https://github.com/OpenAgentsInc/openagents/issues/8801). |
| `apps/oa-updates`          |               21 | No TypeScript project currently covers the tests; tracked by [#8802](https://github.com/OpenAgentsInc/openagents/issues/8802). |
| `apps/qa-runner/generated` |                4 | Generated E2E cases are outside the app's source config; retain as an explicit generated exception.                            |

All other audited app/package test files were root inputs to at least one local
TypeScript config. This audit is about config inclusion, not a claim that every
project's current diagnostics are green.

## Verification

```sh
pnpm --dir apps/pylon run typecheck
node --test apps/pylon/scripts/typecheck-tests.test.mjs
pnpm --dir apps/pylon run test
```

Final local receipt: the typecheck compiled all 317 TypeScript test files with
zero new diagnostics; the negative gate tests passed 2/2; and the complete
Pylon suite passed 2,470 tests with three explicit live-provider skips and zero
failures across 318 test files.
