import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import { mountKhalaCodePlansPanel } from "../src/ui/plans-panel"
import type {
  KhalaCodeDesktopPlanCatalog,
  KhalaCodeDesktopPlanCatalogResult,
  KhalaCodeDesktopPlanPurchaseRequest,
  KhalaCodeDesktopPlanPurchaseResult,
  KhalaCodeDesktopPlanStatusResult,
  KhalaCodeDesktopTraceCaptureConsentWriteRequest,
  KhalaCodeDesktopTraceCaptureConsentWriteResult,
  KhalaCodeDesktopTraceCaptureStatusResult,
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
  setGlobal("Event", window.Event)
  setGlobal("MouseEvent", window.MouseEvent)
  setGlobal("customElements", window.customElements)
  const container = window.document.createElement("section")
  window.document.body.append(container)
  return container as unknown as HTMLElement
}

const flushPanel = async (): Promise<void> => {
  for (let index = 0; index < 50; index += 1) {
    await Promise.resolve()
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

const fixtureTraceCaptureStatus = (
  input: {
    readonly enabled?: boolean
    readonly ownerArmed?: boolean
  } = {},
): KhalaCodeDesktopTraceCaptureStatusResult => ({
  blockerRefs: input.enabled === true && input.ownerArmed !== true
    ? ["blocker.owner.khala_code_desktop_trace_capture_arming_missing"]
    : [],
  disclosureRef: "data.free_tier_capture_disclosure.v1",
  enabled: input.enabled === true,
  marker: {
    payoutEligible: false,
    revenueShareEligible: false,
    settlementEligible: false,
  },
  ok: true,
  ownerArmed: input.ownerArmed === true,
  ownerGateEnv: "KHALA_CODE_DESKTOP_TRACE_CAPTURE_ENABLED",
  path: "/tmp/khala-code-settings.json",
  pipeline: {
    ingestAudience: "owner_only",
    redaction: "rampart_required",
    sessionEvents: "explicit_consent_only",
  },
  promiseId: "khala_code.free_plan_trace_capture.v1",
  reason: input.enabled === true
    ? input.ownerArmed === true
      ? "ready_for_redacted_owner_only_ingest"
      : "owner_not_armed"
    : "consent_disabled",
  schemaVersion: "openagents.khala_code.desktop_trace_capture_status.v1",
  state: "not_captured",
})

type Transports = {
  readonly baseUrl?: string
  readonly catalog?: () => Promise<KhalaCodeDesktopPlanCatalogResult>
  readonly openExternal?: (url: string) => Promise<boolean>
  readonly status?: () => Promise<KhalaCodeDesktopPlanStatusResult>
  readonly purchase?: (
    request?: KhalaCodeDesktopPlanPurchaseRequest,
  ) => Promise<KhalaCodeDesktopPlanPurchaseResult>
  readonly traceCaptureStatus?: () => Promise<KhalaCodeDesktopTraceCaptureStatusResult>
  readonly traceCaptureConsentWrite?: (
    request: KhalaCodeDesktopTraceCaptureConsentWriteRequest,
  ) => Promise<KhalaCodeDesktopTraceCaptureConsentWriteResult>
}

const mountWith = (container: HTMLElement, transports: Transports) =>
  mountKhalaCodePlansPanel(container, {
    ...(transports.baseUrl === undefined ? {} : { baseUrl: transports.baseUrl }),
    catalog: transports.catalog ?? (async () => ({ ok: true, catalog: fixtureCatalog({ paidArmed: false }) })),
    openExternal: transports.openExternal ?? (async () => true),
    purchase: transports.purchase ?? (async () => ({ ok: false, error: "purchase_unavailable" })),
    status: transports.status ?? (async () => ({ state: "unauthenticated" })),
    traceCaptureStatus: transports.traceCaptureStatus ?? (async () => fixtureTraceCaptureStatus()),
    traceCaptureConsentWrite: transports.traceCaptureConsentWrite ??
      (async request => ({ ...fixtureTraceCaptureStatus({ enabled: request.enabled }), saved: true })),
  })

// Oracle for khala_code.plans.checkout_handoff_server_truth.v1
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
    expect(container.querySelector("[data-khala-plans-credits]")?.textContent).toContain("Credits")
    expect(container.querySelector("[data-khala-plans-credits]")?.textContent).toContain(
      "Balance and packages are server-rendered in billing.",
    )

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
    expect(container.querySelector("[data-khala-trace-capture-status]")?.textContent).toBe(
      "Trace capture: not captured — paid plan capture opt-out active.",
    )
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

  // Oracle for khala_code.plans.free_trace_capture_explicit_consent.v1
  test("renders trace capture off by default and never writes before explicit consent", async () => {
    const container = installDom()
    let writes = 0
    const panel = mountWith(container, {
      traceCaptureConsentWrite: async request => {
        writes += 1
        return { ...fixtureTraceCaptureStatus({ enabled: request.enabled }), saved: true }
      },
      traceCaptureStatus: async () => fixtureTraceCaptureStatus(),
    })
    await panel.refresh()
    await flushPanel()

    const checkbox = container.querySelector<HTMLInputElement>(
      "[data-khala-trace-capture-action='consent']",
    )
    expect(checkbox).not.toBeNull()
    expect(checkbox?.checked).toBe(false)
    expect(container.querySelector("[data-khala-trace-capture-status]")?.textContent).toBe(
      "Trace capture: off — no session events are captured.",
    )
    expect(container.textContent).toContain("Redaction failure fails closed to not captured")
    expect(container.textContent).toContain("does not create payout or settlement eligibility")
    expect(writes).toBe(0)
  })

  test("persists trace capture consent only after the user toggles the checkbox", async () => {
    const container = installDom()
    const writes: KhalaCodeDesktopTraceCaptureConsentWriteRequest[] = []
    const panel = mountWith(container, {
      traceCaptureConsentWrite: async request => {
        writes.push(request)
        return { ...fixtureTraceCaptureStatus({ enabled: request.enabled }), saved: true }
      },
      traceCaptureStatus: async () => fixtureTraceCaptureStatus(),
    })
    await panel.refresh()
    await flushPanel()

    const checkbox = container.querySelector<HTMLInputElement>(
      "[data-khala-trace-capture-action='consent']",
    )
    expect(checkbox).not.toBeNull()
    if (checkbox !== null) checkbox.checked = true
    checkbox?.dispatchEvent(new Event("change", { bubbles: true }))
    await flushPanel()

    expect(writes).toEqual([{ enabled: true }])
    expect(container.querySelector<HTMLInputElement>(
      "[data-khala-trace-capture-action='consent']",
    )?.checked).toBe(true)
    expect(container.querySelector("[data-khala-trace-capture-status]")?.textContent).toBe(
      "Trace capture: consent saved, not captured — owner arming is pending.",
    )
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
    expect(container.querySelector("[data-khala-plans-action='credits']")).not.toBeNull()
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

  test("opens the Stripe checkout URL returned by the server and keeps plan state server-resolved", async () => {
    const container = installDom()
    const openedUrls: string[] = []
    let statusCalls = 0
    const panel = mountWith(container, {
      catalog: async () => ({ ok: true, catalog: fixtureCatalog({ paidArmed: true }) }),
      openExternal: async url => {
        openedUrls.push(url)
        return true
      },
      purchase: async () => ({
        ok: true,
        checkoutUrl: "https://checkout.stripe.test/session/cs_test_khala",
        planId: "khala_code.plan.paid.v1",
        purchaseRef: "purchase.khala_code_paid_plan.test",
        rail: "stripe_checkout",
        status: "payment_required",
        stripeCheckoutSessionId: "cs_test_khala",
      }),
      status: async () => {
        statusCalls += 1
        return { state: "unauthenticated" }
      },
    })
    await panel.refresh()
    await flushPanel()
    const statusCallsAfterRefresh = statusCalls

    container
      .querySelector<HTMLButtonElement>("[data-khala-plans-action='purchase']")
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await flushPanel()

    expect(openedUrls).toEqual(["https://checkout.stripe.test/session/cs_test_khala"])
    expect(statusCalls).toBe(statusCallsAfterRefresh + 1)
    expect(container.querySelector("[data-khala-plans-current]")?.textContent).toBe(
      "Current plan: Free (default) — not signed in",
    )
    expect(container.querySelector("[data-khala-plans-purchase-note]")?.textContent).toContain(
      "Paid plan checkout opened",
    )
  })

  test("opens the existing web billing checkout for credits from the same surface", async () => {
    const container = installDom()
    const openedUrls: string[] = []
    const panel = mountWith(container, {
      baseUrl: "https://openagents.test",
      openExternal: async url => {
        openedUrls.push(url)
        return true
      },
    })
    await panel.refresh()
    await flushPanel()

    const creditsButton = container.querySelector<HTMLButtonElement>("[data-khala-plans-action='credits']")
    expect(creditsButton).not.toBeNull()
    expect(creditsButton?.disabled).toBe(false)
    creditsButton?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
    await flushPanel()

    expect(openedUrls).toEqual(["https://openagents.test/billing"])
    expect(container.querySelector("[data-khala-plans-purchase-note]")?.textContent).toContain(
      "Credit checkout opened in openagents.com billing",
    )
  })

  test("reuses one idempotency key across purchase retries until a confirmed success", async () => {
    const container = installDom()
    const purchaseRequests: Array<KhalaCodeDesktopPlanPurchaseRequest | undefined> = []
    let statusCalls = 0
    const panel = mountWith(container, {
      catalog: async () => ({ ok: true, catalog: fixtureCatalog({ paidArmed: true }) }),
      purchase: async request => {
        purchaseRequests.push(request)
        // First attempt looks lost/unavailable; the retry must replay the SAME
        // key so a server that already committed returns the same receipt
        // instead of minting a duplicate purchase.
        return purchaseRequests.length === 1
          ? { ok: false, error: "purchase_unavailable" }
          : {
              ok: true,
              captureExcluded: true,
              entitlementRef: "entitlement.inference.paid_privacy.abc",
              planId: "khala_code.plan.paid.v1",
              receiptRef: "receipt.inference.privacy_entitlement.khala_code_paid_plan_x",
              receiptUrl: "/api/public/inference/privacy-receipts/receipt.x",
            }
      },
      status: async () => {
        statusCalls += 1
        return { state: "unauthenticated" }
      },
    })
    await panel.refresh()
    await flushPanel()
    const statusCallsAfterRefresh = statusCalls

    const clickPurchase = async () => {
      container
        .querySelector<HTMLButtonElement>("[data-khala-plans-action='purchase']")
        ?.dispatchEvent(new MouseEvent("click", { bubbles: true }))
      await flushPanel()
    }
    await clickPurchase()
    await clickPurchase()

    expect(purchaseRequests).toHaveLength(2)
    expect(purchaseRequests[0]?.idempotencyKey).toBe(purchaseRequests[1]?.idempotencyKey)
    // The plan status is re-read after EVERY attempt (a lost response may
    // still have granted the entitlement server-side).
    expect(statusCalls).toBe(statusCallsAfterRefresh + 2)
  })
})
