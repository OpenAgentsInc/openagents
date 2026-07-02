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

const installDom = (): Window => {
  const window = new Window({
    url: "https://khala-code-desktop.test/",
  })
  const raf = (callback: FrameRequestCallback): number =>
    Number(setTimeout(() => callback(performance.now()), 0))
  const caf = (handle: number): void => clearTimeout(handle)

  Object.assign(globalThis, {
    cancelAnimationFrame: caf,
    document: window.document,
    location: window.location,
    navigator: window.navigator,
    requestAnimationFrame: raf,
    window,
  })

  return window
}

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
    installDom()
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
    await new Promise(resolve => setTimeout(resolve, 20))

    expect(container.querySelector("[data-foldkit-mount-id='foldkit-demo-fixture-runtime']")).not.toBeNull()
    expect(container.textContent ?? "").toContain("Foldkit skeleton")
    expect(emitted).toContainEqual({
      _tag: "ProgramMounted",
      mountId: "foldkit-demo-fixture-runtime",
    })

    handle.send({ _tag: "HostPing", nonce: "host-fixture" })
    await new Promise(resolve => setTimeout(resolve, 20))

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
