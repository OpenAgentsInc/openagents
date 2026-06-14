// CL-49: "Balance" — read-only MDK wallet status. STUB — implemented in CL-49.
// Owns this file only.
//
// Implementation notes for CL-49:
// - Read `ctx.node?.wallet`; show balanceSats.toLocaleString() + " sats" (or
//   "—"), and a summary: online/offline · readiness · "receive ✓" when ready.
// - Read-only projection; no wallet keys in the webview.

import type { PaneContext } from "../context"
import { card, emptyLine } from "../dom"

export function renderBalanceCard(container: HTMLElement, ctx: PaneContext): void {
  const wallet = ctx.node?.wallet ?? null
  if (!wallet) return // no wallet on the node — render nothing
  const { section, body } = card("Balance")
  body.append(emptyLine("Balance UI — coming in CL-49."))
  container.append(section)
}
