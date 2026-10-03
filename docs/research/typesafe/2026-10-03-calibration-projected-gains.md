# What calibration should buy us: projected gains

Companion to [Calibration: what TypeSafe means by it, and why we aren't really
doing it](2026-10-03-calibration.md). That doc says what is missing. This one says
what we expect to get once it is in, so each gain can be confirmed or killed by
a measurement instead of argued about.

The worked example is an authored golden:
[`crates/coderbench/goldens/calibrated-change-bottle-etag`](../../../crates/coderbench/goldens/calibrated-change-bottle-etag.meta.json)
(`.atif.jsonl` trace, `.meta.json` with provenance `authored`, `.projection.json`
with every number and its derivation). It is a **projection**, not a recording.
The baseline numbers beside it are observed rows from
`bench/efficiency/results/2026-10-03b.jsonl`.

## Where we are (observed, 2026-10-03b, 84 runs, every arm 21/21)

| Arm, vs raw Claude Code | Cost | Time |
| --- | --- | --- |
| Lean Claude session (default Claude route) | 0.45× | 0.86× |
| Routed default (Codex loop) | 0.32× | 1.83× |
| Raw Codex | 0.38× | 1.17× |

Also measured: routed start-up 2.2 s; the router's `answer` question is 13 points
overconfident raw and 6 with its map, but the map is not served; `HARD_AT` was
fitted on 7 tasks with no hard example; before #10254 the lean session spent up
to 5 extra turns re-verifying.

## The calibrated path (one small change)

1. **Route.** Jev reads `code_change` at 0.88; the served map gives 0.81. The
   threshold comes from written costs, not a hand pick: a wrong act costs about
   $0.40 (wasted run plus rework), a clarifying question about $0.10, so act when
   p > 1 − 0.10/0.40 = 0.75. 0.81 acts. One batched Jev call, ~0.6 s.
2. **Class and briefing**, concurrently. Calibrated `hard` 0.18 (with `HARD_AT`
   re-fitted on hard examples) keeps effort at medium; calibrated `needs_survey`
   0.22 skips the workspace survey, saving the measured 4–7 s.
3. **Engine choice.** Pick the lowest expected cost per checked result, counting
   time: (cost + value of time) / calibrated pass probability. For bottle-etag the
   Codex session wins ($0.072, 36 s, p 0.95) over lean Claude ($0.128, 40 s, p 0.97)
   and the Codex loop ($0.073 but 77 s). The loop's 1.83× time stops being the
   default: it is chosen only when time is worth nothing.
4. **Delegate** with a 1.8k-token briefing. **Early stop**: when calibrated
   P(frozen checks pass | agent says done) is ≥ 0.90, no extra verification turn.
5. **Independent check**, once.
6. **Recalibration record**: the outcome is joined to all five decisions and goes
   into the nightly refit.

Projected for this task: **$0.071, 39.4 s, pass p≈0.95**: 0.19× cost and
0.53× time of raw Claude Code; equal cost and 1.11× time of raw Codex; half the
time of today's routed default at the same cost.

## Projected study-wide gains vs raw Claude Code (honest ranges)

| Measure | Today | Projected | Why |
| --- | --- | --- | --- |
| Cost | 0.32–0.45× | **0.25–0.35×** | Calibrated engine choice sends small changes to the Codex session, keeps lean Claude where its calibrated pass rate is higher. |
| Time | 0.86–1.83× | **0.75–0.95×** | Drops the Codex loop as a default; removes survey, start-up and re-verification overhead (2–15 s a run). |
| Pass rate | 21/21 (easy set) | **equal; +2–5 pts on hard tasks** | `HARD_AT` fitted with hard examples sends hard tasks to higher effort instead of guessing. |
| Wrong-route rate | not recorded | **−30% to −50%** | Serving the answer map halves overconfidence (13 → 6 pts ECE). |

Versus raw Codex, the projection is roughly cost parity at 0.85–1.1× time,
with the router doing the choosing.

## Assumptions

- Calibrated per-class pass rates stay close to today's (every arm passed 21/21);
  the engine choice only pays if the cheaper engine's calibrated pass rate is
  within a few points on that class.
- The cost pair ($0.40 wrong act, $0.10 question) and the time value
  ($0.002/s) are placeholders until the owner sets them.
- The Codex session is priced from raw Codex; today's measured Codex session cost
  0.94× raw Codex (#10250), so the cost gain depends on the session matching raw.
- The answer map's offline gain holds when served.

## How we'll know

| Gain | Confirmed when | Killed when |
| --- | --- | --- |
| Calibrated route | Calibration 1 (#10385) records decisions; live ECE ≤ 0.07 and wrong routes fall ≥ 30% over 200+ routes | live ECE stays > 0.10 or wrong routes don't fall |
| Cost-derived thresholds | Calibration 2 (#10386): held-out route set shows fewer costly errors at equal escalations | errors don't fall or escalations rise > 20% |
| Engine choice | A standing study arm "routed-calibrated" lands ≤ 0.35× cost and ≤ 0.95× time with equal passes | cost > 0.40× or passes drop |
| Early stop / no survey | per-run turns and time fall by ≥ 5 s on small changes with equal passes | passes drop on any class |
| Hard tasks | Calibration 3 (#10387) adds hard tasks; pass rate on them rises with calibrated `HARD_AT` | no difference or worse |
| Recalibration loop | the nightly refit runs, `/efficiency` shows a Decisions section with n and ECE per question | refit drifts or ECE rises > 0.05 between refits |

Until those land, every projected number here is a target, and the golden says so.
