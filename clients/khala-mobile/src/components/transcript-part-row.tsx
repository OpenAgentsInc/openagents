import { Text, View } from "react-native"

import type { KhalaRuntimeLane } from "@openagentsinc/khala-sync"

import type { TranscriptPart } from "../sync/khala-runtime-transcript-core"

const TURN_STATUS_LABEL: Record<
  Extract<TranscriptPart, { kind: "turn-status" }>["status"],
  string
> = {
  completed: "turn completed",
  failed: "turn failed",
  interrupted: "turn interrupted",
  running: "turn started"
}

/** Short, user-facing provider label for the per-turn lane badge (#8405).
 * Only the two user-selectable lanes get a friendly name — every other lane
 * (ai_sdk_core, khala_sync_mobile_control, test_fixture, …) is an internal
 * routing detail no chat transcript should ever show, so it falls back to
 * the raw lane string rather than inventing a label nobody picked. */
const LANE_LABEL: Partial<Record<KhalaRuntimeLane, string>> = {
  claude_pylon: "Claude",
  codex_app_server: "Codex"
}
const laneLabel = (lane: KhalaRuntimeLane): string => LANE_LABEL[lane] ?? lane

const TOOL_STATUS_COLOR: Record<Extract<TranscriptPart, { kind: "tool" }>["status"], string> = {
  called: "text-warning",
  completed: "text-success",
  failed: "text-danger"
}

/** Renders one AI-SDK-shaped runtime transcript part with a distinct
 * component per part type — text bubble, collapsible-style reasoning
 * block, tool-call/result card, usage footer, turn-status divider. */
export const TranscriptPartRow = ({ part }: { part: TranscriptPart }) => {
  switch (part.kind) {
    case "text":
      return (
        <View className="rounded-xl border border-border bg-surfaceRaised px-3 py-2">
          <Text className="font-sans text-base text-text">{part.text}</Text>
        </View>
      )
    case "reasoning":
      return (
        <View className="rounded-xl border border-borderMuted bg-surface px-3 py-2">
          <Text className="font-mono text-xs uppercase tracking-wide text-textFaint">
            reasoning
          </Text>
          <Text className="mt-1 font-mono text-sm italic text-textMuted">{part.text}</Text>
        </View>
      )
    case "tool":
      return (
        <View className="rounded-xl border border-border bg-surface px-3 py-2">
          <View className="flex-row items-center justify-between">
            <Text className="font-mono text-sm text-text">🔧 {part.toolName}</Text>
            <Text className={`font-mono text-xs ${TOOL_STATUS_COLOR[part.status]}`}>
              {part.status}
            </Text>
          </View>
          {part.errorMessageSafe === undefined ? null : (
            <Text className="mt-1 font-mono text-xs text-danger">{part.errorMessageSafe}</Text>
          )}
        </View>
      )
    case "usage":
      return (
        <Text className="font-mono text-xs text-textFaint">
          {part.inputTokens ?? 0} in · {part.outputTokens ?? 0} out · {part.totalTokens ?? 0} total
          tokens
        </Text>
      )
    case "turn-status":
      return (
        <View className="flex-row items-center justify-center gap-1.5 self-center">
          <Text className="font-mono text-xs uppercase tracking-wide text-textFaint">
            — {TURN_STATUS_LABEL[part.status]} —
          </Text>
          <View className="rounded-full border border-borderMuted bg-surface px-1.5 py-0.5">
            <Text className="font-mono text-[10px] uppercase tracking-wide text-textFaint">
              {laneLabel(part.lane)}
            </Text>
          </View>
        </View>
      )
  }
}
