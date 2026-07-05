import { Pressable, View } from "react-native"

import type { KhalaRuntimeLane } from "@openagentsinc/khala-sync"

import { handoffLaneLabel, handoffTargetLane } from "../sync/khala-cross-agent-handoff-core"
import { summarizeToolPart, type TranscriptPart, type ToolSummaryTone } from "../sync/khala-runtime-transcript-core"
import { KhalaText } from "./khala-text"
import { TouchableFeedback } from "./touchable-feedback"

const TURN_STATUS_LABEL: Record<
  Extract<TranscriptPart, { kind: "turn-status" }>["status"],
  string
> = {
  completed: "turn completed",
  failed: "turn failed",
  interrupted: "turn interrupted",
  running: "turn started"
}

const TOOL_TONE_CLASS_NAME: Record<ToolSummaryTone, string> = {
  danger: "text-danger",
  muted: "text-textMuted",
  success: "text-success",
  warning: "text-warning",
}

const TOOL_ICON: Record<ReturnType<typeof summarizeToolPart>["icon"], string> = {
  edit: "✎",
  run: "▻",
  tool: "⌁",
}

/** Short, user-facing provider label for the per-turn lane badge (#8405).
 * Only the two user-selectable lanes get a friendly name — every other lane
 * (ai_sdk_core, khala_sync_mobile_control, test_fixture, …) is an internal
 * routing detail no chat transcript should ever show, so it falls back to
 * the raw lane string rather than inventing a label nobody picked. Reuses
 * `handoffLaneLabel` (#8407) so this file carries only one lane->label
 * mapping instead of two. */
const laneLabel = handoffLaneLabel

export type TranscriptPartRowProps = Readonly<{
  part: TranscriptPart
  /** Cross-agent handoff (#8407) — called with the completed turn's own id
   * and the OTHER lane to hand it off to. Omitted (or the turn-status part
   * has no eligible target lane, e.g. an internal routing lane) means no
   * button renders — this row stays a pure display component either way. */
  onRequestHandoff?: (input: { turnId: string; sourceLane: KhalaRuntimeLane; targetLane: KhalaRuntimeLane }) => void
  /** True while a handoff is in flight for THIS row's turn — shows a
   * disabled "asking…" state instead of the normal button label. */
  handoffPending?: boolean
  /** True while handoff is unavailable thread-wide (e.g. another turn is
   * already active) — mirrors the composer's own idle-only lane picker
   * (#8405): retargeting only ever applies to starting a brand-new turn,
   * never while one is already running. */
  handoffDisabled?: boolean
}>

/** Renders one AI-SDK-shaped runtime transcript part in the screenshot target
 * rhythm: large plain prose, collapsed one-line tool summaries, and compact
 * status affordances. */
export const TranscriptPartRow = ({
  handoffDisabled,
  handoffPending,
  onRequestHandoff,
  part
}: TranscriptPartRowProps) => {
  switch (part.kind) {
    case "text":
      return (
        <View className="px-1 py-1">
          <KhalaText className="text-[22px] leading-8 text-text" variant="body">
            {part.text}
          </KhalaText>
        </View>
      )
    case "reasoning":
      return (
        <View className="px-1 py-1">
          <KhalaText className="text-[20px] leading-7 text-textMuted" variant="muted">
            {part.text}
          </KhalaText>
        </View>
      )
    case "tool": {
      const summary = summarizeToolPart(part)
      return (
        <TouchableFeedback
          accessibilityLabel={summary.label}
          accessibilityRole="button"
          className="rounded-lg"
          highlightColor="rgba(79, 208, 255, 0.08)"
        >
          <View className="flex-row items-center gap-2 px-1 py-2">
            <KhalaText className={`w-5 text-center text-xl ${TOOL_TONE_CLASS_NAME[summary.tone]}`} variant="mono">
              {TOOL_ICON[summary.icon]}
            </KhalaText>
            <KhalaText
              className={`min-w-0 flex-1 text-[20px] leading-7 ${TOOL_TONE_CLASS_NAME[summary.tone]}`}
              numberOfLines={1}
              variant="muted"
            >
              {summary.label}
            </KhalaText>
            <KhalaText className="text-[24px] text-textFaint" variant="faint">
              ›
            </KhalaText>
          </View>
        </TouchableFeedback>
      )
    }
    case "usage":
      return (
        <KhalaText className="px-1 py-1" variant="faint">
          {part.inputTokens ?? 0} in · {part.outputTokens ?? 0} out · {part.totalTokens ?? 0} total tokens
        </KhalaText>
      )
    case "turn-status": {
      const targetLane = part.status === "completed" ? handoffTargetLane(part.lane) : undefined
      const canRequestHandoff = targetLane !== undefined && onRequestHandoff !== undefined
      return (
        <View className="items-start gap-2 px-1 py-1">
          <View className="flex-row items-center gap-2">
            <KhalaText className="text-textFaint" variant="faint">
              {TURN_STATUS_LABEL[part.status]}
            </KhalaText>
            <View className="rounded-full border border-borderMuted bg-surface px-1.5 py-0.5">
              <KhalaText className="text-[10px]" variant="faint">
                {laneLabel(part.lane)}
              </KhalaText>
            </View>
          </View>
          {canRequestHandoff ? (
            <Pressable
              accessibilityLabel={`Ask ${laneLabel(targetLane)} to review this`}
              accessibilityRole="button"
              className="rounded-full border border-borderMuted bg-surface px-2 py-1"
              disabled={handoffDisabled === true || handoffPending === true}
              onPress={() =>
                onRequestHandoff({ sourceLane: part.lane, targetLane, turnId: part.turnId })
              }
            >
              <KhalaText className="text-[10px] uppercase tracking-wide text-accent" variant="faint">
                {handoffPending === true ? "asking…" : `ask ${laneLabel(targetLane)} to review this`}
              </KhalaText>
            </Pressable>
          ) : null}
        </View>
      )
    }
  }
}
