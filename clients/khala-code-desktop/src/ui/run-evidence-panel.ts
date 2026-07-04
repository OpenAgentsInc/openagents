import type {
  KhalaCodeDesktopOutsideUserRunReportRequest,
  KhalaCodeDesktopOutsideUserRunReportResult,
  KhalaCodeDesktopOutsideUserRunReceipt,
} from "../shared/rpc"

export type KhalaCodeRunEvidencePanelHandle = Readonly<{
  refresh: () => Promise<void>
}>

export type KhalaCodeRunEvidencePanelOptions = Readonly<{
  report: (
    request?: KhalaCodeDesktopOutsideUserRunReportRequest,
  ) => Promise<KhalaCodeDesktopOutsideUserRunReportResult>
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

const reportIdempotencyKey = (): string =>
  `khala-code:outside-user-run:${Date.now().toString(36)}:${Math.random().toString(36).slice(2, 10)}`

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

export const mountKhalaCodeRunEvidencePanel = (
  container: HTMLElement,
  options: KhalaCodeRunEvidencePanelOptions,
): KhalaCodeRunEvidencePanelHandle => {
  let note: string | null = null
  let receipt: KhalaCodeDesktopOutsideUserRunReceipt | null = null
  let posting = false
  let pendingReportKey: string | null = null

  const render = (): void => {
    container.querySelector("[data-khala-run-evidence-root]")?.remove()
    const section = el("section", "khala-settings-section khala-run-evidence")
    section.dataset.khalaRunEvidenceRoot = ""
    section.append(
      el("h3", "khala-settings-section-title", "Run evidence"),
      el(
        "p",
        "khala-run-evidence-subtitle",
        "Post a public-safe receipt for this install.",
      ),
      el(
        "p",
        "khala-run-evidence-copy",
        "Shared fields: app version, platform, Codex readiness, Pylon readiness. No paths, prompts, tokens, or logs.",
      ),
    )

    const button = el("button", "khala-run-evidence-submit", "Post run receipt")
    button.type = "button"
    button.dataset.khalaRunEvidenceAction = "post"
    button.disabled = posting
    section.append(button)

    if (note !== null) {
      const status = el("div", "khala-run-evidence-note", note)
      status.dataset.khalaRunEvidenceNote = ""
      status.setAttribute("role", "status")
      section.append(status)
    }

    if (receipt !== null) {
      const ref = el("div", "khala-run-evidence-receipt", receipt.receiptRef)
      ref.dataset.khalaRunEvidenceReceipt = receipt.receiptRef
      section.append(ref)
    }

    container.append(section)
  }

  const postReceipt = async (): Promise<void> => {
    if (posting) return
    posting = true
    note = "Posting run receipt..."
    render()
    pendingReportKey ??= reportIdempotencyKey()
    try {
      const result = await options.report({ idempotencyKey: pendingReportKey })
      if (result.ok) {
        receipt = result.receipt
        note = result.idempotent
          ? "Receipt already posted for this request."
          : "Receipt posted."
        pendingReportKey = null
      } else {
        note = "Receipt not posted: service unavailable."
      }
    } catch (error) {
      note = `Receipt not posted: ${errorMessage(error)}`
    }
    posting = false
    render()
  }

  container.addEventListener("click", event => {
    const target = event.target instanceof Element
      ? event.target.closest<HTMLButtonElement>("[data-khala-run-evidence-action=\"post\"]")
      : null
    if (target === null || target.disabled) return
    event.preventDefault()
    void postReceipt()
  })

  render()

  return {
    async refresh() {
      render()
    },
  }
}
