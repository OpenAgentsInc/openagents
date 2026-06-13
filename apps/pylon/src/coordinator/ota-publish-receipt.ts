export const OTA_PUBLISH_RECEIPT_SCHEMA = "openagents.pylon.ota_publish_receipt.v1"

export type OtaPublishReceiptInput = {
  runtimeVersion: string
  branch: string
  updateGroupId: string
  assetCount: number
  publishedAt: string
  originIntentRef?: string
}

export type OtaPublishReceipt = {
  schema: string
  runtimeVersion: string
  branch: string
  updateGroupId: string
  assetCount: number
  publishedAt: string
  originIntentRef: string | null
}

const RECEIPT_KEYS = [
  "schema",
  "runtimeVersion",
  "branch",
  "updateGroupId",
  "assetCount",
  "publishedAt",
  "originIntentRef",
].sort()

export function buildOtaPublishReceipt(input: OtaPublishReceiptInput): OtaPublishReceipt {
  return {
    schema: OTA_PUBLISH_RECEIPT_SCHEMA,
    runtimeVersion: input.runtimeVersion,
    branch: input.branch,
    updateGroupId: input.updateGroupId,
    assetCount: input.assetCount,
    publishedAt: input.publishedAt,
    originIntentRef: input.originIntentRef ?? null,
  }
}

export function validateOtaPublishReceipt(receipt: unknown): receipt is OtaPublishReceipt {
  if (receipt === null || typeof receipt !== "object" || Array.isArray(receipt)) {
    return false
  }

  const keys = Object.keys(receipt).sort()
  if (keys.length !== RECEIPT_KEYS.length || keys.some((key, index) => key !== RECEIPT_KEYS[index])) {
    return false
  }

  const record = receipt as Record<string, unknown>
  return (
    record.schema === OTA_PUBLISH_RECEIPT_SCHEMA &&
    typeof record.runtimeVersion === "string" &&
    typeof record.branch === "string" &&
    typeof record.updateGroupId === "string" &&
    Number.isInteger(record.assetCount) &&
    (record.assetCount as number) >= 0 &&
    typeof record.publishedAt === "string" &&
    (typeof record.originIntentRef === "string" || record.originIntentRef === null)
  )
}
