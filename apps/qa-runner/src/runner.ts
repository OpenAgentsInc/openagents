// The QA runner: execute a computer-use session and emit artifacts.
//
// Given a Target, a Brain (decision-maker), and an isolation Backend, the runner:
//   1) provisions a fresh isolated session (localBackend now; cloudVmBackend seam)
//   2) acquires a real browser inside it (the Probe computer-use browser surface,
//      with acquireUseRelease flush-on-timeout so a crash still flushes artifacts)
//   3) pumps the brain, executing each step and recording honest assertions
//   4) writes result.json + artifacts (video + trace + screenshots), all
//      public-safe (tripwire-checked before write)
//   5) tears the session down
//
// fakes-in-CI / real-for-proof: unit tests inject a fake chromium via the
// backend; the real-chromium path is exercised by run-once / demo:login.

import { mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { Effect } from "effect";
import { withBrowserSurface, type BrowserSurface } from "@openagentsinc/probe-runtime";
import type { Backend, BackendSession } from "./backend";
import type { Brain, BrainStep } from "./brain";
import { assertPublicSafeResult, type QaRunResult, type QaRunStep } from "./result";
import { checkStepAllowed, type Target } from "./target";
import { type Commitment, verifyCommitments } from "./verify";
import { runStepWithPolicy, type StepPolicy, type TimerLike, realTimer } from "./timeouts";

export interface RunInput {
  readonly target: Target;
  readonly brain: Brain;
  readonly backend: Backend;
  /** Directory artifacts (video/trace/screenshots/result.json) are written to. */
  readonly artifactDir: string;
  /** Run the browser headed (films a visible session). */
  readonly headed?: boolean;
  /** Safety cap on the number of steps. */
  readonly maxSteps?: number;
  /** Injectable clock (for deterministic result timestamps in tests). */
  readonly now?: () => Date;
  /**
   * Optional COMMITMENTS (#6192): what this run must PROVE + the evidence type.
   * Declared up front; checked at the END by the verify stage, which emits an
   * additive `verify` investigator verdict (CONFIRMED/REFUTED/INCONCLUSIVE) on
   * the result. Anti-fabrication: a false claim yields REFUTED (a valid finding,
   * never a fake pass); an unobserved outcome is INCONCLUSIVE, never CONFIRMED.
   */
  readonly commitments?: ReadonlyArray<Commitment>;

  // ── Hardening (#6193): per-step timeout / retry / continuation. All ADDITIVE
  // and OPT-IN — omitting them preserves the prior behavior exactly (no per-step
  // deadline, no retries, stop-on-first-failure). ──────────────────────────────

  /**
   * Default per-step execution policy: a hard per-step timeout and an optional
   * bounded, deterministic, opt-in retry policy. A hung step no longer hangs the
   * run; a flaky step may be retried a BOUNDED number of times. A step that only
   * passes after a retry is recorded with `attempts > 1` so the flake is VISIBLE
   * — never a silent flaky-pass. Default: no timeout, no retry.
   */
  readonly stepPolicy?: StepPolicy;
  /**
   * Per-step-kind policy overrides (e.g. give `navigate` a longer deadline than
   * `assert`). Falls back to `stepPolicy`, then to "no policy", per kind.
   */
  readonly stepPolicyByKind?: Partial<Record<BrainStep["kind"], StepPolicy>>;
  /**
   * Partial-failure continuation policy. Default `false` keeps the prior
   * capture-on-failure behavior: stop pumping at the first failure so the video
   * ends on the broken state. When `true`, the run records the failure and KEEPS
   * GOING through the remaining steps (so one assertion failure doesn't mask
   * later ones), and the overall status is still `fail` — a continued run is
   * never a fake pass. A restriction refusal (#6190) always stops the run.
   */
  readonly continueOnFailure?: boolean;
  /**
   * Injectable timer for deterministic timeouts/retry delays in tests. Defaults
   * to the real `setTimeout`-backed timer in production.
   */
  readonly timer?: TimerLike;
}

export interface RunOutcome {
  readonly result: QaRunResult;
  readonly resultPath: string;
}

async function applyAssertion(
  browser: BrowserSurface,
  step: Extract<BrainStep, { kind: "assert" }>,
): Promise<{ ok: boolean; message?: string }> {
  const check = step.check;
  switch (check.kind) {
    case "url-includes": {
      const url = await browser.page.url();
      return url.includes(check.value)
        ? { ok: true }
        : { ok: false, message: `expected url to include "${check.value}", got "${url}"` };
    }
    case "url-not-includes": {
      const url = await browser.page.url();
      return !url.includes(check.value)
        ? { ok: true }
        : { ok: false, message: `expected url NOT to include "${check.value}", got "${url}"` };
    }
    case "text-contains": {
      const text = await browser.readText(check.selector);
      return text.includes(check.value)
        ? { ok: true }
        : { ok: false, message: `expected text to contain "${check.value}"` };
    }
    case "text-not-contains": {
      const text = await browser.readText(check.selector);
      return !text.includes(check.value)
        ? { ok: true }
        : { ok: false, message: `expected text NOT to contain "${check.value}"` };
    }
  }
}

/** Mutable, shared step accumulator so the crash/interrupt finalizer can flush
 *  whatever was captured BEFORE the throw/interrupt (#6193). */
interface DriveState {
  readonly steps: QaRunStep[];
  failure?: string;
}

interface DriveConfig {
  readonly maxSteps: number;
  readonly target: Target;
  readonly stepPolicy?: StepPolicy;
  readonly stepPolicyByKind?: Partial<Record<BrainStep["kind"], StepPolicy>>;
  readonly continueOnFailure: boolean;
  readonly timer: TimerLike;
}

/**
 * Execute the session against an already-acquired browser surface, writing into
 * the shared `state` (steps + first failure). Honest: a failed assertion or a
 * thrown error yields a non-passing outcome — no fabricated success.
 *
 * Hardening (#6193): each browser-touching step runs under its per-step policy
 * (timeout + bounded deterministic opt-in retry). A step that only passes after
 * a retry is recorded with `attempts > 1` (a VISIBLE flake, never a silent
 * flaky-pass). Under `continueOnFailure`, a failed step is recorded and the pump
 * KEEPS GOING (status is still `fail`); a restriction refusal always stops.
 * Because `state` is shared, the result finalizer can flush partial steps even
 * if this is interrupted mid-step.
 */
async function driveSession(
  browser: BrowserSurface,
  brain: Brain,
  config: DriveConfig,
  state: DriveState,
): Promise<void> {
  const { maxSteps, target, continueOnFailure, timer } = config;
  const policyFor = (kind: BrainStep["kind"]): StepPolicy | undefined =>
    config.stepPolicyByKind?.[kind] ?? config.stepPolicy;

  for (let index = 0; index < maxSteps; index++) {
    let step: BrainStep | null;
    try {
      step = await brain.next({ stepIndex: index, browser });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      state.failure ??= `brain error at step ${index}: ${message}`;
      break;
    }
    if (step === null) break;
    const current = step;

    const record = (
      status: "ok" | "failed",
      label: string,
      detail?: Record<string, string | number | boolean>,
    ) => state.steps.push({ index, kind: current.kind, label, status, ...(detail ? { detail } : {}) });

    // Restriction enforcement (#6190): refuse a mutating step against a
    // read-only target BEFORE touching the browser. Honest: the refusal is
    // recorded as a failed step + a run failure (never a silent skip, never a
    // fabricated pass). A restriction refusal ALWAYS stops the run, even under
    // continueOnFailure — it is policy, not a flaky step.
    const allowed = checkStepAllowed(target, current.kind);
    if (!allowed.allowed) {
      record("failed", current.label ?? current.kind, { restriction: "read-only", reason: allowed.reason });
      state.failure ??= allowed.reason;
      break;
    }

    let stepFailed = false;
    try {
      // Run the browser action under the per-step policy (timeout + bounded,
      // deterministic, opt-in retry). `attempts > 1` is surfaced so a reviewer
      // SEES a retried flake — never a silent flaky-pass.
      const outcome = await runStepWithPolicy(
        current.label ?? current.kind,
        policyFor(current.kind),
        () => executeStep(browser, current),
        timer,
      );
      const flake = outcome.attempts > 1 ? { attempts: outcome.attempts } : undefined;
      const r = outcome.value;
      if (r.ok) record("ok", r.label, flake);
      else {
        record("failed", r.label, { ...(r.detail ?? {}), ...(flake ?? {}) });
        state.failure ??= r.failure ?? `${current.kind} failed`;
        stepFailed = true;
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      record("failed", current.label ?? current.kind, { error: message });
      state.failure ??= `${current.kind} failed: ${message}`;
      stepFailed = true;
    }

    // capture-on-failure (default): stop pumping at the first failure so the
    // video ends on the broken state. continueOnFailure: keep going through the
    // remaining steps (status stays `fail`) so one failure doesn't mask later
    // ones — a continued run is never a fake pass.
    if (stepFailed && !continueOnFailure) break;
  }
}

/** The successful/failed outcome of a single browser-touching step. */
interface StepExecResult {
  readonly ok: boolean;
  readonly label: string;
  readonly detail?: Record<string, string | number | boolean>;
  readonly failure?: string;
}

/**
 * Execute ONE brain step against the browser, returning a structured outcome
 * (ok/failed + label + honest failure summary). A thrown browser error
 * propagates to the policy wrapper (which may retry/timeout); a non-throwing
 * "failed" outcome (e.g. a wait that timed out, a false assertion) is returned
 * as `ok: false` so the drive loop records it without retrying a real red.
 */
async function executeStep(browser: BrowserSurface, step: BrainStep): Promise<StepExecResult> {
  switch (step.kind) {
    case "navigate":
      await browser.navigate(step.url);
      return { ok: true, label: step.label ?? `navigate to ${step.url}` };
    case "click":
      await browser.click(step.selector, step.label);
      return { ok: true, label: step.label ?? `click ${step.selector}` };
    case "type":
      // never record the typed text
      await browser.type(step.selector, step.text, step.label);
      return { ok: true, label: step.label ?? `type into ${step.selector}` };
    case "wait-for": {
      const met = await browser.waitFor(
        step.condition,
        step.timeoutMs !== undefined ? { timeoutMs: step.timeoutMs } : undefined,
      );
      return met
        ? { ok: true, label: step.label ?? "wait satisfied" }
        : {
            ok: false,
            label: step.label ?? "wait timed out",
            failure: `wait-for did not complete: ${JSON.stringify(step.condition)}`,
          };
    }
    case "screenshot":
      await browser.screenshot(step.label);
      return { ok: true, label: `screenshot ${step.label}` };
    case "assert": {
      const outcome = await applyAssertion(browser, step);
      return outcome.ok
        ? { ok: true, label: step.label }
        : {
            ok: false,
            label: step.label,
            detail: { reason: outcome.message ?? "assertion failed" },
            failure: `${step.label}: ${outcome.message ?? "assertion failed"}`,
          };
    }
  }
}

/**
 * Build the public-safe result from whatever the session ACTUALLY produced.
 * Pure (no I/O except reading the artifact dir for the screenshot list). The
 * crash/interrupt finalizer and the happy path both call this, so a partial run
 * still produces a complete, honest result (status `fail`, the captured steps,
 * and an `interrupted` failure summary when applicable).
 */
function buildResult(
  input: RunInput,
  state: DriveState,
  startedAt: Date,
  endedAt: Date,
  pwArtifacts: { videoPath?: string; videoFormat?: "mp4" | "webm"; tracePath?: string } | undefined,
): QaRunResult {
  const screenshots = (() => {
    try {
      return readdirSync(input.artifactDir)
        .filter((f: string) => f.endsWith(".png"))
        .sort();
    } catch {
      return [] as string[];
    }
  })();

  const status: "pass" | "fail" = state.failure === undefined ? "pass" : "fail";

  // Verify stage (#6192): if the run declared commitments, check the PRODUCED
  // steps/status against them and emit the investigator verdict. Anti-
  // fabrication lives in `verifyCommitments`: it can only CONFIRM observed
  // evidence; a contradiction is REFUTED; an unobserved outcome is
  // INCONCLUSIVE. This runs on whatever the session actually produced — it
  // never restages evidence to match an expected outcome.
  const verify =
    input.commitments !== undefined && input.commitments.length > 0
      ? verifyCommitments({
          commitments: input.commitments,
          steps: state.steps,
          runStatus: status,
        })
      : undefined;

  const result: QaRunResult = {
    schemaVersion: "openagents.qa_runner.result.v1",
    status,
    target: { name: input.target.name, baseUrl: input.target.baseUrl },
    brain: input.brain.name,
    backend: input.backend.name,
    startedAt: startedAt.toISOString(),
    endedAt: endedAt.toISOString(),
    durationMs: endedAt.getTime() - startedAt.getTime(),
    steps: state.steps,
    artifacts: {
      ...(pwArtifacts?.videoPath ? { video: relative(input.artifactDir, pwArtifacts.videoPath) } : {}),
      ...(pwArtifacts?.videoFormat ? { videoFormat: pwArtifacts.videoFormat } : {}),
      ...(pwArtifacts?.tracePath ? { trace: relative(input.artifactDir, pwArtifacts.tracePath) } : {}),
      screenshots,
    },
    ...(state.failure ? { failure: state.failure } : {}),
    ...(verify !== undefined
      ? {
          verify: {
            verdict: verify.verdict,
            findings: verify.findings.map((f) => ({
              id: f.id,
              claim: f.claim,
              verdict: f.verdict,
              evidenceSummary: f.evidenceSummary,
            })),
            observed: verify.observed,
          },
        }
      : {}),
  };

  // Tripwire: never write a result that leaks a forbidden field.
  assertPublicSafeResult(result);
  return result;
}

/** Persist the result to result.json. Idempotent: safe to call once per run. */
function persistResult(input: RunInput, result: QaRunResult): string {
  const resultPath = join(input.artifactDir, "result.json");
  writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);
  return resultPath;
}

export function runQaSession(input: RunInput): Effect.Effect<RunOutcome, Error> {
  const now = input.now ?? (() => new Date());
  const maxSteps = input.maxSteps ?? 50;
  const timer = input.timer ?? realTimer;
  return Effect.gen(function* () {
    mkdirSync(input.artifactDir, { recursive: true });
    const startedAt = now();

    // Shared, mutable drive state so the crash/interrupt finalizer can flush
    // whatever was captured BEFORE a throw/interrupt (#6193).
    const state: DriveState = { steps: [] };
    let acquired: Awaited<ReturnType<BackendSession["acquireBrowser"]>> | undefined;
    let persisted: { result: QaRunResult; resultPath: string } | undefined;

    // Always-flush finalizer: build + persist result.json from whatever the
    // session produced, on success, throw, OR interruption (SIGINT / timeout).
    // This is the artifact-flush-on-crash guarantee: the browser surface already
    // flushes video/trace/screenshots via withBrowserSurface's release; this
    // finalizer guarantees result.json is ALSO written. Best-effort: it must
    // never mask the primary error, and it never fabricates a pass (a partial
    // run has status `fail` and an honest failure summary).
    const flushResult = Effect.sync(() => {
      if (persisted !== undefined) return; // already written on the happy path
      state.failure ??= "run interrupted before completion (artifacts flushed)";
      try {
        const result = buildResult(input, state, startedAt, now(), acquired?.artifacts());
        persistResult(input, result);
      } catch {
        // never let the finalizer throw and mask the real interrupt/error
      }
    });

    const body = Effect.gen(function* () {
      const session = yield* Effect.tryPromise({
        try: () =>
          input.backend.provision({
            target: input.target,
            artifactDir: input.artifactDir,
            ...(input.headed !== undefined ? { headed: input.headed } : {}),
          }),
        catch: (e) => (e instanceof Error ? e : new Error(String(e))),
      });

      yield* withBrowserSurface(
        async () => {
          acquired = await session.acquireBrowser();
          return acquired;
        },
        { artifactDir: input.artifactDir },
        (browser) =>
          Effect.promise(() =>
            driveSession(
              browser,
              input.brain,
              {
                maxSteps,
                target: input.target,
                ...(input.stepPolicy !== undefined ? { stepPolicy: input.stepPolicy } : {}),
                ...(input.stepPolicyByKind !== undefined ? { stepPolicyByKind: input.stepPolicyByKind } : {}),
                continueOnFailure: input.continueOnFailure ?? false,
                timer,
              },
              state,
            ),
          ),
      );

      yield* Effect.ignore(
        Effect.tryPromise({
          try: () => session.teardown(),
          catch: (e) => (e instanceof Error ? e : new Error(String(e))),
        }),
      );

      const result = buildResult(input, state, startedAt, now(), acquired?.artifacts());
      const resultPath = persistResult(input, result);
      persisted = { result, resultPath };
      return { result, resultPath } satisfies RunOutcome;
    });

    // `ensuring` runs the finalizer on success, failure, AND interruption.
    return yield* Effect.ensuring(body, flushResult);
  });
}
