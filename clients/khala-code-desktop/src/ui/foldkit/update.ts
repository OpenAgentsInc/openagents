import { Effect } from "effect"
import type { Command } from "foldkit"

import {
  FleetCockpitCompletedPortEmit,
  type KhalaCodeFleetCockpitMessage,
} from "./message.js"
import type { KhalaCodeFleetCockpitModel } from "./model.js"
import type {
  KhalaCodeFleetCockpitProgramPort,
  KhalaCodeFleetCockpitProgramPortMessage,
} from "./ports.js"

type UpdateResult = readonly [
  KhalaCodeFleetCockpitModel,
  ReadonlyArray<Command.Command<KhalaCodeFleetCockpitMessage>>,
]

const noCommands: ReadonlyArray<Command.Command<KhalaCodeFleetCockpitMessage>> = []

const emitProgramPort = (
  port: KhalaCodeFleetCockpitProgramPort,
  message: KhalaCodeFleetCockpitProgramPortMessage,
): Command.Command<KhalaCodeFleetCockpitMessage> => ({
  name: "KhalaCodeFleetCockpitEmitProgramPort",
  args: { tag: message._tag },
  effect: Effect.sync(() => {
    port.emit(message)
    return FleetCockpitCompletedPortEmit()
  }),
})

export const makeKhalaCodeFleetCockpitUpdate =
  (port: KhalaCodeFleetCockpitProgramPort) =>
  (
    model: KhalaCodeFleetCockpitModel,
    message: KhalaCodeFleetCockpitMessage,
  ): UpdateResult => {
    switch (message._tag) {
      case "FleetCockpitClickedRefresh":
        return [
          { ...model, refreshBusy: true },
          [
            emitProgramPort(port, {
              _tag: "ProgramRequestedRefresh",
            }),
          ],
        ]
      case "FleetCockpitClickedConnectAccount":
        if (model.connectBusy) return [model, noCommands]
        return [
          { ...model, connectBusy: true },
          [
            emitProgramPort(port, {
              _tag: "ProgramRequestedConnectAccount",
            }),
          ],
        ]
      case "FleetCockpitClickedRunControl":
        if (model.activeRunRef === null || model.controlInFlight !== null) {
          return [model, noCommands]
        }
        return [
          { ...model, controlInFlight: message.verb },
          [
            emitProgramPort(port, {
              _tag: "ProgramRequestedFleetRunControl",
              verb: message.verb,
            }),
          ],
        ]
      case "FleetCockpitReceivedHostPort":
        switch (message.message._tag) {
          case "HostFleetCockpitLoading":
            return [{ ...model, error: null, phase: "loading" }, noCommands]
          case "HostFleetCockpitStatus":
            return [
              {
                ...model,
                ...message.message.snapshot,
                error: null,
                phase: "ready",
                refreshBusy: false,
              },
              noCommands,
            ]
          case "HostFleetCockpitError":
            return [
              {
                ...model,
                error: message.message.message,
                phase: "error",
                refreshBusy: false,
              },
              noCommands,
            ]
          case "HostFleetCockpitBusy":
            return [
              {
                ...model,
                connectBusy: message.message.connectBusy,
                controlInFlight: message.message.controlInFlight,
                refreshBusy: message.message.refreshBusy,
              },
              noCommands,
            ]
        }
      case "FleetCockpitMounted":
        return [
          model,
          [
            emitProgramPort(port, {
              _tag: "ProgramMounted",
              mountId: model.mountId,
            }),
          ],
        ]
      case "FleetCockpitUnmounted":
        return [
          model,
          [
            emitProgramPort(port, {
              _tag: "ProgramUnmounted",
              mountId: model.mountId,
            }),
          ],
        ]
      case "FleetCockpitCompletedPortEmit":
        return [model, noCommands]
    }
  }
