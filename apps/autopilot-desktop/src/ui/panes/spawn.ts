// CL-57: "Spawn" pane — spawn a bounded session directly. STUB — implemented in
// CL-57. Owns this file only.
//
// Implementation notes for CL-57:
// - Form: adapter (codex | claude_agent), objective, optional verify[]; validate
//   with `validateSpawnRequest` from "@openagentsinc/autopilot-control-protocol".
// - Submit via `ctx.request.spawnSession({ adapter, objective, verify })`; on
//   success navigate to the new session's detail; surface refusals honestly.

import type { PaneContext } from "../context"
import { emptyLine } from "../dom"

export function renderSpawnPane(container: HTMLElement, ctx: PaneContext): void {
  void ctx
  const h = document.createElement("h1")
  h.className = "pane-title"
  h.textContent = "Spawn"
  container.append(h)
  container.append(emptyLine("Spawn a bounded session — coming in CL-57."))
}
