import { useMemo, useState } from "react"
import { Pressable, Text, TextInput, View } from "react-native"

import { Pill, ScreenShell } from "../components/shell"
import { assertDelegationPrompt, validateDelegationPrompt } from "../security/delegation-prompt"
import { createMobileKhalaSyncPreviewState } from "../sync/khala-sync-mobile"

export default function ChatScreen() {
  const { chatThreads } = useMemo(createMobileKhalaSyncPreviewState, [])
  const [prompt, setPrompt] = useState("")
  const validation = validateDelegationPrompt(prompt)

  const submit = () => {
    try {
      assertDelegationPrompt(prompt)
    } catch {
      return
    }
    setPrompt("")
  }

  return (
    <ScreenShell
      subtitle="Owner-private sync scope"
      title="Chat"
    >
      <View className="gap-3">
        {chatThreads.map(thread => (
          <View
            className="rounded-xl border border-border bg-surfaceRaised p-4"
            key={thread.threadId}
          >
            <View className="flex-row items-center justify-between gap-3">
              <Text className="shrink font-sans text-lg font-semibold text-text">
                {thread.title}
              </Text>
              <Text className="font-mono text-sm tabular-nums text-textMuted">
                {thread.messageCount}
              </Text>
            </View>
            <Text className="mt-2 font-mono text-sm text-textFaint">
              {thread.threadId}
            </Text>
          </View>
        ))}
      </View>
      <View className="gap-3 rounded-xl border border-border bg-surface p-4">
        <TextInput
          aria-label="Message"
          className="min-h-28 rounded-lg border border-borderStrong bg-bg px-3 py-3 font-sans text-base text-text"
          multiline
          onChangeText={setPrompt}
          placeholder="Ask Khala"
          placeholderTextColor="#7e8a98"
          value={prompt}
        />
        <View className="flex-row items-center justify-between gap-3">
          <Pill tone={validation.ok ? "success" : "warning"}>
            {validation.ok ? "ready" : "blocked"}
          </Pill>
          <Pressable
            accessibilityRole="button"
            className="rounded-lg bg-accent px-4 py-2 active:bg-accentSoft"
            onPress={submit}
          >
            <Text className="font-sans text-base font-semibold text-bg">
              Send
            </Text>
          </Pressable>
        </View>
      </View>
    </ScreenShell>
  )
}
