import type { Tone } from "./verify-view-model"

export type Artifact = Readonly<{
  name: string
  digestRef: string
  contentType?: string
}>

export type Receipt = Readonly<{
  kind: string
  digestRef: string
  status: "ok" | "pending" | "failed"
}>

export type ArtifactRowViewModel = {
  name: string
  digestRef: string
  displayDigestRef: string
  contentType: string
  contentTypeLabel: string
}

export type ReceiptRowViewModel = {
  kind: string
  digestRef: string
  displayDigestRef: string
  status: Receipt["status"]
  statusTone: Extract<Tone, "success" | "warning" | "danger">
}

const truncateDigestRef = (digestRef: string): string => {
  if (digestRef.length <= 32) return digestRef

  return `${digestRef.slice(0, 18)}...${digestRef.slice(-10)}`
}

const receiptStatusTone = (status: Receipt["status"]): ReceiptRowViewModel["statusTone"] => {
  switch (status) {
    case "ok":
      return "success"
    case "pending":
      return "warning"
    case "failed":
      return "danger"
  }
}

export function artifactRowsViewModel(artifacts: readonly Artifact[]): ArtifactRowViewModel[] {
  return artifacts.map((artifact) => {
    const contentType = artifact.contentType ?? "unknown"

    return {
      name: artifact.name,
      digestRef: artifact.digestRef,
      displayDigestRef: truncateDigestRef(artifact.digestRef),
      contentType,
      contentTypeLabel: `${artifact.contentType ?? "content-type: unknown"} / size: ref-only`,
    }
  })
}

export function receiptRowsViewModel(receipts: readonly Receipt[]): ReceiptRowViewModel[] {
  return receipts.map((receipt) => ({
    kind: receipt.kind,
    digestRef: receipt.digestRef,
    displayDigestRef: truncateDigestRef(receipt.digestRef),
    status: receipt.status,
    statusTone: receiptStatusTone(receipt.status),
  }))
}
