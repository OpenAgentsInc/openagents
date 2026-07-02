import { Effect } from "effect"
import type { Command } from "foldkit"

import type { KhalaCodeFoldkitMessage } from "./message.js"
import { FoldkitDemoCompletedPortEmit } from "./message.js"
import type { KhalaCodeFoldkitModel } from "./model.js"
import type {
  KhalaCodeFoldkitProgramPort,
  KhalaCodeFoldkitProgramPortMessage,
} from "./ports.js"

type UpdateResult = readonly [
  KhalaCodeFoldkitModel,
  ReadonlyArray<Command.Command<KhalaCodeFoldkitMessage>>,
]

const noCommands: ReadonlyArray<Command.Command<KhalaCodeFoldkitMessage>> = []

const emitProgramPort = (
  port: KhalaCodeFoldkitProgramPort,
  message: KhalaCodeFoldkitProgramPortMessage,
): Command.Command<KhalaCodeFoldkitMessage> => ({
  name: "KhalaCodeFoldkitEmitProgramPort",
  args: { tag: message._tag },
  effect: Effect.sync(() => {
    port.emit(message)
    return FoldkitDemoCompletedPortEmit()
  }),
})

export const makeKhalaCodeFoldkitUpdate =
  (port: KhalaCodeFoldkitProgramPort) =>
  (
    model: KhalaCodeFoldkitModel,
    message: KhalaCodeFoldkitMessage,
  ): UpdateResult => {
    switch (message._tag) {
      case "FoldkitDemoClickedPing": {
        const next = { ...model, pingCount: model.pingCount + 1 }
        return [
          next,
          [
            emitProgramPort(port, {
              _tag: "ProgramPong",
              count: next.pingCount,
              nonce: `ui:${next.pingCount}`,
            }),
          ],
        ]
      }
      case "FoldkitDemoReceivedHostPort":
        switch (message.message._tag) {
          case "HostPing": {
            const next = { ...model, pingCount: model.pingCount + 1 }
            return [
              next,
              [
                emitProgramPort(port, {
                  _tag: "ProgramPong",
                  count: next.pingCount,
                  nonce: message.message.nonce,
                }),
              ],
            ]
          }
          case "HostSetLabel":
            return [{ ...model, label: message.message.label }, noCommands]
        }
      case "FoldkitDemoMounted":
        return [
          model,
          [
            emitProgramPort(port, {
              _tag: "ProgramMounted",
              mountId: model.mountId,
            }),
          ],
        ]
      case "FoldkitDemoUnmounted":
        return [
          model,
          [
            emitProgramPort(port, {
              _tag: "ProgramUnmounted",
              mountId: model.mountId,
            }),
          ],
        ]
      case "FoldkitDemoCompletedPortEmit":
        return [model, noCommands]
    }
  }
