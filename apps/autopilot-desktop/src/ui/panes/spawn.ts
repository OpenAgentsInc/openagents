// CL-57: "Spawn" pane — spawn a bounded session directly (distinct from "Ask
// Autopilot"). Renders a form with adapter selector, objective textarea, and
// optional verify field. Validates with the shared validateSpawnRequest from
// @openagentsinc/autopilot-control-protocol before calling ctx.request.spawnSession.

import { validateSpawnRequest } from "@openagentsinc/autopilot-control-protocol"
import type { PaneContext } from "../context"
import { card } from "../dom"

// Pure helper: split a multiline text block into trimmed, non-empty lines.
// Used to convert the verify textarea value into a string[] for the spawn request.
export function parseVerifyLines(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0)
}

export function renderSpawnPane(container: HTMLElement, ctx: PaneContext): void {
  const h = document.createElement("h1")
  h.className = "pane-title"
  h.textContent = "Spawn"
  container.append(h)

  const { section, body } = card("New Session")
  container.append(section)

  // Adapter selector
  const adapterLabel = document.createElement("label")
  adapterLabel.textContent = "Adapter"
  adapterLabel.style.cssText = "display:block;color:#8a8c93;font-size:12px;margin-bottom:6px;text-transform:uppercase;letter-spacing:0.05em;"
  body.append(adapterLabel)

  const adapterSelect = document.createElement("select")
  adapterSelect.style.cssText = [
    "background:#151515",
    "border:1px solid #525458",
    "border-radius:6px",
    "color:#d7d8e5",
    "display:block",
    "font-size:13px",
    "margin-bottom:16px",
    "padding:10px 12px",
    "width:100%",
  ].join(";")

  const codexOpt = document.createElement("option")
  codexOpt.value = "codex"
  codexOpt.textContent = "codex"
  const claudeOpt = document.createElement("option")
  claudeOpt.value = "claude_agent"
  claudeOpt.textContent = "claude_agent"
  adapterSelect.append(codexOpt, claudeOpt)
  body.append(adapterSelect)

  // Objective textarea
  const objectiveLabel = document.createElement("label")
  objectiveLabel.textContent = "Objective"
  objectiveLabel.style.cssText = "display:block;color:#8a8c93;font-size:12px;margin-bottom:6px;text-transform:uppercase;letter-spacing:0.05em;"
  body.append(objectiveLabel)

  const objectiveArea = document.createElement("textarea")
  objectiveArea.rows = 5
  objectiveArea.placeholder = "Describe the session objective…"
  objectiveArea.style.cssText = [
    "background:#151515",
    "border:1px solid #525458",
    "border-radius:6px",
    "box-sizing:border-box",
    "color:#d7d8e5",
    "display:block",
    "font-family:inherit",
    "font-size:13px",
    "line-height:1.5",
    "margin-bottom:16px",
    "padding:10px 12px",
    "resize:vertical",
    "width:100%",
  ].join(";")
  body.append(objectiveArea)

  // Verify field (optional, one command per line)
  const verifyLabel = document.createElement("label")
  verifyLabel.textContent = "Verify commands (optional — one per line)"
  verifyLabel.style.cssText = "display:block;color:#8a8c93;font-size:12px;margin-bottom:6px;text-transform:uppercase;letter-spacing:0.05em;"
  body.append(verifyLabel)

  const verifyArea = document.createElement("textarea")
  verifyArea.rows = 3
  verifyArea.placeholder = "bun test\nbun run typecheck"
  verifyArea.style.cssText = [
    "background:#151515",
    "border:1px solid #525458",
    "border-radius:6px",
    "box-sizing:border-box",
    "color:#d7d8e5",
    "display:block",
    "font-family:Courier, monospace",
    "font-size:12px",
    "line-height:1.5",
    "margin-bottom:16px",
    "padding:10px 12px",
    "resize:vertical",
    "width:100%",
  ].join(";")
  body.append(verifyArea)

  // Error/status line
  const statusLine = document.createElement("p")
  statusLine.style.cssText = "color:#d32f2f;font-size:13px;margin:0 0 12px;min-height:18px;"
  body.append(statusLine)

  // Submit button
  const submitBtn = document.createElement("button")
  submitBtn.textContent = "Spawn session"
  submitBtn.style.cssText = [
    "background:#fff",
    "border:none",
    "border-radius:6px",
    "color:#000",
    "cursor:pointer",
    "font-size:14px",
    "font-weight:700",
    "padding:12px 20px",
    "width:100%",
  ].join(";")
  body.append(submitBtn)

  submitBtn.addEventListener("click", () => {
    statusLine.style.color = "#d32f2f"
    statusLine.textContent = ""

    const adapter = adapterSelect.value as "codex" | "claude_agent"
    const objective = objectiveArea.value
    const verify = parseVerifyLines(verifyArea.value)

    const v = validateSpawnRequest({ adapter, objective })
    if (!v.ok) {
      statusLine.textContent = v.errors[0] ?? "invalid request"
      return
    }

    submitBtn.disabled = true
    submitBtn.textContent = "Spawning…"
    statusLine.style.color = "#8a8c93"
    statusLine.textContent = "sending…"

    void ctx.request
      .spawnSession({
        adapter: v.adapter as "codex" | "claude_agent",
        objective: v.objective,
        verify: verify.length > 0 ? verify : undefined,
      })
      .then((result) => {
        if (result.ok) {
          ctx.navigate("session-detail", result.sessionRef)
        } else {
          statusLine.style.color = "#d32f2f"
          statusLine.textContent = result.error ?? "spawn failed"
          submitBtn.disabled = false
          submitBtn.textContent = "Spawn session"
        }
      })
      .catch((e: unknown) => {
        statusLine.style.color = "#d32f2f"
        statusLine.textContent = e instanceof Error ? e.message : "spawn failed"
        submitBtn.disabled = false
        submitBtn.textContent = "Spawn session"
      })
  })
}
