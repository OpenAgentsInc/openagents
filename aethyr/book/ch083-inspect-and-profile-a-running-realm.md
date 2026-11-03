# 83. Inspect and profile a running realm

> **Status: unimplemented — design draft.** Today each check is its own
> script under `coax/web/`: `perf.ts` times Everglade's frames in headless
> Chrome, `realm-perf.coax` times every realm in `realms.json` (built,
> #12649), `frametime.ts` reads GPU time per pass, `goldens.ts` walks every
> realm against its golden screenshot, and `?probe` reads a frame back.
> Analytics events come from `apps/openagents-web/analytics.coax`. No
> realm-wide inspector or shared frame report exists. This chapter plans
> them. Nothing in `aethyr/` runs yet.

When a realm drops a frame or draws the wrong thing, you need to see where
the time went and what state produced the frame. This chapter gives every
realm the same frame report, the same probes and the same release checks.
[Chapter 37](ch037-measure-the-engine-honestly.md) measures the physics
engine; this chapter measures a whole realm as a player sees it.

## What this chapter teaches

- How to read a frame report: CPU time per stage, GPU time per pass and
  frame spacing.
- How to tell a slow page from a busy machine with a control page.
- How goldens and probes catch a page that starts but draws nothing.
- What analytics may record, and what it never records.

## Outline

1. **The 60 FPS rule.** A case fails when its 95th percentile frame is
   over 17.5 ms or any frame is over 33 ms. `perf.ts` opens Everglade at
   four places and times (road at noon, commons at dusk, river at night,
   market at dusk), at device pixel ratios 1 and 2, at 1440x900, and walks
   about 13 s, casting a meteor every 6.7 s. `realm-perf.coax` takes
   every realm in `realms.json` at dpr 1 and 2 at 1440x900 and as a
   390x844 phone at dpr 3, walks about 8 s in each, and runs after every
   release against production as a report compared with the last
   release's (`releases/<time>/REALMS.tsv`); it never blocks a release.
2. **A busy machine.** A failing case is tried up to three times beside a
   control page (production's Everglade, standing still). When the control
   misses frames too, the check waits 10 s and tries again, at most 6
   times. The output says which case failed and why.
3. **GPU time per pass.** `frametime.ts` puts a timestamp query on every
   render pass; `--skip` drops numbered passes to cost them; `--phone`
   emulates a touch phone.
4. **Stage timers.** A planned per-frame report from the loop of
   [chapter 1](ch001-a-frame-has-a-fixed-step.md): steps run, input,
   simulation, extraction and draw submission, each in microseconds, shown
   in a HUD overlay and readable by tests.
5. **Probes.** `?probe` reads the frame back; `?probe&at=SECONDS` runs the
   clock that far in 0.1 s steps; `?probe&do=STEPS` runs input before the
   probed frame. Probes are development only.
6. **Goldens.** `goldens.ts` walks the landing and the house, then opens
   every realm in `realms.json` by its address; each must say ready, throw nothing,
   draw enough lit pixels and match its golden. `release.sh` runs it on
   staging before promoting.
7. **Analytics.** Common events (session-start, load-engine, world-enter,
   alive every 30 s, world-leave) and per-realm helpers such as
   `track_walk` and `track_tally`. No words and no personal data are
   recorded; the ingest keeps at most 16 properties an event, 50 events a
   batch and 5,000 a session.
8. **Native debugging.** `coax/DEBUGGING.md` covers source-level
   debugging and compiler timings for the native build of a realm.

## What exists today

- [coax/web/perf.ts](../../coax/web/perf.ts): Everglade's 60 FPS cases.
- [coax/web/realm-perf.coax](../../coax/web/realm-perf.coax): every
  realm's 60 FPS cases, generated from `realms.json`.
- [coax/web/frametime.ts](../../coax/web/frametime.ts): GPU time per pass.
- [coax/web/goldens.ts](../../coax/web/goldens.ts): the realm walk and the
  analytics contract.
- [coax/web/host.js](../../coax/web/host.js): `?probe`, `at` and `do`.
- [docs/verse/analytics-events.md](../../docs/verse/analytics-events.md):
  what each realm records.
- [coax/DEBUGGING.md](../../coax/DEBUGGING.md): native debugging.

## What it builds

- `aethyr/frame-report.coax`: stage timers, budgets per stage and a fixed
  report record.
- A HUD overlay that draws the report, off by default.
- A realm list built from `realms.json`, so every realm has 60 FPS cases,
  not only Everglade. Built: `coax/web/realm-perf.coax` (#12649).

## Acceptance checks and oracles

The chapter is done when:

- Every listed realm has at least one 60 FPS case at dpr 1 and 2, and a
  phone case at 390x844.
- The frame report's stage times sum to within 0.5 ms of the measured
  frame time.
- The overlay costs under 0.2 ms a frame, measured with and without it.
- Turning the report on does not change the session digest of
  [chapter 5](ch005-replay-a-session-exactly.md).
- `goldens.ts` and the analytics contract still pass on staging.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md) (the loop's stages),
[5](ch005-replay-a-session-exactly.md) (replay for a reported frame),
[15](ch015-ui-and-the-hud.md) (the overlay) and
[16](ch016-budgets-that-refuse.md) (budgets). Coax:
[chapter 12](../../coax/book/ch12-debugging-and-cost.md) for debugging and
measuring cost.
