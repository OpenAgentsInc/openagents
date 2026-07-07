import { Pressable, View, type TextStyle, type ViewStyle } from "react-native"

import type { KhalaRuntimeLane } from "@openagentsinc/khala-sync"

import { handoffLaneLabel, handoffTargetLane } from "../sync/khala-cross-agent-handoff-core"
import { summarizeToolPart, type TranscriptPart, type ToolSummaryTone } from "../sync/khala-runtime-transcript-core"
import { Text, useAppTheme } from "../ignite"
import type { Theme, ThemedStyle } from "../ignite"
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

const toolToneColor = (theme: Theme, tone: ToolSummaryTone): string => {
  switch (tone) {
    case "danger":
      return theme.colors.error
    case "success":
      return theme.colors.palette.secondary300
    case "warning":
      return theme.colors.palette.accent200
    case "muted":
    default:
      return theme.colors.textDim
  }
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
 * status affordances — on the ported Infinite Red Ignite `Text` primitive +
 * theme tokens (`../ignite`). */
export const TranscriptPartRow = ({
  handoffDisabled,
  handoffPending,
  onRequestHandoff,
  part
}: TranscriptPartRowProps) => {
  const { theme, themed } = useAppTheme()
  switch (part.kind) {
    case "text":
      return (
        <View style={themed($prosePad)}>
          <Text style={[$prose, { color: theme.colors.text }]}>{part.text}</Text>
        </View>
      )
    case "reasoning":
      return (
        <View style={themed($prosePad)}>
          <Text style={[$reasoning, { color: theme.colors.textDim }]}>{part.text}</Text>
        </View>
      )
    case "tool": {
      const summary = summarizeToolPart(part)
      const toneColor = toolToneColor(theme, summary.tone)
      return (
        <TouchableFeedback
          accessibilityLabel={summary.label}
          accessibilityRole="button"
          style={$toolTouchable}
          highlightColor="rgba(232, 193, 180, 0.10)"
        >
          <View style={themed($toolRow)}>
            <Text style={[$toolIcon, { color: toneColor }]}>{TOOL_ICON[summary.icon]}</Text>
            <Text numberOfLines={1} style={[$toolLabel, { color: toneColor }]}>
              {summary.label}
            </Text>
            <Text style={[$chevron, { color: theme.colors.textDim }]}>›</Text>
          </View>
        </TouchableFeedback>
      )
    }
    case "usage":
      return (
        <Text size="xxs" style={themed($usage)}>
          {part.inputTokens ?? 0} in · {part.outputTokens ?? 0} out · {part.totalTokens ?? 0} total tokens
        </Text>
      )
    case "turn-status": {
      const targetLane = part.status === "completed" ? handoffTargetLane(part.lane) : undefined
      const canRequestHandoff = targetLane !== undefined && onRequestHandoff !== undefined
      return (
        <View style={themed($statusContainer)}>
          <View style={themed($statusRow)}>
            <Text size="xxs" style={themed($faint)} text={TURN_STATUS_LABEL[part.status]} />
            <View style={themed($laneBadge)}>
              <Text style={[$laneBadgeText, { color: theme.colors.textDim }]} text={laneLabel(part.lane)} />
            </View>
          </View>
          {canRequestHandoff ? (
            <Pressable
              accessibilityLabel={`Ask ${laneLabel(targetLane)} to review this`}
              accessibilityRole="button"
              style={themed($handoffButton)}
              disabled={handoffDisabled === true || handoffPending === true}
              onPress={() =>
                onRequestHandoff({ sourceLane: part.lane, targetLane, turnId: part.turnId })
              }
            >
              <Text
                style={[$handoffText, { color: theme.colors.tint }]}
                text={handoffPending === true ? "asking…" : `ask ${laneLabel(targetLane)} to review this`}
              />
            </Pressable>
          ) : null}
        </View>
      )
    }
  }
}

const $prosePad: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.xxxs,
  paddingVertical: spacing.xxxs
})

const $toolTouchable: ViewStyle = { borderRadius: 8 }

const $toolRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.xs,
  paddingHorizontal: spacing.xxxs,
  paddingVertical: spacing.xxs
})

const $usage: ThemedStyle<TextStyle> = ({ colors, spacing }) => ({
  color: colors.textDim,
  paddingHorizontal: spacing.xxxs,
  paddingVertical: spacing.xxxs
})

const $statusContainer: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  alignItems: "flex-start",
  gap: spacing.xs,
  paddingHorizontal: spacing.xxxs,
  paddingVertical: spacing.xxxs
})

const $statusRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.xs
})

const $laneBadge: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  borderRadius: 999,
  borderWidth: 1,
  borderColor: colors.palette.neutral400,
  backgroundColor: colors.palette.neutral200,
  paddingHorizontal: spacing.xs,
  paddingVertical: 2
})

const $handoffButton: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  borderRadius: 999,
  borderWidth: 1,
  borderColor: colors.palette.neutral400,
  backgroundColor: colors.palette.neutral200,
  paddingHorizontal: spacing.xs,
  paddingVertical: spacing.xxs
})

const $faint: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })

const $prose: TextStyle = { fontSize: 22, lineHeight: 32 }
const $reasoning: TextStyle = { fontSize: 20, lineHeight: 28 }
const $toolIcon: TextStyle = { width: 20, textAlign: "center", fontSize: 20 }
const $toolLabel: TextStyle = { minWidth: 0, flex: 1, fontSize: 20, lineHeight: 28 }
const $chevron: TextStyle = { fontSize: 24 }
const $laneBadgeText: TextStyle = { fontSize: 10, lineHeight: 14 }
const $handoffText: TextStyle = { fontSize: 10, lineHeight: 14, textTransform: "uppercase", letterSpacing: 0.5 }
