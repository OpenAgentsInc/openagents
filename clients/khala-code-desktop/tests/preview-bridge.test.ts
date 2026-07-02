import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"

const desktopCwd = resolve(import.meta.dir, "..")
const desktopEntry = join(desktopCwd, "src/bun/index.ts")
const fixtureAppServerPath = join(desktopCwd, "src/bun/fixture-codex-app-server.ts")
const rpcTokenHeader = "x-khala-code-preview-token"

const children: Bun.Subprocess[] = []

afterEach(() => {
  for (const child of children.splice(0)) {
    child.kill("SIGKILL")
  }
})

const randomPort = (): number => 52_000 + Math.floor(Math.random() * 8_000)

const tempRoot = async (): Promise<string> =>
  mkdtemp(join(tmpdir(), "khala-code-preview-bridge-"))

const startPreviewBridge = async (
  env: Record<string, string> = {},
): Promise<{ readonly baseUrl: string; readonly child: Bun.Subprocess; readonly token: string }> => {
  const port = randomPort()
  const token = env.KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN ?? `preview-token-${port}`
  const root = await tempRoot()
  const child = Bun.spawn([process.execPath, desktopEntry], {
    cwd: desktopCwd,
    env: {
      ...process.env,
      CODEX_HOME: join(root, "codex-home"),
      KHALA_CODE_BUN_BINARY: process.execPath,
      KHALA_CODE_CODEX_APP_SERVER_FIXTURE: "1",
      KHALA_CODE_CODEX_APP_SERVER_FIXTURE_PATH: fixtureAppServerPath,
      KHALA_CODE_DESKTOP_OPEN_WINDOW: "0",
      KHALA_CODE_DESKTOP_PREVIEW_PORT: String(port),
      KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN: token,
      KHALA_CODE_DESKTOP_WORKSPACE: root,
      KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED: "1",
      KHALA_CODE_TOKEN_USAGE_DISABLED: "1",
      ...env,
    },
    stderr: "pipe",
    stdout: "pipe",
  })
  children.push(child)
  const baseUrl = `http://127.0.0.1:${port}`
  await waitForHealth(baseUrl, child)
  return { baseUrl, child, token }
}

const waitForHealth = async (baseUrl: string, child: Bun.Subprocess): Promise<void> => {
  const deadline = Date.now() + 8_000
  let lastError = ""
  while (Date.now() < deadline) {
    if (child.exitCode !== null) break
    try {
      const response = await fetch(`${baseUrl}/health`)
      if (response.ok) return
      lastError = `${response.status} ${response.statusText}`
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error)
    }
    await Bun.sleep(50)
  }
  const [stdout, stderr] = await Promise.all([
    new Response(child.stdout as ReadableStream<Uint8Array>).text().catch(() => ""),
    new Response(child.stderr as ReadableStream<Uint8Array>).text().catch(() => ""),
  ])
  throw new Error(`preview bridge did not become healthy: ${lastError}\nstdout:\n${stdout}\nstderr:\n${stderr}`)
}

const rpc = async (
  baseUrl: string,
  method: string,
  token: string | null,
  args: readonly unknown[] = [],
): Promise<Response> =>
  fetch(`${baseUrl}/rpc/${method}`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      ...(token === null ? {} : { [rpcTokenHeader]: token }),
    },
    body: JSON.stringify({ args }),
  })

const readUntilSseEvent = async (
  reader: ReadableStreamDefaultReader<Uint8Array>,
  expectedEvent: string,
): Promise<unknown> => {
  const decoder = new TextDecoder()
  const deadline = Date.now() + 8_000
  let buffer = ""
  while (Date.now() < deadline) {
    const read = await reader.read()
    if (read.done) break
    buffer += decoder.decode(read.value, { stream: true })
    const frames = buffer.split("\n\n")
    buffer = frames.pop() ?? ""
    for (const frame of frames) {
      if (!frame.includes(`event: ${expectedEvent}`)) continue
      const dataLine = frame.split("\n").find(line => line.startsWith("data: "))
      if (dataLine === undefined) continue
      return JSON.parse(dataLine.slice("data: ".length)) as unknown
    }
  }
  throw new Error(`timed out waiting for SSE event ${expectedEvent}`)
}

describe("Khala Code preview bridge auth and SSE", () => {
  test("rejects missing tokens and accepts the per-boot header token", async () => {
    const { baseUrl, token } = await startPreviewBridge()

    const rejected = await rpc(baseUrl, "appInfo", null)
    expect(rejected.status).toBe(401)
    await expect(rejected.json()).resolves.toMatchObject({
      error: "unauthorized",
      tag: "rpc_unauthorized",
    })

    const accepted = await rpc(baseUrl, "appInfo", token)
    expect(accepted.status).toBe(200)
    await expect(accepted.json()).resolves.toMatchObject({
      app: "Khala Code Desktop",
      ok: true,
    })
  })

  test("read-only mode rejects mutating RPC methods", async () => {
    const { baseUrl, token } = await startPreviewBridge({
      KHALA_CODE_DESKTOP_PREVIEW_READONLY: "1",
    })

    const response = await rpc(baseUrl, "submitChatMessage", token, [{
      messages: [{ body: "blocked", id: "user-1", role: "user" }],
      sessionId: "session-readonly",
    }])

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toMatchObject({
      error: "read_only",
      method: "submitChatMessage",
      tag: "rpc_read_only",
    })
  })

  test("streams chat turn events over /rpc/events with the fixture app-server", async () => {
    const { baseUrl, token } = await startPreviewBridge()
    const events = await fetch(`${baseUrl}/rpc/events`, {
      headers: { [rpcTokenHeader]: token },
    })
    expect(events.status).toBe(200)
    expect(events.headers.get("content-type")).toContain("text/event-stream")
    const reader = events.body!.getReader()

    try {
      const eventPromise = readUntilSseEvent(reader, "chatTurnEvent")
      const controller = new AbortController()
      const turnPromise = fetch(`${baseUrl}/rpc/submitChatMessage`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          [rpcTokenHeader]: token,
        },
        signal: controller.signal,
        body: JSON.stringify({
          args: [{
            messages: [{ body: "Run the fixture turn", id: "user-1", role: "user" }],
            sessionId: "session-fixture",
            startNewThread: true,
          }],
        }),
      }).catch(() => undefined)

      await expect(eventPromise).resolves.toMatchObject({
        event: {
          type: "thread_ready",
        },
        type: "chatTurnEvent",
      })
      controller.abort()
      await turnPromise
    } finally {
      await reader.cancel().catch(() => undefined)
    }
  }, 15_000)
})

describe("preview rpc read-only classification", () => {
  test("every RPC method is explicitly classified (fails closed on drift)", async () => {
    const { KhalaCodeDesktopRpcMethodNames } = await import("../src/shared/rpc.js")
    const { mutatingPreviewRpcMethods, readOnlySafePreviewRpcMethods } = await import("../src/bun/preview-rpc-policy.js")
    for (const name of KhalaCodeDesktopRpcMethodNames) {
      const inMutating = mutatingPreviewRpcMethods.has(name)
      const inSafe = readOnlySafePreviewRpcMethods.has(name)
      expect(`${name}:${inMutating || inSafe}`).toBe(`${name}:true`)
      expect(`${name}:${inMutating && inSafe}`).toBe(`${name}:false`)
    }
  })
})
