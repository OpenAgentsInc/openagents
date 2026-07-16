import { Window } from "happy-dom"
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, describe, expect, test } from "vite-plus/test"

import { dispatchWorkbenchItem, type WorkbenchApprovalDispatchItem } from "./dispatch.tsx"

/**
 * Scoped to the T9 #8866 `case "approval":` branch only — deliberately its
 * own file (not a shared `dispatch.test.tsx`) so parallel Wave-2 lanes
 * (T4-T8, each owning a different `dispatchWorkbenchItem` branch) never
 * collide creating the same new test file.
 */
const restores: Array<() => void> = []
const roots = new Set<Root>()

const installDom = () => {
  const window = new Window({ url: "http://localhost/" })
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    Event: window.Event,
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
  return { container }
}

afterEach(async () => {
  await act(async () => {
    for (const root of roots) root.unmount()
    roots.clear()
    await new Promise(resolve => setTimeout(resolve, 0))
  })
  restores.splice(0).reverse().forEach(restore => restore())
})

const createTestRoot = (container: HTMLDivElement): Root => {
  const root = createRoot(container)
  roots.add(root)
  return root
}

const approvalItem = (
  overrides: Partial<WorkbenchApprovalDispatchItem> = {},
): WorkbenchApprovalDispatchItem => ({
  kind: "approval",
  source: "codex",
  status: "completed",
  ...overrides,
})

describe('dispatchWorkbenchItem — case "approval" (T9 #8866, read-only history)', () => {
  test("renders the shared DesktopApprovalCard, read-only (no interactive buttons)", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await act(async () => {
      root.render(dispatchWorkbenchItem(
        approvalItem({ decision: "approved", detail: "pnpm test --filter desktop" }),
        { itemKey: "approval-1" },
      ))
    })
    const card = container.querySelector('[data-kind="approval"]')
    expect(card).not.toBeNull()
    expect(card?.getAttribute("data-decision")).toBe("approved")
    expect(card?.getAttribute("data-timeline-key")).toBe("approval-1")
    expect(container.querySelector("code")?.textContent).toBe("pnpm test --filter desktop")
    // A history row is already decided — no onDecision means no buttons, ever.
    expect(container.querySelector("button")).toBeNull()
    expect(card?.textContent).toContain("Approved")
  })

  test("a denied history row shows the muted-red denied badge", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await act(async () => {
      root.render(dispatchWorkbenchItem(
        approvalItem({ decision: "denied", status: "declined", detail: "rm -rf /" }),
        { itemKey: "approval-2" },
      ))
    })
    expect(container.querySelector('.oa-react-approval-decision[data-decision="denied"]')?.textContent).toContain("Denied")
    expect(container.querySelector("button")).toBeNull()
  })

  test("a status-declined row with no explicit decision string still reads as denied (toApprovalDecision fallback)", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await act(async () => {
      root.render(dispatchWorkbenchItem(approvalItem({ status: "declined" }), { itemKey: "approval-3" }))
    })
    expect(container.querySelector('.oa-react-approval-decision[data-decision="denied"]')?.textContent).toContain("Denied")
  })

  test("an item with neither a decision nor a terminal status renders the neutral read-only Pending indicator", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await act(async () => {
      root.render(dispatchWorkbenchItem(approvalItem({ status: "in_progress" }), { itemKey: "approval-4" }))
    })
    expect(container.querySelector("button")).toBeNull()
    expect(container.querySelector('.oa-react-approval-decision[data-decision="pending"]')?.textContent).toBe("Pending")
  })
})
