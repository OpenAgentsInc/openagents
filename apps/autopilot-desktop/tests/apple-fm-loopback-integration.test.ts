import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync } from "node:fs"
import { mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"
import { fetchAppleFmReadiness, fetchNodeState, startAppleFmSession } from "../src/bun/pylon-control"
import { createBootstrapSummary, parseBootstrapArgs } from "../../pylon/src/bootstrap"
import { collectPylonAppleFmStatus } from "../../pylon/src/node/apple-fm-status"
import { startControlServer } from "../../pylon/src/node/control-server"
import { createControlSessionActions } from "../../pylon/src/node/control-sessions"
import { makePylonNodeRuntime } from "../../pylon/src/node/runtime"

const servers: Bun.Server[] = []
const fakeBridgeSessionId = "apple_fm_session_123e4567-e89b-12d3-a456-426614174111"

afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true)
})

async function withFixture<T>(
  fn: (fixture: {
    proofDir: string
    pylonHome: string
    summary: ReturnType<typeof createBootstrapSummary>
    worktree: string
  }) => Promise<T>,
) {
  const root = mkdtempSync(join(tmpdir(), "desktop-apple-fm-loopback-"))
  try {
    const pylonHome = join(root, "pylon-home")
    const proofDir = join(root, "proofs")
    const worktree = join(root, "worktree")
    await mkdir(pylonHome, { recursive: true })
    await mkdir(worktree, { recursive: true })
    await writeFile(join(worktree, "README.md"), "# Public Fixture\n\nprivate fixture body\n", "utf8")
    const summary = createBootstrapSummary(
      parseBootstrapArgs(["--json", "--pylon-ref", "pylon.test.desktop-apple-fm-loopback"]),
      { PYLON_HOME: pylonHome },
    )
    return await fn({ proofDir, pylonHome, summary, worktree })
  } finally {
    await rm(root, { recursive: true, force: true })
  }
}

function fakeReadyBridge(): Bun.Server {
  let callbackUrl = ""
  let callbackToken = ""
  const server = Bun.serve({
    port: 0,
    fetch: async (request) => {
      const url = new URL(request.url)
      if (url.pathname === "/health") {
        return Response.json({
          ready: true,
          model: "apple-foundation-model",
          platform: "macOS-arm64-test",
          version: "fake-bridge",
        })
      }
      if (url.pathname === "/v1/sessions" && request.method === "POST") {
        const body = (await request.json()) as {
          tool_callback?: { url?: string; session_token?: string }
          tools?: Array<{ name?: string }>
        }
        callbackUrl = body.tool_callback?.url ?? ""
        callbackToken = body.tool_callback?.session_token ?? ""
        expect(body.tools?.map((tool) => tool.name).sort()).toEqual([
          "code_search",
          "list_files",
          "read_file",
        ])
        return Response.json({ session: { id: fakeBridgeSessionId } })
      }
      if (url.pathname === `/v1/sessions/${fakeBridgeSessionId}/responses/stream`) {
        const callback = await fetch(callbackUrl, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            session_token: callbackToken,
            tool_name: "read_file",
            arguments: {
              generation_id: "tool-call-1",
              content: { path: "README.md" },
              is_complete: true,
            },
          }),
        })
        const callbackBody = (await callback.json()) as { output?: string }
        return new Response(
          [
            "event: snapshot",
            "data: {\"kind\":\"snapshot\",\"model\":\"apple-foundation-model\",\"output\":\"working locally\"}",
            "",
            "event: completed",
            `data: ${JSON.stringify({
              kind: "completed",
              model: "apple-foundation-model",
              output: `local answer after tool: ${callbackBody.output?.slice(0, 24) ?? "ok"}`,
              usage: { total_tokens_detail: { value: 13, truth: "estimated" } },
            })}`,
            "",
          ].join("\n"),
          { headers: { "content-type": "text/event-stream" } },
        )
      }
      return Response.json({ error: "not found", path: url.pathname }, { status: 404 })
    },
  })
  servers.push(server)
  return server
}

describe("Apple FM desktop loopback integration", () => {
  test("desktop client drives a fake-bridge local Apple FM session through Pylon control", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const bridge = fakeReadyBridge()
      const env = { PROBE_APPLE_FM_BASE_URL: String(bridge.url) }
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const runtime = yield* makePylonNodeRuntime
            const sessions = createControlSessionActions({
              env,
              proofsDir: proofDir,
              summary,
            })
            const token = "test-token-0123456789abcdef"
            const server = yield* startControlServer(runtime, {
              token,
              port: 0,
              actions: {
                walletSend: async () => ({ dispatched: false }),
                walletReceive: async () => ({ unavailable: true }),
                walletAdmitPayoutTarget: async () => ({ admitted: false }),
                appleFmStatus: () =>
                  collectPylonAppleFmStatus({
                    summary,
                    env,
                    fetch,
                    now: new Date("2026-06-15T00:00:00.000Z"),
                  }),
                sessions,
              },
            })

            const readiness = yield* Effect.promise(() =>
              fetchAppleFmReadiness({ baseUrl: server.url, token }),
            )
            expect(readiness).toMatchObject({
              ok: true,
              available: true,
              status: "ready",
              backendKind: "apple_fm_bridge",
              model: "apple-foundation-model",
            })

            const started = yield* Effect.promise(() =>
              startAppleFmSession({
                baseUrl: server.url,
                token,
                prompt: "Use read_file on README.md, then answer with a public-safe summary.",
                worktreePath: worktree,
                timeoutSeconds: 300,
              }),
            )
            expect(started).toMatchObject({
              ok: true,
              blockerRefs: [],
            })

            let state = yield* Effect.promise(() => fetchNodeState({ baseUrl: server.url, token }))
            for (let attempt = 0; attempt < 50 && state.sessions[0]?.state !== "completed"; attempt += 1) {
              yield* Effect.sleep("20 millis")
              state = yield* Effect.promise(() => fetchNodeState({ baseUrl: server.url, token }))
            }

            const session = state.sessions.find((row) => row.sessionRef === started.sessionRef)
            expect(session).toMatchObject({
              adapter: "apple_fm",
              lane: "local",
              state: "completed",
            })
            const eventText = JSON.stringify(state.events[started.sessionRef] ?? [])
            expect(eventText).toContain("Apple FM local backend ready")
            expect(eventText).toContain("Apple FM tool read_file: success")
            expect(eventText).not.toContain("Use read_file on README.md")
            expect(eventText).not.toContain("private fixture body")
            expect(eventText).not.toContain("session_token")
            expect(eventText).not.toContain("tool-callback")
            expect(eventText).not.toContain("Bearer ")
            expect(state.artifacts[started.sessionRef]).toMatchObject({
              kind: "proof",
              outcome: "completed",
              commandCount: 1,
              totalTokens: 13,
            })
            expect(JSON.stringify(state.artifacts[started.sessionRef])).not.toContain(fakeBridgeSessionId)
          }),
        ),
      )
    })
  })
})
