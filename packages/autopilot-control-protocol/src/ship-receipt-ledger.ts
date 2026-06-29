import type { AutonomousShipReceipt } from "./autonomous-ship-receipt.js"
import { validateShipReceipt } from "./autonomous-ship-receipt.js"

export type ShipReceiptLedger = {
  append(receipt: unknown): { accepted: boolean }
  list(): AutonomousShipReceipt[]
  summary(): { count: number, allowedCount: number, deniedCount: number }
}

export function createShipReceiptLedger(): ShipReceiptLedger {
  const receipts: AutonomousShipReceipt[] = []

  return {
    append(receipt: unknown): { accepted: boolean } {
      if (!validateShipReceipt(receipt)) return { accepted: false }

      receipts.push({ ...(receipt as AutonomousShipReceipt) })
      return { accepted: true }
    },

    list(): AutonomousShipReceipt[] {
      return receipts.map((receipt) => ({ ...receipt }))
    },

    summary(): { count: number, allowedCount: number, deniedCount: number } {
      let allowedCount = 0

      for (const receipt of receipts) {
        if (receipt.allowed) allowedCount += 1
      }

      return {
        count: receipts.length,
        allowedCount,
        deniedCount: receipts.length - allowedCount,
      }
    },
  }
}
