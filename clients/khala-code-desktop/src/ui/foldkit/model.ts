import { Schema as S } from "effect"

export const FleetCockpitControlVerb = S.Literals([
  "pause",
  "resume",
  "drain",
  "stop",
])
export type FleetCockpitControlVerb = typeof FleetCockpitControlVerb.Type

export const KhalaCodeFleetCockpitSnapshot = S.Struct({
  activeAssignments: S.Number,
  activeRunActual: S.NullOr(S.Number),
  activeRunRef: S.NullOr(S.String),
  activeRunRemaining: S.NullOr(S.Number),
  activeRunState: S.NullOr(S.String),
  activeRunTarget: S.NullOr(S.Number),
  freeSlots: S.NullOr(S.Number),
  inFlightLabel: S.NullOr(S.String),
  maxSlots: S.NullOr(S.Number),
  observedAt: S.String,
  pylonLabel: S.String,
  pylonStatus: S.String,
  readyAccounts: S.Number,
  tokenRateLabel: S.String,
  totalAccounts: S.Number,
})
export type KhalaCodeFleetCockpitSnapshot =
  typeof KhalaCodeFleetCockpitSnapshot.Type

export const KhalaCodeFleetCockpitModel = S.Struct({
  activeAssignments: S.Number,
  activeRunActual: S.NullOr(S.Number),
  activeRunRef: S.NullOr(S.String),
  activeRunRemaining: S.NullOr(S.Number),
  activeRunState: S.NullOr(S.String),
  activeRunTarget: S.NullOr(S.Number),
  connectBusy: S.Boolean,
  controlInFlight: S.NullOr(FleetCockpitControlVerb),
  error: S.NullOr(S.String),
  freeSlots: S.NullOr(S.Number),
  inFlightLabel: S.NullOr(S.String),
  maxSlots: S.NullOr(S.Number),
  mountId: S.String,
  observedAt: S.NullOr(S.String),
  phase: S.Literals(["loading", "ready", "error"]),
  pylonLabel: S.String,
  pylonStatus: S.String,
  readyAccounts: S.Number,
  refreshBusy: S.Boolean,
  tokenRateLabel: S.String,
  totalAccounts: S.Number,
})
export type KhalaCodeFleetCockpitModel =
  typeof KhalaCodeFleetCockpitModel.Type

export const initialKhalaCodeFleetCockpitModel = (
  mountId: string,
): KhalaCodeFleetCockpitModel => ({
  activeAssignments: 0,
  activeRunActual: null,
  activeRunRef: null,
  activeRunRemaining: null,
  activeRunState: null,
  activeRunTarget: null,
  connectBusy: false,
  controlInFlight: null,
  error: null,
  freeSlots: null,
  inFlightLabel: null,
  maxSlots: null,
  mountId,
  observedAt: null,
  phase: "loading",
  pylonLabel: "local Pylon",
  pylonStatus: "loading",
  readyAccounts: 0,
  refreshBusy: false,
  tokenRateLabel: "pending",
  totalAccounts: 0,
})
