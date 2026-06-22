import { stylexAttrs } from "@openagentsinc/ui/stylex-foldkit"
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
      ...stylexAttrs<AutopilotUiMessage>(domainStyles.panel),
      h.DataAttribute("autopilot-earnings-panel", ""),
    ],
    [
      h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.header), [
        h.div(stylexAttrs<AutopilotUiMessage>(domainStyles.stackSmall), [
          h.h2(stylexAttrs<AutopilotUiMessage>(domainStyles.title), [
            "Earnings",
          ]),
          h.p(stylexAttrs<AutopilotUiMessage>(domainStyles.muted), [
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
          ...stylexAttrs<AutopilotUiMessage>(domainStyles.list),
          h.DataAttribute("autopilot-earnings-entries", ""),
        ],
        input.entries.length === 0
          ? [
              h.li(stylexAttrs<AutopilotUiMessage>(domainStyles.empty), [
                "No earnings yet",
              ]),
            ]
          : input.entries.map((entry) =>
              h.li(
                [
                  ...stylexAttrs<AutopilotUiMessage>(domainStyles.earningsRow),
                  h.DataAttribute("autopilot-earnings-ref", entry.ref),
                ],
                [
                  h.code(stylexAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [
                    entry.ref,
                  ]),
                  h.span(stylexAttrs<AutopilotUiMessage>(domainStyles.successValue), [
                    satsLabel(entry.amountSats),
                  ]),
                  h.time(stylexAttrs<AutopilotUiMessage>(domainStyles.muted), [
                    entry.at,
                  ]),
                ],
              ),
            ),
      ),
    ],
  )
