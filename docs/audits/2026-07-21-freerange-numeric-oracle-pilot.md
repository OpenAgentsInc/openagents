# Freerange Numeric-Oracle Pilot — 2026-07-21

A bounded pilot of `@chenglou/freerange` as a conditional numeric oracle for
real layout and numeric helpers. The owning issue is FREERANGE-03 (#9126). The
source teardown is
[`docs/teardowns/2026-07-21-freerange-teardown.md`](../teardowns/2026-07-21-freerange-teardown.md).

**Disposition: keep-bounded.** Freerange proves real numeric invariants on
pure, import-free numeric helpers with zero code changes, and it found no
defects in the clean helpers it could analyze. Most application code is outside
its subset because of ordinary constructs (string methods, exponentiation,
array methods, generic arrays, `Date` parsing, environment records). No
behavior-preserving refactor added real value, so the pilot deliberately did
not change product code. Freerange stays a pinned, isolated, advisory tool.

## Setup

- Tool: `@chenglou/freerange@0.0.1`, the exact version audited in the teardown.
  Version 0.0.2 is now published (a watch item).
- Isolation: the tool ran in a throwaway Bun project outside the monorepo. The
  monorepo Node 24 and pnpm toolchain contract was not touched. Nothing was
  added to any workspace `package.json`.
- Method: copy an import-free numeric helper into the throwaway project, then
  run `fr` (findings) and `fr --audit` (contracts). Freerange analyzes
  same-file functions only, so a file with local imports cannot be analyzed in
  isolation. Every subject below is import-free.

## What Freerange proved, with zero changes

`packages/autopilot-control-protocol/src/account-capacity-bar.ts` — both
functions fully analyzed:

```
capacityBar
  ensures: return.pct is a finite number from 0 through 100
  assumes: input.usedPct is null or a finite non-NaN number
  assumes: input.exhausted is a boolean
clampPercent
  requires: Number.isFinite(value)
  ensures: return is a finite number from 0 through 100
```

The proven `0 through 100` bound on `capacityBar.pct` is a real interface
invariant that a progress bar relies on. Freerange derived it from the
`Math.min`/`Math.max` clamp with no annotations and no code change. The
`assumes: input.usedPct is null or a finite non-NaN number` line is the printed
trust: the bound holds under the stated input assumption.

A batch of nine import-free numeric helpers (ranking, cloud cost estimate,
connection retry, cursor, deploy poll state, node summary view, notification
quiet hours, durable-stream protocol, account capacity bar) ran clean:

```
0 findings (0 errors, 0 warnings).
coverage: 10/15 named top-level function declarations fully analyzed; 5 unsupported.
```

Zero findings across the analyzable functions is itself a useful result: those
helpers are free of the `NaN`, `Infinity`, division-by-zero, and out-of-bounds
classes Freerange checks.

## Where Freerange stopped, and why

The unsupported functions were blocked by ordinary TypeScript constructs, not
by code smells:

| Blocker                              | Example site                              |
| ------------------------------------ | ----------------------------------------- |
| environment record `Record<string,…>`| `apple-fm-energy-estimate.ts` power config|
| `Date.parse`                         | `apple-fm-energy-estimate.ts` window       |
| `value.toFixed`                      | `apple-fm-energy-estimate.ts` `round`      |
| `**` exponentiation                  | `connection-retry.ts` backoff              |
| `String.toLowerCase` / `String.trim` | `deploy-poll-state.ts`, `node-summary-view.ts` |
| `Array.filter`                       | `node-summary-view.ts` session count       |
| generic `T[]` parameter              | `notification-quiet-hours.ts`              |

`apps/pylon/src/node/apple-fm-energy-estimate.ts` was the clearest mixed case:
1 of 6 functions analyzed (`unavailableEstimate`, with real contracts on its
numeric and array fields), the rest blocked by the constructs above.

## Why the pilot did not refactor product code

The issue anticipated refactoring one or two calculations into the subset. On
inspection, none of the available refactors was a genuine improvement:

- The `round` helper uses `value.toFixed(digits)` and feeds numeric energy
  fields that become receipt evidence. Replacing it with `Math.round`-based
  arithmetic changes the rounding behavior. That is a behavior change in an
  evidence-producing calculation, not a safe normalization.
- `connection-retry.ts` uses `2 ** attempt`, but the result is already capped
  with `Math.min(…, maxMs)`. The code is correct. `**` is a Freerange subset
  limit, not a defect.
- The `Array.filter`, `String.toLowerCase`, `String.trim`, and generic-array
  cases are ordinary, correct code. Rewriting them as manual loops or narrower
  shapes only to satisfy the tool is the overfitting that both the Freerange
  engineering notes ("only normalize when the application actually wants that
  runtime behavior") and this repository's discipline ("do not overfit to the
  tool") warn against.

Findings mode found zero defects, so there was no bug-fix refactor to make
either. Forcing an analyzability-only refactor would have traded correct,
readable code for a green from an advisory tool. The pilot declined that
trade.

## Frictions to keep visible

- **Bun-only CLI.** `fr` runs under Bun. The monorepo runs on Node 24 and
  pnpm. Any use stays an isolated, pinned invocation, never a toolchain
  dependency.
- **Same-file analysis only.** A file with local imports cannot be analyzed in
  place. In-repo use would need co-located helpers or extracted copies.
- **Text output, no machine format.** `fr` prints human text with no JSON mode,
  so receipts-grade or inventory integration would need parsing or an upstream
  feature request.
- **TypeScript 6 compiler API.** Freerange pins the TypeScript 6 API. The
  monorepo uses a newer TypeScript. Compatibility was fine for isolated
  import-free files but is a standing risk for broader use.
- **Uniform finite requirement.** Every plain `number` parameter carries an
  automatic finite caller requirement, even when the function guards
  non-finite input internally (as `clampPercent` does). Freerange still prints
  the requirement, so its contract can read as more conservative than the code.

## Authoring guidance (bounded)

The pilot supports one narrow, low-cost practice for **pure numeric helpers**,
where Freerange already delivers value:

- Keep important numeric calculations in small, named, synchronous helpers with
  numeric parameters and explicit `Math.min`/`Math.max` clamps and
  `Number.isFinite` guards. Helpers written this way — like
  `clampPercent`/`capacityBar` — get a proven range for free and stay analyzable
  by a mechanical oracle.

This is guidance for the numeric-helper shape only. It is not a mandate to
restructure application code, adopt the tool into the toolchain, or treat a
Freerange green as acceptance authority.

## Boundary

A Freerange contract is a conditional, designed-oracle result. It is scoped by
its coverage line and conditional on its printed `assumes` lines. It is never
acceptance, release, or public-claim authority. This pilot changed no product
code, added no dependency to the workspace, and flipped no promise.
