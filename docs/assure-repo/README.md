# ASSURE-REPO — programmatic verification of the OpenAgents codebase

ASSURE-REPO makes the OpenAgents monorepo the first fully inventoried
verification subject of its own verifiable-software program. It maps every
verification-bearing surface to its oracles, grades assurance coverage, audits
suites for false greens, runs a standing verification sweep, and checks the
repository's own documented claims.

- Epic: [#9055](https://github.com/OpenAgentsInc/openagents/issues/9055)
- Owning intent:
  [`specs/openagents/assure-repo-codebase-verification.product-spec.md`](../../specs/openagents/assure-repo-codebase-verification.product-spec.md)
- Design rationale: `docs/fable/2026-07-19-verifiable-software.md` (Addendum III)

It creates **no second completion gate**: `pnpm run check` remains the
repository definition of green. ASSURE-REPO maps and grades what that green
actually proves.

## Packets

| Packet | Issue                                                            | State     | What it adds                                    |
| ------ | ---------------------------------------------------------------- | --------- | ----------------------------------------------- |
| AR-0   | [#9056](https://github.com/OpenAgentsInc/openagents/issues/9056) | delivered | Typed surface inventory with loss accounting    |
| AR-1   | [#9057](https://github.com/OpenAgentsInc/openagents/issues/9057) | delivered | Assurance obligations graded over the inventory |
| AR-2   | [#9058](https://github.com/OpenAgentsInc/openagents/issues/9058) | planned   | False-green audit + mutation evidence           |
| AR-3   | [#9059](https://github.com/OpenAgentsInc/openagents/issues/9059) | planned   | Standing verification sweep (Full Auto lane)    |
| AR-4   | [#9060](https://github.com/OpenAgentsInc/openagents/issues/9060) | planned   | Drift oracles for the repository's own claims   |

## The surface inventory (AR-0)

`surface-inventory.v1.json` is a machine-readable, deterministically generated
inventory of every verification-bearing surface: apps, packages, workers,
crates, public-API contract surfaces, release pipelines, product specs, and
curated governed documents. Each surface either binds to its oracles
(behavior contracts, tests, product-spec/assurance obligations) or carries an
explicit `unverified` reason. The load-bearing invariant is **no silent
surfaces**: a row with neither an oracle ref nor an explicit reason fails
validation.

An oracle ref is an **index entry, not a verdict**: it proves an oracle is
authored for the surface, not that the surface is proven. AR-1 grades the
obligation state; AR-3 carries the observed verdict.

### Obligation grading (AR-1)

Each surface carries an `obligation` state graded in the assurance-spec
vocabulary, with the four coverage facts kept independent — `mapped`,
`designed`, `observed`, `accepted` — plus `inconclusive` (a real coverage
gap) and `out-of-scope` (a typed disposition). Grading **never emits
`observed` or `accepted`**: those require a passing, source-bound AR-3 sweep
receipt or owner acceptance. A designed oracle is not a passing observation.
`pnpm exec assure-repo coverage` prints the per-program-area report; there is
**no blended score** — that is structurally excluded.

### Determinism and the freshness guard

The artifact has no wall-clock timestamps and is sorted throughout, so
`pnpm run check:assure-repo` regenerates it in memory and byte-compares against
the committed file. A repository change that would alter the derived inventory
fails the guard until the artifact is regenerated — that deterministic check is
the "freshness relative to `main`" mechanism.

### Commands

```sh
pnpm run generate:assure-repo   # regenerate surface-inventory.v1.json
pnpm run check:assure-repo      # regenerate + byte-compare + validate (exit 1 on drift)
pnpm run test:assure-repo       # unit + real-repo tests
node --import tsx packages/assure-repo/src/cli.ts summary   # print the coverage summary
```

`check:assure-repo` is wired into `check:fast` (the pre-push gate) and into
`scripts/check-manifest.ts`.

### Files

- `surface-inventory.v1.json` — the generated inventory (committed).
- `surface-policy.v1.json` — the non-derivable classifications: explicit
  `unverified` reasons for surfaces that genuinely lack oracles, the curated
  governed-document set (AR-4 will bind drift oracles to these), and AR-1
  out-of-scope dispositions. Keeps the generator deterministic while letting a
  human state _why_ a surface is unverified instead of the generator guessing.
- Implementation: [`packages/assure-repo`](../../packages/assure-repo/).

### Scope of AR-0 rev 1

Surfaces are enumerated at package / app / worker / crate / public-endpoint /
release-pipeline / document granularity, derived from the pnpm workspace, the
Cargo workspace, the behavior-contract registries, and the product-spec set.
Individual HTTP routes, Electron IPC channels, and per-endpoint contracts are
represented by their owning worker/app/public-endpoint surface, not yet
subdivided; fine-grained route/IPC enumeration is a bounded follow-up recorded
in the inventory's `summary.coverageNotes`.
