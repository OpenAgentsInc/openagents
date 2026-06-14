// CL-44: tiny DOM helpers shared by the shell, panes, and cards. Kept
// dependency-free so every pane/card can import them without coupling.

export function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;")
}

// Render an empty titled card section. Panes/cards fill it via the returned
// body element. Returns { section, body } so callers append into `body`.
export function card(title: string): { section: HTMLElement; body: HTMLElement } {
  const section = document.createElement("section")
  section.className = "card"
  const h = document.createElement("h2")
  h.className = "card-title"
  h.textContent = title
  section.append(h)
  const body = document.createElement("div")
  body.className = "card-body"
  section.append(body)
  return { section, body }
}

// A muted "nothing here yet" line for empty states.
export function emptyLine(text: string): HTMLElement {
  const p = document.createElement("p")
  p.className = "empty-state"
  p.textContent = text
  return p
}
