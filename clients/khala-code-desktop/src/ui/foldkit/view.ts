import { iconView, type IconName } from "@openagentsinc/ui/icon"
import type { Document } from "foldkit/html"
import { html } from "foldkit/html"

import {
  FleetCockpitClickedConnectAccount,
  FleetCockpitClickedRefresh,
  FleetCockpitClickedRunControl,
} from "./message.js"
import type { KhalaCodeFleetCockpitMessage } from "./message.js"
import type {
  FleetCockpitControlVerb,
  KhalaCodeFleetCockpitModel,
} from "./model.js"

const titleize = (value: string): string =>
  value.replace(/[_-]+/g, " ").replace(/\b\w/g, char => char.toUpperCase())

const valueLabel = (value: number | string | null): string =>
  value === null ? "pending" : String(value)

const slotsLabel = (model: KhalaCodeFleetCockpitModel): string => {
  if (model.freeSlots === null || model.maxSlots === null) return "pending"
  return `${model.freeSlots}/${model.maxSlots}`
}

const activeRunLabel = (model: KhalaCodeFleetCockpitModel): string =>
  model.activeRunRef === null
    ? "none"
    : `${model.activeRunRef} · ${titleize(model.activeRunState ?? "unknown")}`

const buttonLabel = (
  label: string,
  busyLabel: string,
  busy: boolean,
): string => busy ? busyLabel : label

const controlIcon: Record<FleetCockpitControlVerb, IconName> = {
  drain: "Circle",
  pause: "Pause",
  resume: "Play",
  stop: "Stop",
}

export const view = (model: KhalaCodeFleetCockpitModel): Document => {
  const h = html<KhalaCodeFleetCockpitMessage>()
  const chip = (label: string, value: string) =>
    h.span([h.Class("khala-fleet-chip")], [
      h.span([h.Class("khala-fleet-chip-label")], [label]),
      h.span([h.Class("khala-fleet-chip-value")], [value]),
    ])
  const control = (verb: FleetCockpitControlVerb) =>
    h.button(
      [
        h.Type("button"),
        h.Class(verb === "stop" ? "khala-fleet-run khala-fleet-run-danger" : "khala-fleet-run"),
        h.Disabled(model.activeRunRef === null || model.controlInFlight !== null),
        h.OnClick(FleetCockpitClickedRunControl({ verb })),
      ],
      [
        iconView<KhalaCodeFleetCockpitMessage>(controlIcon[verb], "khala-fleet-button-icon"),
        h.span([h.Class("khala-fleet-button-label")], [
          model.controlInFlight === verb ? `${titleize(verb)}...` : titleize(verb),
        ]),
      ],
    )

  return {
    title: "Khala Code",
    body: h.section(
      [
        h.Class("khala-fleet-cockpit"),
        h.DataAttribute("foldkit-mount-id", model.mountId),
        h.DataAttribute("state", model.phase),
      ],
      [
        h.header([h.Class("khala-fleet-header")], [
          h.div([h.Class("khala-fleet-cockpit-heading")], [
            h.h2([h.Class("khala-fleet-title")], ["Fleet cockpit"]),
            h.p([h.Class("khala-fleet-cockpit-subtitle")], [
              model.phase === "error"
                ? model.error ?? "Fleet status unavailable."
                : `${model.readyAccounts}/${model.totalAccounts} accounts ready · ${model.activeAssignments} active`,
            ]),
          ]),
          h.div([h.Class("khala-fleet-actions")], [
            h.button(
              [
                h.Type("button"),
                h.Class("khala-fleet-refresh"),
                h.Disabled(model.connectBusy),
                h.OnClick(FleetCockpitClickedConnectAccount()),
              ],
              [
                iconView<KhalaCodeFleetCockpitMessage>("Plus", "khala-fleet-button-icon"),
                h.span([h.Class("khala-fleet-button-label")], [
                  buttonLabel("Connect account", "Connecting", model.connectBusy),
                ]),
              ],
            ),
            h.button(
              [
                h.Type("button"),
                h.Class("khala-fleet-refresh"),
                h.Disabled(model.refreshBusy),
                h.OnClick(FleetCockpitClickedRefresh()),
              ],
              [
                iconView<KhalaCodeFleetCockpitMessage>("Reload", "khala-fleet-button-icon"),
                h.span([h.Class("khala-fleet-button-label")], [
                  buttonLabel("Refresh", "Refreshing", model.refreshBusy),
                ]),
              ],
            ),
          ]),
        ]),
        h.section([h.Class("khala-fleet-cockpit-strip")], [
          chip("pylon", `${model.pylonLabel} · ${titleize(model.pylonStatus)}`),
          chip("slots", slotsLabel(model)),
          chip("tokens", model.tokenRateLabel),
          chip("in flight", model.inFlightLabel ?? "none"),
          chip("run", activeRunLabel(model)),
          chip("target", valueLabel(model.activeRunTarget)),
          chip("actual", valueLabel(model.activeRunActual)),
          chip("remaining", valueLabel(model.activeRunRemaining)),
        ]),
        model.activeRunRef === null
          ? h.p([h.Class("khala-fleet-empty")], ["No active FleetRun."])
          : h.div(
              [
                h.Class("khala-fleet-run-controls khala-fleet-cockpit-controls"),
              ],
              [control("pause"), control("resume"), control("drain"), control("stop")],
            ),
      ],
    ),
  }
}
