// Convert an OpenAgents Desktop conversation object into an ATIF-v1.7
// trajectory (issue: local-conversation -> public /trace/{uuid} ingest).
//
// Claude Code and Codex store one JSONL file per session, so their converters
// (claude-code-to-atif.ts, codex-to-atif.ts) read a file. OpenAgents Desktop
// instead keeps conversations as ARRAY ELEMENTS inside a single per-profile
// `KhalaDesktop/conversations.json`, so the caller resolves the conversation
// object by id (see conversation-source.ts) and hands the OBJECT here.
//
// This converter is deliberately defensive: the desktop conversation `messages`
// shape has varied over the app's life and can be empty. We map whatever is
// present onto valid, sequentially-numbered ATIF steps and never throw on an
// unexpected block; anything unrecognized becomes step text. Redaction and the
// public-safety tripwire run downstream (publish-trace / the ingest API), not
// here — this converter does NOT redact.

import {
  ATIF_SCHEMA_VERSION,
  type AtifStep,
  type AtifToolCall,
  type AtifTrajectory,
  type Json,
} from "@openagentsinc/atif/emit";

/** The desktop conversation object as stored in `conversations.json`. */
export interface OpenAgentsConversation {
  readonly id?: string;
  readonly title?: string;
  readonly messages?: unknown;
  readonly createdAt?: string;
  readonly updatedAt?: string;
  readonly [k: string]: unknown;
}

export interface OpenAgentsConversationToAtifOptions {
  /** Override the trajectory session id (defaults to the conversation id). */
  readonly sessionId?: string;
  /** Document id (defaults to `${sessionId}-trajectory`). */
  readonly trajectoryId?: string;
  /** Agent display name. */
  readonly agentName?: string;
  /** Fallback model id when a message omits one. */
  readonly defaultModelName?: string;
}

type Block = Record<string, unknown>;

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);

const stringify = (value: unknown): string => {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
};

/** Map a message `role` onto an ATIF step `source`. */
const sourceForRole = (role: unknown): AtifStep["source"] => {
  const r = typeof role === "string" ? role.toLowerCase() : "";
  if (r === "assistant" || r === "agent" || r === "model") return "agent";
  if (r === "system" || r === "tool") return "system";
  return "user";
};

/** Pull text / reasoning / tool_use / tool_result out of a message content. */
const extractContent = (
  content: unknown,
): {
  text: string;
  reasoning: string | undefined;
  toolCalls: AtifToolCall[];
  observations: { sourceCallId: string; content: string }[];
} => {
  const textParts: string[] = [];
  const reasoningParts: string[] = [];
  const toolCalls: AtifToolCall[] = [];
  const observations: { sourceCallId: string; content: string }[] = [];

  if (typeof content === "string") {
    if (content.trim()) textParts.push(content.trim());
  } else if (Array.isArray(content)) {
    for (const raw of content) {
      if (!isRecord(raw)) {
        const s = stringify(raw).trim();
        if (s) textParts.push(s);
        continue;
      }
      const block = raw as Block;
      const type = typeof block.type === "string" ? block.type : "";
      if (type === "tool_use" || type === "tool_call") {
        const id =
          (typeof block.id === "string" && block.id) ||
          (typeof block.tool_call_id === "string" && block.tool_call_id) ||
          `call-${toolCalls.length + 1}`;
        const name =
          (typeof block.name === "string" && block.name) ||
          (typeof block.function_name === "string" && block.function_name) ||
          "tool";
        const args = isRecord(block.input)
          ? (block.input as Record<string, Json>)
          : isRecord(block.arguments)
            ? (block.arguments as Record<string, Json>)
            : {};
        toolCalls.push({ tool_call_id: id, function_name: name, arguments: args });
        continue;
      }
      if (type === "tool_result") {
        const sourceCallId =
          (typeof block.tool_use_id === "string" && block.tool_use_id) ||
          (typeof block.tool_call_id === "string" && block.tool_call_id) ||
          "";
        const resultText = extractResultText(block.content);
        if (sourceCallId) observations.push({ sourceCallId, content: resultText });
        else if (resultText.trim()) textParts.push(resultText.trim());
        continue;
      }
      if (type === "thinking" || type === "reasoning" || type === "analysis") {
        const v = block.thinking ?? block.text;
        const s = typeof v === "string" ? v.trim() : stringify(v).trim();
        if (s) reasoningParts.push(s);
        continue;
      }
      if (type === "redacted_thinking") continue;
      if (typeof block.text === "string") {
        if (block.text.trim()) textParts.push(block.text.trim());
        continue;
      }
      const s = stringify(block).trim();
      if (s) textParts.push(s);
    }
  } else if (content !== null && content !== undefined) {
    const s = stringify(content).trim();
    if (s) textParts.push(s);
  }

  const reasoning = reasoningParts.join("\n\n");
  return {
    text: textParts.join("\n\n"),
    reasoning: reasoning || undefined,
    toolCalls,
    observations,
  };
};

/** Flatten a tool_result `content` (string or block array) to one string. */
const extractResultText = (content: unknown): string => {
  if (typeof content === "string") return content.trim();
  if (Array.isArray(content)) {
    const parts: string[] = [];
    for (const item of content) {
      if (isRecord(item) && typeof (item as Block).text === "string") {
        const t = ((item as Block).text as string).trim();
        if (t) parts.push(t);
      } else {
        const s = stringify(item).trim();
        if (s) parts.push(s);
      }
    }
    return parts.join("\n");
  }
  return content === null || content === undefined ? "" : stringify(content).trim();
};

/**
 * Convert one OpenAgents Desktop conversation object into an ATIF-v1.7
 * trajectory. Always returns a structurally valid trajectory with at least one
 * step (an empty conversation yields a single explanatory `system` step) so the
 * downstream validator/tripwire have something concrete to gate.
 */
export function convertOpenAgentsConversationToAtif(
  conversation: OpenAgentsConversation,
  options: OpenAgentsConversationToAtifOptions = {},
): AtifTrajectory {
  const sessionId = options.sessionId ?? (conversation.id ?? "openagents-conversation");
  const trajectoryId = options.trajectoryId ?? `${sessionId}-trajectory`;
  const agentName = options.agentName ?? "OpenAgents";
  const defaultModelName = options.defaultModelName ?? "openagents/khala";

  const rawMessages = Array.isArray(conversation.messages) ? conversation.messages : [];
  const steps: AtifStep[] = [];
  // Track emitted tool_call ids so observations only reference real calls (the
  // ATIF structural validator rejects a dangling observation source_call_id).
  const emittedCallIds = new Set<string>();

  for (const raw of rawMessages) {
    if (!isRecord(raw)) continue;
    const message = raw as Record<string, unknown>;
    const source = sourceForRole(message.role);
    const { text, reasoning, toolCalls, observations } = extractContent(
      message.content ?? message.text,
    );

    const validObservations = observations.filter((o) => o.sourceCallId);
    // Nothing usable on this message -> skip it rather than emit an empty step.
    if (
      !text &&
      !reasoning &&
      toolCalls.length === 0 &&
      validObservations.length === 0
    ) {
      continue;
    }

    const isAgent = source === "agent";
    // Agent-only fields (reasoning_content / tool_calls) are only legal on an
    // `agent` step; on any other source we fold reasoning into the message.
    const stepMessage =
      !isAgent && reasoning ? (text ? `${text}\n\n${reasoning}` : reasoning) : text;

    if (isAgent && toolCalls.length > 0) {
      for (const call of toolCalls) emittedCallIds.add(call.tool_call_id);
    }
    const usableObservations = validObservations.filter((o) =>
      emittedCallIds.has(o.sourceCallId),
    );

    const step: AtifStep = {
      step_id: steps.length + 1,
      source,
      message: stepMessage,
      ...(typeof message.timestamp === "string"
        ? { timestamp: message.timestamp }
        : {}),
      ...(isAgent && reasoning ? { reasoning_content: reasoning } : {}),
      ...(isAgent
        ? {
            model_name:
              (typeof message.model === "string" && message.model) || defaultModelName,
          }
        : {}),
      ...(isAgent && toolCalls.length > 0 ? { tool_calls: toolCalls } : {}),
      ...(usableObservations.length > 0
        ? {
            observation: {
              results: usableObservations.map((o) => ({
                source_call_id: o.sourceCallId,
                content: o.content,
              })),
            },
          }
        : {}),
    };

    steps.push(step);
  }

  if (steps.length === 0) {
    steps.push({
      step_id: 1,
      source: "system",
      message:
        "This OpenAgents conversation had no reconstructable messages in the " +
        "local store. OpenAgents coding sessions execute through Codex/Claude " +
        "workers, whose per-session logs convert with higher fidelity.",
    });
  }

  return {
    schema_version: ATIF_SCHEMA_VERSION,
    session_id: sessionId,
    trajectory_id: trajectoryId,
    agent: {
      name: agentName,
      version: "0.1.0",
      model_name: defaultModelName,
    },
    ...(typeof conversation.title === "string" && conversation.title
      ? { notes: `Conversation: ${conversation.title}` }
      : {}),
    steps,
    final_metrics: { total_steps: steps.length },
  };
}
