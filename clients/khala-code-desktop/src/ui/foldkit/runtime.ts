import { Effect, Fiber } from "effect"
import { Runtime } from "foldkit"
import type { MakeRuntimeReturn } from "foldkit/runtime"

import { KhalaCodeFoldkitMessage } from "./message.js"
import { initialKhalaCodeFoldkitModel, KhalaCodeFoldkitModel } from "./model.js"
import {
  makeKhalaCodeFoldkitPorts,
  type KhalaCodeFoldkitPorts,
} from "./ports.js"
import { makeKhalaCodeFoldkitSubscriptions } from "./subscriptions.js"
import { makeKhalaCodeFoldkitUpdate } from "./update.js"
import { view } from "./view.js"

export type KhalaCodeFoldkitEmbedHandle = Readonly<{
  mountId: string
  ports: KhalaCodeFoldkitPorts
  send: KhalaCodeFoldkitPorts["host"]["send"]
  unmount: () => void
}>

export type KhalaCodeFoldkitEmbedOptions = Readonly<{
  mountId?: string
  ports?: KhalaCodeFoldkitPorts
}>

const nextMountId = (() => {
  let seq = 0
  return () => {
    seq += 1
    return `khala-code-foldkit-${seq}`
  }
})()

const startRuntimeFiber = (program: MakeRuntimeReturn): Fiber.Fiber<void> =>
  Effect.runFork(program.start())

export const embedKhalaCodeFoldkitProgram = (
  container: HTMLElement,
  options: KhalaCodeFoldkitEmbedOptions = {},
): KhalaCodeFoldkitEmbedHandle => {
  const mountId = options.mountId ?? nextMountId()
  const ports = options.ports ?? makeKhalaCodeFoldkitPorts()
  const runtimeContainer = document.createElement("div")
  runtimeContainer.id = mountId
  runtimeContainer.dataset.khalaCodeFoldkitRuntime = "true"
  container.replaceChildren(runtimeContainer)

  const program = Runtime.makeProgram({
    Model: KhalaCodeFoldkitModel,
    init: () => [initialKhalaCodeFoldkitModel(mountId), []],
    update: makeKhalaCodeFoldkitUpdate(ports.program),
    view,
    subscriptions: makeKhalaCodeFoldkitSubscriptions(ports.host),
    container: runtimeContainer,
    devTools: {
      show: "Development",
      Message: KhalaCodeFoldkitMessage,
    },
    crash: {
      report: ({ error }) => {
        console.error("[khala-code-desktop/foldkit] crash:", error)
      },
    },
  })

  const fiber = startRuntimeFiber(program)
  let mounted = true

  return {
    mountId,
    ports,
    send: ports.host.send,
    unmount: () => {
      if (!mounted) return
      mounted = false
      ports.program.emit({ _tag: "ProgramUnmounted", mountId })
      Effect.runFork(Fiber.interrupt(fiber))
      container.replaceChildren()
    },
  }
}

export const mountKhalaCodeFoldkitDemo = (
  container: HTMLElement | null,
): KhalaCodeFoldkitEmbedHandle | null =>
  container === null ? null : embedKhalaCodeFoldkitProgram(container)

