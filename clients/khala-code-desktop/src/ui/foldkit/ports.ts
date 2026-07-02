import { Schema as S } from "effect"
import { ts } from "foldkit/schema"

import {
  FleetCockpitControlVerb,
  KhalaCodeFleetCockpitSnapshot,
} from "./model.js"

export const KhalaCodeFleetCockpitHostPortMessage = S.Union([
  ts("HostFleetCockpitLoading", {}),
  ts("HostFleetCockpitStatus", {
    snapshot: KhalaCodeFleetCockpitSnapshot,
  }),
  ts("HostFleetCockpitError", {
    message: S.String,
  }),
  ts("HostFleetCockpitBusy", {
    connectBusy: S.Boolean,
    controlInFlight: S.NullOr(FleetCockpitControlVerb),
    refreshBusy: S.Boolean,
  }),
])
export type KhalaCodeFleetCockpitHostPortMessage =
  typeof KhalaCodeFleetCockpitHostPortMessage.Type

export const KhalaCodeFleetCockpitProgramPortMessage = S.Union([
  ts("ProgramMounted", {
    mountId: S.String,
  }),
  ts("ProgramRequestedRefresh", {}),
  ts("ProgramRequestedConnectAccount", {}),
  ts("ProgramRequestedFleetRunControl", {
    verb: FleetCockpitControlVerb,
  }),
  ts("ProgramUnmounted", {
    mountId: S.String,
  }),
])
export type KhalaCodeFleetCockpitProgramPortMessage =
  typeof KhalaCodeFleetCockpitProgramPortMessage.Type

const decodeHostPortMessage = S.decodeUnknownSync(KhalaCodeFleetCockpitHostPortMessage)
const decodeProgramPortMessage = S.decodeUnknownSync(KhalaCodeFleetCockpitProgramPortMessage)

export type KhalaCodeFleetCockpitPortListener<Message> = (message: Message) => void

export type KhalaCodeFleetCockpitHostPort = Readonly<{
  send: (message: unknown) => KhalaCodeFleetCockpitHostPortMessage
  subscribe: (
    listener: KhalaCodeFleetCockpitPortListener<KhalaCodeFleetCockpitHostPortMessage>,
  ) => () => void
}>

export type KhalaCodeFleetCockpitProgramPort = Readonly<{
  emit: (message: unknown) => KhalaCodeFleetCockpitProgramPortMessage
  subscribe: (
    listener: KhalaCodeFleetCockpitPortListener<KhalaCodeFleetCockpitProgramPortMessage>,
  ) => () => void
}>

const makePort = <Message>(
  decode: (message: unknown) => Message,
): Readonly<{
  publish: (message: unknown) => Message
  subscribe: (listener: KhalaCodeFleetCockpitPortListener<Message>) => () => void
}> => {
  const listeners = new Set<KhalaCodeFleetCockpitPortListener<Message>>()
  return {
    publish: (message) => {
      const decoded = decode(message)
      for (const listener of listeners) listener(decoded)
      return decoded
    },
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}

export type KhalaCodeFleetCockpitPorts = Readonly<{
  host: KhalaCodeFleetCockpitHostPort
  program: KhalaCodeFleetCockpitProgramPort
}>

export const makeKhalaCodeFleetCockpitPorts = (): KhalaCodeFleetCockpitPorts => {
  const host = makePort(decodeHostPortMessage)
  const program = makePort(decodeProgramPortMessage)
  return {
    host: {
      send: host.publish,
      subscribe: host.subscribe,
    },
    program: {
      emit: program.publish,
      subscribe: program.subscribe,
    },
  }
}
