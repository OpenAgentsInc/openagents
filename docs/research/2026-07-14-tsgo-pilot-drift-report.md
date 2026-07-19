# tsgo pilot drift report: `@effect/tsgo` native typechecking on `packages/khala-tools`

Date: 2026-07-14
Issue: TC-4 (#8775), part of the T3 Code Vite Plus adaptation plan
(`docs/teardowns/2026-07-13-t3-code-teardown.md` §17).
Scope: bounded pilot on ONE package. Nothing here is wired into the canonical
typecheck path. Root `typecheck` and the package `typecheck` script are
unchanged.

## Target package and why

`packages/khala-tools`, chosen over the other proposed candidate
`packages/agent-runtime-schema` on both axes the issue names:

| Package | TS files | Lines | `effect` import sites |
|---|---|---|---|
| `packages/khala-tools` | 48 | ~17,731 | 48 |
| `packages/agent-runtime-schema` | 10 | ~6,261 | 4 |

khala-tools is ~3x the code and ~12x the Effect-API density, and it pulls in
three workspace source dependencies (`agent-runtime-schema`, `mcp-contract`,
`pipeline-signals`) through its `src`-level exports, so the compile graph
exercises `Schema`, `Context.Service`, `Layer`, `Effect.gen`, and stream/tool
plumbing across multiple packages — the closest thing in the repo to T3 Code's
Effect-heavy server.

## Exact versions and environment

| Component | Version |
|---|---|
| `@effect/tsgo` | `0.13.2` (exact pin, devDependency of `packages/khala-tools`) |
| `@typescript/native-preview` | `7.0.0-dev.20260604.1` (exact pin, devDependency of `packages/khala-tools`) |
| patched `tsgo --version` | `7.0.0-dev+effect-tsgo.0.13.2` |
| `typescript` (canonical `tsc`) | `6.0.3` (workspace catalog `^6.0.3`) |
| `effect` | `4.0.0-beta.70` (workspace catalog) |
| bun | 1.3.11 |
| Machine | Apple M5 Max, 128 GB RAM, macOS 26.4 |

Version-selection note: the latest `@effect/tsgo` at pilot time (`0.19.0`,
published 2026-07-10) has **dropped the `@typescript/native-preview` backend**.
Its `effect-tsgo patch` fails with `NativeBackendNotInstalledError: No native
TypeScript backend is installed. Install "typescript" >= 7 ... or
"@typescript/native"`. Our catalog `typescript` is `^6.0.3` and bumping it is a
root-level change out of scope for this pilot (and TC-1 territory), so the
pilot pins the T3-Code-proven pair instead: `@effect/tsgo 0.13.2` +
`@typescript/native-preview 7.0.0-dev.20260604.1` — the exact pins in
`projects/repos/t3code/pnpm-workspace.yaml`.

## The `effect-tsgo patch` story

Yes, the patch is required for the Effect diagnostics. No, it is not required
for tsgo to typecheck at all.

What `effect-tsgo patch` does: it locates the platform-specific native Go
binary installed by `@typescript/native-preview` (here
`node_modules/.bun/@typescript+native-preview-darwin-arm64@7.0.0-dev.20260604.1/.../lib/tsgo`,
24.9 MB), backs it up as `tsgo.original`, and **replaces it with a re-built
30.5 MB binary that embeds the Effect Language Service** (Effect-specific
diagnostics and hover). It then self-verifies ("Verification succeeded").

Observed properties:

- Without the patch, the pristine `tsgo` binary typechecks the package fine —
  it just emits zero Effect-specific diagnostics (verified by running the
  backed-up `tsgo.original` directly: 0 diagnostics, exit 0).
- The patch mutates `node_modules`, so it must be re-run after every
  `bun install`. T3 Code runs it from root `prepare`. Because TC-1 (#8772) is
  concurrently reworking root `package.json`, this pilot keeps it package-local
  as `typecheck:tsgo:patch` and deliberately does NOT chain it into
  `typecheck:tsgo`, because:
- The patch is re-run **safe** but not idempotent on disk: each re-run backs up
  the current binary as `tsgo.original.N` (~30 MB each), accumulating garbage
  in `node_modules`. One patch per install is the right cadence. Patch itself
  takes ~0.6 s.

Pilot usage: `cd packages/khala-tools && bun run typecheck:tsgo:patch` once
after install, then `bun run typecheck:tsgo` as often as you like.

## Drift: diagnostics emitted by exactly one compiler

Both compilers were run repeatedly over the package (tsgo x6+, tsc x5+).
tsgo output was byte-identical across runs (matching md5 over 3 captured
runs). Tsc output was identical (empty) across runs.

- `tsc -p tsconfig.json --noEmit`: **0 diagnostics**, exit 0.
- patched `tsgo -p tsconfig.json --noEmit`: **4 diagnostics**, all severity
  `suggestion`, exit 0.
- unpatched `tsgo.original`: 0 diagnostics, exit 0 (proving all 4 come from
  the Effect Language Service, not from the native compiler core).

Every tsgo-only diagnostic, classified:

| # | Location | Code | Message (abridged) | Class |
|---|---|---|---|---|
| 1 | `src/redaction.ts(48,14)` | TS377091 `effect(lazyEffect)` | Service member `revealTransform` returns a lazy Effect. Zero-arg wrapper adds unnecessary indirection | Cosmetic (advisory, correct observation, intentional API shape) |
| 2 | `src/session-rollout.ts(136,10)` | TS377090 `effect(unnecessaryTypeofType)` | `typeof KhalaToolEvent.Type` query can be `KhalaToolEvent` | Cosmetic (advisory style hint) |
| 3 | `src/session-rollout.ts(252,18)` | TS377090 | same as #2 | Cosmetic |
| 4 | `src/session-rollout.ts(253,23)` | TS377090 | same as #2 | Cosmetic |

Counts per class: **false positives: 0, missed errors: 0, cosmetic: 4** (all
four are tsgo-only, all `suggestion` severity, none affect the exit code).
There were zero diagnostics emitted by tsc and not by tsgo.

### Synthetic error-parity probe

Because the package is clean, error-diagnostic drift could only be observed as
absence. To probe it, a temporary file with six representative mistakes
(Effect 3-style `Context.Tag` on Effect 4, wrong success type through
`Effect.gen` yield, property access on a `Schema.decodeUnknownSync` result,
plain structural mismatch, `unknown` narrowing, missing member) was compiled by
both and then deleted (never committed).

Result: **6/6 identical errors** — same codes (TS2339, TS18046, TS2322), same
file/line/column positions, same messages — with two cosmetic differences:

1. In one TS2339 message, tsc renders the import type as
   `typeof import("...", { with: { "resolution-mode": "import" } })` while tsgo
   omits the import-attribute clause. Cosmetic (message display only).
2. Exit codes on failure differ: tsc exits **2**, tsgo exits **1**. Both are
   nonzero, so `&&`-chains and CI behave the same, but anything matching a
   specific exit code would need adjusting. Cosmetic/operational.

## Timings

Median over 4 warm runs each (one untimed warm-up run first, `/usr/bin/time
-l`. Whole-process wall clock including startup):

| Compiler | Warm runs (real, s) | Median (s) | Max RSS (median) |
|---|---|---|---|
| `tsgo -p tsconfig.json --noEmit` (patched 0.13.2) | 0.25, 0.27, 0.26, 0.27 | **0.26** | ~338 MB |
| `tsc -p tsconfig.json --noEmit` (6.0.3) | 2.48, 2.32, 2.35, 2.33 | **2.34** | ~854 MB |

tsgo is ~**9x faster** and uses ~**2.5x less peak memory** on this package.
CPU time tells the same story (tsgo ~0.9 s user vs tsc ~4.4 s user). If the
ratio holds across the ~40-package root `typecheck` chain, that chain's tsc
component would drop from minutes to tens of seconds — but that extrapolation
is exactly what this pilot does NOT claim. It is a one-package measurement.

## Recommendation: conditional GO (opt-in parallel lane), NO-GO for cutover

- **GO** for keeping `typecheck:tsgo` as an opt-in, package-local parallel
  lane on Effect-heavy packages, and for extending the pilot to 2-3 more
  packages if anyone wants the speed during development. Error parity on this
  package was perfect (0 false positives, 0 missed errors, 6/6 probe parity),
  determinism was byte-exact, and the speedup is large.
- **NO-GO** for wiring tsgo into the canonical typecheck path (root
  `typecheck`, the TC-1 #8772 `check` verb, or CI gating) at this time.
  Reasons:
  1. The compiler is a dev-preview (`7.0.0-dev.*`) and the Effect layer
     requires binary-patching `node_modules` after every install — a fragile,
     platform-specific step (per-platform Go binaries) with backup-file
     accumulation on re-runs.
  2. The `@effect/tsgo` backend contract is churning: 0.19.0 already dropped
     `@typescript/native-preview` in favor of `typescript` >= 7, which we
     cannot adopt without a catalog-wide TypeScript major bump.
  3. One-package evidence is not parity evidence for the whole repo (T3's own
     caveat applies here too: type-aware lint integration with tsgo
     diagnostics is unsolved — `projects/repos/t3code/vite.config.ts`).
  4. Exit-code and message-format differences are small but real.

### Revisit triggers

Re-run this pilot (and widen it) when ANY of these fires:

1. The workspace catalog moves to `typescript` >= 7 (then adopt a current
   `@effect/tsgo` without the native-preview pin and re-measure).
2. `@effect/tsgo` ships a backend path that does not require patching binaries
   in `node_modules` (or `@typescript/native-preview` graduates from preview).
3. Root `typecheck` wall clock becomes a measured fleet bottleneck (e.g.
   median full-chain typecheck exceeds ~5 minutes on the standard dev machine).
4. Any real (non-probe) tsc-vs-tsgo mismatch is observed in this repo — that
   is immediate evidence either way and should be appended to this report.
5. Oxlint's tsgolint path integrates with `@effect/tsgo` diagnostics (removes
   the type-aware-lint caveat).

## Repro

```
cd packages/khala-tools
bun run typecheck:tsgo:patch   # once per bun install (~0.6 s)
bun run typecheck:tsgo         # native typecheck, exit 0 + 4 suggestions
bun run typecheck              # canonical tsc, unchanged, exit 0
```
