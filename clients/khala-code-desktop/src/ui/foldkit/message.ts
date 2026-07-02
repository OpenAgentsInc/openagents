import { Schema as S } from "effect"
import { m } from "foldkit/message"

import { FleetCockpitControlVerb } from "./model.js"
import { KhalaCodeFleetCockpitHostPortMessage } from "./ports.js"

export const FleetCockpitClickedRefresh = m("FleetCockpitClickedRefresh")
export const FleetCockpitClickedConnectAccount = m("FleetCockpitClickedConnectAccount")
export const FleetCockpitClickedRunControl = m("FleetCockpitClickedRunControl", {
  verb: FleetCockpitControlVerb,
})
export const FleetCockpitReceivedHostPort = m("FleetCockpitReceivedHostPort", {
  message: KhalaCodeFleetCockpitHostPortMessage,
})
export const FleetCockpitMounted = m("FleetCockpitMounted")
export const FleetCockpitUnmounted = m("FleetCockpitUnmounted")
export const FleetCockpitCompletedPortEmit = m("FleetCockpitCompletedPortEmit")

export const KhalaCodeFleetCockpitMessage = S.Union([
  FleetCockpitClickedRefresh,
  FleetCockpitClickedConnectAccount,
  FleetCockpitClickedRunControl,
  FleetCockpitReceivedHostPort,
  FleetCockpitMounted,
  FleetCockpitUnmounted,
  FleetCockpitCompletedPortEmit,
])
export type KhalaCodeFleetCockpitMessage =
  typeof KhalaCodeFleetCockpitMessage.Type
