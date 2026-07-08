import type { KhalaRuntimeLane, RuntimeTurnEntity } from "@openagentsinc/khala-sync"
import { useEffect, useRef, useState } from "react"
import { Pressable, TextInput, View, type TextStyle, type ViewStyle } from "react-native"

import { ActivityIndicator } from "./activity-indicator"
import { TouchableFeedback } from "./touchable-feedback"
import { useKhalaAuth } from "../auth/khala-auth-context"
import { Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import { mergeTranscriptIntoDraft } from "../native/push-to-talk-core"
import { usePushToTalk } from "../native/use-push-to-talk"
import { registerForPushNotificationsAsync } from "../push/push-notifications-client"
import {
  buildAppendUserMessageIntentArgs,
  buildInterruptTurnIntentArgs,
  buildStartTurnIntentArgs,
  chatMessageBodyRef,
  DEFAULT_RUNTIME_LANE,
  type RuntimeControlIntentTarget
} from "../sync/khala-runtime-compose-core"
import { makeSafeRef } from "../sync/khala-sync-push-core"
import { buildComposerTextWithQuote } from "../sync/swipe-quote-core"
import type { PendingMutation } from "../sync/use-khala-sync-push"

type SendMode = "steer" | "queue"

const TURN_STATUS_LABEL: Record<string, string> = {
  queued: "queued",
  running: "running",
  waiting_for_input: "waiting for input"
}

/** The lanes a user can actively pick from the composer (#8405, #8467) —
 * every other `KhalaRuntimeLane` literal is an internal routing lane
 * (ai_sdk_core, khala_sync_mobile_control, test_fixture, …), never a provider
 * a person chooses in this UI. "Khala" is the server-hosted default lane
 * (drained on Cloud Run, no local Pylon needed); Codex/Claude route to the
 * user's own local Pylon runtime. */
export type ChatComposerExecutionTarget = Readonly<{
  label: string
  target: RuntimeControlIntentTarget
}>

const DEFAULT_EXECUTION_TARGETS: ReadonlyArray<ChatComposerExecutionTarget> = [
  { label: "Khala", target: { executionTargetId: "khala", lane: "hosted_khala" } },
  { label: "Codex", target: { lane: "codex_app_server" } },
  { label: "Claude", target: { lane: "claude_pylon" } }
]

const executionTargetKey = (target: RuntimeControlIntentTarget): string =>
  `${target.lane}.${target.executionTargetId ?? "lane"}`

export type ChatComposerAppendMessage = (
  input: Readonly<{ body: string; messageId: string; threadId: string }>,
) => Promise<Readonly<{ ok: boolean; error?: string }>>

type ChatComposerProps = Readonly<{
  threadId: string
  activeTurn: RuntimeTurnEntity | undefined
  /** Optimistic chat-message append (the sync runtime's overlay-backed
   * `appendMessage`). Bug fix (2026-07-07: "sending a message does nothing"):
   * the chat message is written through the OVERLAY here so it shows in the
   * transcript IMMEDIATELY (local-first) and is durably queued through the
   * same sync session, instead of only going out on the raw control-intent
   * push (a separate client group that produced no optimistic local row).
   * The `runtime.startTurn` / `runtime.appendUserMessage` control intent is
   * still sent via `push` AFTER the message is durably committed, so the
   * dispatch consumer can always resolve the message the intent references.
   * `undefined` while the sync runtime is still opening (send is gated off). */
  appendMessage?: ChatComposerAppendMessage
  /** Which lane to preselect for the NEXT brand-new turn (#8405) — normally
   * the thread's most recent turn's lane (`mostRecentTurnLane`), so a
   * thread that's always talked to Claude keeps defaulting to Claude.
   * `undefined` (a thread with no turns yet) falls back to
   * `DEFAULT_RUNTIME_LANE`. Only read while idle; once a turn is running,
   * its own already-fixed lane governs steer/queue/stop, not this prop. */
  defaultLane?: KhalaRuntimeLane
  /** Per-thread execution targets for the idle picker (CX-4, #8548). When
   * account health is loaded, parent screens pass account-specific targets
   * such as `{ label: "Your Codex", target: { lane: "codex_app_server",
   * executionTargetId: "codex:<accountRefHash>" } }`. The fallback list keeps
   * the pre-existing lane picker behavior while older parents roll forward. */
  executionTargets?: ReadonlyArray<ChatComposerExecutionTarget>
  push: (mutations: ReadonlyArray<PendingMutation>) => Promise<unknown>
  /** Swipe-to-quote request from the thread's transcript list (`SwipeableItem`
   * in `app/thread/[threadId].tsx`, see issue #8393). `id` is the swiped
   * transcript part's own id, so quoting it merges the snippet into the
   * draft exactly once even if this component re-renders before the parent
   * clears its pending-request state. `onQuoteConsumed` clears that state. */
  quoteRequest?: { id: string; snippet: string }
  onQuoteConsumed?: () => void
}>

/** Bottom input bar for a thread, on the ported Infinite Red Ignite `Text`
 * primitive + theme tokens (`../ignite`). Idle: plain send starts a new turn,
 * using whichever lane (Codex/Claude) is picked in the small idle-only lane
 * toggle. While a turn is active: the trailing button becomes Stop (always
 * reachable), and typing a follow-up surfaces an explicit Steer-vs-Queue
 * choice — steer attaches to the running turn's context now, queue starts a
 * distinct turn that waits for the current one to settle. Both of those
 * stay on the ACTIVE turn's own lane (never the idle picker's current
 * value) — a running turn's provider can't be changed mid-flight from here;
 * that's cross-agent delegation, #8407. */
export const ChatComposer = ({
  activeTurn,
  appendMessage,
  defaultLane,
  executionTargets = DEFAULT_EXECUTION_TARGETS,
  onQuoteConsumed,
  push,
  quoteRequest,
  threadId
}: ChatComposerProps) => {
  const { theme, themed } = useAppTheme()
  const [text, setText] = useState("")
  const [sending, setSending] = useState(false)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [mode, setMode] = useState<SendMode>("steer")
  const [selectedExecutionTarget, setSelectedExecutionTarget] = useState<RuntimeControlIntentTarget>(
    executionTargets.find(option => option.target.lane === (defaultLane ?? DEFAULT_RUNTIME_LANE))?.target ?? {
      lane: defaultLane ?? DEFAULT_RUNTIME_LANE
    },
  )
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
  const { baseUrl, token } = useKhalaAuth()

  // `defaultLane` often arrives after first render (the runtime_turn
  // collection is still loading), so keep syncing to it until the user
  // deliberately picks a lane themselves — after that, respect their choice
  // even if `defaultLane` recomputes (e.g. a new turn lands from elsewhere).
  useEffect(() => {
    if (!laneTouched && defaultLane !== undefined) {
      setSelectedExecutionTarget(
        executionTargets.find(option => option.target.lane === defaultLane)?.target ?? { lane: defaultLane },
      )
    }
  }, [defaultLane, executionTargets, laneTouched])

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
  // Send needs the overlay-backed append path (the sync runtime) — without it
  // a control intent would reference a chat message that was never created,
  // which is exactly the "sending does nothing" bug. Disabled (not silently
  // no-op) while the runtime is still opening.
  const canSend = trimmed.length > 0 && !sending && appendMessage !== undefined

  const sendMessage = async (sendMode: SendMode) => {
    if (!canSend) return
    setSending(true)
    setErrorMessage(null)
    const body = trimmed
    const nowIso = new Date().toISOString()
    const messageId = makeSafeRef("msg")
    const bodyRef = chatMessageBodyRef(messageId)
    try {
      // Optimistic, local-first chat-message append: shows the message in the
      // transcript IMMEDIATELY (overlay-backed) and durably commits it through
      // the sync session BEFORE the control intent that references it goes out
      // — so a plain send can never look like it "did nothing" (2026-07-07).
      // The append is idempotent by messageId; awaiting its commit here keeps
      // the message ahead of the turn intent so the dispatch consumer can
      // always resolve `bodyRef`.
      if (appendMessage !== undefined) {
        const appended = await appendMessage({ body, messageId, threadId })
        if (!appended.ok) {
          throw new Error(appended.error ?? "Could not send your message.")
        }
      }
      if (hasActiveTurn && sendMode === "steer" && activeTurn !== undefined) {
        // Steering attaches to a turn that's already dispatching on a fixed
        // provider — target its lane, not whatever the (hidden, while a
        // turn is active) idle picker currently holds.
        await push([
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
        const target =
          hasActiveTurn && activeTurn !== undefined ? { lane: activeTurn.lane } : selectedExecutionTarget
        await push([
          { args: buildStartTurnIntentArgs({ bodyRef, nowIso, target, threadId, turnId }), name: "runtime.startTurn" }
        ])
        // Push notification permission prompt fires exactly here — the first
        // time the user ever dispatches a task, never on app launch (MM-G1,
        // #8485; khala_mobile.push.permission_prompt_on_first_task_dispatch.v1).
        // Fire-and-forget: never blocks or fails the send.
        void registerForPushNotificationsAsync({
          apiBaseUrl: baseUrl,
          bearerToken: token,
          event: "task_dispatched"
        })
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

  const pill = (selected: boolean): ViewStyle => ({
    flex: 1,
    alignItems: "center",
    borderRadius: 999,
    borderWidth: 1,
    paddingVertical: theme.spacing.xs,
    borderColor: selected ? theme.colors.tint : theme.colors.palette.neutral400,
    backgroundColor: selected ? theme.colors.palette.neutral300 : theme.colors.palette.neutral200
  })

  const optionRow = hasActiveTurn ? (
    <View style={themed($optionRowWrap)}>
      <Pressable
        accessibilityRole="button"
        accessibilityState={{ selected: mode === "steer" }}
        style={pill(mode === "steer")}
        onPress={() => setMode("steer")}
      >
        <Text size="xxs" style={themed($pillLabel)} text="Steer" />
      </Pressable>
      <Pressable
        accessibilityRole="button"
        accessibilityState={{ selected: mode === "queue" }}
        style={pill(mode === "queue")}
        onPress={() => setMode("queue")}
      >
        <Text size="xxs" style={themed($pillLabel)} text="Queue" />
      </Pressable>
    </View>
  ) : (
    <View accessibilityLabel="Provider" style={themed($optionRowWrap)}>
      {executionTargets.map(({ label, target }) => (
        <Pressable
          accessibilityLabel={`Send with ${label}`}
          accessibilityRole="button"
          accessibilityState={{ selected: executionTargetKey(selectedExecutionTarget) === executionTargetKey(target) }}
          style={pill(executionTargetKey(selectedExecutionTarget) === executionTargetKey(target))}
          key={executionTargetKey(target)}
          onPress={() => {
            setLaneTouched(true)
            setSelectedExecutionTarget(target)
          }}
        >
          <Text size="xxs" style={themed($pillLabel)} text={label} />
        </Pressable>
      ))}
    </View>
  )

  return (
    <View style={themed($container)}>
      {errorMessage === null ? null : (
        <Text size="xxs" numberOfLines={2} style={themed($error)} text={errorMessage} />
      )}
      {activeStatusLabel === undefined ? null : (
        <View style={themed($statusBadge)}>
          {activeTurn?.status === "running" ? (
            <ActivityIndicator color={theme.colors.tint} size={12} />
          ) : (
            <View style={themed($statusDot)} />
          )}
          <Text size="xxs" style={themed($statusLabel)} text={activeStatusLabel} />
        </View>
      )}
      {showOptions ? optionRow : null}
      <View style={themed($inputPill)}>
        <Pressable
          accessibilityLabel={showOptions ? "Hide composer options" : "Show composer options"}
          accessibilityRole="button"
          style={themed($iconButton)}
          hitSlop={8}
          onPress={() => setShowOptions(current => !current)}
        >
          <Text style={[$plusGlyph, { color: theme.colors.text }]}>+</Text>
        </Pressable>
        <TextInput
          style={themed($textInput)}
          multiline
          onChangeText={setText}
          placeholder={hasActiveTurn ? "Follow up" : "Message"}
          placeholderTextColor={theme.colors.textDim}
          value={text}
        />
        <Pressable
          accessibilityLabel={pushToTalk.accessibilityLabel}
          accessibilityRole="button"
          style={themed($iconButton)}
          disabled={!pushToTalk.pressable}
          onPress={pushToTalk.press}
        >
          {pushToTalk.phase === "checking" ? (
            <ActivityIndicator color={theme.colors.textDim} size={20} />
          ) : (
            <Text
              style={[
                $micGlyph,
                {
                  color:
                    pushToTalk.phase === "recording"
                      ? theme.colors.error
                      : pushToTalk.pressable
                        ? theme.colors.text
                        : theme.colors.textDim
                }
              ]}
            >
              {pushToTalk.phase === "recording" ? "●" : "◉"}
            </Text>
          )}
        </Pressable>
        {hasActiveTurn ? (
          <TouchableFeedback
            accessibilityLabel="Stop"
            accessibilityRole="button"
            style={[themed($sendButton), { backgroundColor: theme.colors.text }]}
            disabled={sending}
            highlightColor="rgba(0, 0, 0, 0.14)"
            onPress={stopActiveTurn}
          >
            {sending ? (
              <ActivityIndicator color={theme.colors.background} size={24} />
            ) : (
              <Text style={[$stopGlyph, { color: theme.colors.background }]}>■</Text>
            )}
          </TouchableFeedback>
        ) : (
          <TouchableFeedback
            accessibilityLabel="Send"
            accessibilityRole="button"
            style={[themed($sendButton), { backgroundColor: canSend ? theme.colors.text : theme.colors.palette.neutral400 }]}
            disabled={!canSend}
            highlightColor="rgba(0, 0, 0, 0.14)"
            onPress={() => sendMessage("queue")}
          >
            {sending ? (
              <ActivityIndicator color={theme.colors.background} size={24} />
            ) : (
              <Text style={[$sendGlyph, { color: canSend ? theme.colors.background : theme.colors.textDim }]}>↑</Text>
            )}
          </TouchableFeedback>
        )}
      </View>
    </View>
  )
}

/** Zero on every platform. The thread screen's `KeyboardAvoidingView` uses
 * `padding` on iOS with THIS offset; the SafeAreaView's bottom inset already
 * sits below that view, so any non-zero offset double-counts and opens a dead
 * gap under the input when the keyboard is up (owner report, 2026-07-06). */
export const chatComposerKeyboardVerticalOffset = 0

const $container: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  backgroundColor: "transparent",
  paddingHorizontal: spacing.md,
  paddingTop: spacing.xs,
  paddingBottom: spacing.sm
})

const $optionRowWrap: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexDirection: "row",
  gap: spacing.xs,
  marginBottom: spacing.xs,
  paddingHorizontal: spacing.xxxs
})

const $pillLabel: ThemedStyle<TextStyle> = ({ colors }) => ({
  color: colors.text,
  textTransform: "uppercase",
  letterSpacing: 0.5
})

const $statusBadge: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.xs,
  alignSelf: "flex-start",
  marginBottom: spacing.xs,
  borderRadius: 999,
  borderWidth: 1,
  borderColor: colors.tint,
  backgroundColor: colors.palette.neutral200,
  paddingHorizontal: spacing.sm,
  paddingVertical: spacing.xxs
})

const $statusDot: ThemedStyle<ViewStyle> = ({ colors }) => ({
  height: 8,
  width: 8,
  borderRadius: 4,
  backgroundColor: colors.tint
})

const $statusLabel: ThemedStyle<TextStyle> = ({ colors }) => ({
  color: colors.tint,
  textTransform: "uppercase",
  letterSpacing: 0.5
})

const $inputPill: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  minHeight: 64,
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.xs,
  borderRadius: 999,
  borderWidth: 1,
  borderColor: colors.palette.neutral400,
  backgroundColor: colors.palette.neutral300,
  paddingHorizontal: spacing.sm,
  paddingVertical: spacing.xs
})

const $iconButton: ThemedStyle<ViewStyle> = () => ({
  height: 44,
  width: 44,
  alignItems: "center",
  justifyContent: "center",
  borderRadius: 22
})

const $textInput: ThemedStyle<TextStyle> = ({ colors, typography, spacing }) => ({
  maxHeight: 112,
  minHeight: 40,
  flex: 1,
  paddingHorizontal: spacing.xxs,
  paddingVertical: spacing.xs,
  fontFamily: typography.primary.normal,
  fontSize: 19,
  lineHeight: 24,
  color: colors.text
})

const $error: ThemedStyle<TextStyle> = ({ colors, spacing }) => ({
  color: colors.error,
  marginBottom: spacing.xxs,
  paddingHorizontal: spacing.sm
})

const $sendButton: ThemedStyle<ViewStyle> = () => ({
  height: 48,
  width: 48,
  alignItems: "center",
  justifyContent: "center",
  borderRadius: 24
})

const $plusGlyph: TextStyle = { fontSize: 34, lineHeight: 36 }
const $micGlyph: TextStyle = { fontSize: 30, lineHeight: 32 }
const $stopGlyph: TextStyle = { fontSize: 20, lineHeight: 24 }
const $sendGlyph: TextStyle = { fontSize: 26, lineHeight: 32 }
