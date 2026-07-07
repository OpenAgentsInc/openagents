import type { Meta, StoryObj } from "@storybook/react-native"
import type { KhalaRuntimeLane, RuntimeTurnEntity } from "@openagentsinc/khala-sync"
import { View } from "react-native"
import { useSharedValue } from "react-native-reanimated"

import { KhalaAuthProvider } from "../auth/khala-auth-context"
import type { TranscriptPart } from "../sync/khala-runtime-transcript-core"
import { ChatComposer } from "./chat-composer"
import { KhalaListItem } from "./khala-list-item"
import { ReText } from "./re-text"
import { SwipeableItem } from "./swipeable-item"
import { SwipeQuoteDonut } from "./swipeable-item/swipe-quote-donut"
import { TranscriptPartRow } from "./transcript-part-row"

const meta = {
  title: "Khala/Components/Chat",
  component: View,
  parameters: { layout: "fullscreen" },
  decorators: [
    (Story) => (
      <KhalaAuthProvider>
        <View className="flex-1 gap-4 p-5">
          <Story />
        </View>
      </KhalaAuthProvider>
    ),
  ],
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

const transcriptParts: TranscriptPart[] = [
  { id: "status-running", kind: "turn-status", lane: "codex_app_server", status: "running", turnId: "turn_story_running" },
  { id: "text-1", kind: "text", text: "I found the local Storybook port issue and pointed this simulator at 8082." },
  { id: "reasoning-1", kind: "reasoning", text: "Checking the native shell before touching another agent's simulator." },
  {
    id: "tool-1",
    kind: "tool",
    status: "completed",
    toolCallId: "call_story_1",
    toolName: "exec_command",
  },
  { id: "usage-1", kind: "usage", inputTokens: 1240, outputTokens: 380, totalTokens: 1620 },
  { id: "status-completed", kind: "turn-status", lane: "claude_pylon", status: "completed", turnId: "turn_story_done" },
]

const activeTurn: RuntimeTurnEntity = {
  createdAt: new Date().toISOString(),
  eventCount: 3,
  lane: "codex_app_server" as KhalaRuntimeLane,
  latestIntentId: null,
  ownerUserId: "storybook-owner",
  settledAt: null,
  startedAt: new Date().toISOString(),
  status: "running",
  threadId: "thread_storybook",
  turnId: "turn_storybook",
  updatedAt: new Date().toISOString(),
}

const DonutExample = () => {
  const progress = useSharedValue(0.72)
  return <SwipeQuoteDonut progress={progress} size={56} strokeWidth={4} />
}

const ReTextExample = () => {
  const text = useSharedValue("Animated text input bridge")
  return <ReText className="text-text" text={text} />
}

export const TranscriptRows: Story = {
  render: () => (
    <View className="gap-3">
      {transcriptParts.map((part, index) => (
        <TranscriptPartRow
          key={`${part.kind}-${index}`}
          part={part}
          onRequestHandoff={() => undefined}
        />
      ))}
    </View>
  ),
}

export const SwipeToQuote: Story = {
  render: () => (
    <View className="gap-5">
      <SwipeableItem onSwipeComplete={() => undefined}>
        <KhalaListItem
          variant="surface"
          title="Swipe this transcript row"
          detail="A donut fills as the row moves far enough to quote."
          meta="Codex"
        />
      </SwipeableItem>
      <View className="flex-row items-center gap-4">
        <DonutExample />
        <ReTextExample />
      </View>
    </View>
  ),
}

export const ComposerIdle: Story = {
  render: () => (
    <ChatComposer
      activeTurn={undefined}
      appendMessage={async () => ({ ok: true })}
      defaultLane="hosted_khala"
      push={async () => undefined}
      threadId="thread_storybook"
    />
  ),
}

export const ComposerWhileRunning: Story = {
  render: () => (
    <ChatComposer
      activeTurn={activeTurn}
      appendMessage={async () => ({ ok: true })}
      push={async () => undefined}
      quoteRequest={{ id: "quote_storybook", snippet: "The simulator is already running Storybook." }}
      threadId="thread_storybook"
    />
  ),
}
