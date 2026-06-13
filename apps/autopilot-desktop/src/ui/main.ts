// Boot shell for Autopilot Desktop. Plain DOM hello-world; the Foldkit/Effect
// runtime + live Pylon control client (over loopback) land in CL-5.
// The view script loads in <head>, so render after the DOM is ready.
import { CONTROL_SCHEMA_TAG } from "@openagentsinc/autopilot-control-protocol"

function render(): void {
  const root = document.createElement("main")
  const h = document.createElement("h1")
  h.textContent = "🛩️  Autopilot Desktop"
  root.append(h)
  const a = document.createElement("p")
  a.textContent = "Electrobun + Bun + (Foldkit next) — the desktop client shell is alive."
  root.append(a)
  const b = document.createElement("p")
  b.innerHTML = `shared protocol: <code>${CONTROL_SCHEMA_TAG}</code>`
  root.append(b)
  const c = document.createElement("p")
  c.textContent = "Next (CL-5): connect to the local Pylon node over loopback and render live sessions."
  root.append(c)
  const sessions = document.createElement("section")
  sessions.id = "sessions"
  // TODO: Bun main will push live Pylon node state into this placeholder via RPC.
  root.append(sessions)
  document.body.append(root)
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", render)
} else {
  render()
}
