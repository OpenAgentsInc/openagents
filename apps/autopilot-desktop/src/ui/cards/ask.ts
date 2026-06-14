// CL-47: "Ask Autopilot" (submit a work intent) + "Your asks" (ship-status
// round-trip). Owns this file only.
//
// Part 1 — "Ask Autopilot": title input + body textarea + "Send to node" button.
//   Validates via validateIntentDraft; on success calls ctx.request.submitIntent.
// Part 2 — "Your asks": lists ctx.node?.intents (up to 5) with a colored dot
//   and a short ship-status label produced by the pure shipStatusLine helper.

import { validateIntentDraft } from "@openagentsinc/autopilot-control-protocol"
import type { PaneContext } from "../context"
import { card, emptyLine, escapeHtml } from "../dom"

// --- Pure helper: intent ship-status → short label ---

export type ShipStatusLine = {
  readonly text: string
  readonly terminal: boolean
  /** CSS color hint (not authoritative — callers may override). */
  readonly dotColor: string
}

/**
 * Map an intent status string to a short label + terminal flag.
 * Pure and exportable so it can be unit-tested without a DOM.
 */
export function shipStatusLine(status: string): ShipStatusLine {
  switch (status) {
    case "received":
      return { text: "received", terminal: false, dotColor: "#8b949e" }
    case "planning":
      return { text: "planning…", terminal: false, dotColor: "#58a6ff" }
    case "fanning_out":
      return { text: "agents working…", terminal: false, dotColor: "#58a6ff" }
    case "shipping":
      return { text: "shipping…", terminal: false, dotColor: "#d29922" }
    case "shipped":
      return { text: "✓ shipped", terminal: true, dotColor: "#3fb950" }
    case "failed":
      return { text: "✗ failed", terminal: true, dotColor: "#f85149" }
    default:
      return { text: status, terminal: false, dotColor: "#8b949e" }
  }
}

// --- DOM render ---

export function renderAskCard(container: HTMLElement, ctx: PaneContext): void {
  // ── Part 1: "Ask Autopilot" submission form ────────────────────────────────

  const { section: formSection, body: formBody } = card("Ask Autopilot")

  // Title input
  const titleInput = document.createElement("input")
  titleInput.type = "text"
  titleInput.placeholder = "title — what do you want done?"
  titleInput.style.width = "100%"
  titleInput.style.boxSizing = "border-box"
  titleInput.style.marginBottom = "8px"
  formBody.append(titleInput)

  // Body textarea
  const bodyInput = document.createElement("textarea")
  bodyInput.placeholder = "details (optional)"
  bodyInput.rows = 3
  bodyInput.style.width = "100%"
  bodyInput.style.boxSizing = "border-box"
  bodyInput.style.resize = "vertical"
  bodyInput.style.marginBottom = "8px"
  formBody.append(bodyInput)

  // Submit button
  const sendBtn = document.createElement("button")
  sendBtn.textContent = "Send to node"
  formBody.append(sendBtn)

  // Status line (error / sending / sent)
  const statusLine = document.createElement("p")
  statusLine.className = "deploy-status"
  statusLine.style.marginTop = "8px"
  statusLine.style.display = "none"
  formBody.append(statusLine)

  const showStatus = (text: string, kind: "info" | "error" | "success") => {
    statusLine.style.display = "block"
    statusLine.style.color = kind === "error" ? "#f85149" : kind === "success" ? "#3fb950" : "#8b949e"
    statusLine.innerHTML = escapeHtml(text)
  }

  const clearStatus = () => {
    statusLine.style.display = "none"
    statusLine.textContent = ""
  }

  sendBtn.addEventListener("click", () => {
    const raw = { title: titleInput.value, body: bodyInput.value }
    const validation = validateIntentDraft(raw)

    if (!validation.ok) {
      showStatus(`error: ${validation.errors[0] ?? "invalid input"}`, "error")
      return
    }

    sendBtn.disabled = true
    clearStatus()
    showStatus("sending…", "info")

    void ctx.request
      .submitIntent({ title: validation.title, body: validation.body })
      .then((r) => {
        if (r.ok) {
          showStatus(`sent · ${r.status}`, "success")
          titleInput.value = ""
          bodyInput.value = ""
        } else {
          showStatus(`error: ${r.error ?? r.status}`, "error")
        }
      })
      .catch((e: unknown) => {
        showStatus(`error: ${e instanceof Error ? e.message : String(e)}`, "error")
      })
      .finally(() => {
        sendBtn.disabled = false
      })
  })

  container.append(formSection)

  // ── Part 2: "Your asks" — intent history list ──────────────────────────────

  const { section: listSection, body: listBody } = card("Your asks")

  const intents = ctx.node?.intents ?? []

  if (intents.length === 0) {
    listBody.append(emptyLine("No asks yet."))
  } else {
    const list = document.createElement("ul")
    list.style.listStyle = "none"
    list.style.margin = "0"
    list.style.padding = "0"

    for (const intent of intents.slice(0, 5)) {
      const sl = shipStatusLine(intent.status)
      const label = intent.title.trim() !== "" ? intent.title : intent.intentId.slice(-8)

      const li = document.createElement("li")
      li.style.display = "flex"
      li.style.alignItems = "center"
      li.style.gap = "8px"
      li.style.padding = "4px 0"

      const dot = document.createElement("span")
      dot.style.display = "inline-block"
      dot.style.width = "8px"
      dot.style.height = "8px"
      dot.style.borderRadius = "50%"
      dot.style.flexShrink = "0"
      dot.style.backgroundColor = sl.dotColor

      const text = document.createElement("span")
      text.className = "deploy-help"
      text.style.overflow = "hidden"
      text.style.textOverflow = "ellipsis"
      text.style.whiteSpace = "nowrap"
      text.innerHTML = `${escapeHtml(label)} · ${escapeHtml(sl.text)}`

      li.append(dot, text)
      list.append(li)
    }

    listBody.append(list)
  }

  container.append(listSection)
}
