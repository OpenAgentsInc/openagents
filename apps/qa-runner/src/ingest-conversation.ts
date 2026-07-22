// Build an ATIF-v1.7 trajectory from a local conversation id (issue:
// local-conversation -> public /trace/{uuid} ingest).
//
// This is the pure library step that sits between source resolution
// (conversation-source.ts) and publishing (publish-trace.ts). It resolves the
// id, dispatches to the matching converter, and returns the trajectory plus the
// resolved-source metadata. It does NOT redact and does NOT touch the network;
// the CLI (ingest-conversation-cli.ts) owns redaction preflight + publish.

import { type AtifTrajectory } from "@openagentsinc/atif/emit";

import { convertClaudeCodeJsonl } from "./claude-code-to-atif";
import { convertCodexRolloutTextToAtif } from "./codex-to-atif";
import {
  type ConversationSourceKind,
  type ResolveOptions,
  type ResolvedConversation,
  resolveConversation,
} from "./conversation-source";
import { convertOpenAgentsConversationToAtif } from "./openagents-conversation-to-atif";

export interface BuildTrajectoryOptions extends ResolveOptions {
  /** Force a specific source; omit/"auto" to auto-detect. */
  readonly kind?: ConversationSourceKind | "auto";
  /** Agent display name for the trajectory header. */
  readonly agentName?: string;
  /** Fallback model id when the source omits one. */
  readonly defaultModelName?: string;
}

export interface BuildTrajectoryResult {
  readonly resolved: ResolvedConversation;
  readonly trajectory: AtifTrajectory;
}

/** The ingest API's hard per-trajectory step cap (trace-store-routes MAX_STEPS). */
export const INGEST_MAX_STEPS = 2000;

/**
 * Cap a trajectory to its first `maxSteps` steps. Keeping a PREFIX preserves
 * structural validity: step ids stay sequential from 1, and every kept
 * observation's `source_call_id` still references a tool_call that appears
 * earlier in the kept prefix (a tool result always follows its call). A capped
 * trajectory records the truncation in `notes` so the trace is honest about it.
 * Returns the trajectory unchanged when it is already within the cap.
 */
export function capTrajectorySteps(
  trajectory: AtifTrajectory,
  maxSteps: number,
): AtifTrajectory {
  if (maxSteps <= 0 || trajectory.steps.length <= maxSteps) return trajectory;
  const kept = trajectory.steps.slice(0, maxSteps);
  const dropped = trajectory.steps.length - kept.length;
  const truncationNote = `Truncated to the first ${maxSteps} of ${trajectory.steps.length} steps for public ingest (${dropped} later steps omitted).`;
  return {
    ...trajectory,
    notes: trajectory.notes ? `${trajectory.notes}\n${truncationNote}` : truncationNote,
    steps: kept,
    ...(trajectory.final_metrics === undefined
      ? {}
      : { final_metrics: { ...trajectory.final_metrics, total_steps: kept.length } }),
  };
}

/**
 * Resolve `id` to a local conversation and convert it to an ATIF trajectory.
 * Throws `ConversationNotFoundError` when no source matches.
 */
export function buildTrajectoryFromConversationId(
  id: string,
  options: BuildTrajectoryOptions = {},
): BuildTrajectoryResult {
  const resolved = resolveConversation(id, options.kind ?? "auto", {
    ...(options.home === undefined ? {} : { home: options.home }),
    ...(options.userData === undefined ? {} : { userData: options.userData }),
  });

  const convertOptions = {
    sessionId: id,
    ...(options.agentName === undefined ? {} : { agentName: options.agentName }),
    ...(options.defaultModelName === undefined
      ? {}
      : { defaultModelName: options.defaultModelName }),
  };

  let trajectory: AtifTrajectory;
  if (resolved.kind === "claude") {
    trajectory = convertClaudeCodeJsonl(resolved.jsonl, convertOptions);
  } else if (resolved.kind === "codex") {
    trajectory = convertCodexRolloutTextToAtif(resolved.jsonl, {
      sessionId: id,
      ...(options.defaultModelName === undefined
        ? {}
        : { modelName: options.defaultModelName }),
    });
  } else {
    trajectory = convertOpenAgentsConversationToAtif(resolved.conversation, convertOptions);
  }

  return { resolved, trajectory };
}
