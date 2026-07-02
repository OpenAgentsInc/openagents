# Khala Code Visual Smoke Gate

Issue #8013 adds a scoped Tier-1 pre-push gate for the three Khala Code
fixture visual smokes:

1. `bun run --cwd clients/khala-code-desktop smoke:part2-ui`
2. `bun run --cwd clients/khala-code-desktop smoke:cockpit-visual`
3. `bun run --cwd clients/khala-code-desktop smoke:composer-visual`

The gate runs only when the pushed `main` range touches:

- `clients/khala-code-desktop/`
- `packages/ui/`

Other changes are skip-safe. The gate is warning-only from July 2, 2026
through July 8, 2026. It flips to hard-fail mode on July 9, 2026. The dated
flip is intentional: it gives one week of local signal while still ensuring
desktop/UI changes cannot keep merging with stale visual smokes.

## Local Operation

The repo pre-push hook invokes:

```sh
bun scripts/qa-visual-smoke-gate.ts
```

The script receives the pushed `main` range from `.githooks/pre-push` and
compares only those files. Outside the hook it falls back to `origin/main...HEAD`
plus the working tree diff. If changed-file discovery fails, it runs
conservatively as a desktop change.

The default wall-clock budget is 295 seconds, keeping the gate below the
five-minute Tier-1 bound. Override only for local diagnosis:

```sh
OA_KHALA_VISUAL_SMOKE_GATE_TIMEOUT_MS=450000 bun scripts/qa-visual-smoke-gate.ts
```

To test the future hard-fail behavior before July 9:

```sh
OA_KHALA_VISUAL_SMOKE_GATE_MODE=enforce bun scripts/qa-visual-smoke-gate.ts
```

To force a run for a non-desktop change:

```sh
OA_FORCE_KHALA_VISUAL_SMOKE_GATE=1 bun scripts/qa-visual-smoke-gate.ts
```

## Regression Coverage

`scripts/qa-visual-smoke-gate.test.ts` covers the July 2, 2026 regression
class: a landed `clients/khala-code-desktop/src/ui/main.ts` change cannot skip
the visual tier. In hard-fail mode, a failing first fixture smoke blocks the
push.

## Packaged Native AX Follow-Up

The Tier-1 visual gate still covers deterministic fixture screenshots from the
desktop source tree. The headed packaged-window lane is separate and owner-armed:
see [`khala-code-packaged-native-ax-runbook.md`](./khala-code-packaged-native-ax-runbook.md)
for the `QA_NATIVE_DESKTOP=1` Electrobun `.app` smoke.
