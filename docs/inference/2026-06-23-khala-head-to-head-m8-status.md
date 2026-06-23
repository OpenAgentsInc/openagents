# Khala M8 head-to-head — status and comparison (measured / executed-verifier half)

*2026-06-23. This is the **measurement + runbook half** of Khala M8 (#6016,
EPIC #6017): the MEASURED, Bitcoin-settlement-armed head-to-head data point we
already have on record. It is built from the prod run we already paid for — no new
paid inference was burned to produce this doc.*

> **Honest framing up front (updated 2026-06-23):** the recorded run's
> `verified:true` originally came from the M2 **static regex pre-screen**, which
> never ran the game. We have now **executed** the exact north-star artifact
> through the real headless acceptance runner, and the honest result is
> **`verified:false` — it fails 6/6 acceptance checks** (it crashes on load and
> never plays). So this is the **measured / settlement-armed** half, with an
> **execution-gated verifier that works and a north-star artifact that does not
> pass it.** A real `failed` beats a fake `passed`. The two remaining
> differentiators — **watched-in-Verse** and **playable-in-our-world** — are
> pending the Verse lanes (crackling render fix in flight; the in-world three.js
> game is not yet built). This doc does not claim M8 is published.

## The benchmark

Prompt (the north-star, identical to the public comparison):

> build a really high quality single html file crossy road game with three.js

Reported public comparison (**external claims, NOT ours**):

| source | tokens | $ | wall-clock | reported verdict |
| --- | --- | --- | --- | --- |
| Sakana Fugu Ultra | ~89k | ~$7.32 | ~22 min | faster/cheaper; defects: inverted turns, wonky camera, no SFX |
| Claude Opus 4.8 Ultracode | ~940k | ~$37.85 | ~79 min | higher quality; defects: retry loops, wrong restart position |

These numbers are carried in every manifest as `externalReportedClaims` with
`citationStatus: reported_without_primary_url` and an
`external_claim_primary_url_missing` blocker. They are **never** mixed into our
measured scoreboard.

## Our recorded verified run

On **2026-06-22**, prod `openagents.com` `khala-code` streamed the full
crossy-road north-star prompt end to end:

- **wall-clock:** ~106s, **no 524** (the streaming SSE 524 fix from PR #6031 /
  `docs/inference/2026-06-22-long-running-inference-response-strategies.md`).
- **stream:** the SSE generation completed (~11.4k SSE frames consumed by the
  M8 streaming runner without tripping the Cloudflare edge idle timeout).
- **verifier verdict (as originally recorded):** the terminal `openagents` block
  returned `verification: "test_passed"`, `verified: true`, `scalar_reward: 1` —
  but that came from the M2 **static regex pre-screen**, which never ran the game.
- **verifier verdict (now, EXECUTED):** the preserved artifact was run through the
  real headless acceptance runner on **2026-06-23** and the honest executed verdict
  is `executed: true`, `verification: "failed"`, `verified: false`,
  `scalar_reward: 0` — **0 of 6** acceptance checks passed. See
  "Executed-acceptance verdict" below.
- **coordinator:** the heuristic router (v0), not the learned M6/M7 conductor.

This is **our Khala data point**. It is recorded as an evidence manifest at
[`docs/inference/fixtures/khala-head-to-head-recorded-run.v1.json`](fixtures/khala-head-to-head-recorded-run.v1.json)
so the reducer and the metric-table emitter operate on it exactly like any other
manifest, and we do **not** re-run the paid endpoint to regenerate it.

### Executed-acceptance verdict (RESOLVED — we actually ran it)

The original `test_passed` / `verified:true` / `scalar_reward:1` verdict came from
the M2 verifier **as it existed on 2026-06-22**, a **static regex pre-screen over
the HTML source — it never executed the artifact**. A same-day finding
([`docs/inference/2026-06-22-verified-work-must-execute-the-artifact.md`](2026-06-22-verified-work-must-execute-the-artifact.md))
showed a `verified:true` crossy-road game with four real defects (crash on load,
PLAY did nothing, camera flew ~100×/hop, world stopped generating).

**On 2026-06-23 we resolved this honestly by EXECUTING the artifact.** The exact
north-star completion was recovered from the verified prod SSE stream
(`/tmp/khala-stream.txt`, model `openagents/khala-code`), reconstructed to a single
HTML file (fingerprint `fnv1a32:10cb2792`, 53,426 bytes), and preserved at
[`scripts/khala-demo/artifacts/khala-crossy-road-northstar-run.v1.html`](../../scripts/khala-demo/artifacts/khala-crossy-road-northstar-run.v1.html).
It was run through the real headless acceptance runner
(`apps/openagents.com/workers/api/src/inference/acceptance-runner/`, Playwright +
chromium) and then through `verifyKhalaCodeCompletion`, via the harness
[`scripts/khala-demo/run-executed-acceptance.mjs`](../../scripts/khala-demo/run-executed-acceptance.mjs).

**The executed verdict:** `executed: true`, `verification: "failed"`,
`verified: false`, `scalarReward: 0` — **0 of 6** acceptance checks passed.

| acceptance check | executed result |
| --- | --- |
| `loads_without_errors` | ❌ FAIL — 1 page error during construction (`localStorage` read in the `Game` ctor throws in an opaque origin; the ctor never finishes, the `start-btn` listener never attaches — the documented crash-on-load defect, executed). |
| `play_starts_game` | ❌ FAIL — PLAY did not start the game (`started=undefined`, loop never ticked). |
| `forward_input_advances_player` | ❌ FAIL — forward input did not advance the player. |
| `camera_follow_delta_bounded` | ❌ FAIL — no bounded camera motion observed. |
| `world_keeps_generating_ahead` | ❌ FAIL — world rows ahead unknown. |
| `restart_resets_state` | ❌ FAIL — restart did not reset. |

Two distinct, honest reasons for the red, both legitimate:

1. **A real load-time crash.** The game constructor reads `localStorage` (and, in
   the as-shipped ordering, touches UI before DOM refs are assigned), so it throws
   before the game is wired up. The game genuinely never starts. This is the same
   class of defect the finding documented — now caught by execution, not by eye.
2. **The artifact never exposes the runner's state contract**
   (`__openagentsCrossyRoadState/Start/Restart`), so the five state-dependent
   checks cannot read game state and **fail honestly** rather than producing a
   false green. The runner is explicit that an artifact not exposing the contract
   "cannot be execution-verified — its checks fail honestly, never a false green."

So `verified` for this run is now **`false` (executed)** — a real `failed`, not a
fake `passed`. The runner is a genuine discriminator: its own paired regression
fixtures PASS a working game and FAIL the 4-bug one (`runner.test.ts`), and the
preserved north-star artifact is independently asserted to FAIL the executed suite
(`scripts/khala-demo/run-executed-acceptance.test.mjs`). The
`verifier_static_prescreen_not_executed` blocker is **superseded** by the executed
verdict; the manifest's `verifierRef` is now
`verifier.khala_code.executed_acceptance_suite.v1`.

## Measured metric table (OUR run only)

Generated from the recorded manifest with the metric-table emitter
(`scripts/khala-demo/emit-metric-table.mjs`). Every value the recorded run did
**not** actually carry is reported as `not_measured` — never fabricated, never a
misleading `$0.00`.

Manifest: `recorded.khala.head_to_head.crossy_road.verified_run.v1`
(evidenceMode: `fixture_scaffold`)
Verified-rate: **0** (the EXECUTED acceptance suite failed 6/6 — see above)

| lane | model | coordinator | tokens | $ | wall-clock | verified | cost/accepted-outcome | AO/kWh | in-world vs gateway | settled |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| khala | `openagents/khala-code` | heuristic_router | not_measured | not_measured | 1m 46s | failed (no) | not_applicable | not_measured | not_measured | no |

### Why each `not_measured` is honest

- **tokens / $:** the recorded run is from the issue-comment receipt narrative;
  the verified-run record we have carries the verdict + wall-clock + no-524
  evidence, but **not** a token count or a USD/`cost_msat` figure. We do not
  invent them. (Token/cost capture is a runner-side telemetry add, not a re-run:
  the streaming runner can record `usage` + `cost_msat` from a future recorded
  stream without burning a fresh paid generation.)
- **cost-per-accepted-outcome:** derived from $, so it inherits `not_measured`.
  The metric is *defined and computed* (it would equal the run cost for an
  accepted run) — we just don't have the cost input for this recorded run.
- **AO/kWh:** energy telemetry is never estimated; no measured kWh exists for
  this run.
- **in-world-vs-gateway split:** derived from `verse.inWorldWorkUnits` /
  `gatewayWorkUnits`; this run was a gateway serve with no in-world Verse units
  recorded, so the split is `not_measured` rather than asserted.
- **settled:** see M3 below — settlement is **code-complete and owner-armed**,
  but no sats moved for this recorded run (the owner real-settlement gate is OFF
  by default), so `settled: false` is the honest value.

### verified-rate

`verified-rate = accepted runs / runs with a verifier verdict = 0/1 = 0`. The
**executed** acceptance suite produced an unaccepted (`failed`) verdict for our
run, so cost-per-accepted-outcome is `not_applicable`. This is the honest number:
the earlier `1` was the static pre-screen, which we have now superseded by
actually running the game (it does not load/play). A real `0` here is the correct
outcome — `verified` now means "we ran it," and we ran it.

## The four differentiators — HONEST status

| # | differentiator | milestone | status | evidence |
| --- | --- | --- | --- | --- |
| 1 | Autopilot → Khala (coordinator composes the pool) | M1 (#6009) | ✅ | cockpit `khala-turn.ts` submits to Khala; streaming default (PR #6031) |
| 2 | Verified, not vibes (rubric → accepted outcome + receipt) | M2 (#6010) | ✅ mechanism / ❌ this run | the **execution-gated** verifier now exists and was run: the recorded run's artifact `verified:false`, `verification:failed`, `scalarReward:0` (0/6 executed checks) — an HONEST executed verdict, not the old static pre-screen. The verification *machinery* is real; this particular north-star artifact does not pass it. receipt `recorded.receipt.khala.crossy_road.executed_failed_scalar_reward_0.v1` |
| 3 | Bitcoin-settled (worker + validator paid, public receipts) | M3 (#6011 / PR #6053) | ✅ code-complete / 🟡 owner-armed | `khala-accepted-outcome-settlement.ts` fires RL-2 escrow→firm-up→Spark payout on the first executed-verified backfill; double-gated and **inert by default** (`OPENAGENTS_KHALA_LOOP_ARMED` + `OPENAGENTS_REAL_SETTLEMENT_GATE`, both OFF ⇒ no sats move) |
| 4a | Watched in the Verse | M5 (#6013) | 🟡 | crackling render fix in flight; gateway/Pylon scene exists, playback ref not yet emitted for this run |
| 4b | Playable in our world (three.js game inside our three-effect Verse) | M8 tail | ❌ | **not built** — the in-world three.js game running inside the three-effect Verse is the remaining piece |

So: differentiator **1 is in hand**; **2's verifier mechanism is now execution-gated
and real, but THIS north-star artifact fails it** (honest `verified:false`); **3 is
code-complete / owner-armed**; **4a is in flight**, **4b is not built**. The honest
read of M8 today is: we have a working **executed** verifier and a real
head-to-head data point — and that data point's artifact does **not** pass
execution. That is the correct, honest state; it does not block having the
verification machinery, but it does mean we cannot claim a *passing* verified game
for this run. The watched + playable half is pending the Verse lanes
(M4/crackling/durable-streams), which are concurrent and own those surfaces — this
doc does not touch them.

## What is needed to FULLY publish M8

The reducer's `closureAudit.canClose` / `livePromotionAudit` gate (#6016-E)
still returns blocked. To turn this measured half into a published M8:

1. **Frontier baseline run** — run the same prompt against a frontier
   OpenAI-compatible baseline lane, side by side (`frontier_live_run` check).
2. **Token + cost telemetry on the Khala run** — capture `usage` + `cost_msat`
   (+ an msat→USD rate) so tokens / $ / cost-per-accepted-outcome stop being
   `not_measured`. This is a recorded-stream/runner telemetry add, **not** a
   re-run of the paid generation.
3. **Execution-gated verifier** — ✅ **DONE (2026-06-23).** The static pre-screen
   was replaced by the `executed_acceptance_suite` runner; the north-star artifact
   was run through it (harness `scripts/khala-demo/run-executed-acceptance.mjs`) and
   produced the honest executed verdict (`verified:false`, 0/6). The
   `verifier_static_prescreen_not_executed` blocker is superseded. The remaining
   verified-work item is no longer "build the executed verifier" but "**produce an
   artifact that PASSES it**" — i.e. the coder loop must generate a game that
   actually loads and plays (and exposes the runner state contract) so the executed
   verdict is `verified:true`.
4. **Live settlement receipts** — arm `OPENAGENTS_KHALA_LOOP_ARMED` +
   `OPENAGENTS_REAL_SETTLEMENT_GATE` (owner-gated) and capture the worker +
   validator Spark settlement receipt refs (`settlement_receipts` check).
5. **Verse playback ref** — the M5 crackling/serve view emits a playback ref for
   the run (`verse_playback` check). [4a above]
6. **Playable-in-world ref** — the generated game runs inside the three-effect
   Verse and the manifest carries a `playableInWorldRef`
   (`artifact_playable_in_world` check). [4b above]
7. **Energy telemetry** — measured kWh for AO/kWh (`energy_telemetry` check).
8. **Learned conductor (optional for honesty, required for the "composition"
   story)** — M6/M7 `live_conductor` mode rather than the heuristic router
   (`m7_live_conductor` check). The current run is a valid head-to-head on the
   heuristic router; it is just not yet the learned-composition story.
9. **Publication** — flip `publication.status` to `published` with a
   `publicationRef`, after the owner-signed claim upgrade (no world-first /
   AO/kWh / broad product claim without the DE-10 pack + owner sign-off).

## Reproduce / inspect this data point (no paid inference)

Reduce the recorded manifest and read the full scoreboard + closure/promotion
audit:

```sh
bun scripts/khala-demo/reduce-head-to-head.mjs \
  docs/inference/fixtures/khala-head-to-head-recorded-run.v1.json
```

Emit just the measured metric table (Markdown, the table above):

```sh
bun scripts/khala-demo/emit-metric-table.mjs \
  docs/inference/fixtures/khala-head-to-head-recorded-run.v1.json
```

Or as JSON for downstream rendering:

```sh
bun scripts/khala-demo/emit-metric-table.mjs \
  docs/inference/fixtures/khala-head-to-head-recorded-run.v1.json --json
```

Run the focused tests (emitter + reducer + renderer):

```sh
bun test scripts/khala-demo/emit-metric-table.test.mjs \
  scripts/khala-demo/reduce-head-to-head.test.mjs \
  scripts/khala-demo/render-publication.test.mjs
```

Re-run the EXECUTED acceptance verdict against the preserved north-star artifact
(real headless chromium; no paid inference). Requires
`bunx playwright install chromium` once:

```sh
bun scripts/khala-demo/run-executed-acceptance.mjs
# prints both the AcceptanceVerdict and the khala-code verdict; exits non-zero
# because the artifact does NOT pass (verified:false, 0/6). Pass --json for JSON,
# or a path / - to run a different artifact.
```

Regression proof the verdict is execution-backed (the preserved artifact must FAIL
the executed suite):

```sh
bun test scripts/khala-demo/run-executed-acceptance.test.mjs
```

## The exact command that produced the recorded run

The verified prod run was produced by the M8 streaming runner against prod with
an agent token (owner-gated; do **not** re-run to regenerate the data point):

```sh
KHALA_BASE_URL=https://openagents.com/v1 \
KHALA_AGENT_TOKEN=<agent-token> \
KHALA_MODEL=openagents/khala-code \
FRONTIER_BASE_URL=<frontier-base-url> \
FRONTIER_TOKEN=<frontier-token> \
bun scripts/khala-demo/run-head-to-head.mjs --out /tmp/khala-h2h-live.json
```

Streaming SSE is the interactive default (the 524 fix); `--no-stream` forces the
legacy blocking transport for debug only. The recorded data point above is the
Khala lane of one such run; the frontier lane and the live token/cost/settlement/
Verse/energy fields are still to be captured per the "fully publish" list.

## References

- Issue: `github:OpenAgentsInc/openagents#6016` (M8) — EPIC `#6017`.
- Roadmap: [`docs/inference/khala-buildout-roadmap.md`](khala-buildout-roadmap.md).
- Runbook / evidence contract:
  [`docs/inference/khala-head-to-head-demo.md`](khala-head-to-head-demo.md).
- 524 fix: [`docs/inference/2026-06-22-long-running-inference-response-strategies.md`](2026-06-22-long-running-inference-response-strategies.md)
  (PR #6031).
- Verifier-must-execute finding:
  [`docs/inference/2026-06-22-verified-work-must-execute-the-artifact.md`](2026-06-22-verified-work-must-execute-the-artifact.md).
- Executed acceptance runner (driven, not edited, by this lane):
  `apps/openagents.com/workers/api/src/inference/acceptance-runner/`
  (`runner.ts`, `cli.ts`, `verdict.ts`) +
  `apps/openagents.com/workers/api/src/inference/khala-code-verifier.ts`.
- Executed-verdict harness + preserved artifact:
  [`scripts/khala-demo/run-executed-acceptance.mjs`](../../scripts/khala-demo/run-executed-acceptance.mjs),
  [`scripts/khala-demo/run-executed-acceptance.test.mjs`](../../scripts/khala-demo/run-executed-acceptance.test.mjs),
  [`scripts/khala-demo/artifacts/khala-crossy-road-northstar-run.v1.html`](../../scripts/khala-demo/artifacts/khala-crossy-road-northstar-run.v1.html).
- M3 settlement: `github:OpenAgentsInc/openagents#6011` / PR `#6053`.
