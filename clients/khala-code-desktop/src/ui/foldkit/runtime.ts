import { Effect, Fiber } from "effect"
import { Runtime } from "foldkit"
import type { MakeRuntimeReturn } from "foldkit/runtime"

import { KhalaCodeFleetCockpitMessage } from "./message.js"
import {
  initialKhalaCodeFleetCockpitModel,
  KhalaCodeFleetCockpitModel,
} from "./model.js"
import {
  makeKhalaCodeFleetCockpitPorts,
  type KhalaCodeFleetCockpitPorts,
} from "./ports.js"
import { makeKhalaCodeFleetCockpitSubscriptions } from "./subscriptions.js"
import { makeKhalaCodeFleetCockpitUpdate } from "./update.js"
import { view } from "./view.js"

export type KhalaCodeFleetCockpitEmbedHandle = Readonly<{
  mountId: string
  ports: KhalaCodeFleetCockpitPorts
  send: KhalaCodeFleetCockpitPorts["host"]["send"]
  unmount: () => void
}>

export type KhalaCodeFleetCockpitEmbedOptions = Readonly<{
  mountId?: string
  ports?: KhalaCodeFleetCockpitPorts
}>

const nextMountId = (() => {
  let seq = 0
  return () => {
    seq += 1
    return `khala-code-fleet-cockpit-${seq}`
  }
})()

const startRuntimeFiber = (program: MakeRuntimeReturn): Fiber.Fiber<void> =>
  Effect.runFork(program.start())

export const embedKhalaCodeFleetCockpitProgram = (
  container: HTMLElement,
  options: KhalaCodeFleetCockpitEmbedOptions = {},
): KhalaCodeFleetCockpitEmbedHandle => {
  const mountId = options.mountId ?? nextMountId()
  const ports = options.ports ?? makeKhalaCodeFleetCockpitPorts()
  const runtimeContainer = document.createElement("div")
  runtimeContainer.id = mountId
  runtimeContainer.dataset.khalaCodeFleetCockpitRuntime = "true"
  container.replaceChildren(runtimeContainer)

  const program = Runtime.makeProgram({
    Model: KhalaCodeFleetCockpitModel,
    init: () => [initialKhalaCodeFleetCockpitModel(mountId), []],
    update: makeKhalaCodeFleetCockpitUpdate(ports.program),
    view,
    subscriptions: makeKhalaCodeFleetCockpitSubscriptions(ports.host),
    container: runtimeContainer,
    devTools: {
      show: "Development",
      Message: KhalaCodeFleetCockpitMessage,
    },
    crash: {
      report: ({ error }) => {
        console.error("[khala-code-desktop/fleet-cockpit] crash:", error)
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

export const mountKhalaCodeFleetCockpit = (
  container: HTMLElement | null,
): KhalaCodeFleetCockpitEmbedHandle | null =>
  container === null ? null : embedKhalaCodeFleetCockpitProgram(container)
