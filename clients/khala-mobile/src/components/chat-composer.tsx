import type { KhalaRuntimeLane, RuntimeTurnEntity } from "@openagentsinc/khala-sync"
import { useEffect, useRef, useState } from "react"
import { Platform, Pressable, TextInput, View } from "react-native"

import { ActivityIndicator } from "./activity-indicator"
import { KhalaText } from "./khala-text"
import { mergeTranscriptIntoDraft } from "../native/push-to-talk-core"
import { usePushToTalk } from "../native/use-push-to-talk"
import {
  buildAppendUserMessageIntentArgs,
  buildChatAppendMessageArgs,
  buildInterruptTurnIntentArgs,
  buildStartTurnIntentArgs,
  chatMessageBodyRef,
  DEFAULT_RUNTIME_LANE
} from "../sync/khala-runtime-compose-core"
import { makeSafeRef } from "../sync/khala-sync-push-core"
import { buildComposerTextWithQuote } from "../sync/swipe-quote-core"
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
  /** Swipe-to-quote request from the thread's transcript list (`SwipeableItem`
   * in `app/thread/[threadId].tsx`, see issue #8393). `id` is the swiped
   * transcript part's own id, so quoting it merges the snippet into the
   * draft exactly once even if this component re-renders before the parent
   * clears its pending-request state. `onQuoteConsumed` clears that state. */
  quoteRequest?: { id: string; snippet: string }
  onQuoteConsumed?: () => void
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
export const ChatComposer = ({
  activeTurn,
  defaultLane,
  onQuoteConsumed,
  push,
  quoteRequest,
  threadId
}: ChatComposerProps) => {
  const [text, setText] = useState("")
  const [sending, setSending] = useState(false)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [mode, setMode] = useState<SendMode>("steer")
  const [selectedLane, setSelectedLane] = useState<KhalaRuntimeLane>(defaultLane ?? DEFAULT_RUNTIME_LANE)
  const [laneTouched, setLaneTouched] = useState(false)
  const [showOptions, setShowOptions] = useState(false)
  const lastQuoteRequestId = useRef<string | undefined>(undefined)
  // Push-to-talk dictation (#8350) — ported from the never-routed
  // `legacy-screens/settings.tsx`'s availability probe, extended with the
  // actual press/transcribe state machine neither legacy screen had (see
  // `../native/use-push-to-talk.ts`'s doc comment for why both platforms'
  // native calls currently always reject).
  const pushToTalk = usePushToTalk({
    onError: message => setErrorMessage(message),
    onTranscript: transcript => setText(current => mergeTranscriptIntoDraft(current, transcript))
  })

  // `defaultLane` often arrives after first render (the runtime_turn
  // collection is still loading), so keep syncing to it until the user
  // deliberately picks a lane themselves — after that, respect their choice
  // even if `defaultLane` recomputes (e.g. a new turn lands from elsewhere).
  useEffect(() => {
    if (!laneTouched && defaultLane !== undefined) setSelectedLane(defaultLane)
  }, [defaultLane, laneTouched])

  // Merges a swipe-to-quote request into the draft exactly once per request
  // id (see `ChatComposerProps.quoteRequest` above), then tells the parent to
  // clear it. Guarding on the id (not just definedness) means a re-render
  // between "merge" and "parent clears its state" can't double-prepend the
  // same quote.
  useEffect(() => {
    if (quoteRequest === undefined || lastQuoteRequestId.current === quoteRequest.id) return
    lastQuoteRequestId.current = quoteRequest.id
    setText(current => buildComposerTextWithQuote(current, quoteRequest.snippet))
    onQuoteConsumed?.()
  }, [onQuoteConsumed, quoteRequest])

  const trimmed = text.trim()
  const hasActiveTurn = activeTurn !== undefined
  const canSend = trimmed.length > 0 && !sending

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
      setShowOptions(false)
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

  const activeStatusLabel =
    activeTurn === undefined ? undefined : TURN_STATUS_LABEL[activeTurn.status] ?? activeTurn.status

  const optionRow = hasActiveTurn ? (
    <View className="mb-2 flex-row gap-2 px-1">
      <Pressable
        accessibilityRole="button"
        accessibilityState={{ selected: mode === "steer" }}
        className={`flex-1 items-center rounded-full border py-2 ${
          mode === "steer" ? "border-accent bg-surfaceActive" : "border-borderMuted bg-surface"
        }`}
        onPress={() => setMode("steer")}
      >
        <KhalaText className="text-[11px] uppercase tracking-wide text-text" variant="faint">
          Steer
        </KhalaText>
      </Pressable>
      <Pressable
        accessibilityRole="button"
        accessibilityState={{ selected: mode === "queue" }}
        className={`flex-1 items-center rounded-full border py-2 ${
          mode === "queue" ? "border-accent bg-surfaceActive" : "border-borderMuted bg-surface"
        }`}
        onPress={() => setMode("queue")}
      >
        <KhalaText className="text-[11px] uppercase tracking-wide text-text" variant="faint">
          Queue
        </KhalaText>
      </Pressable>
    </View>
  ) : (
    <View accessibilityLabel="Provider" className="mb-2 flex-row gap-2 px-1">
      {PICKABLE_LANES.map(({ label, lane }) => (
        <Pressable
          accessibilityLabel={`Send with ${label}`}
          accessibilityRole="button"
          accessibilityState={{ selected: selectedLane === lane }}
          className={`flex-1 items-center rounded-full border py-2 ${
            selectedLane === lane ? "border-accent bg-surfaceActive" : "border-borderMuted bg-surface"
          }`}
          key={lane}
          onPress={() => {
            setLaneTouched(true)
            setSelectedLane(lane)
          }}
        >
          <KhalaText className="text-[11px] uppercase tracking-wide text-text" variant="faint">
            {label}
          </KhalaText>
        </Pressable>
      ))}
    </View>
  )

  return (
    <View className="bg-transparent px-4 pb-3 pt-2">
      {errorMessage === null ? null : (
        <KhalaText className="mb-1 px-3 text-danger" numberOfLines={2} variant="faint">
          {errorMessage}
        </KhalaText>
      )}
      {activeStatusLabel === undefined ? null : (
        <KhalaText className="mb-1 px-3 text-textFaint" variant="faint">
          turn {activeStatusLabel}
        </KhalaText>
      )}
      {showOptions ? optionRow : null}
      <View className="min-h-16 flex-row items-center gap-2 rounded-full border border-borderMuted bg-surfaceRaised px-3 py-2">
        <Pressable
          accessibilityLabel={showOptions ? "Hide composer options" : "Show composer options"}
          accessibilityRole="button"
          className="h-11 w-11 items-center justify-center rounded-full"
          hitSlop={8}
          onPress={() => setShowOptions(current => !current)}
        >
          <KhalaText className="text-[34px] leading-9 text-text" variant="body">
            +
          </KhalaText>
        </Pressable>
        <TextInput
          className="max-h-28 min-h-10 flex-1 px-1 py-2 font-sans text-[19px] leading-6 text-text"
          multiline
          onChangeText={setText}
          placeholder={hasActiveTurn ? "Follow up" : "Message"}
          placeholderTextColor={khalaMobileTheme.textMuted}
          value={text}
        />
        <Pressable
          accessibilityLabel={pushToTalk.accessibilityLabel}
          accessibilityRole="button"
          className="h-11 w-11 items-center justify-center rounded-full"
          disabled={!pushToTalk.pressable}
          onPress={pushToTalk.press}
        >
          {pushToTalk.phase === "checking" ? (
            <ActivityIndicator color={khalaMobileTheme.textMuted} size={20} />
          ) : (
            <KhalaText
              className={`text-[30px] leading-8 ${
                pushToTalk.phase === "recording"
                  ? "text-danger"
                  : pushToTalk.pressable
                    ? "text-text"
                    : "text-textFaint"
              }`}
              variant="body"
            >
              {pushToTalk.phase === "recording" ? "●" : "◉"}
            </KhalaText>
          )}
        </Pressable>
        {hasActiveTurn ? (
          <Pressable
            accessibilityLabel="Stop"
            accessibilityRole="button"
            className="h-12 w-12 items-center justify-center rounded-full bg-text"
            disabled={sending}
            onPress={stopActiveTurn}
          >
            {sending ? (
              <ActivityIndicator color={khalaMobileTheme.background} size={24} />
            ) : (
              <KhalaText className="text-[20px] leading-6 text-bg" variant="body">
                ■
              </KhalaText>
            )}
          </Pressable>
        ) : (
          <Pressable
            accessibilityLabel="Send"
            accessibilityRole="button"
            className={`h-12 w-12 items-center justify-center rounded-full ${canSend ? "bg-text" : "bg-surfaceMuted"}`}
            disabled={!canSend}
            onPress={() => sendMessage("queue")}
          >
            {sending ? (
              <ActivityIndicator color={khalaMobileTheme.background} size={24} />
            ) : (
              <KhalaText className={`text-[26px] leading-8 ${canSend ? "text-bg" : "text-textFaint"}`} variant="body">
                ↑
              </KhalaText>
            )}
          </Pressable>
        )}
      </View>
    </View>
  )
}

export const chatComposerKeyboardVerticalOffset = Platform.select({ default: 0, ios: 88 })
