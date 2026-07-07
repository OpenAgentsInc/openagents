import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import {
  KhalaApprovalPrompt,
  KhalaCodeBlock,
  KhalaCodingConversation,
  KhalaDiagnosticCard,
  KhalaDiffBlock,
  KhalaFileChangeRow,
  KhalaTerminalPanel,
  KhalaTodoDock,
  KhalaToolCard,
  type KhalaConversationMessage,
} from "./khala-agent-chat-renderers"
import { KhalaText } from "./khala-text"

const meta = {
  title: "Khala/Agent Chat Renderers",
  component: View,
  parameters: { layout: "fullscreen" },
  decorators: [
    (Story) => (
      <View className="flex-1 bg-bg">
        <Story />
      </View>
    ),
  ],
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

const appPatch = `diff --git a/clients/khala-mobile/src/components/khala-button.tsx b/clients/khala-mobile/src/components/khala-button.tsx
@@ -85,7 +85,7 @@ export const KhalaButton = ({
       disabled={unavailable}
       highlightColor={variantHighlightColor[variant]}
     >
-      {loading ? <ActivityIndicator color={loadingColor[variant]} size={36} /> : null}
+      {loading ? <ActivityIndicator color={loadingColor[variant]} size={52} strokeWidth={5} type="large" /> : null}
       {text === undefined && children === undefined ? null : (
         <KhalaText
           className="text-center font-semibold"
@@ -101,3 +101,8 @@ export const KhalaButton = ({
   )
 }
+
+export const KhalaButtonBusyPreview = () => (
+  <KhalaButton loading variant="primary" text="Sending" />
+)
`

const storybookPatch = `diff --git a/src/stories/agent-chat.stories.tsx b/src/stories/agent-chat.stories.tsx
@@ -1,6 +1,7 @@
 import { View } from "react-native"
 import { KhalaToolCard } from "../components/khala-agent-chat-renderers"
+import { KhalaDiffBlock } from "../components/khala-agent-chat-renderers"

 export const CodingTurn = () => (
   <View>
-    <KhalaToolCard title="Read transcript" />
+    <KhalaToolCard title="Read transcript" status="completed" />
   </View>
 )
`

const codeSnippet = `export const summarizeTool = (name: string) => {
  if (name.includes("apply_patch")) return "Edited files"
  if (name.includes("rg")) return "Searched workspace"
  return "Tool call"
}`

const terminalOutput = `$ bun run --cwd clients/khala-mobile typecheck
$ tsc -p tsconfig.json --noEmit

$ bun run --cwd clients/khala-mobile storybook-generate
$ sb-rn-get-stories`

const toolOutput = `cwd: /Users/christopherdavid/work/openagents
rg -n "ActivityIndicator|spinner" clients/khala-mobile/src
clients/khala-mobile/src/app.tsx:35 ActivityIndicator size={220}
clients/khala-mobile/src/components/khala-button.tsx:90 ActivityIndicator size={52}`

const componentMessages: ReadonlyArray<KhalaConversationMessage> = [
  {
    id: "user-1",
    role: "user",
    meta: "10:56",
    parts: [
      <KhalaText key="body">
        Make sure the mobile chat can render coding-agent output: commands, diffs, file changes, approvals,
        diagnostics, and a real composed conversation.
      </KhalaText>,
    ],
  },
  {
    id: "assistant-1",
    role: "assistant",
    meta: "codex_app_server",
    parts: [
      <KhalaTodoDock
        key="todos"
        todos={[
          { label: "Read Khala Code desktop transcript renderer", status: "completed" },
          { label: "Study opencode timeline, permission, terminal, and review panels", status: "completed" },
          { label: "Port mobile-native Storybook renderers", status: "in_progress" },
          { label: "Run Storybook export and push main", status: "pending" },
        ]}
      />,
      <KhalaToolCard
        key="read"
        detail="Read desktop transcript-render.ts and opencode timeline rows"
        output={toolOutput}
        status="completed"
        title="Gather renderer references"
        type="search"
      />,
      <KhalaCodeBlock key="code" code={codeSnippet} filename="khala-agent-chat-renderers.tsx" language="tsx" />,
    ],
  },
]

const fullConversationMessages: ReadonlyArray<KhalaConversationMessage> = [
  {
    id: "user-1",
    role: "user",
    meta: "owner",
    parts: [
      <KhalaText key="body">
        The spinner is still too small. Fix it, show me the Storybook screenshot, and keep the agent chat renderer complete.
      </KhalaText>,
    ],
  },
  {
    id: "assistant-1",
    role: "assistant",
    meta: "running",
    parts: [
      <KhalaToolCard
        key="search"
        detail="Searched mobile, desktop, and opencode chat surfaces"
        output={toolOutput}
        status="completed"
        title="Workspace search"
        type="search"
      />,
      <KhalaApprovalPrompt
        key="approval"
        body="Codex wants to run the local iOS Storybook export and write a screenshot under /tmp."
        decisions={["Allow once", "Allow session", "Deny"]}
        title="Run local verification"
      />,
      <KhalaToolCard
        key="export"
        detail="Storybook iOS export completed"
        output={terminalOutput}
        status="completed"
        title="Verify Storybook"
        type="command"
      />,
    ],
  },
  {
    id: "assistant-2",
    role: "assistant",
    meta: "patch",
    parts: [
      <View key="files" className="gap-2">
        <KhalaFileChangeRow added={21} detail="New mobile-native renderer catalog" path="src/components/khala-agent-chat-renderers.tsx" />
        <KhalaFileChangeRow added={160} detail="Conversation examples for coding-agent turns" path="src/components/khala-agent-chat-renderers.stories.tsx" />
        <KhalaFileChangeRow added={5} removed={1} detail="Bigger busy button spinner" path="src/components/khala-button.tsx" />
      </View>,
      <KhalaDiffBlock key="diff" filename="clients/khala-mobile/src/components/khala-button.tsx" patch={appPatch} />,
      <KhalaDiagnosticCard
        key="diagnostic"
        body="Gesture-handler reports JS-thread callback warnings in Storybook only; the catalog renders and the export passes."
        title="Non-blocking simulator warning"
        tone="warning"
      />,
    ],
  },
  {
    id: "assistant-3",
    role: "assistant",
    meta: "complete",
    parts: [
      <KhalaText key="body">
        Storybook now has native examples for the desktop/opencode coding-agent primitives and a full transcript screen.
      </KhalaText>,
      <KhalaTerminalPanel key="terminal" output={terminalOutput} title="Verification" />,
    ],
  },
]

export const ToolRowsAndCards: Story = {
  render: () => (
    <View className="gap-3 p-4">
      <KhalaToolCard detail="rg --files clients/khala-mobile" status="completed" title="Search files" type="search" />
      <KhalaToolCard output={toolOutput} status="running" title="Run typecheck" type="command" />
      <KhalaToolCard detail="Demo fixture only: pretend sandbox denied a dry-run mutation" status="failed" title="Synthetic Tool Failure" type="tool" />
      <KhalaFileChangeRow added={42} removed={8} path="clients/khala-mobile/src/components/transcript-part-row.tsx" />
    </View>
  ),
}

export const CodeAndDiffs: Story = {
  render: () => (
    <View className="gap-4 p-4">
      <KhalaCodeBlock code={codeSnippet} filename="src/components/khala-agent-chat-renderers.tsx" language="tsx" />
      <KhalaDiffBlock filename="src/stories/agent-chat.stories.tsx" patch={storybookPatch} />
    </View>
  ),
}

export const ApprovalDiagnosticsAndTodos: Story = {
  render: () => (
    <View className="gap-4 p-4">
      <KhalaTodoDock
        todos={[
          { label: "Inspect desktop transcript renderer", status: "completed" },
          { label: "Port opencode permission and diff affordances", status: "in_progress" },
          { label: "Push to main", status: "pending" },
        ]}
      />
      <KhalaApprovalPrompt body="Allow `bun run typecheck` and `expo export` for this worktree." title="Command permission" />
      <KhalaDiagnosticCard body="One story imports an app-only native module." title="Storybook runtime error" tone="danger" />
    </View>
  ),
}

export const ComponentConversation: Story = {
  render: () => <KhalaCodingConversation messages={componentMessages} />,
}

export const FullCodingAgentConversation: Story = {
  render: () => <KhalaCodingConversation messages={fullConversationMessages} />,
}
