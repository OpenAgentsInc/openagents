// CL-52: "Session Detail" pane — focused view of one session: verify line,
// artifact line, Cancel, expandable event timeline.
//
// Owns this file only. No edits to other files except the new test file.

import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type { SessionArtifactStats, SessionEventRow } from "../../shared/rpc"
import type { PaneContext } from "../context"
import { emptyLine, escapeHtml } from "../dom"

// ── Pure helpers (exported so tests can cover them without DOM) ────────────

/** Returns the verify-line text and tone class for a session. */
export function verifyLineText(session: SessionSummary): { text: string; toneClass: string } {
  // The shared SessionSummary type does not carry artifactRef / errorClass
  // today, but the Pylon wire format does include them when present. We access
  // them via a safe cast so future protocol additions are picked up without
  // breaking older nodes that omit them.
  const ext = session as unknown as { artifactRef?: string | null; errorClass?: string | null }

  if (session.state === "completed") {
    const suffix = ext.artifactRef ? ` · artifact ${ext.artifactRef.slice(-12)}` : ""
    return { text: `✓ verify passed${suffix}`, toneClass: "verify-completed" }
  }
  if (session.state === "failed") {
    const suffix = ext.errorClass ? ` · ${ext.errorClass}` : ""
    return { text: `✗ verify failed${suffix}`, toneClass: "verify-failed" }
  }
  if (session.state === "cancelled") {
    return { text: "cancelled", toneClass: "verify-cancelled" }
  }
  return { text: `${session.state}…`, toneClass: "verify-cancelled" }
}

/** Returns the artifact-line text from stats, or "" when stats is absent. */
export function artifactLineText(stats: SessionArtifactStats | undefined | null): string {
  if (!stats) return ""
  const parts = [`artifact: ${stats.outcome ?? stats.kind}`]
  if (stats.editedFileCount != null) parts.push(`${stats.editedFileCount} files`)
  if (stats.commandCount != null) parts.push(`${stats.commandCount} cmds`)
  if (stats.totalTokens != null) parts.push(`${stats.totalTokens} tok`)
  return parts.join(" · ")
}

// ── DOM helpers ────────────────────────────────────────────────────────────

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  opts: { className?: string; textContent?: string; style?: Partial<CSSStyleDeclaration> } = {},
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag)
  if (opts.className) node.className = opts.className
  if (opts.textContent != null) node.textContent = opts.textContent
  if (opts.style) Object.assign(node.style, opts.style)
  return node
}

function renderEventTimeline(
  container: HTMLElement,
  events: SessionEventRow[],
): void {
  if (events.length === 0) {
    container.append(emptyLine("No events yet."))
    return
  }

  const ul = el("ul", { className: "session-timeline" })

  // Track expanded state per eventIndex. We key by eventIndex (number) since
  // eventIndex is stable across re-renders of a single pane mount.
  const expandedSet = new Set<number>()

  function rebuildRow(li: HTMLLIElement, event: SessionEventRow): void {
    li.innerHTML = ""

    const expandable = (event.full != null && event.full.length > 0) || event.detail.length > 30
    const isOpen = expandedSet.has(event.eventIndex)
    const label = isOpen ? event.full || event.detail || event.phase : event.detail || event.phase
    const time = event.observedAt.slice(11, 19)

    const detailSpan = el("span", { className: "event-detail" })
    detailSpan.textContent = label

    let metaText = `${event.phase} · #${event.eventIndex} · ${time}`
    if (expandable) metaText += isOpen ? " · tap to collapse" : " · tap to expand"
    const metaSpan = el("span", { className: "event-meta", textContent: metaText })

    li.append(detailSpan, metaSpan)
  }

  for (const event of events) {
    const li = el("li", { className: `event-row event-${escapeHtml(event.state)}` })
    rebuildRow(li, event)

    const expandable = (event.full != null && event.full.length > 0) || event.detail.length > 30
    if (expandable) {
      li.style.cursor = "pointer"
      li.addEventListener("click", () => {
        if (expandedSet.has(event.eventIndex)) {
          expandedSet.delete(event.eventIndex)
        } else {
          expandedSet.add(event.eventIndex)
        }
        rebuildRow(li, event)
      })
    }

    ul.append(li)
  }

  container.append(ul)
}

// ── Main pane render ───────────────────────────────────────────────────────

export function renderSessionDetailPane(container: HTMLElement, ctx: PaneContext): void {
  // "‹ sessions" back button
  const back = el("button", { className: "link-button", textContent: "‹ sessions" })
  back.addEventListener("click", () => ctx.navigate("sessions"))
  container.append(back)

  const ref = ctx.selectedSessionRef
  const session = ref ? (ctx.node?.sessions.find((s) => s.sessionRef === ref) ?? null) : null

  if (!session) {
    container.append(emptyLine("Session not found."))
    return
  }

  // Session ref heading
  const refEl = el("p", { className: "detail-ref", textContent: ref ?? "" })
  container.append(refEl)

  // Verify line
  const { text: verifyText, toneClass } = verifyLineText(session)
  const verifyEl = el("p", { className: `verify-line ${toneClass}`, textContent: verifyText })
  container.append(verifyEl)

  // Artifact line (optional — only when stats present)
  const stats = ref ? ctx.node?.artifacts?.[ref] : undefined
  const artText = artifactLineText(stats)
  if (artText.length > 0) {
    const artEl = el("p", { className: "artifact-line", textContent: artText })
    container.append(artEl)
  }

  // Cancel button — only for cancellable states
  const cancellable = session.state === "running" || session.state === "queued" || (session.state as string) === "started"
  if (cancellable && ref) {
    const cancelBtn = el("button", { textContent: "Cancel session" })
    cancelBtn.style.border = "1px solid #ff6b6b"
    cancelBtn.style.color = "#ff6b6b"
    cancelBtn.style.background = "transparent"
    cancelBtn.style.fontFamily = "inherit"
    cancelBtn.style.fontSize = ".9rem"
    cancelBtn.style.padding = ".35rem .75rem"
    cancelBtn.style.borderRadius = ".3rem"
    cancelBtn.style.cursor = "pointer"
    cancelBtn.style.marginTop = ".4rem"
    cancelBtn.addEventListener("click", () => {
      cancelBtn.disabled = true
      void ctx.request
        .cancelSession({ sessionRef: ref })
        .then(() => ctx.refresh())
        .catch(() => {
          cancelBtn.disabled = false
        })
    })
    container.append(cancelBtn)
  }

  // Event timeline
  const events = ref ? (ctx.node?.events?.[ref] ?? []) : []
  renderEventTimeline(container, events)
}
