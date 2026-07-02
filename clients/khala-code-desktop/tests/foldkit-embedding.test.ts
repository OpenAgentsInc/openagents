import { describe, expect, test } from "bun:test"
import { Effect, Schema as S } from "effect"
import { Window } from "happy-dom"

import {
  KhalaCodeFleetCockpitHostPortMessage,
  KhalaCodeFleetCockpitProgramPortMessage,
  makeKhalaCodeFleetCockpitPorts,
} from "../src/ui/foldkit/ports"
import { FleetCockpitClickedRunControl } from "../src/ui/foldkit/message"
import { initialKhalaCodeFleetCockpitModel } from "../src/ui/foldkit/model"
import { makeKhalaCodeFleetCockpitUpdate } from "../src/ui/foldkit/update"

const installDom = (): {
  readonly flushAnimationFrame: () => void
  readonly window: Window
} => {
  const window = new Window({
    url: "https://khala-code-desktop.test/",
  })
  const animationFrames = new Map<number, FrameRequestCallback>()
  let nextFrame = 0
  const raf = (callback: FrameRequestCallback): number => {
    nextFrame += 1
    animationFrames.set(nextFrame, callback)
    return nextFrame
  }
  const caf = (handle: number): void => {
    animationFrames.delete(handle)
  }
  const defineGlobal = (key: keyof typeof globalThis, value: unknown): void => {
    Object.defineProperty(globalThis, key, {
      configurable: true,
      value,
      writable: true,
    })
  }

  defineGlobal("cancelAnimationFrame", caf)
  defineGlobal("document", window.document)
  defineGlobal("location", window.location)
  defineGlobal("navigator", window.navigator)
  defineGlobal("requestAnimationFrame", raf)
  defineGlobal("window", window)
  window.requestAnimationFrame = raf as unknown as typeof window.requestAnimationFrame
  window.cancelAnimationFrame = caf as unknown as typeof window.cancelAnimationFrame

  return {
    flushAnimationFrame: () => {
      const callbacks = [...animationFrames.values()]
      animationFrames.clear()
      for (const callback of callbacks) callback(performance.now())
    },
    window,
  }
}

const flushDomWork = async (
  dom: ReturnType<typeof installDom>,
  count = 20,
): Promise<void> => {
  for (let index = 0; index < count; index += 1) {
    dom.flushAnimationFrame()
    await yieldTask()
  }
}

const yieldTask = (): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, 0))

const fixtureSnapshot = {
  activeAssignments: 3,
  activeRunActual: 3,
  activeRunRef: "fleet.run.fixture",
  activeRunRemaining: 4,
  activeRunState: "running",
  activeRunTarget: 5,
  freeSlots: 7,
  inFlightLabel: "120 token(s), 30/min",
  maxSlots: 10,
  observedAt: "2026-07-02T00:00:00.000Z",
  pylonLabel: "pylon.fixture",
  pylonStatus: "online",
  readyAccounts: 2,
  tokenRateLabel: "42/min exact",
  totalAccounts: 3,
} as const

describe("khala code Foldkit fleet cockpit", () => {
  test("keeps program update pure until returned commands run", async () => {
    const ports = makeKhalaCodeFleetCockpitPorts()
    const emitted: unknown[] = []
    ports.program.subscribe(message => emitted.push(message))
    const update = makeKhalaCodeFleetCockpitUpdate(ports.program)
    const model = {
      ...initialKhalaCodeFleetCockpitModel("fixture-cockpit"),
      activeRunRef: "fleet.run.fixture",
    }

    const [next, commands] = update(
      model,
      FleetCockpitClickedRunControl({ verb: "pause" }),
    )

    expect(next.controlInFlight).toBe("pause")
    expect(emitted).toEqual([])
    expect(commands).toHaveLength(1)

    await Effect.runPromise(commands[0]!.effect)

    expect(emitted).toEqual([
      { _tag: "ProgramRequestedFleetRunControl", verb: "pause" },
    ])
  })

  test("decodes both host and program port schemas", () => {
    expect(S.decodeUnknownSync(KhalaCodeFleetCockpitHostPortMessage)({
      _tag: "HostFleetCockpitStatus",
      snapshot: fixtureSnapshot,
    })).toEqual({ _tag: "HostFleetCockpitStatus", snapshot: fixtureSnapshot })
    expect(S.decodeUnknownSync(KhalaCodeFleetCockpitProgramPortMessage)({
      _tag: "ProgramRequestedFleetRunControl",
      verb: "drain",
    })).toEqual({ _tag: "ProgramRequestedFleetRunControl", verb: "drain" })

    expect(() =>
      S.decodeUnknownSync(KhalaCodeFleetCockpitProgramPortMessage)({
        _tag: "ProgramRequestedFleetRunControl",
        verb: "restart",
      }),
    ).toThrow()
  })

  test("mounts, receives status, emits refresh, and unmounts", async () => {
    const dom = installDom()
    const { embedKhalaCodeFleetCockpitProgram } = await import("../src/ui/foldkit/runtime")
    const container = document.createElement("section")
    container.id = "fleet-cockpit-fixture"
    document.body.append(container)
    const ports = makeKhalaCodeFleetCockpitPorts()
    const emitted: unknown[] = []
    ports.program.subscribe(message => emitted.push(message))

    const handle = embedKhalaCodeFleetCockpitProgram(container, {
      mountId: "fleet-cockpit-fixture-runtime",
      ports,
    })
    await flushDomWork(dom)

    expect(container.querySelector("[data-foldkit-mount-id='fleet-cockpit-fixture-runtime']")).not.toBeNull()
    expect(container.textContent ?? "").toContain("Fleet cockpit")
    expect(emitted).toContainEqual({
      _tag: "ProgramMounted",
      mountId: "fleet-cockpit-fixture-runtime",
    })

    handle.send({ _tag: "HostFleetCockpitStatus", snapshot: fixtureSnapshot })
    await flushDomWork(dom)

    expect(container.textContent ?? "").toContain("2/3 accounts ready")
    expect(container.textContent ?? "").toContain("42/min exact")
    expect(container.textContent ?? "").toContain("fleet.run.fixture")

    const refresh = [...container.querySelectorAll("button")]
      .find(button => button.textContent === "Refresh")
    expect(refresh).not.toBeUndefined()
    refresh?.click()
    await flushDomWork(dom)

    expect(emitted).toContainEqual({ _tag: "ProgramRequestedRefresh" })

    handle.unmount()

    expect(container.childElementCount).toBe(0)
    expect(emitted).toContainEqual({
      _tag: "ProgramUnmounted",
      mountId: "fleet-cockpit-fixture-runtime",
    })
  })
})
