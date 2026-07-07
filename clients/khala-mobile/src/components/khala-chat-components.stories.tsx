import type { Meta, StoryObj } from "@storybook/react-native"
import type { KhalaRuntimeLane, RuntimeTurnEntity } from "@openagentsinc/khala-sync"
import { Pressable, TextInput, View } from "react-native"
import { useSharedValue } from "react-native-reanimated"

import type { TranscriptPart } from "../sync/khala-runtime-transcript-core"
import { KhalaListItem } from "./khala-list-item"
import { KhalaText } from "./khala-text"
import { ReText } from "./re-text"
import { SwipeableItem } from "./swipeable-item"
import { SwipeQuoteDonut } from "./swipeable-item/swipe-quote-donut"

const meta = {
  title: "Khala/Components/Chat",
  component: View,
  parameters: { layout: "fullscreen" },
  decorators: [
    (Story) => (
      <View className="flex-1 gap-4 p-5">
        <Story />
      </View>
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

const TranscriptPartExample = ({ part }: { readonly part: TranscriptPart }) => {
  const title =
    part.kind === "turn-status"
      ? `${part.lane} ${part.status}`
      : part.kind === "tool"
        ? `${part.toolName} ${part.status}`
        : part.kind === "usage"
          ? `${part.totalTokens} tokens`
          : part.kind
  const detail =
    part.kind === "text" || part.kind === "reasoning"
      ? part.text
      : part.kind === "tool"
        ? part.toolCallId
        : part.kind === "usage"
          ? `${part.inputTokens} in / ${part.outputTokens} out`
          : part.kind === "writeback"
            ? `${part.repositoryFullName} ${part.pullRequestNumber === undefined ? part.branch : `PR #${part.pullRequestNumber}`}`
            : part.turnId

  return (
    <View className="rounded-lg border border-border bg-surfaceRaised p-3">
      <KhalaText className="font-semibold text-text" variant="caption">
        {title}
      </KhalaText>
      <KhalaText className="mt-1" variant="muted">
        {detail}
      </KhalaText>
    </View>
  )
}

const ComposerShell = ({ running = false }: { readonly running?: boolean }) => (
  <View className="gap-2 rounded-2xl border border-border bg-surfaceRaised p-3">
    {running ? (
      <View className="self-start rounded-full border border-accent/40 bg-accent/10 px-3 py-1">
        <KhalaText className="text-accent" variant="caption">
          Codex running
        </KhalaText>
      </View>
    ) : null}
    <View className="min-h-[54px] flex-row items-center gap-2 rounded-full border border-border bg-bg px-3">
      <Pressable accessibilityRole="button" className="h-10 w-10 items-center justify-center rounded-full bg-surface">
        <KhalaText className="text-xl">+</KhalaText>
      </Pressable>
      <TextInput
        className="min-w-0 flex-1 text-text"
        editable={false}
        multiline
        placeholder={running ? "Follow up" : "Message"}
        placeholderTextColor="#7f90a6"
        value={running ? "Queue a note for the active turn" : ""}
      />
      <Pressable accessibilityRole="button" className="h-10 w-10 items-center justify-center rounded-full bg-text">
        <KhalaText className="text-bg">{running ? "■" : "↑"}</KhalaText>
      </Pressable>
    </View>
  </View>
)

export const TranscriptRows: Story = {
  render: () => (
    <View className="gap-3">
      {transcriptParts.map((part, index) => (
        <TranscriptPartExample key={`${part.kind}-${index}`} part={part} />
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
  render: () => <ComposerShell />,
}

export const ComposerWhileRunning: Story = {
  render: () => <ComposerShell running />,
}
