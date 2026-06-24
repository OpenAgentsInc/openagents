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
import type { Backend } from "./backend";
import type { Brain, BrainStep } from "./brain";
import { assertPublicSafeResult, type QaRunResult, type QaRunStep } from "./result";
import type { Target } from "./target";
import { type Commitment, verifyCommitments } from "./verify";

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

/**
 * Execute the session against an already-acquired browser surface. Returns the
 * step records and an optional failure summary (first failed assertion / error).
 * Honest: a failed assertion or a thrown error yields a non-passing outcome — no
 * fabricated success.
 */
async function driveSession(
  browser: BrowserSurface,
  brain: Brain,
  maxSteps: number,
): Promise<{ steps: QaRunStep[]; failure?: string }> {
  const steps: QaRunStep[] = [];
  let failure: string | undefined;
  for (let index = 0; index < maxSteps; index++) {
    let step: BrainStep | null;
    try {
      step = await brain.next({ stepIndex: index, browser });
    } catch (error) {
      failure = `brain error at step ${index}: ${error instanceof Error ? error.message : String(error)}`;
      break;
    }
    if (step === null) break;

    const record = (status: "ok" | "failed", label: string, detail?: Record<string, string | number | boolean>) =>
      steps.push({ index, kind: step!.kind, label, status, ...(detail ? { detail } : {}) });

    try {
      switch (step.kind) {
        case "navigate":
          await browser.navigate(step.url);
          record("ok", step.label ?? `navigate to ${step.url}`);
          break;
        case "click":
          await browser.click(step.selector, step.label);
          record("ok", step.label ?? `click ${step.selector}`);
          break;
        case "type":
          await browser.type(step.selector, step.text, step.label);
          // never record the typed text
          record("ok", step.label ?? `type into ${step.selector}`);
          break;
        case "wait-for": {
          const met = await browser.waitFor(
            step.condition,
            step.timeoutMs !== undefined ? { timeoutMs: step.timeoutMs } : undefined,
          );
          if (met) record("ok", step.label ?? "wait satisfied");
          else {
            record("failed", step.label ?? "wait timed out");
            failure = `wait-for did not complete: ${JSON.stringify(step.condition)}`;
          }
          break;
        }
        case "screenshot":
          await browser.screenshot(step.label);
          record("ok", `screenshot ${step.label}`);
          break;
        case "assert": {
          const outcome = await applyAssertion(browser, step);
          if (outcome.ok) record("ok", step.label);
          else {
            record("failed", step.label, { reason: outcome.message ?? "assertion failed" });
            failure = `${step.label}: ${outcome.message ?? "assertion failed"}`;
          }
          break;
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      record("failed", step.label ?? step.kind, { error: message });
      failure = `${step.kind} failed: ${message}`;
    }
    // capture-on-failure: stop pumping once we have an honest failure so the
    // video ends on the broken state (failure visible in the recording).
    if (failure) break;
  }
  return failure !== undefined ? { steps, failure } : { steps };
}

export function runQaSession(input: RunInput): Effect.Effect<RunOutcome, Error> {
  const now = input.now ?? (() => new Date());
  const maxSteps = input.maxSteps ?? 50;
  return Effect.gen(function* () {
    mkdirSync(input.artifactDir, { recursive: true });
    const startedAt = now();

    const session = yield* Effect.tryPromise({
      try: () =>
        input.backend.provision({
          target: input.target,
          artifactDir: input.artifactDir,
          ...(input.headed !== undefined ? { headed: input.headed } : {}),
        }),
      catch: (e) => (e instanceof Error ? e : new Error(String(e))),
    });

    let acquired: Awaited<ReturnType<typeof session.acquireBrowser>> | undefined;
    const driveResult = yield* withBrowserSurface(
      async () => {
        acquired = await session.acquireBrowser();
        return acquired;
      },
      { artifactDir: input.artifactDir },
      (browser) => Effect.promise(() => driveSession(browser, input.brain, maxSteps)),
    );

    yield* Effect.ignore(
      Effect.tryPromise({
        try: () => session.teardown(),
        catch: (e) => (e instanceof Error ? e : new Error(String(e))),
      }),
    );

    const endedAt = now();
    const pwArtifacts = acquired?.artifacts();
    const screenshots = readdirSync(input.artifactDir)
      .filter((f: string) => f.endsWith(".png"))
      .sort();

    const status: "pass" | "fail" = driveResult.failure === undefined ? "pass" : "fail";

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
            steps: driveResult.steps,
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
      steps: driveResult.steps,
      artifacts: {
        ...(pwArtifacts?.videoPath ? { video: relative(input.artifactDir, pwArtifacts.videoPath) } : {}),
        ...(pwArtifacts?.videoFormat ? { videoFormat: pwArtifacts.videoFormat } : {}),
        ...(pwArtifacts?.tracePath ? { trace: relative(input.artifactDir, pwArtifacts.tracePath) } : {}),
        screenshots,
      },
      ...(driveResult.failure ? { failure: driveResult.failure } : {}),
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

    const resultPath = join(input.artifactDir, "result.json");
    writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);
    return { result, resultPath };
  });
}
