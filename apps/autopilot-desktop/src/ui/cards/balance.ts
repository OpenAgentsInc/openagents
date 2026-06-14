// CL-49: "Balance" — read-only MDK wallet status card.
// Renders a `.card` titled "Balance" with a large sat value and a summary
// line (online/offline · readiness · receive ✓). Read-only projection only;
// no wallet keys are exposed in the webview.

import type { PaneContext } from "../context"
import type { WalletStatusRow } from "../../shared/rpc"
import { card } from "../dom"

// Pure helper: derive the display strings from a WalletStatusRow.
// Exported for unit tests.
export function walletSummary(wallet: WalletStatusRow): { value: string; summary: string } {
  const value =
    typeof wallet.balanceSats === "number" ? wallet.balanceSats.toLocaleString() + " sats" : "—"

  const onlineLabel = wallet.daemonOnline ? "wallet online" : "wallet offline"
  let summary = `${onlineLabel} · ${wallet.readiness}`
  if (wallet.receiveReady) summary += " · receive ✓"

  return { value, summary }
}

export function renderBalanceCard(container: HTMLElement, ctx: PaneContext): void {
  const wallet = ctx.node?.wallet ?? null
  if (!wallet) return // no wallet on the node — render nothing

  const { section, body } = card("Balance")

  const { value, summary } = walletSummary(wallet)

  const valueLine = document.createElement("p")
  valueLine.textContent = value
  valueLine.style.fontSize = "1.4rem"
  valueLine.style.fontWeight = "600"
  valueLine.style.margin = "0 0 0.25rem"
  body.append(valueLine)

  const summaryLine = document.createElement("p")
  summaryLine.textContent = summary
  summaryLine.style.fontSize = "0.85rem"
  summaryLine.style.opacity = "0.7"
  summaryLine.style.margin = "0"
  body.append(summaryLine)

  container.append(section)
}
