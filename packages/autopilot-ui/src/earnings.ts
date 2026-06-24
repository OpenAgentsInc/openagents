import { classAttrs } from "@openagentsinc/ui/class-foldkit"
import type { Html } from "foldkit/html"
import { html } from "foldkit/html"
import { domainStyles } from "./domain-styles.js"
import type { AutopilotUiMessage } from "./view.js"
import { statusChip } from "./view.js"

export type EarningsSummary = Readonly<{
  balanceSats: number
  entries: ReadonlyArray<{
    ref: string
    amountSats: number
    at: string
  }>
}>

const h = html<AutopilotUiMessage>()

const satsLabel = (amountSats: number): string => `${amountSats} sats`

export const EarningsPanel = (input: EarningsSummary): Html =>
  h.section(
    [
      ...classAttrs<AutopilotUiMessage>(domainStyles.panel),
      h.DataAttribute("autopilot-earnings-panel", ""),
    ],
    [
      h.div(classAttrs<AutopilotUiMessage>(domainStyles.header), [
        h.div(classAttrs<AutopilotUiMessage>(domainStyles.stackSmall), [
          h.h2(classAttrs<AutopilotUiMessage>(domainStyles.title), [
            "Earnings",
          ]),
          h.p(classAttrs<AutopilotUiMessage>(domainStyles.muted), [
            "Read-only balance",
          ]),
        ]),
        statusChip({
          label: satsLabel(input.balanceSats),
          tone: "success",
          attrs: [h.DataAttribute("autopilot-earnings-balance", String(input.balanceSats))],
        }),
      ]),
      h.ol(
        [
          ...classAttrs<AutopilotUiMessage>(domainStyles.list),
          h.DataAttribute("autopilot-earnings-entries", ""),
        ],
        input.entries.length === 0
          ? [
              h.li(classAttrs<AutopilotUiMessage>(domainStyles.empty), [
                "No earnings yet",
              ]),
            ]
          : input.entries.map((entry) =>
              h.li(
                [
                  ...classAttrs<AutopilotUiMessage>(domainStyles.earningsRow),
                  h.DataAttribute("autopilot-earnings-ref", entry.ref),
                ],
                [
                  h.code(classAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [
                    entry.ref,
                  ]),
                  h.span(classAttrs<AutopilotUiMessage>(domainStyles.successValue), [
                    satsLabel(entry.amountSats),
                  ]),
                  h.time(classAttrs<AutopilotUiMessage>(domainStyles.muted), [
                    entry.at,
                  ]),
                ],
              ),
            ),
      ),
    ],
  )
