# Khala Code Lag Offender Burn-Down

Date: 2026-07-02
Status: ROADMAP_QA Q2.6 / issue #8022 partial burn-down evidence.

Q2.6 is the umbrella for fixing ranked lag offenders from the Q2.3 lag
profiling sweep. The final closeout bar remains seven consecutive real-app
nightlies with every Q2.2 budget green.

## Fixed Slice: Thread Switch Render Volume

The first burn-down slice targets the thread-switch/full-render class called
out by the original QA audit.

Changes:

- The thread-switch benchmark now mocks the current `sessionCatalog` RPC used
  by the sidebar, so the benchmark can run after the session-catalog migration.
- Cached thread switches render the same capped recent-message window used by
  the authoritative resume path instead of repainting every cached message on
  click.
- Full transcript hydration is scheduled on the next animation frame with an
  80ms backup deadline, replacing the loose idle-only path that let hydration
  drift behind the budget.
- The benchmark report now includes `clickToHydratedRenderMs` in addition to
  the existing optimistic/full render timings.

## Evidence

Local benchmark artifacts are intentionally written under ignored `var/`
paths and are not committed.

Before this slice, the default benchmark could not run because the sidebar no
longer used the old `codexThreadList` mock. After repairing the mock, the
baseline showed the cached switch repainting all 120 messages optimistically:

```json
{
  "cached": {
    "clickToFullRenderMs": 310.8,
    "clickToOptimisticRenderMs": 12.8,
    "optimisticMessageCount": 120,
    "routeWallMs": 755
  },
  "cold": {
    "clickToFullRenderMs": 819.2,
    "clickToOptimisticRenderMs": 30.3,
    "routeWallMs": 1129.9
  }
}
```

After the render-volume and hydration changes, an isolated low-RPC benchmark
(`--cold-resume-delay-ms 50 --cached-resume-delay-ms 15`) records Q2.2
`thread_switch.full_render_ms` below the 400ms budget on both cold and cached
paths:

```json
{
  "cached": {
    "clickToFullRenderMs": 309.2,
    "clickToHydratedRenderMs": 460.4,
    "clickToOptimisticRenderMs": 10.6,
    "optimisticMessageCount": 80
  },
  "cold": {
    "clickToFullRenderMs": 329.1,
    "clickToHydratedRenderMs": 484.9,
    "clickToOptimisticRenderMs": 32.2
  }
}
```

The default benchmark still includes an intentional 800ms cold-route delay, so
its cold `clickToFullRenderMs` remains above the render budget by construction.
Use the low-RPC run to isolate render work; use the default run to track
end-to-end route latency separately.

## Remaining Q2.6 Gate

This slice does not by itself satisfy Q2.6's final seven-night real-app green
streak. That requires the owned runner to accumulate seven consecutive
`qa-status-surface.json` reports where every Q2.2 budget has real samples and
passes.

## Verification

Focused checks:

```bash
bun test clients/khala-code-desktop/tests/thread-switch-benchmark.test.ts clients/khala-code-desktop/tests/app-shell.test.ts
bun clients/khala-code-desktop/scripts/thread-switch-benchmark.ts --out var/qa-8022/thread-switch-after.json
bun clients/khala-code-desktop/scripts/thread-switch-benchmark.ts --port 50022 --cold-resume-delay-ms 50 --cached-resume-delay-ms 15 --out var/qa-8022/thread-switch-after-low-rpc.json
```

Pinned issue verify:

```bash
bun run --cwd clients/khala-code-desktop verify
```
