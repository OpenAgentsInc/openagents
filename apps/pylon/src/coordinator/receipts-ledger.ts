export type ReceiptLedgerInput = {
  schema: string
  observedAt: string
  [key: string]: unknown
}

export type ReceiptLedgerEntry = ReceiptLedgerInput

export type ReceiptsLedger = {
  total: number
  bySchema: Record<string, number>
  latest: { schema: string; observedAt: string } | null
  sorted: ReceiptLedgerEntry[]
}

export function indexReceipts(receipts: ReceiptLedgerInput[]): ReceiptsLedger {
  const validReceipts = receipts.filter(isIndexableReceipt)
  const sorted = validReceipts
    .map((receipt) => ({ ...receipt }))
    .sort(compareReceiptsByObservedAtDesc)
  const latest = sorted[0]

  return {
    total: sorted.length,
    bySchema: countBySchema(sorted),
    latest: latest
      ? {
          schema: latest.schema,
          observedAt: latest.observedAt,
        }
      : null,
    sorted,
  }
}

function isIndexableReceipt(receipt: ReceiptLedgerInput): boolean {
  return isNonEmptyString(receipt.schema) && isNonEmptyString(receipt.observedAt)
}

function compareReceiptsByObservedAtDesc(
  left: ReceiptLedgerInput,
  right: ReceiptLedgerInput,
): number {
  return compareObservedAtDesc(left.observedAt, right.observedAt)
}

function compareObservedAtDesc(left: string, right: string): number {
  const leftTime = Date.parse(left)
  const rightTime = Date.parse(right)

  if (Number.isFinite(leftTime) && Number.isFinite(rightTime)) {
    return rightTime - leftTime
  }

  if (Number.isFinite(leftTime)) {
    return -1
  }

  if (Number.isFinite(rightTime)) {
    return 1
  }

  return right.localeCompare(left)
}

function countBySchema(
  receipts: ReadonlyArray<ReceiptLedgerInput>,
): Record<string, number> {
  return receipts.reduce<Record<string, number>>((counts, receipt) => {
    counts[receipt.schema] = (counts[receipt.schema] ?? 0) + 1
    return counts
  }, {})
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0
}
