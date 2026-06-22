import { stylexAttrs } from "@openagentsinc/ui/stylex-foldkit"
import type { Html } from "foldkit/html"
import { html } from "foldkit/html"
import { domainStyles } from "./domain-styles.js"
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
      ...stylexAttrs<AutopilotUiMessage>(domainStyles.panel),
      h.DataAttribute("autopilot-cloud-quota-panel", ""),
    ],
    [
      h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.header), [
        h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.stack), [
          h.span(stylexAttrs<AutopilotUiMessage>(domainStyles.label), [
            "Credit balance",
          ]),
          h.code(
            [
              ...stylexAttrs<AutopilotUiMessage>(domainStyles.value),
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
          ...stylexAttrs<AutopilotUiMessage>(domainStyles.subPanel),
          h.DataAttribute("autopilot-cloud-compute-ref", input.compute.usedRef),
        ],
        [
          h.span(stylexAttrs<AutopilotUiMessage>(domainStyles.label), [
            input.compute.meterLabel,
          ]),
          h.code(stylexAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [
            input.compute.usedRef,
          ]),
        ],
      ),
    ],
  )
