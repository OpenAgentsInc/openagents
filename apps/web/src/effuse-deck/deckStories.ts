/**
 * Deck-only story definitions. No dependency on storybook bundle so deck
 * slides always resolve regardless of code-splitting or load order.
 */
import { html } from "@openagentsinc/effuse"

import { autopilotChatTemplate } from "../effuse-pages/autopilot"
import { autopilotSidebarTemplate } from "../effuse-pages/autopilotSidebar"

import type { AutopilotAuthModel, AutopilotChatData, RenderedMessage } from "../effuse-pages/autopilot"
import type { AutopilotSidebarModel } from "../effuse-pages/autopilotSidebar"

const authClosed: AutopilotAuthModel = {
  isAuthed: false,
  authedEmail: null,
  step: "closed",
  email: "",
  code: "",
  isBusy: false,
  errorText: null,
}

const sidebarModel: AutopilotSidebarModel = {
  collapsed: false,
  pathname: "/autopilot",
  user: null,
  userMenuOpen: false,
}

function msgUser(id: string, text: string): RenderedMessage {
  return { id, role: "user", renderParts: [{ kind: "text", text }] }
}
function msgAssistant(id: string, text: string): RenderedMessage {
  return { id, role: "assistant", renderParts: [{ kind: "text", text, state: "done" }] }
}

function renderOnboardingFlow() {
  const data: AutopilotChatData = {
    messages: [
      msgAssistant("a1", "Autopilot online.\n\nGreetings, user. What shall I call you?"),
      msgUser("u1", "Chris"),
      msgAssistant("a2", "Hi Chris. What would you like me to call myself? (Agent name—e.g. Autopilot, DevBuddy)"),
      msgUser("u2", "Autopilot is fine"),
      msgAssistant("a3", "Got it. How should I sound—calm, direct, casual? (Agent vibe)"),
      msgUser("u3", "Direct and concise"),
      msgAssistant("a4", "I'll keep responses direct. Anything off-limits? (e.g. no personal data, no financial advice)"),
      msgUser("u4", "No personal info, keep it work-focused"),
      msgAssistant("a5", "All set. I can help with code, your repo, and tasks. Connect a GitHub repo to pull in context, or just ask."),
      msgUser("u5", "Connect my GitHub repo"),
    ],
    isBusy: false,
    isAtBottom: true,
    inputValue: "",
    errorText: null,
    auth: authClosed,
  }
  return html`
    <div class="flex h-full min-h-0 w-full min-w-0 rounded border border-border-dark bg-bg-secondary overflow-hidden">
      <div class="shrink-0 h-full">${autopilotSidebarTemplate(sidebarModel)}</div>
      <div class="flex-1 min-w-0 flex flex-col border-l border-border-dark">
        ${autopilotChatTemplate(data)}
      </div>
    </div>
  `
}

function renderPostOnboarding() {
  const data: AutopilotChatData = {
    messages: [
      msgUser("m1", "Add a health check endpoint to the API"),
      msgAssistant("m2", "Checking your repo structure…\n\n"),
      {
        id: "m3",
        role: "assistant",
        renderParts: [
          {
            kind: "tool",
            model: {
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
            },
          },
        ],
      },
      msgAssistant(
        "m4",
        "I see `src/` with `routes/` and `health.ts` already. I'll add a GET `/health` that returns service status:\n\n```ts\n// src/routes/health.ts\nexport async function healthHandler(req: Request) {\n  return Response.json({ ok: true, ts: Date.now() });\n}\n```\n\nShould I apply this patch?",
      ),
    ],
    isBusy: false,
    isAtBottom: true,
    inputValue: "",
    errorText: null,
    auth: { ...authClosed, isAuthed: true, authedEmail: "you@example.com" },
  }
  return html`
    <div class="flex h-full min-h-0 w-full min-w-0 rounded border border-border-dark bg-bg-secondary overflow-hidden">
      <div class="shrink-0 h-full">${autopilotSidebarTemplate(sidebarModel)}</div>
      <div class="flex-1 min-w-0 flex flex-col border-l border-border-dark">
        ${autopilotChatTemplate(data)}
      </div>
    </div>
  `
}

export const DECK_STORY_BY_ID: Record<string, { render: () => ReturnType<typeof html> }> = {
  "autopilot-onboarding-flow": { render: renderOnboardingFlow },
  "autopilot-post-onboarding": { render: renderPostOnboarding },
}
