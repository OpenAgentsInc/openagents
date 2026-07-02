import { describe, expect, test } from "bun:test"
import { Effect, Schema as S } from "effect"
import { Window } from "happy-dom"

import {
  KhalaCodeFoldkitHostPortMessage,
  KhalaCodeFoldkitProgramPortMessage,
  makeKhalaCodeFoldkitPorts,
} from "../src/ui/foldkit/ports"
import { FoldkitDemoReceivedHostPort } from "../src/ui/foldkit/message"
import { initialKhalaCodeFoldkitModel } from "../src/ui/foldkit/model"
import { makeKhalaCodeFoldkitUpdate } from "../src/ui/foldkit/update"

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

describe("khala code Foldkit embedding skeleton", () => {
  test("keeps program update pure until returned commands run", async () => {
    const ports = makeKhalaCodeFoldkitPorts()
    const emitted: unknown[] = []
    ports.program.subscribe(message => emitted.push(message))
    const update = makeKhalaCodeFoldkitUpdate(ports.program)
    const model = initialKhalaCodeFoldkitModel("fixture-foldkit")

    const [next, commands] = update(
      model,
      FoldkitDemoReceivedHostPort({
        message: { _tag: "HostPing", nonce: "fixture-nonce" },
      }),
    )

    expect(next).toEqual({
      label: "Foldkit skeleton",
      mountId: "fixture-foldkit",
      pingCount: 1,
    })
    expect(emitted).toEqual([])
    expect(commands).toHaveLength(1)

    await Effect.runPromise(commands[0]!.effect)

    expect(emitted).toEqual([
      { _tag: "ProgramPong", count: 1, nonce: "fixture-nonce" },
    ])
  })

  test("decodes both host and program port schemas", () => {
    expect(S.decodeUnknownSync(KhalaCodeFoldkitHostPortMessage)({
      _tag: "HostSetLabel",
      label: "Mounted",
    })).toEqual({ _tag: "HostSetLabel", label: "Mounted" })
    expect(S.decodeUnknownSync(KhalaCodeFoldkitProgramPortMessage)({
      _tag: "ProgramPong",
      count: 2,
      nonce: "round-trip",
    })).toEqual({ _tag: "ProgramPong", count: 2, nonce: "round-trip" })

    expect(() =>
      S.decodeUnknownSync(KhalaCodeFoldkitHostPortMessage)({
        _tag: "HostPing",
        nonce: 42,
      }),
    ).toThrow()
  })

  test("mounts, round-trips ports, and unmounts from a designated container", async () => {
    const dom = installDom()
    const { embedKhalaCodeFoldkitProgram } = await import("../src/ui/foldkit/runtime")
    const container = document.createElement("section")
    container.id = "foldkit-demo-fixture"
    document.body.append(container)
    const ports = makeKhalaCodeFoldkitPorts()
    const emitted: unknown[] = []
    ports.program.subscribe(message => emitted.push(message))

    const handle = embedKhalaCodeFoldkitProgram(container, {
      mountId: "foldkit-demo-fixture-runtime",
      ports,
    })
    await flushDomWork(dom)

    expect(container.querySelector("[data-foldkit-mount-id='foldkit-demo-fixture-runtime']")).not.toBeNull()
    expect(container.textContent ?? "").toContain("Foldkit skeleton")
    expect(emitted).toContainEqual({
      _tag: "ProgramMounted",
      mountId: "foldkit-demo-fixture-runtime",
    })

    handle.send({ _tag: "HostPing", nonce: "host-fixture" })
    await flushDomWork(dom)

    expect(container.textContent ?? "").toContain("Ping 1")
    expect(emitted).toContainEqual({
      _tag: "ProgramPong",
      count: 1,
      nonce: "host-fixture",
    })

    handle.unmount()

    expect(container.childElementCount).toBe(0)
    expect(emitted).toContainEqual({
      _tag: "ProgramUnmounted",
      mountId: "foldkit-demo-fixture-runtime",
    })
  })
})
