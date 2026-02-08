import { html } from "@openagentsinc/effuse"

import { autopilotBlueprintPanelTemplate } from "../../effuse-pages/autopilotBlueprint"
import { autopilotChatTemplate } from "../../effuse-pages/autopilot"
import { autopilotControlsTemplate } from "../../effuse-pages/autopilotControls"
import { autopilotSidebarTemplate } from "../../effuse-pages/autopilotSidebar"

import type { ToolPartModel } from "@openagentsinc/effuse"
import type { AutopilotChatData, AutopilotAuthModel, RenderedMessage } from "../../effuse-pages/autopilot"
import type { AutopilotBlueprintPanelModel } from "../../effuse-pages/autopilotBlueprint"
import type { AutopilotControlsModel } from "../../effuse-pages/autopilotControls"
import type { AutopilotSidebarModel } from "../../effuse-pages/autopilotSidebar"
import type { Story } from "../types"

const toolPartExample: ToolPartModel = {
  status: "tool-result",
  toolName: "filesystem.readFile",
  toolCallId: "toolcall_01",
  summary: "Read file src/index.ts",
  details: {
    input: {
      preview: JSON.stringify({ path: "src/index.ts" }, null, 2),
      truncated: false,
    },
    output: {
      preview: "export const hello = 'world'\\n",
      truncated: true,
      blob: { id: "blob_01", hash: "sha256:deadbeef", size: 420, mime: "text/plain" },
    },
  },
}

const authClosed: AutopilotAuthModel = {
  isAuthed: false,
  authedEmail: null,
  step: "closed",
  email: "",
  code: "",
  isBusy: false,
  errorText: null,
}

const authEmail: AutopilotAuthModel = {
  ...authClosed,
  step: "email",
  email: "you@example.com",
}

const authCode: AutopilotAuthModel = {
  ...authClosed,
  step: "code",
  email: "you@example.com",
  code: "123456",
}

const authAuthed: AutopilotAuthModel = {
  ...authClosed,
  isAuthed: true,
  authedEmail: "you@example.com",
}

const messageUser = (id: string, text: string): RenderedMessage => ({
  id,
  role: "user",
  renderParts: [{ kind: "text", text }],
})

const messageAssistantText = (
  id: string,
  text: string,
  state: "streaming" | "done",
): RenderedMessage => ({
  id,
  role: "assistant",
  renderParts: [{ kind: "text", text, state }],
})

const messageAssistantTool = (id: string): RenderedMessage => ({
  id,
  role: "assistant",
  renderParts: [{ kind: "tool", model: toolPartExample }],
})

const chatData = (overrides?: Partial<AutopilotChatData>): AutopilotChatData => ({
  messages: [],
  isBusy: false,
  isAtBottom: true,
  inputValue: "",
  errorText: null,
  auth: authClosed,
  ...overrides,
})

const sidebarModel = (overrides?: Partial<AutopilotSidebarModel>): AutopilotSidebarModel => ({
  collapsed: false,
  pathname: "/autopilot",
  user: { email: "you@example.com", firstName: "Chris", lastName: "D" },
  userMenuOpen: false,
  ...overrides,
})

const blueprintModel = (overrides?: Partial<AutopilotBlueprintPanelModel>): AutopilotBlueprintPanelModel => ({
  updatedAtLabel: "just now",
  isLoading: false,
  canEdit: true,
  isSaving: false,
  errorText: null,
  mode: "form",
  rawErrorText: null,
  rawDraft: "{\\n  \\\"name\\\": \\\"autopilot\\\"\\n}",
  draft: {
    userHandle: "bobo",
    agentName: "autopilot",
    identityVibe: "calm",
    characterVibe: "direct",
    characterBoundaries: "- no personal info\\n- be concise",
  },
  ...overrides,
})

const controlsModel = (overrides?: Partial<AutopilotControlsModel>): AutopilotControlsModel => ({
  isExportingBlueprint: false,
  isBusy: false,
  isResettingAgent: false,
  ...overrides,
})

export const autopilotStories: ReadonlyArray<Story> = [
  {
    id: "autopilot-atoms-buttons",
    title: "Autopilot/Atoms/Buttons",
    kind: "atom",
    render: () =>
      html`
        <div class="flex flex-wrap items-center gap-3 p-6">
          <button
            type="button"
            class="inline-flex h-9 items-center justify-center rounded px-3 text-sm font-medium bg-accent text-bg-primary border border-accent hover:bg-accent-muted hover:border-accent-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-accent font-mono"
          >
            Primary
          </button>
          <button
            type="button"
            class="inline-flex h-9 items-center justify-center rounded px-3 text-sm font-medium bg-surface-primary text-text-primary border border-border-dark hover:bg-surface-secondary hover:border-border-hover focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus font-mono"
          >
            Secondary
          </button>
          <button
            type="button"
            disabled
            class="inline-flex h-9 items-center justify-center rounded px-3 text-sm font-medium bg-surface-primary text-text-primary border border-border-dark opacity-60 font-mono"
          >
            Disabled
          </button>
        </div>
      `,
  },
  {
    id: "autopilot-atoms-inputs",
    title: "Autopilot/Atoms/Inputs",
    kind: "atom",
    render: () =>
      html`
        <div class="flex flex-col gap-4 p-6 max-w-lg">
          <input
            type="text"
            value="Message Autopilot…"
            class="h-9 w-full rounded border border-border-dark bg-surface-primary px-3 text-sm text-text-primary placeholder:text-text-dim outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
          />
          <textarea
            rows="5"
            class="w-full resize-y rounded border border-border-dark bg-surface-primary px-3 py-2 text-sm text-text-primary placeholder:text-text-dim outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
          >Longer text…</textarea>
        </div>
      `,
  },
  {
    id: "autopilot-molecules-auth-closed",
    title: "Autopilot/Molecules/Auth Panel (Closed)",
    kind: "molecule",
    render: () => autopilotChatTemplate(chatData({ auth: authClosed })),
  },
  {
    id: "autopilot-molecules-auth-email",
    title: "Autopilot/Molecules/Auth Panel (Email)",
    kind: "molecule",
    render: () => autopilotChatTemplate(chatData({ auth: authEmail })),
  },
  {
    id: "autopilot-molecules-auth-code",
    title: "Autopilot/Molecules/Auth Panel (Code)",
    kind: "molecule",
    render: () => autopilotChatTemplate(chatData({ auth: authCode })),
  },
  {
    id: "autopilot-molecules-auth-authed",
    title: "Autopilot/Molecules/Auth Panel (Authed)",
    kind: "molecule",
    render: () => autopilotChatTemplate(chatData({ auth: authAuthed })),
  },
  {
    id: "autopilot-molecules-message-user",
    title: "Autopilot/Molecules/Message (User)",
    kind: "molecule",
    render: () =>
      autopilotChatTemplate(
        chatData({
          messages: [messageUser("m1", "hello autopilot")],
          auth: authAuthed,
        }),
      ),
  },
  {
    id: "autopilot-molecules-message-assistant-streaming",
    title: "Autopilot/Molecules/Message (Assistant, Streaming Markdown)",
    kind: "molecule",
    render: () =>
      autopilotChatTemplate(
        chatData({
          messages: [
            messageAssistantText(
              "m2",
              "Here is a plan:\\n\\n- route contract\\n- cache rules\\n- strict hydration\\n",
              "streaming",
            ),
          ],
          auth: authAuthed,
        }),
      ),
  },
  {
    id: "autopilot-molecules-tool-part",
    title: "Autopilot/Molecules/Tool Part Card",
    kind: "molecule",
    render: () =>
      autopilotChatTemplate(
        chatData({
          messages: [messageAssistantTool("m3")],
          auth: authAuthed,
        }),
      ),
  },
  {
    id: "autopilot-organisms-chat-with-streaming",
    title: "Autopilot/Organisms/Chat (Streaming + Tool Part)",
    kind: "organism",
    render: () =>
      autopilotChatTemplate(
        chatData({
          messages: [
            messageUser("m1", "make a plan for the MVP"),
            messageAssistantText("m2", "Working on it…\\n\\n### Phase 1\\n- do X\\n", "streaming"),
            messageAssistantTool("m3"),
          ],
          isBusy: true,
          isAtBottom: false,
          inputValue: "next steps?",
          auth: authClosed,
        }),
      ),
  },
  {
    id: "autopilot-organisms-sidebar-expanded",
    title: "Autopilot/Organisms/Sidebar (Expanded)",
    kind: "organism",
    render: () =>
      html`<div class="h-screen min-h-0">${autopilotSidebarTemplate(sidebarModel({ collapsed: false }))}</div>`,
  },
  {
    id: "autopilot-organisms-sidebar-collapsed",
    title: "Autopilot/Organisms/Sidebar (Collapsed)",
    kind: "organism",
    render: () =>
      html`<div class="h-screen min-h-0">${autopilotSidebarTemplate(sidebarModel({ collapsed: true }))}</div>`,
  },
  {
    id: "autopilot-organisms-blueprint-view",
    title: "Autopilot/Organisms/Blueprint Panel (Form)",
    kind: "organism",
    render: () =>
      html`
        <div class="h-screen min-h-0 w-[360px] border border-border-dark bg-bg-secondary">
          ${autopilotBlueprintPanelTemplate(blueprintModel({ mode: "form" }))}
        </div>
      `,
  },
  {
    id: "autopilot-organisms-blueprint-editing",
    title: "Autopilot/Organisms/Blueprint Panel (Raw)",
    kind: "organism",
    render: () =>
      html`
        <div class="h-screen min-h-0 w-[360px] border border-border-dark bg-bg-secondary">
          ${autopilotBlueprintPanelTemplate(blueprintModel({ mode: "raw" }))}
        </div>
      `,
  },
  {
    id: "autopilot-organisms-controls",
    title: "Autopilot/Organisms/Controls",
    kind: "organism",
    render: () =>
      html`
        <div class="p-6">
          ${autopilotControlsTemplate(controlsModel({ isBusy: false }))}
        </div>
      `,
  },
] as const
