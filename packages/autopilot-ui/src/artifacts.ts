import { stylexAttrs } from "@openagentsinc/ui/stylex-foldkit"
import type { Html } from "foldkit/html"
import { html } from "foldkit/html"
import { domainStyles } from "./domain-styles.js"
import type { AutopilotUiMessage, ChipTone } from "./view.js"
import { statusChip } from "./view.js"

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

const h = html<AutopilotUiMessage>()

const truncateDigestRef = (digestRef: string): string => {
  if (digestRef.length <= 32) return digestRef

  return `${digestRef.slice(0, 18)}...${digestRef.slice(-10)}`
}

const receiptStatusTone = (status: Receipt["status"]): ChipTone => {
  switch (status) {
    case "ok":
      return "success"
    case "pending":
      return "warning"
    case "failed":
      return "danger"
  }
}

export const ArtifactList = (input: { artifacts: ReadonlyArray<Artifact> }): Html =>
  h.section(
    [
      ...stylexAttrs<AutopilotUiMessage>(domainStyles.list),
      h.DataAttribute("autopilot-artifact-list", ""),
    ],
    input.artifacts.length === 0
      ? [
          h.p(stylexAttrs<AutopilotUiMessage>(domainStyles.empty), [
            "No artifacts",
          ]),
        ]
      : input.artifacts.map((artifact) =>
          h.article(
            [
              ...stylexAttrs<AutopilotUiMessage>(domainStyles.artifactRow),
              h.DataAttribute("autopilot-artifact-ref", artifact.digestRef),
            ],
            [
              h.span(stylexAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [
                artifact.name,
              ]),
              h.code(
                [
                  ...stylexAttrs<AutopilotUiMessage>(domainStyles.codeMuted),
                  h.Title(artifact.digestRef),
                ],
                [truncateDigestRef(artifact.digestRef)],
              ),
              statusChip({
                label: `${artifact.contentType ?? "content-type: unknown"} / size: ref-only`,
                attrs: [h.DataAttribute("autopilot-artifact-content-type", artifact.contentType ?? "unknown")],
              }),
            ],
          ),
        ),
  )

export const ReceiptList = (input: { receipts: ReadonlyArray<Receipt> }): Html =>
  h.section(
    [
      ...stylexAttrs<AutopilotUiMessage>(domainStyles.list),
      h.DataAttribute("autopilot-receipt-list", ""),
    ],
    input.receipts.length === 0
      ? [
          h.p(stylexAttrs<AutopilotUiMessage>(domainStyles.empty), [
            "No receipts",
          ]),
        ]
      : input.receipts.map((receipt) =>
          h.article(
            [
              ...stylexAttrs<AutopilotUiMessage>(domainStyles.receiptRow),
              h.DataAttribute("autopilot-receipt-ref", receipt.digestRef),
            ],
            [
              h.span(stylexAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [receipt.kind]),
              h.code(
                [
                  ...stylexAttrs<AutopilotUiMessage>(domainStyles.codeMuted),
                  h.Title(receipt.digestRef),
                ],
                [truncateDigestRef(receipt.digestRef)],
              ),
              statusChip({
                label: receipt.status,
                tone: receiptStatusTone(receipt.status),
                attrs: [h.DataAttribute("autopilot-receipt-status", receipt.status)],
              }),
            ],
          ),
        ),
  )
