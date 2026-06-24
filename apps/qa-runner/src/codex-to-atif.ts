// Codex rollout -> ATIF (epic #6206, issue #6220): convert an OpenAI Codex CLI
// session/rollout into a valid ATIF-v1.7 `Trajectory` we can publish as a
// `/trace/{uuid}` (and later sell as training data, see docs/traces/README.md).
//
// SOURCE FORMAT: Codex writes a per-session JSONL rollout to
//   ~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-<ts>-<session_id>.jsonl
// One JSON object per line. The event kinds we care about (confirmed against a
// live codex 0.142.0 rollout + harbor's reference exporter
// `projects/repos/harbor/src/harbor/agents/installed/codex.py`):
//   - {"type":"session_meta","payload":{"id","session_id","cli_version","cwd",
//        "originator","git","model_provider","instructions",...}}
//   - {"type":"turn_context","payload":{"model","turn_id",...}}          (model)
//   - {"type":"event_msg","payload":{"type":"task_started"|"turn_started"|
//        "task_complete"|"turn_complete"|"turn_aborted"|"token_count", ...}}
//   - {"type":"response_item","payload":{"type":"message","role":"user"|
//        "assistant"|"system"|"developer","content":[{type,text}]}}
//   - {"type":"response_item","payload":{"type":"reasoning","summary":[...]}}
//   - {"type":"response_item","payload":{"type":"function_call"|
//        "custom_tool_call","call_id","name","arguments"(JSON string)}}
//   - {"type":"response_item","payload":{"type":"function_call_output"|
//        "custom_tool_call_output","call_id","output"}}
//   - {"type":"response_item","payload":{"type":"web_search_call","action",...}}
//
// MAPPING (ATIF-v1.7), mirroring harbor's exporter so a tool_call <-> observation
// correlation matches the harbor golden shape:
//   - user prompts                -> source:"user"  (message only)
//   - system/developer messages   -> source:"system"(message only)
//   - one model API call          -> ONE source:"agent" step bundling: the
//        assistant `message`, the preceding `reasoning` as `reasoning_content`,
//        every tool/function/exec/web-search call in that API call as
//        `tool_calls[]`, and each call's output as an
//        `observation.results[]` correlated by `source_call_id == tool_call_id`.
//        API calls are bounded by `token_count` events; per-call token metrics
//        attach where present.
//   - final_metrics               -> aggregated token usage (last total).
//
// PUBLIC SAFETY: the ATIF validator forbids reasoning_content/tool_calls/metrics
// on non-agent steps and requires source_call_id correlation; we honor both. The
// caller is responsible for redaction (issue #6219) before publish — this
// converter is a faithful structural mapping of an on-disk rollout, not a
// redactor. We DO drop encrypted reasoning blobs (Codex `encrypted_content`) and
// only surface plaintext reasoning `summary` text, matching harbor.

import {
  ATIF_SCHEMA_VERSION,
  type AtifFinalMetrics,
  type AtifMetrics,
  type AtifObservationResult,
  type AtifStep,
  type AtifToolCall,
  type AtifTrajectory,
  type Json,
} from "./atif";

// The ATIF validator (atif-validate.ts `AtifMetricsSchema`) accepts a per-step
// `cached_tokens` field, but the read-only `AtifMetrics` interface in atif.ts
// (which we must not edit) does not declare it. Codex rollouts DO report
// `cached_input_tokens` per API call, so we surface it through this superset
// type and still satisfy the validator. `AtifFinalMetrics` already declares
// `total_cached_tokens`, so aggregate caching needs no superset.
export type CodexStepMetrics = AtifMetrics & { readonly cached_tokens?: number };

// ---------------------------------------------------------------------------
// Raw rollout event shapes (only the fields we read; everything else is open).
// ---------------------------------------------------------------------------

/** A single parsed line from a Codex rollout JSONL file. */
export interface CodexRolloutEvent {
  readonly type?: string;
  readonly timestamp?: string;
  readonly payload?: Record<string, unknown>;
}

function asRecord(v: unknown): Record<string, unknown> | undefined {
  return v !== null && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : undefined;
}

function asString(v: unknown): string | undefined {
  return typeof v === "string" ? v : undefined;
}

function asNumber(v: unknown): number | undefined {
  return typeof v === "number" && Number.isFinite(v) ? v : undefined;
}

/** Join the text parts of a Codex content block array. */
function extractMessageText(content: unknown): string {
  if (!Array.isArray(content)) return "";
  const parts: string[] = [];
  for (const block of content) {
    const rec = asRecord(block);
    const text = rec ? asString(rec.text) : undefined;
    if (text !== undefined) parts.push(text);
  }
  return parts.join("");
}

/** Join a Codex reasoning `summary` (array of strings or {text} objects). */
function extractReasoning(summary: unknown): string | undefined {
  if (!Array.isArray(summary) || summary.length === 0) return undefined;
  const parts: string[] = [];
  for (const item of summary) {
    if (typeof item === "string") parts.push(item);
    else {
      const rec = asRecord(item);
      const text = rec ? asString(rec.text) : undefined;
      if (text !== undefined) parts.push(text);
    }
  }
  return parts.length > 0 ? parts.join("\n") : undefined;
}

/**
 * Extract textual output from a Codex tool output blob. The output may be a raw
 * string, or a JSON string/object carrying `{output, metadata}`. We surface the
 * textual `output` (or the whole structure as JSON) so the observation content
 * is always a string. Mirrors harbor's `_parse_output_blob`.
 */
function parseOutputBlob(raw: unknown): string | undefined {
  if (raw === undefined || raw === null) return undefined;
  let parsed: unknown = raw;
  if (typeof raw === "string") {
    try {
      parsed = JSON.parse(raw);
    } catch {
      return raw;
    }
  }
  const rec = asRecord(parsed);
  if (rec) {
    const output = rec.output;
    if (typeof output === "string") return output;
    if (output !== undefined && output !== null) return JSON.stringify(output);
    return JSON.stringify(rec);
  }
  return String(parsed);
}

/** Parse a Codex tool-call `arguments` (a JSON string) into a JSON object. */
function parseToolArguments(raw: unknown): Record<string, Json> {
  if (raw === undefined || raw === null) return {};
  if (typeof raw === "string") {
    try {
      const parsed = JSON.parse(raw);
      const rec = asRecord(parsed);
      if (rec) return rec as Record<string, Json>;
      return { input: raw as Json };
    } catch {
      return { input: raw as Json };
    }
  }
  const rec = asRecord(raw);
  if (rec) return rec as Record<string, Json>;
  return { value: raw as Json };
}

// ---------------------------------------------------------------------------
// Normalized intermediate events (one per message / tool_call), grouped per
// model API call so each API call becomes exactly one agent step.
// ---------------------------------------------------------------------------

interface NormalizedMessage {
  readonly kind: "message";
  readonly apiCallId: string;
  readonly timestamp?: string;
  readonly role: "user" | "assistant" | "system" | "developer";
  readonly text: string;
  readonly reasoning?: string;
}

interface NormalizedToolCall {
  readonly kind: "tool_call";
  readonly apiCallId: string;
  readonly timestamp?: string;
  toolOrder: number;
  readonly callId: string;
  readonly toolName: string;
  readonly arguments: Record<string, Json>;
  reasoning?: string;
  status?: string;
  output?: string;
  metrics?: CodexStepMetrics;
}

type NormalizedEvent = NormalizedMessage | NormalizedToolCall;

/** Per-API-call token metrics derived from a Codex `token_count` event. */
function metricsFromTokenCount(payload: Record<string, unknown>): CodexStepMetrics | undefined {
  const info = asRecord(payload.info);
  if (!info) return undefined;
  const last = asRecord(info.last_token_usage);
  if (!last) return undefined;
  const prompt = asNumber(last.input_tokens);
  const completion = asNumber(last.output_tokens);
  const cached = asNumber(last.cached_input_tokens);
  const metrics: { prompt_tokens?: number; completion_tokens?: number; cached_tokens?: number } = {};
  if (prompt) metrics.prompt_tokens = prompt;
  if (completion) metrics.completion_tokens = completion;
  if (cached) metrics.cached_tokens = cached;
  return Object.keys(metrics).length > 0 ? metrics : undefined;
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

export interface CodexToAtifOptions {
  /** Override the session id (defaults to the rollout's session_meta id). */
  readonly sessionId?: string;
  /** Document id (defaults to `${sessionId}-trajectory`). */
  readonly trajectoryId?: string;
  /** Fallback model name when the rollout omits a turn_context model. */
  readonly modelName?: string;
}

/** Parse the raw text of a Codex rollout JSONL file into events (skips blanks). */
export function parseCodexRollout(jsonl: string): CodexRolloutEvent[] {
  const events: CodexRolloutEvent[] = [];
  for (const line of jsonl.split("\n")) {
    const stripped = line.trim();
    if (stripped.length === 0) continue;
    try {
      const parsed = JSON.parse(stripped);
      const rec = asRecord(parsed);
      if (rec) events.push(rec as CodexRolloutEvent);
    } catch {
      // Skip a malformed line rather than failing the whole rollout.
    }
  }
  return events;
}

/** Map a `role` to an ATIF step `source`. developer/system both -> system. */
function sourceForRole(role: string): "user" | "agent" | "system" {
  if (role === "assistant") return "agent";
  if (role === "user") return "user";
  return "system";
}

/**
 * Convert parsed Codex rollout events into a valid ATIF-v1.7 `Trajectory`.
 *
 * One model API call (bounded by `token_count`) becomes one `source:"agent"`
 * step that bundles the assistant message, its reasoning, and every tool call +
 * correlated observation in that call. User/system messages are their own steps
 * and are never grouped. Step ids are reassigned sequentially from 1 so the
 * output passes `atif-validate`.
 */
export function convertCodexRolloutToAtif(
  events: ReadonlyArray<CodexRolloutEvent>,
  options: CodexToAtifOptions = {},
): AtifTrajectory {
  // --- session header --------------------------------------------------------
  const sessionMeta = events.find((e) => e.type === "session_meta");
  const metaPayload = asRecord(sessionMeta?.payload) ?? {};
  const sessionId =
    options.sessionId ?? asString(metaPayload.id) ?? asString(metaPayload.session_id) ?? "codex-session";
  const trajectoryId = options.trajectoryId ?? `${sessionId}-trajectory`;
  const agentVersion = asString(metaPayload.cli_version) ?? "unknown";

  const agentExtra: Record<string, Json> = {};
  for (const key of ["originator", "cwd", "model_provider"] as const) {
    const value = asString(metaPayload[key]);
    if (value !== undefined) agentExtra[key] = value;
  }

  // First turn_context model wins; fall back to the caller-provided model.
  let modelName: string | undefined = options.modelName;
  for (const e of events) {
    if (e.type === "turn_context") {
      const m = asString(asRecord(e.payload)?.model);
      if (m !== undefined) {
        modelName = m;
        break;
      }
    }
  }

  // --- normalize events, partitioning into model API calls -------------------
  const normalized: NormalizedEvent[] = [];
  const pendingCalls = new Map<string, NormalizedToolCall>();
  const apiCallMetrics = new Map<string, AtifMetrics>();
  let pendingReasoning: string | undefined;
  let apiCallIndex = 1;
  let currentApiCallId = `api_call_${apiCallIndex}`;
  let sawModelOutput = false;
  let toolOrder = 0;

  const finishApiCall = (payload: Record<string, unknown>): void => {
    if (!sawModelOutput) return; // token_count with no model output: ignore.
    const m = metricsFromTokenCount(payload);
    if (m) apiCallMetrics.set(currentApiCallId, m);
    apiCallIndex += 1;
    currentApiCallId = `api_call_${apiCallIndex}`;
    sawModelOutput = false;
    toolOrder = 0;
  };

  for (const event of events) {
    const payload = asRecord(event.payload) ?? {};
    const timestamp = event.timestamp;

    if (event.type === "event_msg") {
      if (asString(payload.type) === "token_count") finishApiCall(payload);
      continue;
    }
    if (event.type !== "response_item") continue;

    const payloadType = asString(payload.type);

    if (payloadType === "reasoning") {
      pendingReasoning = extractReasoning(payload.summary);
      continue;
    }

    if (payloadType === "message") {
      const role = asString(payload.role) ?? "user";
      const text = extractMessageText(payload.content);
      const isAssistant = role === "assistant";
      normalized.push({
        kind: "message",
        apiCallId: currentApiCallId,
        ...(timestamp !== undefined ? { timestamp } : {}),
        role: role as NormalizedMessage["role"],
        text,
        ...(isAssistant && pendingReasoning !== undefined ? { reasoning: pendingReasoning } : {}),
      });
      if (isAssistant) sawModelOutput = true;
      pendingReasoning = undefined;
      continue;
    }

    if (payloadType === "web_search_call") {
      const action = asRecord(payload.action) ?? {};
      const args: Record<string, Json> = { action_type: (asString(action.type) ?? "") as Json };
      if (action.query !== undefined) args.query = action.query as Json;
      if (action.url !== undefined) args.url = action.url as Json;
      normalized.push({
        kind: "tool_call",
        apiCallId: currentApiCallId,
        ...(timestamp !== undefined ? { timestamp } : {}),
        toolOrder: toolOrder++,
        callId: "",
        toolName: "web_search_call",
        arguments: args,
        ...(pendingReasoning !== undefined ? { reasoning: pendingReasoning } : {}),
        ...(asString(payload.status) !== undefined ? { status: asString(payload.status)! } : {}),
      });
      sawModelOutput = true;
      pendingReasoning = undefined;
      continue;
    }

    if (payloadType === "function_call" || payloadType === "custom_tool_call") {
      const callId = asString(payload.call_id);
      if (callId === undefined) continue;
      const rawArgs = payloadType === "function_call" ? payload.arguments : payload.input;
      const call: NormalizedToolCall = {
        kind: "tool_call",
        apiCallId: currentApiCallId,
        ...(timestamp !== undefined ? { timestamp } : {}),
        toolOrder: toolOrder++,
        callId,
        toolName: asString(payload.name) ?? "",
        arguments: parseToolArguments(rawArgs),
        ...(pendingReasoning !== undefined ? { reasoning: pendingReasoning } : {}),
        ...(asString(payload.status) !== undefined ? { status: asString(payload.status)! } : {}),
      };
      pendingCalls.set(callId, call);
      sawModelOutput = true;
      pendingReasoning = undefined;
      continue;
    }

    if (payloadType === "function_call_output" || payloadType === "custom_tool_call_output") {
      const callId = asString(payload.call_id);
      const output = parseOutputBlob(payload.output);
      const existing = callId !== undefined ? pendingCalls.get(callId) : undefined;
      if (existing) {
        pendingCalls.delete(callId!);
        if (output !== undefined) existing.output = output;
        normalized.push(existing);
      } else {
        // Output with no matching call (rare): synthesize a minimal tool_call.
        normalized.push({
          kind: "tool_call",
          apiCallId: currentApiCallId,
          ...(timestamp !== undefined ? { timestamp } : {}),
          toolOrder: toolOrder++,
          callId: callId ?? "",
          toolName: asString(payload.name) ?? "",
          arguments: {},
          ...(output !== undefined ? { output } : {}),
        });
      }
      pendingReasoning = undefined;
      continue;
    }
  }

  // Any tool_call whose output never arrived: still surface it (no observation).
  for (const call of pendingCalls.values()) normalized.push(call);
  // Restore on-disk order (pending flush appended out of order).
  normalized.sort((a, b) => {
    const ta = a.timestamp ?? "";
    const tb = b.timestamp ?? "";
    if (ta !== tb) return ta < tb ? -1 : 1;
    const oa = a.kind === "tool_call" ? a.toolOrder : -1;
    const ob = b.kind === "tool_call" ? b.toolOrder : -1;
    return oa - ob;
  });

  // Attach per-API-call metrics to each normalized event.
  for (const ev of normalized) {
    const m = apiCallMetrics.get(ev.apiCallId);
    if (m && ev.kind === "tool_call") ev.metrics = m;
  }

  // --- group per API call: each assistant API call -> one bundled agent step --
  const steps: AtifStep[] = [];
  let stepId = 1;

  interface Bundle {
    apiCallId: string;
    timestamp?: string;
    messageParts: string[];
    reasoning?: string;
    toolCalls: NormalizedToolCall[];
    metrics?: CodexStepMetrics;
  }
  let bundle: Bundle | null = null;

  const flushBundle = (): void => {
    if (!bundle) return;
    const b = bundle;
    bundle = null;

    const toolCalls: AtifToolCall[] = [];
    const obsResults: AtifObservationResult[] = [];
    b.toolCalls
      .slice()
      .sort((x, y) => x.toolOrder - y.toolOrder)
      .forEach((tc, i) => {
        // Guarantee a non-empty, step-unique tool_call_id (validator + harbor
        // correlate observation.source_call_id to a tool_call_id in the step).
        const id = tc.callId !== undefined && tc.callId.length > 0 ? tc.callId : `${b.apiCallId}_call_${i}`;
        toolCalls.push({ tool_call_id: id, function_name: tc.toolName, arguments: tc.arguments });
        if (tc.output !== undefined) obsResults.push({ source_call_id: id, content: tc.output });
        if (!b.metrics && tc.metrics) b.metrics = tc.metrics;
        if (b.reasoning === undefined && tc.reasoning !== undefined) b.reasoning = tc.reasoning;
      });

    const step: {
      step_id: number;
      timestamp?: string;
      source: "agent";
      model_name?: string;
      message: string;
      reasoning_content?: string;
      tool_calls?: AtifToolCall[];
      observation?: { results: AtifObservationResult[] };
      metrics?: CodexStepMetrics;
    } = {
      step_id: stepId++,
      source: "agent",
      message: b.messageParts.filter((p) => p.length > 0).join("\n\n"),
    };
    if (b.timestamp !== undefined) step.timestamp = b.timestamp;
    if (modelName !== undefined) step.model_name = modelName;
    if (b.reasoning !== undefined) step.reasoning_content = b.reasoning;
    if (toolCalls.length > 0) step.tool_calls = toolCalls;
    if (obsResults.length > 0) step.observation = { results: obsResults };
    if (b.metrics !== undefined) step.metrics = b.metrics;
    steps.push(step as AtifStep);
  };

  for (const ev of normalized) {
    if (ev.kind === "message" && ev.role !== "assistant") {
      // Non-agent message: flush any open bundle, then its own standalone step.
      flushBundle();
      const userStep: { step_id: number; timestamp?: string; source: "user" | "system"; message: string } = {
        step_id: stepId++,
        source: sourceForRole(ev.role) === "user" ? "user" : "system",
        message: ev.text,
      };
      if (ev.timestamp !== undefined) userStep.timestamp = ev.timestamp;
      steps.push(userStep as AtifStep);
      continue;
    }

    // Assistant message or tool_call: accumulate into the per-API-call bundle.
    if (!bundle || bundle.apiCallId !== ev.apiCallId) {
      flushBundle();
      bundle = {
        apiCallId: ev.apiCallId,
        ...(ev.timestamp !== undefined ? { timestamp: ev.timestamp } : {}),
        messageParts: [],
        toolCalls: [],
      };
    }
    if (ev.kind === "message") {
      if (ev.text.length > 0) bundle.messageParts.push(ev.text);
      if (ev.reasoning !== undefined && bundle.reasoning === undefined) bundle.reasoning = ev.reasoning;
      if (ev.timestamp !== undefined) bundle.timestamp = ev.timestamp;
    } else {
      bundle.toolCalls.push(ev);
      if (ev.metrics && !bundle.metrics) bundle.metrics = ev.metrics;
    }
  }
  flushBundle();

  // --- final metrics: last token_count total --------------------------------
  let finalMetrics: AtifFinalMetrics | undefined;
  for (let i = events.length - 1; i >= 0; i--) {
    const e = events[i]!;
    if (e.type !== "event_msg") continue;
    const payload = asRecord(e.payload);
    if (!payload || asString(payload.type) !== "token_count") continue;
    const info = asRecord(payload.info);
    const total = info ? asRecord(info.total_token_usage) : undefined;
    if (!total) continue;
    const prompt = asNumber(total.input_tokens);
    const completion = asNumber(total.output_tokens);
    const cached = asNumber(total.cached_input_tokens);
    const fm: {
      total_prompt_tokens?: number;
      total_completion_tokens?: number;
      total_cached_tokens?: number;
      total_steps: number;
    } = { total_steps: steps.length };
    if (prompt) fm.total_prompt_tokens = prompt;
    if (completion) fm.total_completion_tokens = completion;
    if (cached) fm.total_cached_tokens = cached;
    finalMetrics = fm as AtifFinalMetrics;
    break;
  }
  if (!finalMetrics) finalMetrics = { total_steps: steps.length } as AtifFinalMetrics;

  const trajectory: AtifTrajectory = {
    schema_version: ATIF_SCHEMA_VERSION,
    session_id: sessionId,
    trajectory_id: trajectoryId,
    agent: {
      name: "codex",
      version: agentVersion,
      ...(modelName !== undefined ? { model_name: modelName } : {}),
      ...(Object.keys(agentExtra).length > 0 ? { extra: agentExtra } : {}),
    },
    notes:
      "Converted from an OpenAI Codex CLI rollout (~/.codex/sessions/*.jsonl) by " +
      "the OpenAgents qa-runner codex-to-atif converter (issue #6220). One model " +
      "API call maps to one agent step; redact before publish (issue #6219).",
    steps,
    final_metrics: finalMetrics,
  };

  return trajectory;
}

/** Convenience: parse a rollout's raw JSONL text and convert it to ATIF. */
export function convertCodexRolloutTextToAtif(jsonl: string, options: CodexToAtifOptions = {}): AtifTrajectory {
  return convertCodexRolloutToAtif(parseCodexRollout(jsonl), options);
}

export const serializeCodexTrajectory = (t: AtifTrajectory): string => `${JSON.stringify(t, null, 2)}\n`;

// ---------------------------------------------------------------------------
// CLI: `bun run src/codex-to-atif.ts --in <rollout.jsonl> [--out <file>] ...`
// ---------------------------------------------------------------------------

function parseArgs(argv: ReadonlyArray<string>): Record<string, string> {
  const args: Record<string, string> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a.startsWith("--")) {
      const next = argv[i + 1];
      if (next !== undefined && !next.startsWith("--")) {
        args[a.slice(2)] = next;
        i++;
      } else args[a.slice(2)] = "true";
    }
  }
  return args;
}

async function main(): Promise<void> {
  const { readFileSync, writeFileSync } = await import("node:fs");
  const { assertValidAtif } = await import("./atif-validate");
  const args = parseArgs(process.argv.slice(2));
  const inPath = args.in;
  if (!inPath) {
    console.error(
      "usage: bun run src/codex-to-atif.ts --in <rollout.jsonl> [--out <file>] " +
        "[--session-id <id>] [--model <name>]",
    );
    process.exit(2);
    return;
  }
  const jsonl = readFileSync(inPath, "utf8");
  const trajectory = convertCodexRolloutTextToAtif(jsonl, {
    ...(args["session-id"] ? { sessionId: args["session-id"] } : {}),
    ...(args.model ? { modelName: args.model } : {}),
  });

  // Tripwire: never emit an invalid trajectory.
  assertValidAtif(trajectory);

  const serialized = serializeCodexTrajectory(trajectory);
  if (args.out) {
    writeFileSync(args.out, serialized);
    console.error("=== codex -> ATIF (issue #6220) ===");
    console.error("session:   ", trajectory.session_id);
    console.error("model:     ", trajectory.agent.model_name ?? "(unknown)");
    console.error("steps:     ", trajectory.steps.length);
    console.error("out:       ", args.out);
  } else {
    process.stdout.write(serialized);
  }
}

if (import.meta.main) {
  await main();
}
