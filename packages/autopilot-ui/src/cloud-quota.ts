import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { AutopilotUiMessage, ChipTone } from "./view.js"
import { statusChip } from "./view.js"

export type CloudQuotaPanelInput = Readonly<{
  creditBalance: number
  compute: Readonly<{
    usedRef: string
    meterLabel: string
  }>
  failover?: Readonly<{
    active: boolean
    reasonRef?: string
  }>
}>

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const failoverTone = (failover: CloudQuotaPanelInput["failover"]): ChipTone => {
  if (failover?.active !== true) return "neutral"

  return failover.reasonRef === undefined ? "warning" : "danger"
}

const failoverLabel = (failover: CloudQuotaPanelInput["failover"]): string => {
  if (failover?.active !== true) return "failover: inactive"
  if (failover.reasonRef === undefined) return "failover: active"

  return `failover: ${failover.reasonRef}`
}

export const CloudQuotaPanel = (input: CloudQuotaPanelInput): Html =>
  h.section(
    [
      className(
        "grid gap-3 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-4 text-[var(--text,#d7d8e5)]",
      ),
      h.DataAttribute("autopilot-cloud-quota-panel", ""),
    ],
    [
      h.div([className("flex flex-wrap items-start justify-between gap-2")], [
        h.div([className("grid min-w-0 gap-1")], [
          h.span([className("font-mono text-xs uppercase text-[var(--text-secondary,#8a8c93)]")], [
            "Credit balance",
          ]),
          h.code(
            [
              className("min-w-0 truncate font-mono text-lg font-bold text-[var(--primary,#fff)]"),
              h.DataAttribute("autopilot-cloud-credit-balance", String(input.creditBalance)),
            ],
            [`${input.creditBalance} credits`],
          ),
        ]),
        statusChip({
          label: failoverLabel(input.failover),
          tone: failoverTone(input.failover),
          attrs: [
            h.DataAttribute(
              "autopilot-cloud-failover",
              input.failover?.active === true ? "active" : "inactive",
            ),
          ],
        }),
      ]),
      h.div(
        [
          className(
            "grid gap-2 border border-[var(--outline,#525458)] bg-transparent p-3 sm:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)] sm:items-center",
          ),
          h.DataAttribute("autopilot-cloud-compute-ref", input.compute.usedRef),
        ],
        [
          h.span([className("font-mono text-xs uppercase text-[var(--text-secondary,#8a8c93)]")], [
            input.compute.meterLabel,
          ]),
          h.code([className("min-w-0 truncate font-mono text-sm text-[var(--primary,#fff)]")], [
            input.compute.usedRef,
          ]),
        ],
      ),
    ],
  )
