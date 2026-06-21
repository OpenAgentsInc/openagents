import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
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

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

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
    [className("grid gap-2"), h.DataAttribute("autopilot-artifact-list", "")],
    input.artifacts.length === 0
      ? [
          h.p([className("m-0 text-sm text-[var(--text-secondary,#8a8c93)]")], [
            "No artifacts",
          ]),
        ]
      : input.artifacts.map((artifact) =>
          h.article(
            [
              className(
                "grid gap-2 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-3 text-[var(--text,#d7d8e5)] sm:grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)_12rem] sm:items-center",
              ),
              h.DataAttribute("autopilot-artifact-ref", artifact.digestRef),
            ],
            [
              h.span([className("min-w-0 truncate font-mono text-sm text-[var(--primary,#fff)]")], [
                artifact.name,
              ]),
              h.code(
                [
                  className("min-w-0 truncate font-mono text-xs text-[var(--text-secondary,#8a8c93)]"),
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
    [className("grid gap-2"), h.DataAttribute("autopilot-receipt-list", "")],
    input.receipts.length === 0
      ? [
          h.p([className("m-0 text-sm text-[var(--text-secondary,#8a8c93)]")], [
            "No receipts",
          ]),
        ]
      : input.receipts.map((receipt) =>
          h.article(
            [
              className(
                "grid gap-2 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-3 text-[var(--text,#d7d8e5)] sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_7rem] sm:items-center",
              ),
              h.DataAttribute("autopilot-receipt-ref", receipt.digestRef),
            ],
            [
              h.span([className("font-mono text-sm text-[var(--primary,#fff)]")], [receipt.kind]),
              h.code(
                [
                  className("min-w-0 truncate font-mono text-xs text-[var(--text-secondary,#8a8c93)]"),
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
