// Claude Code session (.jsonl) -> ATIF-v1.7 Trajectory (issue #6220, near-term).
//
// Converts a Claude Code session log — one JSON object per line: `user`,
// `assistant` (with text / thinking / tool_use content blocks), and `user`
// events carrying `tool_result` blocks — into a valid ATIF-v1.7 `Trajectory`
// that `atif-validate.ts` accepts and `atif-html.ts` renders.
//
// Mapping (mirrors the harbor exporter
// `projects/repos/harbor/src/harbor/agents/installed/claude_code.py`):
//   - dedup events by `uuid`; sort by `timestamp`; sidechain events first.
//   - one ASSISTANT `message.id` -> ONE agent step. Its content blocks become:
//       text blocks      -> the step `message`
//       thinking blocks  -> `reasoning_content`
//       tool_use blocks  -> `tool_calls[]` (tool_call_id = block.id,
//                           function_name = block.name, arguments = block.input)
//     usage on the message -> `metrics` (prompt = input+cache_read+cache_creation,
//     completion = output_tokens).
//   - a USER string / text-block message -> a `source:"user"` step.
//   - a USER `tool_result` block -> an `observation.results[]` entry on the SAME
//     agent step that issued the correlated tool_use (matched by tool_use_id).
//     Orphan tool_results (no matching pending call) become their own agent step
//     carrying just the tool_call + observation, so nothing is lost.
//   - step_ids are renumbered sequentially from 1 (ATIF invariant). The ATIF
//     validator forbids tool_calls/reasoning/metrics on non-agent steps, so the
//     user goal step carries only a `message`.
//
// NOTE ON SAFETY: this converter does NOT redact. The raw Claude Code log is
// full of secrets/paths/PII. Callers MUST run the resulting trajectory through
// `TraceRedactor` (redaction.ts, #6219) before persisting or publishing it.

import {
  ATIF_SCHEMA_VERSION,
  type AtifFinalMetrics,
  type AtifMetrics,
  type AtifStep,
  type AtifToolCall,
  type AtifTrajectory,
  type Json,
} from "./atif";

// --- Raw Claude Code event shapes (only the fields we read) -----------------

interface CcUsage {
  readonly input_tokens?: number;
  readonly output_tokens?: number;
  readonly cache_read_input_tokens?: number;
  readonly cache_creation_input_tokens?: number;
  readonly [k: string]: unknown;
}

interface CcContentBlock {
  readonly type?: string;
  readonly text?: unknown;
  readonly thinking?: unknown;
  readonly id?: string;
  readonly tool_use_id?: string;
  readonly name?: string;
  readonly input?: unknown;
  readonly content?: unknown;
  readonly is_error?: unknown;
  readonly [k: string]: unknown;
}

interface CcMessage {
  readonly id?: string;
  readonly role?: string;
  readonly model?: string;
  readonly content?: unknown;
  readonly usage?: CcUsage;
  readonly [k: string]: unknown;
}

export interface CcEvent {
  readonly uuid?: string;
  readonly type?: string;
  readonly timestamp?: string;
  readonly isSidechain?: boolean;
  readonly sessionId?: string;
  readonly version?: string;
  readonly cwd?: string;
  readonly gitBranch?: string;
  readonly message?: CcMessage;
  readonly toolUseResult?: unknown;
  readonly [k: string]: unknown;
}

export interface ConvertOptions {
  /** Session id for the trajectory header. Defaults to the log's `sessionId`. */
  readonly sessionId?: string;
  /** Trajectory document id. Defaults to `${sessionId}-trajectory`. */
  readonly trajectoryId?: string;
  /** Agent display name. */
  readonly agentName?: string;
  /** Fallback model id when an event omits one (e.g. "openagents/khala"). */
  readonly defaultModelName?: string;
  /** Drop sidechain (subagent) events entirely. Default false (keep them). */
  readonly dropSidechains?: boolean;
}

// --- Helpers ----------------------------------------------------------------

function stringify(value: unknown): string {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

/** Pull text / reasoning / tool_use blocks out of an assistant content value. */
function extractAssistant(content: unknown): {
  text: string;
  reasoning: string | undefined;
  toolUses: CcContentBlock[];
} {
  if (typeof content === "string") {
    return { text: content.trim(), reasoning: undefined, toolUses: [] };
  }
  const textParts: string[] = [];
  const reasoningParts: string[] = [];
  const toolUses: CcContentBlock[] = [];

  if (Array.isArray(content)) {
    for (const raw of content) {
      if (raw === null || typeof raw !== "object") {
        textParts.push(stringify(raw));
        continue;
      }
      const block = raw as CcContentBlock;
      const t = block.type;
      if (t === "tool_use") {
        toolUses.push(block);
        continue;
      }
      if (t === "thinking" || t === "reasoning" || t === "analysis") {
        const v = block.thinking ?? block.text;
        reasoningParts.push(typeof v === "string" ? v.trim() : stringify(v));
        continue;
      }
      // redacted_thinking (encrypted) is intentionally dropped.
      if (t === "redacted_thinking") continue;
      if (typeof block.text === "string") {
        textParts.push(block.text);
        continue;
      }
      textParts.push(stringify(block));
    }
  } else if (content !== null && content !== undefined) {
    textParts.push(stringify(content));
  }

  const text = textParts.filter((p) => p && p.trim()).map((p) => p.trim()).join("\n\n");
  const reasoning = reasoningParts.filter((p) => p && p.trim()).map((p) => p.trim()).join("\n\n");
  return { text, reasoning: reasoning || undefined, toolUses };
}

/** Pull user text + tool_result blocks out of a user content value. */
function extractUser(content: unknown): {
  text: string;
  toolResults: CcContentBlock[];
} {
  if (typeof content === "string") {
    return { text: content, toolResults: [] };
  }
  const textParts: string[] = [];
  const toolResults: CcContentBlock[] = [];
  if (Array.isArray(content)) {
    for (const raw of content) {
      if (raw === null || typeof raw !== "object") {
        textParts.push(stringify(raw));
        continue;
      }
      const block = raw as CcContentBlock;
      if (block.type === "tool_result") {
        toolResults.push(block);
        continue;
      }
      if (block.type === "text" && typeof block.text === "string") {
        textParts.push(block.text);
        continue;
      }
      textParts.push(stringify(block));
    }
  } else if (content !== null && content !== undefined) {
    textParts.push(stringify(content));
  }
  return { text: textParts.filter((p) => p.trim()).join("\n\n"), toolResults };
}

/** Format a tool_result block's content into a single observation string. */
function formatToolResult(block: CcContentBlock): string {
  const parts: string[] = [];
  const content = block.content;
  if (typeof content === "string") {
    if (content.trim()) parts.push(content.trim());
  } else if (Array.isArray(content)) {
    for (const item of content) {
      if (item !== null && typeof item === "object" && (item as CcContentBlock).type === "text") {
        const txt = (item as CcContentBlock).text;
        if (typeof txt === "string" && txt.trim()) parts.push(txt.trim());
      } else {
        const s = stringify(item);
        if (s.trim()) parts.push(s.trim());
      }
    }
  } else if (content !== null && content !== undefined && content !== "") {
    parts.push(stringify(content));
  }
  if (block.is_error === true) parts.push("[error] tool reported failure");
  return parts.join("\n\n").trim();
}

/** Build ATIF metrics from a Claude Code usage object (own-vocabulary mapping). */
function buildMetrics(usage: CcUsage | undefined): AtifMetrics | undefined {
  if (!usage || typeof usage !== "object") return undefined;
  const input = Number(usage.input_tokens ?? 0);
  const cacheRead = Number(usage.cache_read_input_tokens ?? 0);
  const cacheCreate = Number(usage.cache_creation_input_tokens ?? 0);
  const output = Number(usage.output_tokens ?? 0);
  const prompt = input + cacheRead + cacheCreate;
  if (prompt === 0 && output === 0) return undefined;
  return { prompt_tokens: prompt, completion_tokens: output };
}

// --- A mutable working step (pre-renumber) ----------------------------------

// Mutable, so optional fields are typed `| undefined` (non-optional) to satisfy
// `exactOptionalPropertyTypes` when we assign them during the walk. The final
// ATIF step is built with conditional spreads that omit undefined fields.
interface WorkStep {
  source: "user" | "agent" | "system";
  timestamp: string | undefined;
  model_name: string | undefined;
  message: string;
  reasoning_content: string | undefined;
  tool_calls: AtifToolCall[];
  // observation results keyed for correlation; filled by tool_results.
  results: { source_call_id?: string; content: string }[];
  metrics: AtifMetrics | undefined;
}

// --- The converter ----------------------------------------------------------

/** Parse a Claude Code `.jsonl` string into raw events (skips blank/malformed lines). */
export function parseClaudeCodeJsonl(jsonl: string): CcEvent[] {
  const events: CcEvent[] = [];
  for (const line of jsonl.split("\n")) {
    const s = line.trim();
    if (!s) continue;
    try {
      events.push(JSON.parse(s) as CcEvent);
    } catch {
      // skip malformed line (matches harbor's lenient parse)
    }
  }
  return events;
}

/** Convert parsed Claude Code events into an ATIF-v1.7 trajectory. */
export function convertClaudeCodeEvents(
  rawEvents: ReadonlyArray<CcEvent>,
  options: ConvertOptions = {},
): AtifTrajectory {
  // 1. dedup by uuid.
  const seenUuid = new Set<string>();
  const deduped: CcEvent[] = [];
  for (const ev of rawEvents) {
    if (typeof ev.uuid === "string" && ev.uuid) {
      if (seenUuid.has(ev.uuid)) continue;
      seenUuid.add(ev.uuid);
    }
    deduped.push(ev);
  }

  // 2. sort by timestamp (stable), then sidechain-first ordering.
  const sorted = [...deduped].sort((a, b) =>
    (a.timestamp ?? "").localeCompare(b.timestamp ?? ""),
  );
  const ordered = [
    ...sorted.filter((e) => e.isSidechain),
    ...sorted.filter((e) => !e.isSidechain),
  ];

  // 3. header fields.
  const sessionId =
    options.sessionId ?? ordered.find((e) => typeof e.sessionId === "string")?.sessionId ?? "claude-code-session";
  const trajectoryId = options.trajectoryId ?? `${sessionId}-trajectory`;
  const agentName = options.agentName ?? "claude-code";
  const agentVersion = ordered.find((e) => typeof e.version === "string" && e.version)?.version ?? "unknown";
  const defaultModel =
    options.defaultModelName ??
    ordered.find((e) => typeof e.message?.model === "string" && e.message.model)?.message?.model ??
    "unknown";

  // Last usage per assistant message id (streaming updates it on each chunk).
  const lastUsage = new Map<string, CcUsage>();
  for (const ev of ordered) {
    if (ev.type !== "assistant") continue;
    const mid = ev.message?.id;
    const usage = ev.message?.usage;
    if (mid && usage) lastUsage.set(mid, usage);
  }

  // 4. walk events, bundling one assistant message.id into one step.
  const steps: WorkStep[] = [];
  const turnByMsgId = new Map<string, WorkStep>();
  // call_id -> the step that issued it (to attach the matching tool_result).
  const pendingCall = new Map<string, WorkStep>();
  const completedCall = new Set<string>();
  const seenMsgIdForMetrics = new Set<string>();

  for (const ev of ordered) {
    const message = ev.message;
    if (!message || typeof message !== "object") continue;
    const ts = ev.timestamp;

    if (ev.type === "assistant") {
      const { text, reasoning, toolUses } = extractAssistant(message.content);
      const mid = message.id;
      const model = (typeof message.model === "string" && message.model) || defaultModel;

      const existing = mid ? turnByMsgId.get(mid) : undefined;
      const turn: WorkStep =
        existing ??
        {
          source: "agent",
          timestamp: ts,
          model_name: model,
          message: "",
          reasoning_content: undefined,
          tool_calls: [],
          results: [],
          metrics: undefined,
        };
      if (!existing) {
        steps.push(turn);
        if (mid) turnByMsgId.set(mid, turn);
      }

      if (text) turn.message = turn.message ? `${turn.message}\n\n${text}` : text;
      if (reasoning) {
        turn.reasoning_content = turn.reasoning_content
          ? `${turn.reasoning_content}\n\n${reasoning}`
          : reasoning;
      }
      // Metrics once per message id (use the last/accumulated usage).
      if (mid && !seenMsgIdForMetrics.has(mid)) {
        const m = buildMetrics(lastUsage.get(mid) ?? message.usage);
        if (m && !turn.metrics) turn.metrics = m;
        seenMsgIdForMetrics.add(mid);
      } else if (!mid && !turn.metrics) {
        const m = buildMetrics(message.usage);
        if (m) turn.metrics = m;
      }

      for (const block of toolUses) {
        const callId = block.id ?? block.tool_use_id;
        if (!callId) continue;
        if (pendingCall.has(callId) || completedCall.has(callId)) continue;
        const input = block.input;
        const args: Record<string, Json> =
          input !== null && typeof input === "object" && !Array.isArray(input)
            ? (input as Record<string, Json>)
            : { input: (input ?? null) as Json };
        turn.tool_calls.push({
          tool_call_id: callId,
          function_name: typeof block.name === "string" ? block.name : "",
          arguments: args,
        });
        pendingCall.set(callId, turn);
      }
      continue;
    }

    if (ev.type === "user") {
      const { text, toolResults } = extractUser(message.content);

      // Attach tool_results to their issuing step (correlation by id).
      for (const block of toolResults) {
        const callId = block.tool_use_id;
        const output = formatToolResult(block);
        const owner = callId ? pendingCall.get(callId) : undefined;
        if (owner && callId) {
          owner.results.push({ source_call_id: callId, content: output });
          pendingCall.delete(callId);
          completedCall.add(callId);
          continue;
        }
        if (callId && completedCall.has(callId)) continue; // duplicate
        // Orphan tool_result: emit its own agent step with a synthetic call so
        // the observation correlates and nothing is lost.
        const synthId = callId ?? `orphan_${steps.length + 1}`;
        const orphan: WorkStep = {
          source: "agent",
          timestamp: ts,
          model_name: defaultModel,
          message: "",
          reasoning_content: undefined,
          tool_calls: [
            { tool_call_id: synthId, function_name: "tool_result", arguments: {} },
          ],
          results: [{ source_call_id: synthId, content: output }],
          metrics: undefined,
        };
        steps.push(orphan);
        if (callId) completedCall.add(callId);
      }

      // A user text message becomes a user step.
      if (text && text.trim()) {
        steps.push({
          source: "user",
          timestamp: ts,
          model_name: undefined,
          message: text,
          reasoning_content: undefined,
          tool_calls: [],
          results: [],
          metrics: undefined,
        });
      }
      continue;
    }

    // Other event types (mode, system, attachment, …) are not trajectory steps.
  }

  // 5. materialize ATIF steps, renumbering step_id sequentially from 1 and
  //    respecting the validator's non-agent constraints.
  const atifSteps: AtifStep[] = steps
    .filter((s) => {
      // Drop empty agent turns that produced no text, reasoning, or tool call.
      if (s.source === "agent") {
        return s.message.trim() !== "" || s.reasoning_content || s.tool_calls.length > 0;
      }
      return true;
    })
    .map((s, i) => {
      const stepId = i + 1;
      if (s.source === "agent") {
        const step: AtifStep = {
          step_id: stepId,
          ...(s.timestamp ? { timestamp: s.timestamp } : {}),
          source: "agent",
          ...(s.model_name ? { model_name: s.model_name } : {}),
          message: s.message,
          ...(s.reasoning_content ? { reasoning_content: s.reasoning_content } : {}),
          ...(s.tool_calls.length > 0 ? { tool_calls: s.tool_calls } : {}),
          ...(s.results.length > 0 ? { observation: { results: s.results } } : {}),
          ...(s.metrics ? { metrics: s.metrics } : {}),
        };
        return step;
      }
      // user/system step: message only (validator forbids tool/reasoning/metrics).
      const step: AtifStep = {
        step_id: stepId,
        ...(s.timestamp ? { timestamp: s.timestamp } : {}),
        source: s.source,
        message: s.message,
      };
      return step;
    });

  // 6. aggregate final metrics.
  let totalPrompt = 0;
  let totalCompletion = 0;
  let metricSteps = 0;
  for (const s of atifSteps) {
    if (s.metrics) {
      totalPrompt += s.metrics.prompt_tokens ?? 0;
      totalCompletion += s.metrics.completion_tokens ?? 0;
      metricSteps += 1;
    }
  }
  const finalMetrics: AtifFinalMetrics = {
    total_prompt_tokens: totalPrompt,
    total_completion_tokens: totalCompletion,
    total_steps: atifSteps.length,
    extra: { source: "claude-code-import", metric_steps: metricSteps },
  };

  const cwds = Array.from(
    new Set(ordered.map((e) => e.cwd).filter((c): c is string => typeof c === "string" && c.length > 0)),
  );
  const branches = Array.from(
    new Set(
      ordered.map((e) => e.gitBranch).filter((b): b is string => typeof b === "string" && b.length > 0),
    ),
  );

  const trajectory: AtifTrajectory = {
    schema_version: ATIF_SCHEMA_VERSION,
    session_id: sessionId,
    trajectory_id: trajectoryId,
    agent: {
      name: agentName,
      version: agentVersion,
      model_name: defaultModel,
      extra: {
        source: "claude-code",
        ...(cwds.length > 0 ? { cwds } : {}),
        ...(branches.length > 0 ? { git_branches: branches } : {}),
      },
    },
    notes:
      "Imported from a Claude Code session .jsonl by the OpenAgents qa-runner " +
      "claude-code->ATIF converter (#6220). Run through TraceRedactor before publishing.",
    steps: atifSteps,
    final_metrics: finalMetrics,
  };

  return trajectory;
}

/** Convenience: convert a `.jsonl` string straight to a trajectory. */
export function convertClaudeCodeJsonl(jsonl: string, options: ConvertOptions = {}): AtifTrajectory {
  return convertClaudeCodeEvents(parseClaudeCodeJsonl(jsonl), options);
}
