// CL-55: "Sessions" pane — full session list/management (all states, filter,
// search). Owns this file only.

import type { PaneContext } from "../context"
import { renderSessionList } from "../session-list"

// The filter values (the "All" bucket is represented as "all").
type Filter = "all" | "running" | "queued" | "completed" | "failed" | "cancelled"

const FILTERS: Filter[] = ["all", "running", "queued", "completed", "failed", "cancelled"]

/**
 * Pure helper: produce a short human-readable breakdown of session counts
 * grouped by state. States with zero sessions are omitted. Returns an empty
 * string when the array is empty.
 *
 * Example: "3 running · 1 failed"
 */
export function stateBreakdown(sessions: { state: string }[]): string {
  const counts: Record<string, number> = {}
  for (const s of sessions) {
    counts[s.state] = (counts[s.state] ?? 0) + 1
  }
  return Object.entries(counts)
    .map(([state, n]) => `${n} ${state}`)
    .join(" · ")
}

export function renderSessionsPane(container: HTMLElement, ctx: PaneContext): void {
  // Heading
  const h = document.createElement("h1")
  h.className = "pane-title"
  h.textContent = "Sessions"
  container.append(h)

  // State-breakdown line
  const statusLine = document.createElement("p")
  statusLine.className = "node-status"
  const allSessions = ctx.node?.sessions ?? []
  statusLine.textContent = allSessions.length === 0
    ? ctx.node ? "No sessions." : "Connecting…"
    : stateBreakdown(allSessions)
  container.append(statusLine)

  // Filter bar
  let selected: Filter = "all"

  const filterBar = document.createElement("div")
  filterBar.style.cssText = "display:flex;gap:.4rem;flex-wrap:wrap;margin-bottom:.8rem;"
  container.append(filterBar)

  // List container (re-filled on filter change)
  const listEl = document.createElement("div")
  container.append(listEl)

  function renderList(): void {
    if (!ctx.node) {
      listEl.innerHTML = '<p class="empty-state">Connecting…</p>'
      return
    }
    renderSessionList(listEl, ctx.node, {
      filter: (state) => selected === "all" || state === selected,
      onSelect: (ref) => ctx.navigate("session-detail", ref),
    })
  }

  function renderButtons(): void {
    filterBar.innerHTML = ""
    for (const f of FILTERS) {
      const btn = document.createElement("button")
      btn.textContent = f === "all" ? "All" : f
      const active = f === selected
      btn.style.cssText = [
        "padding:.25rem .6rem",
        "border-radius:.3rem",
        "border:1px solid " + (active ? "#7dd3fc" : "#1c2230"),
        "background:" + (active ? "#0c1929" : "transparent"),
        "color:" + (active ? "#7dd3fc" : "#8b93a7"),
        "cursor:pointer",
        "font-size:.8rem",
        "font-family:inherit",
      ].join(";")
      btn.addEventListener("click", () => {
        selected = f
        renderButtons()
        renderList()
      })
      filterBar.append(btn)
    }
  }

  renderButtons()
  renderList()
}
