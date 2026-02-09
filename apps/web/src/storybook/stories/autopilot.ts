import { html } from "@openagentsinc/effuse"

import { autopilotBlueprintPanelTemplate } from "../../effuse-pages/autopilotBlueprint"
import { autopilotChatTemplate } from "../../effuse-pages/autopilot"
import { autopilotControlsTemplate } from "../../effuse-pages/autopilotControls"
import { autopilotSidebarTemplate } from "../../effuse-pages/autopilotSidebar"
import { applyChatWirePart } from "../../effect/chatWire"
import type { ActiveStream } from "../../effect/chatWire"
import { toAutopilotRenderParts } from "../../effuse-app/controllers/autopilotChatParts"
import { dseKitchenSinkStreamV1 } from "../../fixtures/wireTranscripts"

import type { ToolPartModel } from "@openagentsinc/effuse"
import type {
  AutopilotChatData,
  AutopilotAuthModel,
  DseBudgetExceededCardModel,
  DseCompileCardModel,
  DsePromoteCardModel,
  DseRollbackCardModel,
  DseSignatureCardModel,
  RenderedMessage,
  RenderPart,
} from "../../effuse-pages/autopilot"
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

const toolPartRepoTree: ToolPartModel = {
  status: "tool-result",
  toolName: "github.getRepoTree",
  toolCallId: "toolcall_repo",
  summary: "List repo openagents/apps/api",
  details: {
    input: {
      preview: JSON.stringify({ owner: "openagents", repo: "apps/api", ref: "main" }, null, 2),
      truncated: false,
    },
    output: {
      preview: "src/\n  routes/\n  health.ts\n  index.ts\npackage.json\n",
      truncated: false,
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

const messageAssistantToolWith = (id: string, model: ToolPartModel): RenderedMessage => ({
  id,
  role: "assistant",
  renderParts: [{ kind: "tool", model }],
})

const messageAssistantParts = (id: string, renderParts: ReadonlyArray<RenderPart>): RenderedMessage => ({
  id,
  role: "assistant",
  renderParts,
})

const dseSignatureExample: DseSignatureCardModel = {
  id: "dse_sig_1",
  state: "ok",
  signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
  compiled_id: "c_health_router_v1",
  receiptId: "rcpt_select_1",
  durationMs: 98,
  budget: {
    limits: { maxTimeMs: 2500, maxLmCalls: 1, maxOutputChars: 8000 },
    usage: { elapsedMs: 98, lmCalls: 1, outputChars: 512 },
  },
  outputPreview: { preview: '{"toolName":"github.getRepoTree","reason":"List repo for health endpoint"}', truncated: false },
}

const dseCompileExample: DseCompileCardModel = {
  id: "dse_compile_1",
  state: "ok",
  signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
  jobHash: "job_a1b2_health",
  candidates: 12,
  best: { compiled_id: "c_health_router_v2", reward: 0.74 },
  reportId: "compile_report_health_1",
}

const dsePromoteExample: DsePromoteCardModel = {
  id: "dse_promote_1",
  state: "ok",
  signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
  from: "c_health_router_v1",
  to: "c_health_router_v2",
  reason: "compile job job_a1b2_health improved reward 0.62 → 0.74",
}

const dseRollbackExample: DseRollbackCardModel = {
  id: "dse_rollback_1",
  state: "ok",
  signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
  from: "c_health_router_v2",
  to: "c_health_router_v1",
  reason: "manual rollback after canary regression",
}

const dseBudgetExceededExample: DseBudgetExceededCardModel = {
  id: "dse_budget_1",
  state: "error",
  message: "Stopped after exceeding maxLmCalls=2",
  budget: {
    limits: { maxLmCalls: 2 },
    usage: { elapsedMs: 1200, lmCalls: 3, outputChars: 0 },
  },
}

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

const chatDataFromWireTranscript = (opts: {
  readonly stream: ReadonlyArray<{ readonly seq: number; readonly part: unknown }>
  readonly userText?: string
  readonly auth?: AutopilotAuthModel
}): AutopilotChatData => {
  const active: ActiveStream = { id: "run-fixture", messageId: "m-fixture", parts: [] }
  const sorted = [...opts.stream].sort((a, b) => a.seq - b.seq)
  for (const ev of sorted) {
    applyChatWirePart(active, ev.part)
  }

  const assistantParts = toAutopilotRenderParts({ parts: active.parts })
  const userText = opts.userText ?? "render the DSE kitchen sink wire transcript"
  const auth = opts.auth ?? authAuthed

  return chatData({
    messages: [
      messageUser("m-user", userText),
      { id: "m-assistant", role: "assistant", renderParts: assistantParts } satisfies RenderedMessage,
    ],
    auth,
  })
}

/** Exported for deck: onboarding flow (greeting → setup questions → user asks to connect GitHub). */
export const autopilotOnboardingFlowStory: Story = {
  id: "autopilot-onboarding-flow",
  title: "Autopilot/Onboarding flow (name, blueprint, connect GitHub)",
  kind: "organism",
  render: () =>
    html`
      <div class="flex h-full min-h-0 w-full min-w-0 rounded border border-border-dark bg-bg-secondary overflow-hidden">
        <div class="shrink-0 h-full">
          ${autopilotSidebarTemplate(sidebarModel({ collapsed: false }))}
        </div>
        <div class="flex-1 min-w-0 flex flex-col border-l border-border-dark">
          ${autopilotChatTemplate(
            chatData({
              messages: [
                messageAssistantText("a1", "Autopilot online.\n\nGreetings, user. What shall I call you?", "done"),
                messageUser("u1", "Chris"),
                messageAssistantText(
                  "a2",
                  "Hi Chris. What would you like me to call myself? (Agent name—e.g. Autopilot, DevBuddy)",
                  "done",
                ),
                messageUser("u2", "Autopilot is fine"),
                messageAssistantText(
                  "a3",
                  "Got it. How should I sound—calm, direct, casual? (Agent vibe)",
                  "done",
                ),
                messageUser("u3", "Direct and concise"),
                messageAssistantText(
                  "a4",
                  "I'll keep responses direct. Anything off-limits? (e.g. no personal data, no financial advice)",
                  "done",
                ),
                messageUser("u4", "No personal info, keep it work-focused"),
                messageAssistantText(
                  "a5",
                  "All set. I can help with code, your repo, and tasks. Connect a GitHub repo to pull in context, or just ask.",
                  "done",
                ),
                messageUser("u5", "Connect my GitHub repo"),
              ],
              isBusy: false,
              isAtBottom: true,
              inputValue: "",
              auth: authClosed,
            }),
          )}
        </div>
      </div>
    `,
}

/** Exported for deck renderer so this story is always resolvable when deck loads. */
export const autopilotPostOnboardingStory: Story = {
  id: "autopilot-post-onboarding",
  title: "Autopilot/Post onboarding (GitHub connected, coding)",
  kind: "organism",
  render: () =>
    html`
      <div class="flex h-full min-h-0 w-full min-w-0 rounded border border-border-dark bg-bg-secondary overflow-hidden">
        <div class="shrink-0 h-full">
          ${autopilotSidebarTemplate(sidebarModel({ collapsed: false }))}
        </div>
        <div class="flex-1 min-w-0 flex flex-col border-l border-border-dark">
          ${autopilotChatTemplate(
            chatData({
              messages: [
                messageUser("m1", "Add a health check endpoint to the API"),
                messageAssistantText("m2", "Checking your repo structure…\n\n", "done"),
                messageAssistantToolWith("m3", toolPartRepoTree),
                messageAssistantParts("m3b", [
                  { kind: "text", text: "Running policy + compile pipeline:\n\n", state: "done" },
                  { kind: "dse-signature", model: dseSignatureExample },
                  { kind: "dse-compile", model: dseCompileExample },
                  { kind: "dse-promote", model: dsePromoteExample },
                  { kind: "text", text: "Rollback / budget stop examples:\n\n", state: "done" },
                  { kind: "dse-rollback", model: dseRollbackExample },
                  { kind: "dse-budget-exceeded", model: dseBudgetExceededExample },
                ]),
                messageAssistantText(
                  "m4",
                  "I see `src/` with `routes/` and `health.ts` already. I'll add a GET `/health` that returns service status:\n\n```ts\n// src/routes/health.ts\nexport async function healthHandler(req: Request) {\n  return Response.json({ ok: true, ts: Date.now() });\n}\n```\n\nShould I apply this patch?",
                  "done",
                ),
              ],
              isBusy: false,
              isAtBottom: true,
              inputValue: "",
              auth: authAuthed,
            }),
          )}
        </div>
      </div>
    `,
}

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
  {
    id: "autopilot-dashboard-preview",
    title: "Autopilot/Dashboard preview (sidebar + chat)",
    kind: "organism",
    render: () =>
      html`
        <div class="flex h-full min-h-0 w-full min-w-0 rounded border border-border-dark bg-bg-secondary overflow-hidden">
          <div class="shrink-0 h-full">
            ${autopilotSidebarTemplate(sidebarModel({ collapsed: false }))}
          </div>
          <div class="flex-1 min-w-0 flex flex-col border-l border-border-dark">
            ${autopilotChatTemplate(
              chatData({
                messages: [
                  messageUser("m1", "make a plan for the MVP"),
                  messageAssistantText("m2", "Working on it…\n\n### Phase 1\n- Route contract\n- Cache rules\n", "streaming"),
                  messageAssistantTool("m3"),
                ],
                isBusy: true,
                isAtBottom: false,
                inputValue: "next steps?",
                auth: authAuthed,
              }),
            )}
          </div>
        </div>
      `,
  },
  {
    id: "autopilot-dashboard-first-visit",
    title: "Autopilot/Dashboard first visit (intro only)",
    kind: "organism",
    render: () =>
      html`
        <div class="flex h-full min-h-0 w-full min-w-0 rounded border border-border-dark bg-bg-secondary overflow-hidden">
          <div class="shrink-0 h-full">
            ${autopilotSidebarTemplate(sidebarModel({ collapsed: false }))}
          </div>
          <div class="flex-1 min-w-0 flex flex-col border-l border-border-dark">
            ${autopilotChatTemplate(
              chatData({
                messages: [
                  messageAssistantText("intro", "Autopilot online.\n\nGreetings, user. What shall I call you?", "streaming"),
                ],
                isBusy: false,
                isAtBottom: true,
                inputValue: "",
                auth: authClosed,
              }),
            )}
          </div>
        </div>
      `,
  },
  autopilotOnboardingFlowStory,
  autopilotPostOnboardingStory,
  {
    id: "autopilot-organisms-wire-transcript-dse-kitchen-sink",
    title: "Autopilot/Organisms/Wire Transcript (DSE Kitchen Sink)",
    kind: "organism",
    render: () => {
      const data = chatDataFromWireTranscript({
        stream: dseKitchenSinkStreamV1,
        userText: "show me every DSE card type (kitchen sink)",
        auth: authAuthed,
      })

      const rawJson = dseKitchenSinkStreamV1.map((l) => JSON.stringify(l)).join("\n")

      return html`
        <div class="flex h-screen min-h-0 w-full min-w-0 border border-border-dark bg-bg-secondary overflow-hidden">
          <div class="flex-1 min-w-0 flex flex-col">
            ${autopilotChatTemplate(data)}
          </div>
          <aside class="hidden lg:flex w-[520px] min-w-0 border-l border-border-dark bg-bg-secondary">
            <div class="flex-1 min-h-0 overflow-auto p-4">
              <div class="text-xs text-text-dim uppercase tracking-wider">Wire transcript (V1)</div>
              <pre class="mt-2 whitespace-pre-wrap break-words rounded border border-border-dark bg-surface-primary/35 px-3 py-2 text-[11px] leading-relaxed font-mono text-text-primary">${rawJson}</pre>
            </div>
          </aside>
        </div>
      `
    },
  },
] as const
