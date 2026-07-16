import { Window } from "happy-dom"
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, describe, expect, test } from "vite-plus/test"

import { DesktopApprovalCard } from "./approval-card.tsx"

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
    MouseEvent: window.MouseEvent,
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
  return { window, container }
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

describe("DesktopApprovalCard (T9 #8866)", () => {
  test("pending + onDecision renders the default Approve/Deny pair and reports each choice", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    const decisions: Array<"approved" | "denied"> = []
    await act(async () => {
      root.render(<DesktopApprovalCard
        decision="pending"
        description="Allow the tool?"
        itemKey="item-1"
        onDecision={decision => decisions.push(decision)}
        resource="rm -rf /tmp/scratch"
        title="Tool approval"
      />)
    })
    const buttons = [...container.querySelectorAll("button")]
    expect(buttons.map(button => button.textContent)).toEqual(["Deny", "Approve"])
    expect(container.querySelector("code")?.textContent).toBe("rm -rf /tmp/scratch")
    await act(async () => { buttons[1]?.click() })
    await act(async () => { buttons[0]?.click() })
    expect(decisions).toEqual(["approved", "denied"])
  })

  test("pending + actions renders a custom N-way action list instead of the binary pair", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    const selected: Array<string> = []
    await act(async () => {
      root.render(<DesktopApprovalCard
        actions={[
          { key: "accept", label: "Accept", primary: true, onSelect: () => selected.push("accept") },
          { key: "changes", label: "Request changes", onSelect: () => selected.push("changes") },
          { key: "replan", label: "Replan", onSelect: () => selected.push("replan") },
        ]}
        decision="pending"
        description="Review the plan"
        itemKey="item-2"
        resource=""
        title="Plan review"
      />)
    })
    const buttons = [...container.querySelectorAll("button")]
    expect(buttons.map(button => button.textContent)).toEqual(["Accept", "Request changes", "Replan"])
    expect(buttons[0]?.getAttribute("data-primary")).toBe("true")
    expect(container.querySelector("code")).toBeNull()
    await act(async () => { buttons[2]?.click() })
    expect(selected).toEqual(["replan"])
  })

  test("resolved approved/denied render a badge with the default wording, no buttons", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await act(async () => {
      root.render(<DesktopApprovalCard decision="approved" description="" itemKey="item-3" resource="" title="Approval" />)
    })
    expect(container.querySelector("button")).toBeNull()
    // Scoped to `.oa-react-approval-decision` — the outer `<article>` also
    // carries `data-decision`, so a bare attribute selector would match the
    // whole card (title + resource text included) instead of just the badge.
    expect(container.querySelector('.oa-react-approval-decision[data-decision="approved"]')?.textContent).toContain("Approved")
    await act(async () => {
      root.render(<DesktopApprovalCard decision="denied" description="" itemKey="item-3" resource="" title="Approval" />)
    })
    expect(container.querySelector("button")).toBeNull()
    expect(container.querySelector('.oa-react-approval-decision[data-decision="denied"]')?.textContent).toContain("Denied")
  })

  test("decisionLabel overrides the resolved wording for a non-binary outcome", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await act(async () => {
      root.render(<DesktopApprovalCard
        decision="denied"
        decisionLabel="Changes requested"
        description=""
        itemKey="item-4"
        resource=""
        title="Plan review"
      />)
    })
    expect(container.querySelector('.oa-react-approval-decision[data-decision="denied"]')?.textContent).toContain("Changes requested")
  })

  test("a truly read-only pending card (no actions, no onDecision) renders a neutral Pending indicator, never mislabeled Denied", async () => {
    const { container } = installDom()
    const root = createTestRoot(container)
    await act(async () => {
      root.render(<DesktopApprovalCard decision="pending" description="" itemKey="item-5" resource="" title="Approval" />)
    })
    expect(container.querySelector("button")).toBeNull()
    const badge = container.querySelector('.oa-react-approval-decision[data-decision="pending"]')
    expect(badge?.textContent).toBe("Pending")
  })
})
