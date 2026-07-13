import { describe, expect, test } from "bun:test"

import { makeCodexHistoryUtilityFactory } from "./codex-history-utility.ts"

describe("Codex history utility process", () => {
  test("forks an isolated child with no inherited environment or unsigned-library grant", () => {
    const calls: unknown[] = []
    const listeners = new Map<string, (...args: never[]) => void>()
    const posted: unknown[] = []
    let killed = false
    const open = makeCodexHistoryUtilityFactory(
      new URL("file:///Applications/OpenAgents.app/Contents/Resources/app.asar.unpacked/dist/workers/codex-history-worker.js"),
      (modulePath, args, options) => {
        calls.push({ modulePath, args, options })
        return {
          postMessage: (value: unknown) => { posted.push(value) },
          on: (event: string, listener: (...args: never[]) => void) => { listeners.set(event, listener) },
          kill: () => { killed = true; return true },
        }
      },
    )

    const process = open()
    const messages: unknown[] = []
    let exits = 0
    process.onMessage(value => { messages.push(value) })
    process.onExit(() => { exits++ })
    process.postMessage({ request: "catalog" })

    expect(calls).toEqual([{
      modulePath: "/Applications/OpenAgents.app/Contents/Resources/app.asar.unpacked/dist/workers/codex-history-worker.js",
      args: [],
      options: {
        env: {},
        serviceName: "OpenAgents History",
        stdio: "ignore",
        allowLoadingUnsignedLibraries: false,
      },
    }])
    expect(posted).toEqual([{ request: "catalog" }])
    listeners.get("message")?.({ result: "ok" } as never)
    listeners.get("exit")?.()
    expect(messages).toEqual([{ result: "ok" }])
    expect(exits).toBe(1)
    process.terminate()
    expect(killed).toBe(true)
  })
})
