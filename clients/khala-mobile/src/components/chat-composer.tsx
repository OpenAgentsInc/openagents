import type { RuntimeTurnEntity } from "@openagentsinc/khala-sync"
import { useState } from "react"
import { ActivityIndicator, Platform, Pressable, Text, TextInput, View } from "react-native"
import Animated, { useAnimatedStyle, useDerivedValue, withTiming } from "react-native-reanimated"

import {
  buildAppendUserMessageIntentArgs,
  buildChatAppendMessageArgs,
  buildInterruptTurnIntentArgs,
  buildStartTurnIntentArgs,
  chatMessageBodyRef
} from "../sync/khala-runtime-compose-core"
import { makeSafeRef } from "../sync/khala-sync-push-core"
import type { PendingMutation } from "../sync/use-khala-sync-push"

type SendMode = "steer" | "queue"

const TURN_STATUS_LABEL: Record<string, string> = {
  queued: "queued",
  running: "running",
  waiting_for_input: "waiting for input"
}

type ChatComposerProps = Readonly<{
  threadId: string
  activeTurn: RuntimeTurnEntity | undefined
  push: (mutations: ReadonlyArray<PendingMutation>) => Promise<unknown>
}>

/** Bottom input bar for a thread. Idle: plain send starts a new turn. While
 * a turn is active: the trailing button becomes Stop (always reachable),
 * and typing a follow-up surfaces an explicit Steer-vs-Queue choice — steer
 * attaches to the running turn's context now, queue starts a distinct turn
 * that waits for the current one to settle. */
export const ChatComposer = ({ activeTurn, push, threadId }: ChatComposerProps) => {
  const [text, setText] = useState("")
  const [sending, setSending] = useState(false)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [mode, setMode] = useState<SendMode>("steer")

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
        await push([
          chatMutation,
          {
            args: buildAppendUserMessageIntentArgs({
              bodyRef,
              messageId,
              nowIso,
              threadId,
              turnId: activeTurn.turnId
            }),
            name: "runtime.appendUserMessage"
          }
        ])
      } else {
        const turnId = makeSafeRef("turn")
        await push([
          chatMutation,
          { args: buildStartTurnIntentArgs({ bodyRef, nowIso, threadId, turnId }), name: "runtime.startTurn" }
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
      ) : null}
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
          <Pressable
            accessibilityLabel="Stop"
            accessibilityRole="button"
            className="h-10 w-10 items-center justify-center rounded-full bg-danger"
            disabled={sending}
            hitSlop={8}
            onPress={stopActiveTurn}
          >
            {sending ? <ActivityIndicator color="#fff" size="small" /> : <Text className="text-base text-text">■</Text>}
          </Pressable>
        ) : (
          <Pressable
            accessibilityLabel="Send"
            accessibilityRole="button"
            className={`h-10 w-10 items-center justify-center rounded-full ${canSend ? "bg-accent" : "bg-surfaceMuted"}`}
            disabled={!canSend}
            hitSlop={8}
            onPress={() => sendMessage("queue")}
          >
            {sending ? <ActivityIndicator color="#000" size="small" /> : <Text className="text-lg text-bg">↑</Text>}
          </Pressable>
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
