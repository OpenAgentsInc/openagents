import { resolveIntentRef, type IntentReporter } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { Window } from "happy-dom"
import { createRoot } from "react-dom/client"
import { afterEach, describe, expect, test } from "vite-plus/test"

import { ideAgentFixtureManifest, ideAgentFixtureProposal } from "../ide/agent-code-fixture.ts"
import { emptyIdeAgentCodeSnapshot, IdeAgentReviewRefSchema, projectDocumentGenerationForSource } from "../ide/agent-code-contract.ts"
import { assembleActiveFileAgentManifest } from "./ide/agent-code.ts"
import { initialDesktopShellState } from "./shell.ts"

const restores: Array<() => void> = []
const installDom = () => {
  const window = new Window({ url: "http://localhost/" })
  class ResizeObserverStub { observe(): void {}; unobserve(): void {}; disconnect(): void {} }
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    customElements: window.customElements,
    Node: window.Node,
    Element: window.Element,
    SVGElement: window.SVGElement,
    CSSStyleSheet: window.CSSStyleSheet,
    HTMLElement: window.HTMLElement,
    HTMLButtonElement: window.HTMLButtonElement,
    Event: window.Event,
    MouseEvent: window.MouseEvent,
    ResizeObserver: ResizeObserverStub,
    getComputedStyle: window.getComputedStyle.bind(window),
    requestAnimationFrame: window.requestAnimationFrame.bind(window),
    cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
  }
  const previous = new Map<string, PropertyDescriptor | undefined>()
  for (const [name, value] of Object.entries(values)) {
    previous.set(name, Object.getOwnPropertyDescriptor(globalThis, name))
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value })
  }
  restores.push(() => {
    for (const [name, descriptor] of previous) {
      if (descriptor === undefined) delete (globalThis as Record<string, unknown>)[name]
      else Object.defineProperty(globalThis, name, descriptor)
    }
  })
  const container = window.document.createElement("div") as unknown as HTMLDivElement
  window.document.body.appendChild(container as never)
  return container
}

afterEach(async () => {
  await new Promise(resolve => setTimeout(resolve, 0))
  restores.splice(0).reverse().forEach(restore => restore())
})

const settle = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 20))
const recorder = () => {
  const received: Array<{ name: string; payload: unknown }> = []
  const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
  return { received, report }
}

describe("IDE-08 rendered agent-code surfaces", () => {
  test("translates Monaco generation zero once before project review admission", () => {
    expect(projectDocumentGenerationForSource(0)).toBe(1)
    expect(projectDocumentGenerationForSource(4)).toBe(5)
  })

  test("accounts for the complete active-file context inventory while semantic retrieval is off", async () => {
    const initial = initialDesktopShellState("electron/darwin")
    const assembled = await assembleActiveFileAgentManifest({
      ...initial,
      composerFileContext: {
        path: "src/active.ts",
        revisionRef: "workspace.revision.active.1",
        languageMode: "typescript",
        content: "export const active = true\n",
        dirty: true,
      },
      workspaceBrowser: { ...initial.workspaceBrowser, grantRef: "workspace.grant.agent-manifest" },
    }, "2026-07-19T12:00:00.000Z")
    expect(assembled).not.toBeNull()
    if (assembled === null) return
    expect(assembled.manifest.items).toHaveLength(11)
    expect(assembled.manifest.items.map(item => item.source._tag)).toEqual([
      "File", "Unavailable", "Unavailable", "Unavailable", "Unavailable", "Unavailable",
      "Unavailable", "RecentEdit", "LexicalRetrieval", "SemanticRetrieval", "RuntimePolicy",
    ])
    expect(assembled.manifest.items.find(item => item.source._tag === "SemanticRetrieval")?.disposition).toMatchObject({
      _tag: "Omitted",
      reason: "retrieval_disabled",
    })
    const included = assembled.manifest.items.filter(item => item.disposition._tag === "Included")
    expect(assembled.manifest.includedBytes).toBe(included.reduce((total, item) => total + item.byteEstimate, 0))
    expect(assembled.manifest.includedTokens).toBe(included.reduce((total, item) => total + item.tokenEstimate, 0))
    expect(assembled.manifest.omittedCount).toBe(assembled.manifest.items.length - included.length)
    expect(assembled.manifest.effectiveRuntime).toMatchObject({
      permissionMode: "proposal_only",
      semanticRetrieval: "disabled",
      placementRef: "ide.placement.desktop-local",
    })
  })

  test("uses the completed harness account observation instead of guessing an implicit Claude target", async () => {
    const initial = initialDesktopShellState("electron/darwin")
    const assembled = await assembleActiveFileAgentManifest({
      ...initial,
      activeThreadId: "thread-observed-account",
      activeLaneRef: "fable-local",
      selectedHarness: "fable",
      threads: [{
        id: "thread-observed-account",
        title: "Observed account",
        updatedAt: "2026-07-19T12:00:00.000Z",
        notes: [{
          key: "assistant-observed-account",
          role: "assistant",
          text: "done",
          timestamp: "12:00 PM",
          meta: {
            lane: "fable-local",
            model: "claude-fable-5",
            accountRef: "claude-pylon-3",
            turnRef: "turn.fable.observed-account",
          },
        }],
      }],
      providerTargetsByThread: {},
      composerFileContext: {
        path: "src/active.ts",
        revisionRef: "workspace.revision.active.1",
        languageMode: "typescript",
        content: "export const active = true\n",
        dirty: false,
      },
      workspaceBrowser: { ...initial.workspaceBrowser, grantRef: "workspace.grant.observed-account" },
    }, "2026-07-19T12:00:01.000Z")
    expect(assembled?.manifest.effectiveRuntime.accountRef).toBe("claude-pylon-3")
  })

  test("discloses included and omitted context plus the effective runtime", async () => {
    const container = installDom()
    const { AgentContextTray } = await import("./react-agent-context.tsx")
    const manifest = ideAgentFixtureManifest()
    const state = {
      ...initialDesktopShellState("electron/darwin"),
      agentContextTrayOpen: true,
      agentCode: { ...emptyIdeAgentCodeSnapshot(), attachment: manifest.attachment, manifests: [manifest], lifecycle: "attached" as const, revision: 2 },
    }
    const root = createRoot(container)
    root.render(<AgentContextTray state={state} report={recorder().report} />)
    await settle()
    expect(container.textContent).toContain("Context 1 included · 1 omitted")
    expect(container.textContent).toContain("explicit user selection")
    expect(container.textContent).toContain("retrieval disabled")
    expect(container.textContent).toContain("semantic disabled")
    expect(container.textContent).toContain("turn_only")
    root.unmount()
  })

  test("renders Pierre proposal review and dispatches only the selected operation decision", async () => {
    const container = installDom()
    const { AgentProposalReviewPanel } = await import("./react-agent-code.tsx")
    const proposal = ideAgentFixtureProposal({ lifecycle: { _tag: "Reviewing", reviewRef: IdeAgentReviewRefSchema.make("ide.agent-review.fixture") } })
    const { received, report } = recorder()
    const state = {
      ...initialDesktopShellState("electron/darwin"),
      agentReviewProposalRef: proposal.proposalRef,
      agentCode: { ...emptyIdeAgentCodeSnapshot(), attachment: proposal.attachment, proposals: [proposal], lifecycle: "attached" as const, revision: 3 },
    }
    const root = createRoot(container)
    root.render(<AgentProposalReviewPanel state={state} report={report} />)
    await settle()
    expect(container.textContent).toContain("Reviewing")
    expect(container.textContent).toContain("Exact proposal base")
    expect(container.textContent).toContain("Post-apply evidence")
    expect(container.textContent).toContain("Separate from harness completion")
    const creatingConversation = [...container.querySelectorAll("button")]
      .find(button => button.textContent?.includes("Open creating conversation"))
    creatingConversation?.click()
    const accept = [...container.querySelectorAll("button")].find(button => button.textContent?.includes("Accept selected"))
    accept?.click()
    await settle()
    expect(received).toContainEqual({
      name: "DesktopAgentCreatingTurnOpened",
      payload: proposal.proposalRef,
    })
    expect(received).toContainEqual({
      name: "DesktopAgentProposalDecisionRequested",
      payload: {
        proposalRef: proposal.proposalRef,
        disposition: "accept",
        operationRefs: proposal.operations.map(operation => operation.operationRef),
      },
    })
    root.unmount()
  })
})
