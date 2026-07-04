// Khala Code plan-selection surface (promise khala_code.free_paid_plans.v1).
//
// Honesty rules for this panel:
// - The current plan is resolved SERVER-SIDE via khalaCodePlanStatus. Without a
//   configured agent token the panel shows "Free (default)" and never fabricates
//   a plan.
// - Plans are not purchasable while the paid plan purchase seam is flag-gated
//   OFF on the server; while `purchase.armed` is false the panel renders only a
//   disabled control and never invokes the purchase transport. When armed, a
//   purchase may return payment_required before any receipt exists, and the
//   panel opens only the checkout URL returned by the server.
// - Credit packages remain owned by the existing openagents.com billing
//   surface. The desktop panel provides the handoff to web billing but does not
//   fabricate package tiers, balances, or post-purchase state locally.
// - If the catalog cannot be loaded, the panel says so instead of rendering
//   fabricated plan cards.
import type {
  KhalaCodeDesktopPlan,
  KhalaCodeDesktopPlanCatalog,
  KhalaCodeDesktopPlanCatalogResult,
  KhalaCodeDesktopPlanPurchaseRequest,
  KhalaCodeDesktopPlanPurchaseResult,
  KhalaCodeDesktopPlanStatusResult,
} from "../shared/rpc"

export type KhalaCodePlansPanelHandle = Readonly<{
  refresh: () => Promise<void>
}>

export type KhalaCodePlansPanelOptions = Readonly<{
  catalog: () => Promise<KhalaCodeDesktopPlanCatalogResult>
  status: () => Promise<KhalaCodeDesktopPlanStatusResult>
  purchase: (
    request?: KhalaCodeDesktopPlanPurchaseRequest,
  ) => Promise<KhalaCodeDesktopPlanPurchaseResult>
  openExternal: (url: string) => Promise<boolean>
  baseUrl?: string
}>

const el = <Tag extends keyof HTMLElementTagNameMap>(
  tag: Tag,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[Tag] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const purchaseIdempotencyKey = (): string =>
  `khala-code:plan-purchase:${Date.now().toString(36)}:${Math.random().toString(36).slice(2, 10)}`

const OpenAgentsBaseUrl = "https://openagents.com"
const BillingPath = "/billing"
const NOT_ARMED_COPY =
  "Not yet purchasable — the paid plan purchase seam is not armed."

const externalPath = (baseUrl: string, path: string): string =>
  new URL(path, baseUrl).toString()

const purchaseFailureCopy = (
  error: "khala_code_paid_plans_not_enabled" | "unauthenticated" | "purchase_unavailable",
): string => {
  if (error === "khala_code_paid_plans_not_enabled") {
    return "Purchase not completed: paid plan purchases are not enabled on the server yet (khala_code_paid_plans_not_enabled)."
  }
  if (error === "unauthenticated") {
    return "Purchase not completed: no signed-in agent token."
  }
  return "Purchase not completed: the purchase service is unavailable."
}

const purchaseSuccessCopy = (
  result: Extract<KhalaCodeDesktopPlanPurchaseResult, { ok: true }>,
): string => {
  if (result.status === "payment_required") {
    return result.rail === "stripe_checkout"
      ? "Paid plan checkout created. The paid plan activates only after Stripe confirms payment."
      : "Paid plan Lightning invoice created. The paid plan activates only after payment proof is submitted."
  }
  return `Paid plan purchase recorded: ${result.receiptRef}`
}

const checkoutHandoffCopy = (
  kind: "paid-plan" | "credits",
  opened: boolean,
  url: string,
): string => {
  if (kind === "paid-plan") {
    return opened
      ? "Paid plan checkout opened. The paid plan activates only after the server records settled payment."
      : `Paid plan checkout was created, but Khala Code could not open it automatically: ${url}`
  }
  return opened
    ? "Credit checkout opened in openagents.com billing. Credit balance and package state update there from the server."
    : `Credit checkout did not open automatically: ${url}`
}

export const mountKhalaCodePlansPanel = (
  container: HTMLElement,
  options: KhalaCodePlansPanelOptions,
): KhalaCodePlansPanelHandle => {
  const baseUrl = options.baseUrl ?? OpenAgentsBaseUrl
  let catalogResult: KhalaCodeDesktopPlanCatalogResult | null = null
  let statusResult: KhalaCodeDesktopPlanStatusResult | null = null
  let purchaseNote: string | null = null
  let purchasing = false
  let openingCredits = false

  const loadedCatalog = (): KhalaCodeDesktopPlanCatalog | null =>
    catalogResult !== null && catalogResult.ok ? catalogResult.catalog : null

  const paidPlanArmed = (): boolean => {
    const catalog = loadedCatalog()
    if (catalog === null) return false
    return catalog.plans.some(plan => plan.kind === "paid" && plan.purchase?.armed === true)
  }

  const currentPlanText = (): string => {
    if (statusResult === null) return "Current plan: checking with the server..."
    if (statusResult.state === "unauthenticated") {
      return "Current plan: Free (default) — not signed in"
    }
    if (statusResult.state === "unavailable") {
      return "Current plan: unavailable — the plan service could not be reached"
    }
    const plan = statusResult.plan
    const catalogPlan = loadedCatalog()?.plans.find(entry => entry.planId === plan.planId)
    const label = catalogPlan?.label ?? (plan.kind === "paid" ? "Paid" : "Free")
    return plan.captureExcluded
      ? `Current plan: ${label} — capture opt-out active`
      : `Current plan: ${label}`
  }

  const renderPlanCard = (plan: KhalaCodeDesktopPlan): HTMLElement => {
    const card = el("article", "khala-plans-card")
    card.dataset.khalaPlansCard = plan.planId
    card.append(
      el("h4", "khala-plans-card-title", plan.label),
      el("div", "khala-plans-card-tagline", plan.tagline),
      el("div", "khala-plans-card-price", plan.priceLabel),
    )
    if (plan.isDefault) {
      card.append(el("div", "khala-plans-card-default", "Default plan"))
    }
    const terms = el("ul", "khala-plans-card-terms")
    for (const term of plan.terms) {
      terms.append(el("li", "khala-plans-card-term", term))
    }
    card.append(terms)
    if (plan.kind === "paid") {
      const armed = plan.purchase?.armed === true
      card.append(
        el(
          "div",
          "khala-plans-card-availability",
          armed
            ? "The server purchase seam is armed."
            : NOT_ARMED_COPY,
        ),
      )
      const button = el("button", "khala-plans-purchase", "Purchase paid plan")
      button.type = "button"
      button.dataset.khalaPlansAction = "purchase"
      if (!armed || purchasing) button.disabled = true
      card.append(button)
    }
    return card
  }

  const renderCreditCheckout = (): HTMLElement => {
    const card = el("article", "khala-plans-credit-card")
    card.dataset.khalaPlansCredits = ""
    card.append(
      el("h4", "khala-plans-card-title", "Credits"),
      el("div", "khala-plans-card-tagline", "Add usage credits through openagents.com billing."),
      el("div", "khala-plans-credit-authority", "Balance and packages are server-rendered in billing."),
    )
    const button = el("button", "khala-plans-purchase khala-plans-credit-checkout", "Open credit checkout")
    button.type = "button"
    button.dataset.khalaPlansAction = "credits"
    if (openingCredits) button.disabled = true
    card.append(button)
    return card
  }

  const renderBody = (): readonly HTMLElement[] => {
    if (catalogResult === null) {
      return [el("div", "khala-plans-empty", "Loading plan catalog...")]
    }
    if (!catalogResult.ok) {
      const error = el(
        "div",
        "khala-plans-error",
        "Plan catalog unavailable — Khala Code could not load the plan catalog from the server.",
      )
      error.dataset.khalaPlansError = ""
      return [error, renderCreditCheckout()]
    }
    const catalog = catalogResult.catalog
    const summary = el("p", "khala-plans-summary", catalog.summary)
    const cards = el("div", "khala-plans-cards")
    for (const plan of catalog.plans) cards.append(renderPlanCard(plan))
    cards.append(renderCreditCheckout())
    return [summary, cards]
  }

  const render = (): void => {
    container.querySelector("[data-khala-plans-root]")?.remove()
    const section = el("section", "khala-settings-section khala-settings-section--plans")
    section.dataset.khalaPlansRoot = ""
    section.append(
      el("h3", "khala-settings-section-title", "Plan"),
      el(
        "p",
        "khala-plans-subtitle",
        "Khala Code has two plans: Free (pay with data) and Paid (private data: capture opt-out). The paid plan activates only after the server records a settled payment receipt.",
      ),
    )
    const current = el("div", "khala-plans-current", currentPlanText())
    current.dataset.khalaPlansCurrent = ""
    section.append(current, ...renderBody())
    if (purchaseNote !== null) {
      const note = el("div", "khala-plans-purchase-note", purchaseNote)
      note.dataset.khalaPlansPurchaseNote = ""
      note.setAttribute("role", "status")
      section.append(note)
    }
    container.append(section)
  }

  // One idempotency key per purchase INTENT, reused across retries: if the
  // worker committed but the response was lost, the retry replays the same key
  // and gets the same receipt instead of minting a duplicate purchase. The key
  // is cleared only after a confirmed success.
  let pendingPurchaseKey: string | null = null

  const purchase = async (): Promise<void> => {
    if (purchasing) return
    // Fail closed: never invoke the purchase transport while the server seam
    // is not armed, even if a stale or forged click reaches this handler.
    if (!paidPlanArmed()) {
      purchaseNote = NOT_ARMED_COPY
      render()
      return
    }
    purchasing = true
    purchaseNote = "Requesting paid plan purchase..."
    render()
    pendingPurchaseKey ??= purchaseIdempotencyKey()
    try {
      const result = await options.purchase({ idempotencyKey: pendingPurchaseKey })
      purchaseNote = result.ok ? purchaseSuccessCopy(result) : purchaseFailureCopy(result.error)
      if (result.ok && result.status === "payment_required" && result.rail === "stripe_checkout") {
        const opened = await options.openExternal(result.checkoutUrl).catch(() => false)
        purchaseNote = checkoutHandoffCopy("paid-plan", opened, result.checkoutUrl)
      }
      if (result.ok && result.status !== "payment_required") {
        pendingPurchaseKey = null
      }
      // Refresh the server-side plan status after EVERY attempt: a response
      // that failed to decode may still have granted the entitlement, and the
      // status read is the honest source of truth.
      statusResult = await options.status().catch((): KhalaCodeDesktopPlanStatusResult => ({
        state: "unavailable",
      }))
    } catch (error) {
      purchaseNote = `Purchase not completed: ${errorMessage(error)}`
    }
    purchasing = false
    render()
  }

  const openCreditCheckout = async (): Promise<void> => {
    if (openingCredits) return
    openingCredits = true
    purchaseNote = "Opening credit checkout..."
    render()
    const url = externalPath(baseUrl, BillingPath)
    const opened = await options.openExternal(url).catch(() => false)
    purchaseNote = checkoutHandoffCopy("credits", opened, url)
    openingCredits = false
    render()
  }

  container.addEventListener("click", event => {
    const target = event.target instanceof Element
      ? event.target.closest<HTMLButtonElement>("[data-khala-plans-action]")
      : null
    if (target === null || target.disabled) return
    event.preventDefault()
    if (target.dataset.khalaPlansAction === "purchase") void purchase()
    if (target.dataset.khalaPlansAction === "credits") void openCreditCheckout()
  })

  render()

  return {
    async refresh() {
      // The catalog is static per deployment: fetch it until it loads once,
      // then keep the loaded copy so settings-panel re-renders only refetch
      // the (account-scoped) plan status.
      const loaded = catalogResult
      const [catalog, status] = await Promise.all([
        loaded !== null && loaded.ok
          ? Promise.resolve(loaded)
          : options.catalog().catch((): KhalaCodeDesktopPlanCatalogResult => ({
              ok: false,
              error: "catalog_unavailable",
            })),
        options.status().catch((): KhalaCodeDesktopPlanStatusResult => ({
          state: "unavailable",
        })),
      ])
      catalogResult = catalog
      statusResult = status
      render()
    },
  }
}
