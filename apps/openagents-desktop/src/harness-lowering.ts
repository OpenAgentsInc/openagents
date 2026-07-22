/**
 * HARN-09 (#9167) Slice 0: lower neutral harness stream events
 * (`HarnessStreamEvent` = `KhalaRuntimeEvent`) onto the renderer's frozen
 * `ClaudeLocalEvent` envelope — the exact INVERSE of the HARN-03 forward
 * projector in `harness-projection.ts`, for the seven core kinds only.
 *
 * This is the enabling primitive for routing provider lanes through the SDK
 * harness adapters (`makeCodexHarnessAdapter` and friends emit the neutral
 * stream; the dispatcher and renderer speak `ClaudeLocalEvent`). Desktop
 * display-only kinds (plan/meter/question/child/notice/model_effective/
 * tool_progress/composer_admission) have no neutral origin and are NOT
 * produced here — a seam that adopts this lowering must preserve those
 * kinds from its own host-side context.
 */

import type { HarnessStreamEvent } from "@openagentsinc/agent-harness-contract";
import {
  CLAUDE_LOCAL_DELTA_LIMIT,
  CLAUDE_LOCAL_SUMMARY_LIMIT,
  type ClaudeLocalEvent,
} from "./claude-local-contract";

const bounded = (value: string, limit: number): string =>
  value.length > limit ? value.slice(0, limit) : value;

/**
 * Lower one neutral harness event onto zero or more renderer envelope
 * events. Kinds outside the seven-core subset lower to nothing.
 */
export const lowerHarnessEvent = (event: HarnessStreamEvent): ReadonlyArray<ClaudeLocalEvent> => {
  switch (event.kind) {
    case "turn.started":
      return [{ kind: "turn_started" }];
    case "text.delta":
      return event.text === ""
        ? []
        : [{ kind: "text_delta", text: bounded(event.text, CLAUDE_LOCAL_DELTA_LIMIT) }];
    case "reasoning.delta":
      return event.text === ""
        ? []
        : [{ kind: "reasoning", text: bounded(event.text, CLAUDE_LOCAL_SUMMARY_LIMIT) }];
    case "tool.call":
      return [
        {
          kind: "tool_use",
          toolName: bounded(event.toolName, 120),
          summary: bounded(event.toolName, CLAUDE_LOCAL_SUMMARY_LIMIT),
          itemRef: bounded(event.toolCallId, 120),
        },
      ];
    case "tool.result":
      return [
        {
          kind: "tool_result",
          toolName: bounded(event.toolName, 120),
          ok: true,
          summary: bounded(event.toolName, CLAUDE_LOCAL_SUMMARY_LIMIT),
          itemRef: bounded(event.toolCallId, 120),
        },
      ];
    case "tool.error":
      return [
        {
          kind: "tool_result",
          toolName: bounded(event.toolName, 120),
          ok: false,
          summary: bounded(event.messageSafe ?? event.toolName, CLAUDE_LOCAL_SUMMARY_LIMIT),
          itemRef: bounded(event.toolCallId, 120),
        },
      ];
    case "turn.finished":
      return [
        {
          kind: "turn_completed",
          totalTokens: event.usage?.totalTokens ?? null,
        },
      ];
    case "turn.interrupted":
      return [
        {
          kind: "turn_failed",
          reason: "interrupted",
          detail: bounded(event.reasonRef ?? "interrupted", CLAUDE_LOCAL_SUMMARY_LIMIT),
        },
      ];
    default:
      // Neutral kinds without a core renderer mapping (step.*, provider
      // metadata, file.change detail, compaction, agent.child.*) stay on the
      // durable log; the renderer's display-only kinds come from the host.
      return [];
  }
};

/** Lower an ordered neutral event array onto the renderer envelope. */
export const lowerHarnessEvents = (
  events: ReadonlyArray<HarnessStreamEvent>,
): ReadonlyArray<ClaudeLocalEvent> => events.flatMap(lowerHarnessEvent);
