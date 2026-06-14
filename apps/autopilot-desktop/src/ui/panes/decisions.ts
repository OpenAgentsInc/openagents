// CL-56: "Decisions" pane — cross-session approvals/decisions queue.
//
// Renders the full pending-approvals queue from ctx.node?.approvals. Each row
// shows the approval prompt (or kind fallback) with Approve/Deny buttons.
// Clicking either button optimistically removes the row and calls
// ctx.request.resolveApproval; if the result is neither applied nor duplicate,
// ctx.refresh() resyncs to the live node state.
//
// No raw secrets or file-system paths are ever rendered — only prompt/kind.

import type { ApprovalRow } from "../../shared/rpc"
import type { PaneContext } from "../context"
import { escapeHtml } from "../dom"

// Pure helper: returns the display label for an approval row.
// Uses prompt when non-empty, falls back to kind.
export function approvalLabel(a: { prompt: string; kind: string }): string {
  return a.prompt.trim() !== "" ? a.prompt : a.kind
}

export function renderDecisionsPane(container: HTMLElement, ctx: PaneContext): void {
  const h = document.createElement("h1")
  h.className = "pane-title"
  h.textContent = "Decisions"
  container.append(h)

  const approvals: ApprovalRow[] = ctx.node?.approvals ?? []

  if (approvals.length === 0) {
    const empty = document.createElement("p")
    empty.className = "empty-state"
    empty.textContent = "Nothing needs you right now."
    container.append(empty)
    return
  }

  for (const approval of approvals) {
    const card = document.createElement("div")
    card.className = "card"
    card.style.cssText = "margin-bottom:12px;"

    const promptEl = document.createElement("p")
    promptEl.className = "card-title"
    promptEl.style.cssText = "margin:0 0 10px;word-break:break-word;"
    promptEl.innerHTML = escapeHtml(approvalLabel(approval))

    const body = document.createElement("div")
    body.className = "card-body"
    body.style.cssText = "display:flex;gap:8px;"

    const approveBtn = document.createElement("button")
    approveBtn.textContent = "Approve"
    approveBtn.style.cssText =
      "padding:4px 14px;border:none;border-radius:4px;font-size:0.82rem;cursor:pointer;background:#5ee08a;color:#111;font-weight:600;"

    const denyBtn = document.createElement("button")
    denyBtn.textContent = "Deny"
    denyBtn.style.cssText =
      "padding:4px 14px;border:none;border-radius:4px;font-size:0.82rem;cursor:pointer;background:#ff6b6b;color:#111;font-weight:600;"

    const resolve = (decision: "approve" | "deny") => {
      // Optimistic removal — remove the card immediately.
      card.remove()
      void ctx.request
        .resolveApproval({ approvalRef: approval.approvalRef, decision })
        .then((result) => {
          // If neither applied nor duplicate, something unexpected happened;
          // re-fetch to get an accurate view.
          if (!result.applied && !result.duplicate) {
            ctx.refresh()
          }
        })
        .catch(() => {
          // Network error — resync so the row reappears if still pending.
          ctx.refresh()
        })
    }

    approveBtn.addEventListener("click", () => resolve("approve"))
    denyBtn.addEventListener("click", () => resolve("deny"))

    body.append(approveBtn, denyBtn)
    card.append(promptEl, body)
    container.append(card)
  }
}
