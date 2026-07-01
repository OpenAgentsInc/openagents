import { EventEmitter } from "node:events"
import { describe, expect, test } from "bun:test"

import {
  createCodexAppServerHost,
  KHALA_CODE_CODEX_APP_SERVER_ADAPTER_VERSION,
} from "../src/bun/codex-app-server-client"

type FakeChild = EventEmitter & {
  readonly pid: number
  readonly stdout: EventEmitter
  readonly stderr: EventEmitter
  readonly stdin: { readonly write: (line: string) => void }
  readonly kill: () => void
  killed: boolean
}

type WireMessage = {
  readonly id?: number | string
  readonly method?: string
  readonly params?: unknown
  readonly result?: unknown
}

function makeChild(
  onWrite: (message: WireMessage, child: FakeChild) => void,
  pid = 1234,
): FakeChild {
  const child = new EventEmitter() as FakeChild
  Object.assign(child, {
    pid,
    stdout: new EventEmitter(),
    stderr: new EventEmitter(),
    killed: false,
    stdin: {
      write: (line: string) => {
        const message = JSON.parse(line) as WireMessage
        queueMicrotask(() => onWrite(message, child))
      },
    },
    kill: () => {
      child.killed = true
      queueMicrotask(() => child.emit("close", 0, null))
    },
  })
  return child
}

function respond(child: FakeChild, id: number | string | undefined, result: unknown): void {
  if (id === undefined) return
  child.stdout.emit("data", Buffer.from(`${JSON.stringify({ id, result })}\n`))
}

describe("Codex app-server client", () => {
  test("starts codex app-server over stdio and completes initialize/initialized", async () => {
    const writes: WireMessage[] = []
    const host = createCodexAppServerHost({
      env: {
        CODEX_HOME: "/tmp/codex-home",
        KHALA_CODE_CODEX_BINARY: "/opt/codex/bin/codex",
      } as NodeJS.ProcessEnv,
      spawnFn: (command, args, options) => {
        expect(command).toBe("/opt/codex/bin/codex")
        expect(args).toEqual(["app-server", "--stdio"])
        expect(options?.env).toMatchObject({
          CODEX_HOME: "/tmp/codex-home",
          LOG_FORMAT: "json",
        })
        return makeChild((message, child) => {
          writes.push(message)
          if (message.method === "initialize") {
            expect(message.params).toMatchObject({
              clientInfo: {
                name: "khala_code_desktop",
                title: "Khala Code Desktop",
              },
              capabilities: {
                experimentalApi: true,
                mcpServerOpenaiFormElicitation: true,
              },
            })
            respond(child, message.id, {
              userAgent: "codex-test",
              codexHome: "/tmp/codex-home",
              platformFamily: "unix",
              platformOs: "macos",
            })
          }
        })
      },
    })

    const result = await host.start()

    expect(result.ok).toBe(true)
    expect(result.changed).toBe(true)
    expect(result.status).toMatchObject({
      adapterVersion: KHALA_CODE_CODEX_APP_SERVER_ADAPTER_VERSION,
      codexCommand: "/opt/codex/bin/codex",
      codexHome: "/tmp/codex-home",
      initialized: true,
      state: "running",
      transport: "stdio",
    })
    expect(writes.map(message => message.method)).toEqual(["initialize", "initialized"])
    host.dispose()
  })

  test("correlates requests and dispatches notifications", async () => {
    const notifications: string[] = []
    const host = createCodexAppServerHost({
      spawnFn: () => makeChild((message, child) => {
        if (message.method === "initialize") {
          respond(child, message.id, {})
          queueMicrotask(() => {
            child.stdout.emit("data", Buffer.from(`${JSON.stringify({
              method: "thread/status/changed",
              params: { threadId: "thread-1", status: "running" },
            })}\n`))
          })
          return
        }
        if (message.method === "model/list") {
          respond(child, message.id, { models: [{ id: "gpt-5.1-codex" }] })
        }
      }),
    })
    host.subscribe(notification => notifications.push(notification.method))

    await host.start()
    const models = await host.request<{ readonly models: readonly { readonly id: string }[] }>("model/list")

    expect(models.models[0]?.id).toBe("gpt-5.1-codex")
    expect(notifications).toContain("thread/status/changed")
    expect(host.status().pendingRequestCount).toBe(0)
    host.dispose()
  })

  test("continues notification delivery when one subscriber throws", async () => {
    const notifications: string[] = []
    const host = createCodexAppServerHost({
      spawnFn: () => makeChild((message, child) => {
        if (message.method === "initialize") {
          respond(child, message.id, {})
          queueMicrotask(() => {
            child.stdout.emit("data", Buffer.from(`${JSON.stringify({
              method: "thread/status/changed",
              params: { threadId: "thread-1", status: "running" },
            })}\n`))
          })
        }
      }),
    })
    host.subscribe(() => {
      throw new Error("broken subscriber")
    })
    host.subscribe(notification => notifications.push(notification.method))

    await host.start()
    await Promise.resolve()

    expect(notifications).toEqual(["thread/status/changed"])
    expect(host.status().diagnostics).toContain(
      "notification subscriber failed for thread/status/changed: broken subscriber",
    )
    host.dispose()
  })

  test("dispatches server-to-client requests with ids as notifications", async () => {
    const notifications: { readonly id?: number | string; readonly method: string }[] = []
    const host = createCodexAppServerHost({
      spawnFn: () => makeChild((message, child) => {
        if (message.method === "initialize") {
          respond(child, message.id, {})
          queueMicrotask(() => {
            child.stdout.emit("data", Buffer.from(`${JSON.stringify({
              id: 99,
              method: "item/commandExecution/requestApproval",
              params: {
                threadId: "thread-1",
                turnId: "turn-1",
                itemId: "item-command",
                command: "git status",
              },
            })}\n`))
          })
        }
      }),
    })
    host.subscribe(notification => notifications.push({
      ...(notification.id === undefined ? {} : { id: notification.id }),
      method: notification.method,
    }))

    await host.start()
    await Promise.resolve()

    expect(notifications).toContainEqual({
      id: 99,
      method: "item/commandExecution/requestApproval",
    })
    expect(host.status().diagnostics).not.toContain("unknown response id: 99")
    host.dispose()
  })

  test("responds to server-to-client requests with the original id", async () => {
    const writes: WireMessage[] = []
    const host = createCodexAppServerHost({
      spawnFn: () => makeChild((message, child) => {
        writes.push(message)
        if (message.method === "initialize") {
          respond(child, message.id as number | undefined, {})
        }
      }),
    })

    await host.start()
    host.respondToServerRequest("approval-request-1", {
      decision: "accept",
    })
    await Promise.resolve()

    expect(writes).toContainEqual({
      id: "approval-request-1",
      result: {
        decision: "accept",
      },
    })
    host.dispose()
  })

  test("maps app-server request errors and preserves last error", async () => {
    const host = createCodexAppServerHost({
      spawnFn: () => makeChild((message, child) => {
        if (message.method === "initialize") {
          respond(child, message.id, {})
          return
        }
        child.stdout.emit("data", Buffer.from(`${JSON.stringify({
          id: message.id,
          error: { code: -32001, message: "Server overloaded; retry later." },
        })}\n`))
      }),
    })

    await host.start()
    await expect(host.request("thread/list")).rejects.toThrow("Server overloaded; retry later.")
    expect(host.status().lastError).toBe("Server overloaded; retry later.")
    host.dispose()
  })

  test("times out unanswered requests without leaking pending request state", async () => {
    const host = createCodexAppServerHost({
      requestTimeoutMs: 5,
      spawnFn: () => makeChild((message, child) => {
        if (message.method === "initialize") respond(child, message.id, {})
      }),
    })

    await host.start()
    await expect(host.request("thread/list")).rejects.toThrow("Codex app-server request timed out: thread/list")
    expect(host.status().pendingRequestCount).toBe(0)
    host.dispose()
  })

  test("stops and restarts the supervised process", async () => {
    const children: FakeChild[] = []
    const host = createCodexAppServerHost({
      spawnFn: () => {
        const child = makeChild((message, rpcChild) => {
          if (message.method === "initialize") respond(rpcChild, message.id, {})
        }, 2000 + children.length)
        children.push(child)
        return child
      },
    })

    await expect(host.start()).resolves.toMatchObject({ ok: true, changed: true })
    const firstPid = host.status().pid
    await expect(host.restart()).resolves.toMatchObject({ ok: true, action: "restart" })

    expect(children[0]?.killed).toBe(true)
    expect(host.status().pid).not.toBe(firstPid)
    await expect(host.stop()).resolves.toMatchObject({
      ok: true,
      action: "stop",
      changed: true,
      status: { state: "stopped", initialized: false },
    })
  })
})
