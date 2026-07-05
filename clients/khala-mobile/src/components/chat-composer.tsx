import type { KhalaRuntimeLane, RuntimeTurnEntity } from "@openagentsinc/khala-sync"
import { useEffect, useState } from "react"
import { Platform, Pressable, Text, TextInput, View } from "react-native"
import Animated, { useAnimatedStyle, useDerivedValue, withTiming } from "react-native-reanimated"

import { ActivityIndicator } from "./activity-indicator"
import { ArwesButton } from "./arwes-button"
import {
  buildAppendUserMessageIntentArgs,
  buildChatAppendMessageArgs,
  buildInterruptTurnIntentArgs,
  buildStartTurnIntentArgs,
  chatMessageBodyRef,
  DEFAULT_RUNTIME_LANE
} from "../sync/khala-runtime-compose-core"
import { makeSafeRef } from "../sync/khala-sync-push-core"
import type { PendingMutation } from "../sync/use-khala-sync-push"
import { khalaMobileTheme } from "../theme/tokens"

type SendMode = "steer" | "queue"

const TURN_STATUS_LABEL: Record<string, string> = {
  queued: "queued",
  running: "running",
  waiting_for_input: "waiting for input"
}

/** The only two lanes a user can actively pick from the composer today
 * (#8405) — every other `KhalaRuntimeLane` literal is an internal routing
 * lane (ai_sdk_core, khala_sync_mobile_control, test_fixture, …), never a
 * provider a person chooses in this UI. */
const PICKABLE_LANES: ReadonlyArray<{ lane: KhalaRuntimeLane; label: string }> = [
  { label: "Codex", lane: "codex_app_server" },
  { label: "Claude", lane: "claude_pylon" }
]

type ChatComposerProps = Readonly<{
  threadId: string
  activeTurn: RuntimeTurnEntity | undefined
  /** Which lane to preselect for the NEXT brand-new turn (#8405) — normally
   * the thread's most recent turn's lane (`mostRecentTurnLane`), so a
   * thread that's always talked to Claude keeps defaulting to Claude.
   * `undefined` (a thread with no turns yet) falls back to
   * `DEFAULT_RUNTIME_LANE`. Only read while idle; once a turn is running,
   * its own already-fixed lane governs steer/queue/stop, not this prop. */
  defaultLane?: KhalaRuntimeLane
  push: (mutations: ReadonlyArray<PendingMutation>) => Promise<unknown>
}>

/** Bottom input bar for a thread. Idle: plain send starts a new turn, using
 * whichever lane (Codex/Claude) is picked in the small idle-only lane
 * toggle. While a turn is active: the trailing button becomes Stop (always
 * reachable), and typing a follow-up surfaces an explicit Steer-vs-Queue
 * choice — steer attaches to the running turn's context now, queue starts a
 * distinct turn that waits for the current one to settle. Both of those
 * stay on the ACTIVE turn's own lane (never the idle picker's current
 * value) — a running turn's provider can't be changed mid-flight from here;
 * that's cross-agent delegation, #8407. */
export const ChatComposer = ({ activeTurn, defaultLane, push, threadId }: ChatComposerProps) => {
  const [text, setText] = useState("")
  const [sending, setSending] = useState(false)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [mode, setMode] = useState<SendMode>("steer")
  const [selectedLane, setSelectedLane] = useState<KhalaRuntimeLane>(defaultLane ?? DEFAULT_RUNTIME_LANE)
  const [laneTouched, setLaneTouched] = useState(false)

  // `defaultLane` often arrives after first render (the runtime_turn
  // collection is still loading), so keep syncing to it until the user
  // deliberately picks a lane themselves — after that, respect their choice
  // even if `defaultLane` recomputes (e.g. a new turn lands from elsewhere).
  useEffect(() => {
    if (!laneTouched && defaultLane !== undefined) setSelectedLane(defaultLane)
  }, [defaultLane, laneTouched])

  const trimmed = text.trim()
  const hasActiveTurn = activeTurn !== undefined
  const canSend = trimmed.length > 0 && !sending
  const showPicker = hasActiveTurn && trimmed.length > 0
  const [pickerContentHeight, setPickerContentHeight] = useState(0)
  // One `progress` value drives both height and opacity together, so the
  // picker row opens/closes as one coherent motion instead of separately
  // timed properties drifting out of sync (ported technique from Arcade's
  // `DirectMessageReply`).
  const pickerProgress = useDerivedValue(() => withTiming(showPicker ? 1 : 0, { duration: 180 }))
  const pickerAnimatedStyle = useAnimatedStyle(() => ({
    height: pickerProgress.value * pickerContentHeight,
    opacity: pickerProgress.value
  }))

  const sendMessage = async (sendMode: SendMode) => {
    if (!canSend) return
    setSending(true)
    setErrorMessage(null)
    const body = trimmed
    const nowIso = new Date().toISOString()
    const messageId = makeSafeRef("msg")
    const bodyRef = chatMessageBodyRef(messageId)
    try {
      const chatMutation: PendingMutation = {
        args: buildChatAppendMessageArgs({ body, messageId, threadId }),
        name: "chat.appendMessage"
      }
      if (hasActiveTurn && sendMode === "steer" && activeTurn !== undefined) {
        // Steering attaches to a turn that's already dispatching on a fixed
        // provider — target its lane, not whatever the (hidden, while a
        // turn is active) idle picker currently holds.
        await push([
          chatMutation,
          {
            args: buildAppendUserMessageIntentArgs({
              bodyRef,
              messageId,
              nowIso,
              target: { lane: activeTurn.lane },
              threadId,
              turnId: activeTurn.turnId
            }),
            name: "runtime.appendUserMessage"
          }
        ])
      } else {
        // A brand-new turn: idle send uses the picker's current selection;
        // "Queue (after this turn)" while one is active keeps the SAME lane
        // as the turn it's queued behind, so a thread doesn't silently
        // switch providers mid-conversation via the hidden picker value.
        const turnId = makeSafeRef("turn")
        const target = { lane: hasActiveTurn && activeTurn !== undefined ? activeTurn.lane : selectedLane }
        await push([
          chatMutation,
          { args: buildStartTurnIntentArgs({ bodyRef, nowIso, target, threadId, turnId }), name: "runtime.startTurn" }
        ])
      }
      setText("")
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error))
    } finally {
      setSending(false)
    }
  }

  const stopActiveTurn = async () => {
    if (activeTurn === undefined || sending) return
    setSending(true)
    setErrorMessage(null)
    try {
      await push([
        {
          args: buildInterruptTurnIntentArgs({
            nonce: makeSafeRef("nonce"),
            nowIso: new Date().toISOString(),
            target: { lane: activeTurn.lane },
            threadId,
            turnId: activeTurn.turnId
          }),
          name: "runtime.interruptTurn"
        }
      ])
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error))
    } finally {
      setSending(false)
    }
  }

  const pickerRow = (
    <>
      <Pressable
        accessibilityRole="button"
        className={`flex-1 items-center rounded-lg border py-2 ${
          mode === "steer" ? "border-accent bg-surfaceActive" : "border-borderMuted bg-surface"
        }`}
        onPress={() => setMode("steer")}
      >
        <Text className="font-mono text-xs text-text">Steer (send now)</Text>
      </Pressable>
      <Pressable
        accessibilityRole="button"
        className={`flex-1 items-center rounded-lg border py-2 ${
          mode === "queue" ? "border-accent bg-surfaceActive" : "border-borderMuted bg-surface"
        }`}
        onPress={() => setMode("queue")}
      >
        <Text className="font-mono text-xs text-text">Queue (after this turn)</Text>
      </Pressable>
      <Pressable
        accessibilityLabel="Send follow-up"
        accessibilityRole="button"
        className="items-center justify-center rounded-lg bg-accent px-4 py-2"
        disabled={sending}
        onPress={() => sendMessage(mode)}
      >
        <Text className="font-mono text-xs text-bg">Send</Text>
      </Pressable>
    </>
  )

  return (
    <View className="border-t border-borderMuted bg-bg px-3 pb-2 pt-2">
      {errorMessage === null ? null : (
        <Text className="mb-1 font-mono text-xs text-danger" numberOfLines={2}>
          {errorMessage}
        </Text>
      )}
      {hasActiveTurn ? (
        <Text className="mb-1 font-mono text-xs uppercase tracking-wide text-textFaint">
          ● turn {TURN_STATUS_LABEL[activeTurn.status] ?? activeTurn.status}
        </Text>
      ) : (
        // Lane picker (#8405) — only meaningful while idle: a running
        // turn's provider is already fixed, so hide this rather than imply
        // it could retarget an in-flight turn. Kept as two tiny pills (not
        // a heavy picker) reusing the Steer/Queue toggle's visual pattern.
        <View accessibilityLabel="Provider" className="mb-1 flex-row gap-1.5 self-start">
          {PICKABLE_LANES.map(({ label, lane }) => (
            <Pressable
              accessibilityLabel={`Send with ${label}`}
              accessibilityRole="button"
              accessibilityState={{ selected: selectedLane === lane }}
              className={`rounded-full border px-2 py-0.5 ${
                selectedLane === lane ? "border-accent bg-surfaceActive" : "border-borderMuted bg-surface"
              }`}
              key={lane}
              onPress={() => {
                setLaneTouched(true)
                setSelectedLane(lane)
              }}
            >
              <Text className="font-mono text-[10px] uppercase tracking-wide text-textFaint">{label}</Text>
            </Pressable>
          ))}
        </View>
      )}
      <View className="flex-row items-end gap-2">
        <TextInput
          className="max-h-32 flex-1 rounded-2xl border border-border bg-surfaceRaised px-3 py-2 font-sans text-base text-text"
          multiline
          onChangeText={setText}
          placeholder={hasActiveTurn ? "Send a follow-up…" : "Message…"}
          placeholderTextColor="#7e8a98"
          value={text}
        />
        {hasActiveTurn ? (
          // `ArwesButton` (ported from Arcade, see
          // `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.2) pairs
          // the Skia `Frame`'s press-glow with the primary composer CTA,
          // replacing the old instant `bg-danger` className swap with no
          // press feedback.
          <ArwesButton
            accessibilityLabel="Stop"
            alwaysShowBorder
            borderColor={khalaMobileTheme.danger}
            color={khalaMobileTheme.danger}
            disabled={sending}
            onPress={stopActiveTurn}
            style={{ height: 44, width: 44 }}
          >
            <View className="h-11 w-11 items-center justify-center">
              {sending ? (
                <ActivityIndicator color={khalaMobileTheme.danger} size={24} />
              ) : (
                <Text className="text-base text-danger">■</Text>
              )}
            </View>
          </ArwesButton>
        ) : (
          <ArwesButton
            accessibilityLabel="Send"
            alwaysShowBackground={canSend}
            alwaysShowBorder
            disabled={!canSend}
            onPress={() => sendMessage("queue")}
            style={{ height: 44, width: 44 }}
          >
            <View className="h-11 w-11 items-center justify-center">
              {sending ? (
                <ActivityIndicator color={khalaMobileTheme.accent} size={24} />
              ) : (
                <Text className={`text-lg ${canSend ? "text-accent" : "text-textFaint"}`}>↑</Text>
              )}
            </View>
          </ArwesButton>
        )}
      </View>
      <View>
        <Animated.View className="mt-2 overflow-hidden" style={pickerAnimatedStyle}>
          <View className="flex-row gap-2">{pickerRow}</View>
        </Animated.View>
        {/* Invisible, always-mounted twin of the row above, used only to
         * measure its natural height via onLayout. The visible copy above
         * starts animated to height 0, so it can never self-measure — RN
         * doesn't lay out (and thus never fires onLayout for) children of a
         * zero-height `overflow: hidden` container. This clone is never
         * height-constrained, so its onLayout always fires with the row's
         * real height, independent of the open/close animation. */}
        <View
          className="absolute left-0 right-0 top-0 flex-row gap-2 opacity-0"
          onLayout={event => {
            const measured = event.nativeEvent.layout.height
            if (measured > 0 && measured !== pickerContentHeight) setPickerContentHeight(measured)
          }}
          pointerEvents="none"
        >
          {pickerRow}
        </View>
      </View>
    </View>
  )
}

export const chatComposerKeyboardVerticalOffset = Platform.select({ default: 0, ios: 88 })
