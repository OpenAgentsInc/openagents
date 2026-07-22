import { Window } from "happy-dom"
import { act } from "react"
import { createRoot } from "react-dom/client"
import { afterEach, describe, expect, test } from "vite-plus/test"

import { IdePortableClientSnapshotSchema } from "../ide/portable-client-contract.ts"
import { PortableSessionPlacement } from "./react-workspace-surfaces.tsx"
import { initialDesktopShellState } from "./shell.ts"

const restores: Array<() => void> = []
const installDom = (): HTMLDivElement => {
  const window = new Window({ url: "http://localhost/" })
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    HTMLButtonElement: window.HTMLButtonElement,
    SVGElement: window.SVGElement,
    Event: window.Event,
    MouseEvent: window.MouseEvent,
    getComputedStyle: window.getComputedStyle.bind(window),
    IS_REACT_ACT_ENVIRONMENT: true,
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

afterEach(() => {
  while (restores.length > 0) restores.pop()?.()
})

describe("portable coding placement surface", () => {
  test("shows only confirmed opaque identity, generation, placement, health, and custody facts", async () => {
    const container = installDom()
    const root = createRoot(container)
    const base = initialDesktopShellState("test-host", "12:00", "files")
    const portableSessions = IdePortableClientSnapshotSchema.make({
      status: { phase: "live", cursor: 12, pendingCommandCount: 1 },
      sessions: [{
        schema: "openagents.portable_session.v1",
        sessionRef: "session.alpha",
        ownerRef: "owner.alpha",
        identityBasis: "owner_minted",
        workContextRef: "context.alpha",
        eventLogRef: "events.alpha",
        currentProjectionRef: "projection.alpha",
        commandScopeRef: "scope.alpha",
        graph: {
          rootAgentRef: "agent.root",
          nodes: [{
            agentRef: "agent.root",
            threadRef: "thread.root",
            transcriptRef: "transcript.root",
            activityCursor: 9,
            lifecycle: "waiting",
            attachmentGeneration: 4,
          }],
        },
        adoptedFromLocalHistory: false,
      }],
      targetDirectories: [{
        sessionRef: "session.alpha",
        targets: [{
          targetRef: "placement.owner-remote",
          targetClass: "owner_managed",
          adapterRef: "adapter.ssh",
          ownerRef: "owner.alpha",
          compatibilityRef: "compatibility.v1",
          isolation: "owner_host_container",
          dataPosture: "owner_managed_region",
          health: "ready",
        }],
      }],
      attachments: [{
        attachmentRef: "attachment.alpha.4",
        sessionRef: "session.alpha",
        targetRef: "placement.owner-remote",
        generation: 4,
        state: "active",
        descendantAgentRefs: ["agent.root"],
        capabilityLeaseRefs: ["lease.alpha"],
        evidenceRefs: ["evidence.alpha"],
      }],
      commands: [],
      issues: [],
    })
    await act(async () => root.render(<PortableSessionPlacement state={{ ...base, portableSessions }} />))
    expect(container.textContent).toContain("Confirmed Sync authority")
    expect(container.textContent).toContain("owner_managed · ready")
    expect(container.textContent).toContain("attachment.alpha.4 · generation 4")
    expect(container.textContent).toContain("owner_managed_region")
    expect(container.textContent).not.toContain("/Users/")
    await act(async () => root.unmount())
  })
})
