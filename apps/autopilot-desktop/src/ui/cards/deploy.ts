// CL-26 "Deploy to Cloud" card. Triggers a deploy of the node's own Cloud Run
// service (cloudrun · main · production) through our pipeline. The node
// fail-safe-gates execution behind OA_DEPLOY_ENABLE=1, so a tap with the gate
// unset comes back disabled and nothing deploys.

import type { PaneContext } from "../context"

export function renderDeployCard(container: HTMLElement, ctx: PaneContext): void {
  const deploy = ctx.node?.deploy ?? null

  const section = document.createElement("section")
  section.id = "deploy"
  section.className = "card"
  const h = document.createElement("h2")
  h.textContent = "Deploy to Cloud"
  section.append(h)

  const help = document.createElement("p")
  help.className = "deploy-help"
  help.textContent =
    "Deploy this node's Cloud Run service (cloudrun · main · production) through our pipeline. Disabled unless the node has OA_DEPLOY_ENABLE=1."
  section.append(help)

  const btn = document.createElement("button")
  btn.textContent = "Deploy to Cloud"

  const status = document.createElement("p")
  const setStatus = (state: "queued" | "building" | "deployed" | "failed" | "unknown", text: string) => {
    status.className = `deploy-status deploy-${state}`
    status.textContent = text
  }
  setStatus(deploy?.state ?? "unknown", deploy ? `${deploy.state} · ${deploy.message}` : "no deploy yet")

  btn.addEventListener("click", () => {
    btn.disabled = true
    setStatus("queued", "deploying…")
    void ctx.request
      .deployCloud({ target: "cloudrun", ref: "main", env: "production" })
      .then((r) => {
        if (r.accepted) setStatus("queued", "queued · cloudrun · main")
        else if (r.reason === "deploy_disabled") setStatus("unknown", "disabled (set OA_DEPLOY_ENABLE=1 on the node)")
        else setStatus("failed", `not accepted: ${r.errors[0] ?? r.reason}`)
      })
      .catch((e: unknown) => setStatus("failed", `error: ${e instanceof Error ? e.message : String(e)}`))
      .finally(() => {
        btn.disabled = false
      })
  })

  section.append(btn, status)
  container.append(section)
}
