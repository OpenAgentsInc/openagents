// ATIF emitter (epic #6174): map a completed Khala QA run into a valid
// ATIF-v1.7 Agent Trajectory Interchange Format `Trajectory`.
//
// Source of truth: harbor `rfcs/0001-trajectory-format.md` (ATIF-v1.7) and the
// golden examples in `tests/golden/terminus_2/*.trajectory.json`.
//
// INPUT (the build spec): a completed Khala run = `result.json` (a public-safe
// `QaRunResult`) + `session-trace.json` (a deterministic `KhalaSessionTrace`).
// Both are ALREADY public-safe (tripwire-checked at write time): result.json
// carries the narration (step `label`), action `kind`, and `status`; the
// session-trace's `browser` beats carry the typed action + a NEUTRAL target hint
// (path/selector/label, never a secret). We correlate the two by ORDER (the
// runner emits exactly one result step per executed action, and the trace emits
// exactly one `browser` beat per executed action) to recover structured tool-call
// `arguments` without ever touching raw secrets/tokens/typed credentials.
//
// MAPPING (ATIF-v1.7):
//   step 1            -> source:"user", message = the goal
//   one step per turn -> source:"agent": narration as `message`, the decision as
//                        `reasoning_content`, the computer-use action as ONE
//                        `tool_call` (function_name = navigate/click/type/
//                        readText/waitFor/assert/screenshot, arguments = the
//                        action params), the step result/snapshot as an
//                        `observation.results[]` correlated by `source_call_id`,
//                        and `metrics` (cost_usd 0 for own-infra).
//   final agent step  -> the verdict (done/pass|fail) as a `done` tool_call.
//   final_metrics     -> aggregated (cost 0 on own infra).
//
// PUBLIC-SAFE: never embed secrets/tokens/raw provider ids; typed credentials are
// redacted at the source (the `type` action's text is never recorded by the
// runner, and we re-assert the result tripwire over the emitted trajectory).

import { PublicSafetyViolation, type QaRunResult, type QaRunStep } from "./result";
import type { KhalaSessionTrace, SessionBeat } from "./session-trace";

export const ATIF_SCHEMA_VERSION = "ATIF-v1.7";

// The result tripwire forbids any KEY matching /prompt/i, /token/i, etc. — it
// protects result.json, where such a key would mean a leaked prompt/secret. But
// ATIF MANDATES metric field names that contain those substrings (e.g.
// `prompt_tokens`, `total_prompt_tokens`, `completion_token_ids`). So the ATIF
// emitter uses an ATIF-AWARE tripwire: it forbids the same secret-bearing keys
// EXCEPT the small allowlist of ATIF-spec metric field names. This NARROWS the
// exception to exactly the spec-mandated names and still catches everything else
// (token=..., api_key=..., cookie=..., authorization=...).

/** ATIF-v1.7 metric field names that legitimately contain a forbidden substring. */
const ATIF_SAFE_KEYS: ReadonlySet<string> = new Set([
  "prompt_tokens",
  "completion_tokens",
  "cached_tokens",
  "prompt_token_ids",
  "completion_token_ids",
  "total_prompt_tokens",
  "total_completion_tokens",
  "total_cached_tokens",
]);

const ATIF_FORBIDDEN_KEY_PATTERNS: ReadonlyArray<RegExp> = [
  /token/i,
  /secret/i,
  /password/i,
  /passwd/i,
  /cookie/i,
  /authorization/i,
  /bearer/i,
  /api[-_]?key/i,
  /prompt/i,
  /credential/i,
];

/**
 * Assert an ATIF trajectory carries no secret-bearing KEYS, allowing only the
 * ATIF-spec metric field names that contain a forbidden substring. Throws
 * `PublicSafetyViolation` on the first non-allowlisted hit.
 */
export function assertAtifPublicSafe(value: unknown, path = "$"): void {
  if (value === null || typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((v, i) => assertAtifPublicSafe(v, `${path}[${i}]`));
    return;
  }
  for (const [key, v] of Object.entries(value as Record<string, unknown>)) {
    if (!ATIF_SAFE_KEYS.has(key)) {
      for (const pattern of ATIF_FORBIDDEN_KEY_PATTERNS) {
        if (pattern.test(key)) {
          throw new PublicSafetyViolation(`forbidden field "${key}" at ${path}`);
        }
      }
    }
    assertAtifPublicSafe(v, `${path}.${key}`);
  }
}

/** ATIF tool_call argument values — JSON-safe scalars/objects, never secrets. */
export type Json = string | number | boolean | null | { [k: string]: Json } | Json[];

export interface AtifToolCall {
  readonly tool_call_id: string;
  readonly function_name: string;
  readonly arguments: Record<string, Json>;
}

export interface AtifObservationResult {
  readonly source_call_id?: string;
  readonly content?: string;
}

export interface AtifObservation {
  readonly results: ReadonlyArray<AtifObservationResult>;
}

export interface AtifMetrics {
  readonly prompt_tokens?: number;
  readonly completion_tokens?: number;
  readonly cost_usd?: number;
}

export interface AtifStep {
  readonly step_id: number;
  readonly timestamp?: string;
  readonly source: "user" | "agent" | "system";
  readonly model_name?: string;
  readonly message: string;
  readonly reasoning_content?: string;
  readonly tool_calls?: ReadonlyArray<AtifToolCall>;
  readonly observation?: AtifObservation;
  readonly metrics?: AtifMetrics;
}

export interface AtifAgent {
  readonly name: string;
  readonly version: string;
  readonly model_name?: string;
  readonly extra?: Record<string, Json>;
}

export interface AtifFinalMetrics {
  readonly total_prompt_tokens?: number;
  readonly total_completion_tokens?: number;
  readonly total_cached_tokens?: number;
  readonly total_cost_usd?: number;
  readonly total_steps?: number;
  readonly extra?: Record<string, Json>;
}

export interface AtifTrajectory {
  readonly schema_version: string;
  readonly session_id?: string;
  readonly trajectory_id?: string;
  readonly agent: AtifAgent;
  readonly notes?: string;
  readonly steps: ReadonlyArray<AtifStep>;
  readonly final_metrics?: AtifFinalMetrics;
  readonly extra?: Record<string, Json>;
}

/** The QA-side verdict surfaced into the trajectory header (own vocabulary). */
export type AtifVerdict = "PASS" | "REFUTED" | "INCONCLUSIVE";

export interface MapKhalaRunInput {
  readonly result: QaRunResult;
  readonly trace: KhalaSessionTrace;
  /** Run identifier (e.g. the run dir slug). Public-safe; no secrets. */
  readonly sessionId?: string;
  /** Document identifier. Defaults to `${sessionId}-trajectory`. */
  readonly trajectoryId?: string;
  /** Agent display name in the trajectory header. */
  readonly agentName?: string;
  /** Agent version string. */
  readonly agentVersion?: string;
}

// A browser beat (the trace's per-action record). Pulled out for correlation.
type BrowserBeat = Extract<SessionBeat, { kind: "browser" }>;

function browserBeats(trace: KhalaSessionTrace): BrowserBeat[] {
  return trace.beats.filter((b): b is BrowserBeat => b.kind === "browser");
}

/**
 * The function name an ATIF tool_call uses for a result step. The runner records
 * the action `kind` directly (navigate/click/type/readText/waitFor/screenshot/
 * assert), so we pass it through; the synthetic invalid/inference-error steps map
 * to a neutral `error` tool name.
 */
function functionNameForStep(step: QaRunStep): string {
  if (step.kind === "khala") return "error";
  return step.kind;
}

/**
 * Derive PUBLIC-SAFE tool_call arguments for a step. We never carry raw secrets:
 * the typed `type` text is already withheld by the runner, so we only record the
 * neutral target hint (a path/selector/label) the trace already deemed safe, plus
 * the narration. The `target` comes from the correlated browser beat; the
 * narration (the model's stated reason) comes from the result step `label`.
 */
function argumentsForStep(step: QaRunStep, beat: BrowserBeat | undefined): Record<string, Json> {
  const args: Record<string, Json> = {};
  if (beat) {
    // The browser action verb the runner executed (navigate/click/wait/...).
    args.action = beat.action;
    // The neutral target hint (path/role/selector-as-intent) — never a secret.
    args.target = beat.targetHint;
  }
  // The model's stated reason / narration for this action (public-safe label).
  args.narration = step.label;
  return args;
}

/**
 * The observation content for a step: the neutral snapshot the runner recorded.
 * On a failed step the runner stores the failure reason in `detail`; otherwise we
 * synthesize a neutral "ok" snapshot. Never includes raw page text/secrets.
 */
function observationContent(step: QaRunStep): string {
  if (step.status === "failed") {
    const detail = step.detail ?? {};
    const reason = detail.reason ?? detail.error;
    return reason !== undefined ? `FAILED: ${String(reason)}` : `FAILED: ${step.label}`;
  }
  return `ok: ${step.label}`;
}

function verdictClass(verdict: AtifVerdict): string {
  return verdict === "PASS" ? "test_passed" : verdict === "REFUTED" ? "failed" : "none";
}

function verdictSummary(result: QaRunResult, verdict: AtifVerdict): string {
  if (verdict === "PASS") return `Verified ${result.target.name} (${result.steps.length} actions).`;
  return result.failure ?? `${verdict} on ${result.target.name}.`;
}

/** Map the run status + verdict into the trajectory header vocabulary. */
export function atifVerdict(result: QaRunResult): AtifVerdict {
  if (result.status === "pass") return "PASS";
  if (result.failure?.includes("did not reach a verdict")) return "INCONCLUSIVE";
  return "REFUTED";
}

/**
 * Map a completed Khala run into an ATIF-v1.7 `Trajectory`. Deterministic given
 * the same `result` + `trace`. The emitted trajectory is re-checked for public
 * safety (the result tripwire) before return.
 */
export function mapKhalaRunToAtif(input: MapKhalaRunInput): AtifTrajectory {
  const { result, trace } = input;
  const sessionId = input.sessionId ?? "khala-qa-run";
  const trajectoryId = input.trajectoryId ?? `${sessionId}-trajectory`;
  const agentName = input.agentName ?? "openagents-qa-runner";
  const agentVersion = input.agentVersion ?? "0.1.0";
  const model = trace.model; // "openagents/khala"

  const beats = browserBeats(trace);
  const steps: AtifStep[] = [];

  // Step 1: the user goal.
  steps.push({
    step_id: 1,
    timestamp: result.startedAt,
    source: "user",
    message: trace.goal,
  });

  // One agent step per executed action. Correlate result step i with browser
  // beat i (both are emitted one-per-executed-action, in order). The synthetic
  // khala error step (invalid action / inference error) has no browser beat.
  let beatCursor = 0;
  result.steps.forEach((step, i) => {
    const isError = step.kind === "khala";
    const beat = isError ? undefined : beats[beatCursor];
    if (!isError) beatCursor += 1;

    const stepId = i + 2; // step 1 is the user goal
    const callId = `call_${stepId}`;
    const fnName = functionNameForStep(step);
    const args = argumentsForStep(step, beat);

    steps.push({
      step_id: stepId,
      timestamp: result.startedAt,
      source: "agent",
      model_name: model,
      // Narration: the model's stated reason for this action.
      message: step.label,
      // The explicit decision the model committed to (deterministic, public-safe).
      reasoning_content:
        `Chose action "${fnName}"` +
        (beat ? ` against ${beat.targetHint}` : "") +
        `; outcome ${step.status}.`,
      tool_calls: [{ tool_call_id: callId, function_name: fnName, arguments: args }],
      observation: { results: [{ source_call_id: callId, content: observationContent(step) }] },
      // Own-infra: $0. Token counts are not recorded public-safe; omit them.
      metrics: { cost_usd: 0 },
    });
  });

  // Final agent step: the verdict, as a `done` tool_call. This makes the
  // PASS/REFUTED/INCONCLUSIVE outcome a first-class, replayable trajectory event.
  const verdict = atifVerdict(result);
  const verdictStepId = steps.length + 1;
  const verdictCallId = `call_${verdictStepId}`;
  steps.push({
    step_id: verdictStepId,
    timestamp: result.endedAt,
    source: "agent",
    model_name: model,
    message:
      verdict === "PASS"
        ? "Goal verified. Marking the session complete."
        : verdict === "REFUTED"
          ? `Goal not verified: ${result.failure ?? "an assertion failed"}.`
          : "Did not reach a verdict within the step budget.",
    reasoning_content: `Final verdict ${verdict} after ${result.steps.length} action(s).`,
    tool_calls: [
      {
        tool_call_id: verdictCallId,
        function_name: "done",
        arguments: { verdict, summary: verdictSummary(result, verdict) },
      },
    ],
    observation: {
      results: [{ source_call_id: verdictCallId, content: `verification_class=${verdictClass(verdict)}` }],
    },
    metrics: { cost_usd: 0 },
  });

  const trajectory: AtifTrajectory = {
    schema_version: ATIF_SCHEMA_VERSION,
    session_id: sessionId,
    trajectory_id: trajectoryId,
    agent: {
      name: agentName,
      version: agentVersion,
      model_name: model,
      extra: { driver: "khala-computer-use", target: trace.target.name },
    },
    notes:
      "Emitted by the OpenAgents qa-runner ATIF mapper from a real Khala " +
      "computer-use run (result.json + session-trace.json). Token counts are " +
      "withheld for public safety; own-infra cost is $0.",
    steps,
    final_metrics: {
      total_prompt_tokens: 0,
      total_completion_tokens: 0,
      total_cached_tokens: 0,
      total_cost_usd: 0,
      total_steps: steps.length,
      extra: { verdict, duration_ms: result.durationMs, trace_digest: trace.digest },
    },
    extra: {
      target: { name: trace.target.name, baseUrl: trace.target.baseUrl },
      artifacts: {
        video: result.artifacts.video ?? null,
        screenshots: [...result.artifacts.screenshots],
      },
    },
  };

  // Re-assert public-safety over the emitted document (ATIF-aware forbidden-KEYS
  // walk; the spec-mandated token-count field names are explicitly allowlisted).
  assertAtifPublicSafe(trajectory);
  return trajectory;
}

export const serializeTrajectory = (t: AtifTrajectory): string => `${JSON.stringify(t, null, 2)}\n`;
