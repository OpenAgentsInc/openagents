import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
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

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const satsLabel = (amountSats: number): string => `${amountSats} sats`

export const EarningsPanel = (input: EarningsSummary): Html =>
  h.section(
    [
      className(
        "grid gap-4 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-4 text-[var(--text,#d7d8e5)]",
      ),
      h.DataAttribute("autopilot-earnings-panel", ""),
    ],
    [
      h.div([className("flex flex-wrap items-start justify-between gap-3")], [
        h.div([className("grid gap-1")], [
          h.h2([className("m-0 font-mono text-sm font-bold text-[var(--primary,#fff)]")], [
            "Earnings",
          ]),
          h.p([className("m-0 font-mono text-xs text-[var(--text-secondary,#8a8c93)]")], [
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
        [className("grid gap-2"), h.DataAttribute("autopilot-earnings-entries", "")],
        input.entries.length === 0
          ? [
              h.li([className("text-sm text-[var(--text-secondary,#8a8c93)]")], [
                "No earnings yet",
              ]),
            ]
          : input.entries.map((entry) =>
              h.li(
                [
                  className(
                    "grid gap-2 border border-[var(--outline,#525458)] bg-[var(--bg,#0d0d0d)] p-3 sm:grid-cols-[minmax(0,1fr)_8rem_12rem] sm:items-center",
                  ),
                  h.DataAttribute("autopilot-earnings-ref", entry.ref),
                ],
                [
                  h.code([className("min-w-0 truncate font-mono text-xs text-[var(--primary,#fff)]")], [
                    entry.ref,
                  ]),
                  h.span([className("font-mono text-xs font-bold text-[#86efac]")], [
                    satsLabel(entry.amountSats),
                  ]),
                  h.time([className("font-mono text-xs text-[var(--text-secondary,#8a8c93)]")], [
                    entry.at,
                  ]),
                ],
              ),
            ),
      ),
    ],
  )
