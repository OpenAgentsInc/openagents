import { Pressable, Text, View } from "react-native"

import type { KhalaRuntimeLane } from "@openagentsinc/khala-sync"

import { handoffLaneLabel, handoffTargetLane } from "../sync/khala-cross-agent-handoff-core"
import type { TranscriptPart } from "../sync/khala-runtime-transcript-core"
import { BackgroundGradient } from "./background-gradient"

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
 * the raw lane string rather than inventing a label nobody picked. Reuses
 * `handoffLaneLabel` (#8407) so this file carries only one lane->label
 * mapping instead of two. */
const laneLabel = handoffLaneLabel

const TOOL_STATUS_COLOR: Record<Extract<TranscriptPart, { kind: "tool" }>["status"], string> = {
  called: "text-warning",
  completed: "text-success",
  failed: "text-danger"
}

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

/** Renders one AI-SDK-shaped runtime transcript part with a distinct
 * component per part type — text bubble, collapsible-style reasoning
 * block, tool-call/result card, usage footer, turn-status divider. */
export const TranscriptPartRow = ({
  handoffDisabled,
  handoffPending,
  onRequestHandoff,
  part
}: TranscriptPartRowProps) => {
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
    case "tool": {
      // In-flight (`called`, not yet `completed`/`failed`) tool calls drop
      // the static `bg-surface` fill and sit inside a `BackgroundGradient`
      // (ported from Arcade, see
      // `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.9) so the
      // card reads as a "live" in-progress surface. Settled cards keep the
      // plain static fill so the ambient signal stays meaningful.
      const isInFlight = part.status === "called"
      const card = (
        <View className={`rounded-xl border border-border px-3 py-2 ${isInFlight ? "" : "bg-surface"}`}>
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
      if (!isInFlight) return card
      return (
        <BackgroundGradient cornerRadius={12} style={{ borderRadius: 12, overflow: "hidden" }}>
          {card}
        </BackgroundGradient>
      )
    }
    case "usage":
      return (
        <Text className="font-mono text-xs text-textFaint">
          {part.inputTokens ?? 0} in · {part.outputTokens ?? 0} out · {part.totalTokens ?? 0} total
          tokens
        </Text>
      )
    case "turn-status": {
      // "Ask [other provider] to review this" (#8407) — only on a completed
      // turn, and only when the producing lane has a real other-lane target
      // (the two user-pickable lanes; every internal routing lane has none,
      // so `handoffTargetLane` returns undefined and the button just doesn't
      // render — no dead/disabled button for a case that can never work).
      const targetLane = part.status === "completed" ? handoffTargetLane(part.lane) : undefined
      const canRequestHandoff = targetLane !== undefined && onRequestHandoff !== undefined
      return (
        <View className="items-center gap-1 self-center">
          <View className="flex-row items-center justify-center gap-1.5">
            <Text className="font-mono text-xs uppercase tracking-wide text-textFaint">
              — {TURN_STATUS_LABEL[part.status]} —
            </Text>
            <View className="rounded-full border border-borderMuted bg-surface px-1.5 py-0.5">
              <Text className="font-mono text-[10px] uppercase tracking-wide text-textFaint">
                {laneLabel(part.lane)}
              </Text>
            </View>
          </View>
          {canRequestHandoff ? (
            <Pressable
              accessibilityLabel={`Ask ${laneLabel(targetLane)} to review this`}
              accessibilityRole="button"
              className="rounded-full border border-borderMuted bg-surface px-2 py-0.5"
              disabled={handoffDisabled === true || handoffPending === true}
              onPress={() =>
                onRequestHandoff({ sourceLane: part.lane, targetLane, turnId: part.turnId })
              }
            >
              <Text className="font-mono text-[10px] uppercase tracking-wide text-accent">
                {handoffPending === true ? "asking…" : `ask ${laneLabel(targetLane)} to review this`}
              </Text>
            </Pressable>
          ) : null}
        </View>
      )
    }
  }
}
