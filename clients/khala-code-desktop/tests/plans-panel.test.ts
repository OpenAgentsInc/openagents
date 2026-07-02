import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import { mountKhalaCodePlansPanel } from "../src/ui/plans-panel"
import type {
  KhalaCodeDesktopPlanCatalog,
  KhalaCodeDesktopPlanCatalogResult,
  KhalaCodeDesktopPlanPurchaseRequest,
  KhalaCodeDesktopPlanPurchaseResult,
  KhalaCodeDesktopPlanStatusResult,
} from "../src/shared/rpc"

const setGlobal = (key: string, value: unknown): void => {
  Object.defineProperty(globalThis, key, {
    configurable: true,
    value,
    writable: true,
  })
}

const installDom = (): HTMLElement => {
  const window = new Window()
  setGlobal("document", window.document)
  setGlobal("HTMLElement", window.HTMLElement)
  setGlobal("HTMLButtonElement", window.HTMLButtonElement)
  setGlobal("Element", window.Element)
  setGlobal("MouseEvent", window.MouseEvent)
  setGlobal("customElements", window.customElements)
  const container = window.document.createElement("section")
  window.document.body.append(container)
  return container as unknown as HTMLElement
}

const flushPanel = async (): Promise<void> => {
  for (let index = 0; index < 4; index += 1) {
    await Promise.resolve()
    await new Promise(resolve => setTimeout(resolve, 0))
  }
}

const fixtureCatalog = (input: { readonly paidArmed: boolean }): KhalaCodeDesktopPlanCatalog => ({
  authorityBoundary: "The openagents.com Worker is the plan authority; the desktop app only renders it.",
  blockerRefs: ["blocker.product_promises.khala_code_paid_plan_not_purchasable"],
  catalogVersion: "2026-07-01.1",
  plans: [
    {
      captureExcluded: false,
      isDefault: true,
      kind: "free",
      label: "Free",
      planId: "khala_code.plan.free.v1",
      priceLabel: "Free",
      tagline: "Pay with data",
      terms: ["Coding sessions may be captured for training."],
    },
    {
      captureExcluded: true,
      isDefault: false,
      kind: "paid",
      label: "Paid",
      planId: "khala_code.plan.paid.v1",
      priceLabel: "Not yet purchasable",
      purchase: {
        armed: input.paidArmed,
        envFlag: "KHALA_CODE_PAID_PLANS_ENABLED",
        route: "/v1/khala-code/plans/purchases",
      },
      tagline: "Private data",
      terms: ["Capture opt-out: sessions are excluded from training capture."],
    },
  ],
  promiseId: "khala_code.free_paid_plans.v1",
  relatedPromiseIds: ["khala_code.claim_your_agent.v1"],
  schemaVersion: "openagents.khala_code.plan_catalog.v1",
  summary: "Khala Code Episode 245 plan structure: Free pays with data; Paid keeps data private.",
})

type Transports = {
  readonly catalog?: () => Promise<KhalaCodeDesktopPlanCatalogResult>
  readonly status?: () => Promise<KhalaCodeDesktopPlanStatusResult>
  readonly purchase?: (
    request?: KhalaCodeDesktopPlanPurchaseRequest,
  ) => Promise<KhalaCodeDesktopPlanPurchaseResult>
}

const mountWith = (container: HTMLElement, transports: Transports) =>
  mountKhalaCodePlansPanel(container, {
    catalog: transports.catalog ?? (async () => ({ ok: true, catalog: fixtureCatalog({ paidArmed: false }) })),
    purchase: transports.purchase ?? (async () => ({ ok: false, error: "purchase_unavailable" })),
    status: transports.status ?? (async () => ({ state: "unauthenticated" })),
  })

describe("khala code plans panel", () => {
  test("renders both plan cards from the injected catalog with honest paid availability", async () => {
    const container = installDom()
    const panel = mountWith(container, {
      status: async () => ({ state: "unauthenticated" }),
    })
    await panel.refresh()
    await flushPanel()

    expect(container.textContent).toContain("Plan")
    const freeCard = container.querySelector("[data-khala-plans-card='khala_code.plan.free.v1']")
    const paidCard = container.querySelector("[data-khala-plans-card='khala_code.plan.paid.v1']")
    expect(freeCard).not.toBeNull()
    expect(paidCard).not.toBeNull()
    expect(freeCard?.textContent).toContain("Free")
    expect(freeCard?.textContent).toContain("Pay with data")
    expect(freeCard?.textContent).toContain("Coding sessions may be captured for training.")
    expect(paidCard?.textContent).toContain("Paid")
    expect(paidCard?.textContent).toContain("Private data")
    expect(paidCard?.textContent).toContain("Not yet purchasable")
    expect(paidCard?.textContent).toContain("the paid plan purchase seam is not armed")

    // No enabled purchase control while the seam is unarmed.
    const purchaseButton = container.querySelector<HTMLButtonElement>("[data-khala-plans-action='purchase']")
    expect(purchaseButton?.disabled).toBe(true)
    expect(container.querySelector("button[data-khala-plans-action='purchase']:not([disabled])")).toBeNull()
  })

  test("shows the server-resolved paid plan with capture opt-out active", async () => {
    const container = installDom()
    const panel = mountWith(container, {
      status: async () => ({
        plan: {
          captureExcluded: true,
          kind: "paid",
          planId: "khala_code.plan.paid.v1",
          reasonRef: "entitlement.khala_code.paid.test",
        },
        state: "ok",
      }),
    })
    await panel.refresh()
    await flushPanel()

    const current = container.querySelector("[data-khala-plans-current]")
    expect(current?.textContent).toBe("Current plan: Paid — capture opt-out active")
  })

  test("treats a missing agent token as Free (default) without fabricating a plan", async () => {
    const container = installDom()
    const panel = mountWith(container, {
      status: async () => ({ state: "unauthenticated" }),
    })
    await panel.refresh()
    await flushPanel()

    const current = container.querySelector("[data-khala-plans-current]")
    expect(current?.textContent).toBe("Current plan: Free (default) — not signed in")
  })

  test("renders an honest unavailable state when plan status cannot be resolved", async () => {
    const container = installDom()
    const panel = mountWith(container, {
      status: async () => ({ state: "unavailable" }),
    })
    await panel.refresh()
    await flushPanel()

    const current = container.querySelector("[data-khala-plans-current]")
    expect(current?.textContent).toBe(
      "Current plan: unavailable — the plan service could not be reached",
    )
  })

  test("renders an honest catalog error instead of fabricated cards", async () => {
    const container = installDom()
    const panel = mountWith(container, {
      catalog: async () => ({ ok: false, error: "catalog_unavailable" }),
    })
    await panel.refresh()
    await flushPanel()

    expect(container.querySelector("[data-khala-plans-error]")).not.toBeNull()
    expect(container.textContent).toContain("Plan catalog unavailable")
    expect(container.querySelector("[data-khala-plans-card='khala_code.plan.free.v1']")).toBeNull()
    expect(container.querySelector("[data-khala-plans-card='khala_code.plan.paid.v1']")).toBeNull()
    expect(container.querySelector("[data-khala-plans-action='purchase']")).toBeNull()
  })

  test("never invokes the purchase transport while the seam is unarmed", async () => {
    const container = installDom()
    let purchaseCalls = 0
    const panel = mountWith(container, {
      catalog: async () => ({ ok: true, catalog: fixtureCatalog({ paidArmed: false }) }),
      purchase: async () => {
        purchaseCalls += 1
        return { ok: false, error: "khala_code_paid_plans_not_enabled" }
      },
    })
    await panel.refresh()
    await flushPanel()

    container.querySelector<HTMLButtonElement>("[data-khala-plans-action='purchase']")
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await flushPanel()

    expect(purchaseCalls).toBe(0)
  })

  test("invokes purchase when armed and renders the server not-enabled refusal honestly", async () => {
    const container = installDom()
    const purchaseRequests: Array<KhalaCodeDesktopPlanPurchaseRequest | undefined> = []
    const panel = mountWith(container, {
      catalog: async () => ({ ok: true, catalog: fixtureCatalog({ paidArmed: true }) }),
      purchase: async request => {
        purchaseRequests.push(request)
        return { ok: false, error: "khala_code_paid_plans_not_enabled" }
      },
    })
    await panel.refresh()
    await flushPanel()

    const purchaseButton = container.querySelector<HTMLButtonElement>("[data-khala-plans-action='purchase']")
    expect(purchaseButton).not.toBeNull()
    expect(purchaseButton?.disabled).toBe(false)
    purchaseButton?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await flushPanel()

    expect(purchaseRequests).toHaveLength(1)
    expect(typeof purchaseRequests[0]?.idempotencyKey).toBe("string")
    const note = container.querySelector("[data-khala-plans-purchase-note]")
    expect(note?.textContent).toContain("paid plan purchases are not enabled on the server yet")
    expect(note?.textContent).toContain("khala_code_paid_plans_not_enabled")
  })
})
