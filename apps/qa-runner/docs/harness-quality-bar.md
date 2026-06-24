# QA harness quality bar (#6193)

The differentiator for this harness is that it is the **clean, reviewed,
production-grade** one — and that the test setup itself reads as a trustworthy
spec. This doc is the checklist a reviewer (and the author) holds the harness and
every scenario to. It is short on purpose: a reviewer should be able to read it,
then read a scenario, and trust the green.

The "last 10%" Rhys called out is the failure plumbing and the review discipline.
That is exactly what the rules below pin down.

## 1. Honest outcomes — no flaky-pass, ever

- **Assert real outcomes, not tautologies.** A passing step must prove something
  about the target (a URL that did/didn't change, text that is/ isn't present, a
  condition that became true). `expect(true).toBe(true)`, asserting the fixture
  you just set, or asserting only that "the code ran" are not outcomes.
- **A failure is a failure.** A false assertion, a thrown browser error, a
  timed-out step, a restriction refusal, or an interrupted run all yield
  `status: "fail"` with an honest `failure` summary. The runner never fabricates
  a pass and never silently skips a step.
- **A retry is visible, not a cover-up.** Retries are opt-in and bounded
  (`stepPolicy.retry.maxAttempts`). A step that only passes after a retry is
  recorded with `detail.attempts > 1`, so a reviewer SEES the flake. A step that
  never passes within the bound fails — retries can only surface a flake, never
  manufacture a green.
- **REFUTED is a finding, not a red herring.** When a run declares commitments
  (#6192), a false claim is `REFUTED` with the contradicting evidence inline — a
  valid result, never a fake `CONFIRMED`. An unobserved outcome is
  `INCONCLUSIVE`, never upgraded to `CONFIRMED`.

## 2. Deterministic — no sleeps, no wall-clock races

- **Waits are conditions, never sleeps.** Use `wait-for` with a condition
  (`text-visible`, `url-includes`, `selector-visible`); never a fixed `sleep`.
  The browser surface only exposes condition waits for this reason.
- **Timeouts/retries are clock-injectable.** `timeouts.ts` races a deadline
  against an injectable `TimerLike` and runs retry delays through it, so tests
  fire deadlines explicitly with **zero real wall-clock wait** (`timeouts.test.ts`
  runs in ~10ms). Production uses the real `setTimeout`-backed timer.
- **Sharding is completion-driven and order-stable.** The bounded pool
  (`shard.ts`) returns results in INPUT order regardless of which shard finished
  first, and pulls the next item the instant a worker frees up — never a polling
  sleep. Same scenarios, same matrix, run-to-run.
- **Injectable `now()`** keeps result timestamps deterministic in tests.

## 3. Artifacts always flush — even on crash / interrupt

This is the load-bearing guarantee. A run that throws, times out, or is killed
mid-step (SIGINT) must still leave behind the evidence:

- The browser surface flushes **video + trace + screenshots** via
  `withBrowserSurface`'s `acquireUseRelease` release block (runs on throw and on
  fiber interruption).
- The runner wraps the whole session in `Effect.ensuring(..., flushResult)`, so
  **`result.json` is written from whatever steps were captured** — on success,
  on throw, and on interrupt. A partial/interrupted run is `status: "fail"` with
  an `failure` summary that says it was interrupted; it is never a fake pass.
- Proven by `runner-hardening.test.ts`: a hanging step + per-step timeout, and an
  external fiber interrupt mid-step, both leave `trace.zip`, `video.webm`, and a
  schema-valid `result.json` on disk.

## 4. Fast path — parallel beats serial

- A multi-scenario run shards across a **bounded worker pool** (`runShards`,
  concurrency cap). Each scenario provisions its own isolated backend +
  artifact dir, so shards share no mutable state.
- The speedup is asserted, not assumed: `shard.test.ts` benchmarks the SAME
  3-scenario workload serial vs. parallel in one test and requires parallel to
  beat serial by a wide, non-flaky margin.

## 5. Public-safe by construction

- `result.json` and the target matrix are walked by `assertPublicSafeResult`
  before write: no tokens, secrets, prompts, cookies, or credentials. The typed
  text of a `type` step is never recorded (only selector + length).
- Read-only targets (prod by default, #6190) refuse mutating steps with a
  recorded reason — the harness is never the thing that mutates prod.

## Reviewer checklist (per scenario)

- [ ] Every passing step asserts a real outcome about the target (no tautology).
- [ ] The deliberately-broken variant of the journey FAILS (a red is a real red).
- [ ] No `sleep`; all waits are conditions.
- [ ] If retries are enabled, `maxAttempts` is bounded and a persistent failure
      still fails the run.
- [ ] Commitments (if declared) match observed step labels; a false claim is
      REFUTED, not omitted.
- [ ] Nothing secret can reach `result.json` (no raw text/tokens recorded).

## Reviewer checklist (the harness itself)

- [ ] Tests inject fakes/manual timers — no real chromium, network, or sleeps in
      unit CI.
- [ ] Timeout + retry + continuation paths each have a test.
- [ ] An artifact-flush-on-crash AND an artifact-flush-on-interrupt test exist.
- [ ] A parallel-beats-serial benchmark exists and asserts the margin.
- [ ] `bun run typecheck` adds no new errors in `apps/qa-runner/src`.

## Where each guarantee lives

| Guarantee | Code | Test |
| --- | --- | --- |
| Per-step timeout | `timeouts.ts` `withDeadline` | `timeouts.test.ts`, `runner-hardening.test.ts` |
| Bounded opt-in retry (visible flake) | `timeouts.ts` `runStepWithPolicy` | `timeouts.test.ts`, `runner-hardening.test.ts` |
| Partial-failure continuation | `runner.ts` `driveSession` | `runner-hardening.test.ts` |
| Flush on crash / interrupt | `runner.ts` `Effect.ensuring` + `withBrowserSurface` release | `runner-hardening.test.ts` |
| Parallel sharding | `shard.ts` `runShards` | `shard.test.ts` |
| Public-safe result | `result.ts` `assertPublicSafeResult` | `public-safety.test.ts` |
