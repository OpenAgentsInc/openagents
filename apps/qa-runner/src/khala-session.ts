// The Khala session runner: Khala AUTONOMOUSLY drives the computer-use surface.
//
// Unlike `runQaSession` (which pumps a fixed BrainStep list), this runner pumps
// the live Khala driver: each turn the model chooses ONE action, the runner
// executes it against the #6175 BrowserSurface (navigate/click/type/readText/
// waitFor/screenshot/assert) — plus an optional terminal — and feeds a neutral
// observation back to the model. It records BOTH:
//   - a public-safe `QaRunResult` (the playable receipt), and
//   - a deterministic `KhalaSessionTrace` (the distiller's input).
//
// Honesty: an assert that fails, a wait that times out, an action error, or an
// unparseable model reply all produce a real failure — never a fabricated pass.
// A model that never reaches a verdict (step cap) is reported as "incomplete".

import { mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { Effect } from "effect";
import { withBrowserSurface, type BrowserSurface } from "@openagentsinc/probe-runtime";
import type { Backend } from "./backend";
import type { ChatClient, KhalaExecutableAction } from "./khala-driver";
import { makeKhalaDriver, type KhalaTurnRecord } from "./khala-driver";
import type { KhalaAction } from "./khala-action";
import { KhalaActionParseError } from "./khala-action";
import { assertPublicSafeResult, type QaRunResult, type QaRunStep } from "./result";
import {
  assertSessionTracePublicSafe,
  makeSessionTrace,
  shortHash,
  type KhalaSessionTrace,
  type SessionBeat,
  type TypedField,
} from "./session-trace";
import type { Target } from "./target";

export interface KhalaSessionInput {
  readonly target: Target;
  readonly backend: Backend;
  readonly chat: ChatClient;
  readonly goal: string;
  /** Directory artifacts (video/trace/screenshots/result.json/trace.json) go to. */
  readonly artifactDir: string;
  readonly headed?: boolean;
  /** Hard cap on model turns; defaults to 16. */
  readonly maxTurns?: number;
  /** Bounded corrective re-prompts on an invalid action (default 1). */
  readonly reparseAttempts?: number;
  /** Model label for the result/trace (e.g. "openagents/khala"). */
  readonly model?: string;
  /** Log sink (defaults to console.log). */
  readonly log?: (line: string) => void;
  readonly now?: () => Date;
}

export interface KhalaSessionOutcome {
  readonly result: QaRunResult;
  readonly resultPath: string;
  readonly trace: KhalaSessionTrace;
  readonly tracePath: string;
  readonly verdict: "pass" | "fail" | "incomplete";
}

const EFFECT_FOR_TOOL: Record<string, "read" | "mutate"> = {
  navigate: "read",
  click: "mutate",
  type: "mutate",
  readText: "read",
  waitFor: "read",
  screenshot: "read",
  assert: "read",
};

/** Execute one model action against the browser; return a neutral observation + status. */
async function executeAction(
  browser: BrowserSurface,
  action: KhalaExecutableAction,
): Promise<{ observation: string; status: "ok" | "failed"; browserAction: SessionBeat & { kind: "browser" } }> {
  const mk = (
    bAction: (SessionBeat & { kind: "browser" })["action"],
    targetHint: string,
    status: "ok" | "failed",
    observation: string,
  ) => ({ observation, status, browserAction: { kind: "browser" as const, action: bAction, targetHint, status } });

  switch (action.action) {
    case "navigate": {
      await browser.navigate(action.url);
      const url = await browser.page.url();
      return mk("navigate", action.url, "ok", `navigated; current url is ${url}`);
    }
    case "click": {
      await browser.click(action.selector);
      return mk("click", action.selector, "ok", `clicked ${action.selector}`);
    }
    case "type": {
      // The typed text is NEVER echoed back to the model or recorded.
      await browser.type(action.selector, action.text);
      return mk("type", action.selector, "ok", `typed ${action.text.length} chars into ${action.selector}`);
    }
    case "readText": {
      const text = await browser.readText(action.selector);
      // Keep observations small so the model's limited token budget is spent on
      // deciding the next action, not echoing a whole page.
      const trimmed = text.length > 600 ? `${text.slice(0, 600)}…` : text;
      return mk("readText", action.selector ?? "(body)", "ok", `page text:\n${trimmed}`);
    }
    case "waitFor": {
      const met = await browser.waitFor(
        action.condition,
        action.timeoutMs !== undefined ? { timeoutMs: action.timeoutMs } : undefined,
      );
      const hint = "value" in action.condition ? action.condition.value : action.condition.selector;
      return met
        ? mk("wait", `${action.condition.kind}:${hint}`, "ok", `condition met: ${JSON.stringify(action.condition)}`)
        : mk("wait", `${action.condition.kind}:${hint}`, "failed", `condition NOT met (timeout): ${JSON.stringify(action.condition)}`);
    }
    case "screenshot": {
      await browser.screenshot(action.label);
      return mk("screenshot", action.label, "ok", `screenshot taken: ${action.label}`);
    }
    case "assert": {
      const outcome = await applyAssert(browser, action.check);
      return outcome.ok
        ? mk("assert", action.label, "ok", `assertion PASSED: ${action.label}`)
        : mk("assert", action.label, "failed", `assertion FAILED: ${action.label} (${outcome.message})`);
    }
    case "terminal_run": {
      // The browser-only session does not arm a terminal; report honestly so the
      // model adapts (it must not assume a terminal succeeded).
      return mk(
        "readText",
        `terminal:${action.command}`,
        "failed",
        `terminal_run is not available in this browser-only session (command "${action.command}" not executed)`,
      );
    }
  }
}

async function applyAssert(
  browser: BrowserSurface,
  check: Extract<KhalaAction, { action: "assert" }>["check"],
): Promise<{ ok: boolean; message?: string }> {
  switch (check.kind) {
    case "url-includes": {
      const url = await browser.page.url();
      return url.includes(check.value) ? { ok: true } : { ok: false, message: `url "${url}" lacks "${check.value}"` };
    }
    case "url-not-includes": {
      const url = await browser.page.url();
      return !url.includes(check.value) ? { ok: true } : { ok: false, message: `url "${url}" contains "${check.value}"` };
    }
    case "text-contains": {
      const text = await browser.readText(check.selector);
      return text.includes(check.value) ? { ok: true } : { ok: false, message: `text lacks "${check.value}"` };
    }
    case "text-not-contains": {
      const text = await browser.readText(check.selector);
      return !text.includes(check.value) ? { ok: true } : { ok: false, message: `text contains "${check.value}"` };
    }
  }
}

/** Infer typed inputs/outputs from the goal + transcript (deterministic, simple). */
function inferTypedFields(goal: string, records: ReadonlyArray<KhalaTurnRecord>): {
  inputs: TypedField[];
  outputs: TypedField[];
} {
  const inputs: TypedField[] = [{ name: "target", type: "Target", description: "the deployment under test" }];
  if (/login|sign[\s-]?in/i.test(goal)) {
    inputs.push({ name: "path", type: "string", description: "the route to verify" });
  }
  // Outputs are the assertions the model made (their labels), typed as boolean.
  const outputs: TypedField[] = records
    .filter((r) => r.action.action === "assert")
    .map((r) => ({
      name: (r.action as Extract<KhalaAction, { action: "assert" }>).label,
      type: "boolean",
      description: "an outcome asserted by the session",
    }));
  if (outputs.length === 0) {
    outputs.push({ name: "verified", type: "boolean", description: "whether the goal was verified" });
  }
  return { inputs, outputs };
}

/** Lower the transcript into ordered session-trace beats (secrets withheld). */
function transcriptToBeats(
  records: ReadonlyArray<KhalaTurnRecord>,
  verdict: "pass" | "fail" | "incomplete",
): SessionBeat[] {
  const beats: SessionBeat[] = [];
  for (const record of records) {
    const action = record.action;
    if (action.action === "done" || action.action === "fail") continue;
    // a chat turn (the model's decision) — content withheld via a ref.
    beats.push({ kind: "chat_turn", role: "assistant", contentRef: `sha256:${shortHash(JSON.stringify(action))}` });
    if (action.action === "terminal_run") {
      beats.push({ kind: "terminal", commandHash: `sha256:${shortHash(action.command)}`, outcome: "fail" });
      continue;
    }
    const eff = EFFECT_FOR_TOOL[action.action] ?? "read";
    beats.push({ kind: "tool_call", tool: action.action, argsHash: `sha256:${shortHash(JSON.stringify(action))}`, effect: eff });
    const status: "ok" | "failed" = record.observation?.includes("FAILED") || record.observation?.includes("NOT met") ? "failed" : "ok";
    const bAction = browserActionFor(action.action);
    const hint = targetHintFor(action);
    beats.push({ kind: "browser", action: bAction, targetHint: hint, status });
  }
  const vClass = verdict === "pass" ? "test_passed" : verdict === "fail" ? "failed" : "none";
  beats.push({ kind: "verdict", verificationClass: vClass });
  return beats;
}

function browserActionFor(actionKind: string): (SessionBeat & { kind: "browser" })["action"] {
  switch (actionKind) {
    case "navigate":
      return "navigate";
    case "click":
      return "click";
    case "type":
      return "type";
    case "waitFor":
      return "wait";
    case "screenshot":
      return "screenshot";
    case "assert":
      return "assert";
    default:
      return "readText";
  }
}

function targetHintFor(action: Exclude<KhalaAction, { action: "done" } | { action: "fail" }>): string {
  switch (action.action) {
    case "navigate":
      return action.url;
    case "click":
    case "type":
      return action.selector;
    case "readText":
      return action.selector ?? "(body)";
    case "waitFor":
      return "value" in action.condition ? `${action.condition.kind}:${action.condition.value}` : `${action.condition.kind}:${action.condition.selector}`;
    case "screenshot":
      return action.label;
    case "assert":
      return action.label;
    case "terminal_run":
      return `terminal:${action.command}`;
  }
}

/**
 * Run an autonomous Khala-driven QA session. Returns the result + trace + the
 * declared verdict. Emits result.json + session-trace.json + the browser
 * artifacts (video/trace/screenshots). All public-safe (tripwire-checked).
 */
export function runKhalaSession(input: KhalaSessionInput): Effect.Effect<KhalaSessionOutcome, Error> {
  const now = input.now ?? (() => new Date());
  const model = input.model ?? "openagents/khala";
  const log = input.log ?? ((line: string) => console.log(line));
  const driver = makeKhalaDriver({
    goal: input.goal,
    chat: input.chat,
    ...(input.maxTurns !== undefined ? { maxTurns: input.maxTurns } : {}),
    ...(input.reparseAttempts !== undefined ? { reparseAttempts: input.reparseAttempts } : {}),
    log,
  });

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
    const drive = yield* withBrowserSurface(
      async () => {
        acquired = await session.acquireBrowser();
        return acquired;
      },
      { artifactDir: input.artifactDir },
      (browser) =>
        Effect.promise(async () => {
          const steps: QaRunStep[] = [];
          let failure: string | undefined;
          let index = 0;
          for (;;) {
            let action: KhalaExecutableAction | null;
            try {
              action = await driver.nextAction();
            } catch (error) {
              if (error instanceof KhalaActionParseError) {
                failure = `khala emitted an unparseable/invalid action: ${error.message}`;
                steps.push({ index, kind: "khala", label: "model produced invalid action", status: "failed", detail: { reason: error.message } });
                break;
              }
              failure = `khala inference error: ${error instanceof Error ? error.message : String(error)}`;
              steps.push({ index, kind: "khala", label: "inference error", status: "failed", detail: { error: failure } });
              break;
            }
            if (action === null) break; // done / fail / cap

            try {
              const { observation, status, browserAction } = await executeAction(browser, action);
              const label = "reason" in action && action.reason ? action.reason : describeForStep(action);
              steps.push({ index, kind: action.action, label, status, ...(status === "failed" ? { detail: { reason: observation } } : {}) });
              driver.recordObservation(observation);
              if (status === "failed" && (browserAction.action === "assert" || browserAction.action === "wait")) {
                failure = observation;
                // Let the model see the failure and decide; do not hard-stop —
                // but cap is enforced by the driver. The recorded failure stands.
              }
            } catch (error) {
              const message = error instanceof Error ? error.message : String(error);
              steps.push({ index, kind: action.action, label: describeForStep(action), status: "failed", detail: { error: message } });
              driver.recordObservation(`action error: ${message}`);
              failure = `${action.action} failed: ${message}`;
            }
            index += 1;
          }
          return { steps, failure };
        }),
    );

    yield* Effect.ignore(
      Effect.tryPromise({
        try: () => session.teardown(),
        catch: (e) => (e instanceof Error ? e : new Error(String(e))),
      }),
    );

    const endedAt = now();
    const verdict = driver.finalVerdict();
    const records = driver.transcript();
    const pwArtifacts = acquired?.artifacts();
    const screenshots = readdirSync(input.artifactDir).filter((f) => f.endsWith(".png")).sort();

    // Honest status: a pass verdict with no recorded failure is a pass.
    const passed = verdict === "pass" && drive.failure === undefined;
    const status: "pass" | "fail" = passed ? "pass" : "fail";
    const failure =
      drive.failure ??
      (verdict === "incomplete"
        ? "khala did not reach a verdict within the step cap"
        : verdict === "fail"
          ? "khala declared the flow failed"
          : undefined);

    const result: QaRunResult = {
      schemaVersion: "openagents.qa_runner.result.v1",
      status,
      target: { name: input.target.name, baseUrl: input.target.baseUrl },
      brain: "khala",
      backend: input.backend.name,
      startedAt: startedAt.toISOString(),
      endedAt: endedAt.toISOString(),
      durationMs: endedAt.getTime() - startedAt.getTime(),
      steps: drive.steps,
      artifacts: {
        ...(pwArtifacts?.videoPath ? { video: relative(input.artifactDir, pwArtifacts.videoPath) } : {}),
        ...(pwArtifacts?.videoFormat ? { videoFormat: pwArtifacts.videoFormat } : {}),
        ...(pwArtifacts?.tracePath ? { trace: relative(input.artifactDir, pwArtifacts.tracePath) } : {}),
        screenshots,
      },
      ...(failure ? { failure } : {}),
    };
    assertPublicSafeResult(result);
    const resultPath = join(input.artifactDir, "result.json");
    writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);

    // Build + write the deterministic, public-safe session trace.
    const { inputs, outputs } = inferTypedFields(input.goal, records);
    const trace = makeSessionTrace({
      goal: input.goal,
      target: { name: input.target.name, baseUrl: input.target.baseUrl },
      model,
      beats: transcriptToBeats(records, verdict),
      inputs,
      outputs,
      receipts: [`result:${relative(input.artifactDir, resultPath)}`],
    });
    assertSessionTracePublicSafe(trace);
    const tracePath = join(input.artifactDir, "session-trace.json");
    writeFileSync(tracePath, `${JSON.stringify(trace, null, 2)}\n`);

    return { result, resultPath, trace, tracePath, verdict };
  });
}

function describeForStep(action: Exclude<KhalaAction, { action: "done" } | { action: "fail" }>): string {
  switch (action.action) {
    case "navigate":
      return `navigate to ${action.url}`;
    case "click":
      return `click ${action.selector}`;
    case "type":
      return `type into ${action.selector}`;
    case "readText":
      return `read text ${action.selector ?? "(body)"}`;
    case "waitFor":
      return `wait for ${JSON.stringify(action.condition)}`;
    case "screenshot":
      return `screenshot ${action.label}`;
    case "assert":
      return action.label;
    case "terminal_run":
      return `terminal_run ${action.command}`;
  }
}
