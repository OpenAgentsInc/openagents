import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync } from "node:fs"
import { mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  DEFAULT_APPLE_FM_MODELED_POWER_KW,
  estimateAppleFmLocalSessionEnergy,
} from "../src/node/apple-fm-energy-estimate"
import { makeAppleFmWorkspaceTools } from "../src/node/apple-fm-local-session"
import { createControlSessionActions } from "../src/node/control-sessions"

const servers: Bun.Server[] = []
const fakeBridgeSessionId = "apple_fm_session_123e4567-e89b-12d3-a456-426614174000"

afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true)
})

async function withFixture<T>(fn: (fixture: {
  pylonHome: string
  proofDir: string
  summary: ReturnType<typeof createBootstrapSummary>
  worktree: string
}) => Promise<T>) {
  const root = mkdtempSync(join(tmpdir(), "pylon-apple-fm-control-session-"))
  try {
    const pylonHome = join(root, "pylon-home")
    const worktree = join(root, "worktree")
    const proofDir = join(root, "proofs")
    await mkdir(worktree, { recursive: true })
    await mkdir(pylonHome, { recursive: true })
    await writeFile(join(worktree, "README.md"), "# Local Fixture\n\nsecret fixture body\n", "utf8")
    await writeFile(join(worktree, "src.ts"), "export const localValue = 42\n", "utf8")
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json", "--pylon-ref", "pylon.test.apple-fm-session"]), {
      PYLON_HOME: pylonHome,
    })
    return await fn({ pylonHome, proofDir, summary, worktree })
  } finally {
    await rm(root, { recursive: true, force: true })
  }
}

function fakeReadyBridge(input: {
  toolName?: string
  toolInput?: Record<string, unknown>
} = {}): Bun.Server {
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
        const body = await request.json() as {
          tool_callback?: { url?: string; session_token?: string }
          tools?: Array<{ name?: string }>
        }
        callbackUrl = body.tool_callback?.url ?? ""
        callbackToken = body.tool_callback?.session_token ?? ""
        expect(JSON.stringify(body)).not.toContain("Bearer ")
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
            tool_name: input.toolName ?? "read_file",
            arguments: {
              generation_id: "tool-call-1",
              content: input.toolInput ?? { path: "README.md" },
              is_complete: true,
            },
          }),
        })
        const callbackBody = await callback.json() as { output?: string; underlying_error?: string }
        const output = callback.ok
          ? `local answer after tool: ${callbackBody.output?.slice(0, 24) ?? "ok"}`
          : `local answer after refused tool: ${callbackBody.underlying_error ?? callback.status}`
        return new Response(
          [
            "event: snapshot",
            "data: {\"kind\":\"snapshot\",\"model\":\"apple-foundation-model\",\"output\":\"working locally\"}",
            "",
            "event: completed",
            `data: ${JSON.stringify({
              kind: "completed",
              model: "apple-foundation-model",
              output,
              usage: { total_tokens_detail: { value: 11, truth: "estimated" } },
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

function fakeNotReadyBridge(): Bun.Server {
  const server = Bun.serve({
    port: 0,
    fetch: (request) => {
      const url = new URL(request.url)
      if (url.pathname === "/health") {
        return Response.json({
          ready: false,
          unavailableReason: "apple_intelligence_disabled",
          message: "Apple Intelligence is disabled",
        })
      }
      return Response.json({ error: "not found" }, { status: 404 })
    },
  })
  servers.push(server)
  return server
}

async function waitForTerminal(
  actions: ReturnType<typeof createControlSessionActions>,
  sessionRef: string,
) {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const list = await actions.list()
    const row = list.find((entry) => entry.sessionRef === sessionRef)
    if (row?.state === "completed" || row?.state === "failed") return row
    await Bun.sleep(20)
  }
  throw new Error("session did not reach a terminal state")
}

describe("Apple FM local control sessions", () => {
  test("fake bridge session uses a read-only workspace tool and completes as a normal session", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const bridge = fakeReadyBridge()
      const actions = createControlSessionActions({
        env: { PROBE_APPLE_FM_BASE_URL: String(bridge.url) },
        proofsDir: proofDir,
        summary,
      })

      const started = await actions.startAppleFm({
        type: "apple_fm.session.start",
        prompt: "Use read_file on README.md and summarize the local fixture.",
        worktreePath: worktree,
      })
      expect(started.ok).toBe(true)
      if (!started.ok) throw new Error(started.error)

      const row = await waitForTerminal(actions, started.sessionRef)
      expect(row).toMatchObject({
        adapter: "apple_fm",
        lane: "local",
        state: "completed",
        cloudRunner: null,
        resourceUsageReceiptRef: null,
      })

      const events = await actions.events(started.sessionRef)
      const text = JSON.stringify(events.recentEvents)
      expect(text).toContain("Apple FM local backend ready")
      expect(text).toContain("Apple FM tool read_file: success")
      expect(text).not.toContain("Use read_file on README.md")
      expect(text).not.toContain("secret fixture body")
      expect(text).not.toContain("session_token")
      expect(text).not.toContain("tool-callback")

      const artifact = await readFile(join(proofDir, `${started.sessionRef}-proof.json`), "utf8")
      expect(artifact).toContain('"executionPathRef": "control_session.apple_fm_local"')
      expect(artifact).toContain('"externalSessionRef": "session.pylon.apple_fm_bridge.')
      expect(artifact).toContain('"evidenceState": "modeled"')
      expect(artifact).toContain('"methodRef": "method.apple_fm.power.modeled_default_kw_wall_clock"')
      expect(artifact).toContain('"caveat.apple_fm.power.modeled_not_measured"')
      expect(artifact).not.toContain(fakeBridgeSessionId)
      expect(artifact).not.toContain("Use read_file on README.md")
      expect(artifact).not.toContain("secret fixture body")
      expect(artifact).not.toContain("tool-callback")

      const proof = JSON.parse(artifact) as {
        executor?: {
          energyEstimate?: {
            evidenceState?: string
            modeledPowerKw?: number
            measuredPowerKw?: number
            energyKwh?: number | null
            caveatRefs?: string[]
          }
        }
      }
      expect(proof.executor?.energyEstimate).toMatchObject({
        evidenceState: "modeled",
        modeledPowerKw: DEFAULT_APPLE_FM_MODELED_POWER_KW,
      })
      expect(proof.executor?.energyEstimate?.measuredPowerKw).toBeUndefined()
      expect(typeof proof.executor?.energyEstimate?.energyKwh).toBe("number")
      expect(proof.executor?.energyEstimate?.caveatRefs).toContain(
        "caveat.apple_fm.power.not_ao_kwh_without_accepted_outcome",
      )
    })
  })

  test("energy estimate models configured kW from a bounded session window", () => {
    const estimate = estimateAppleFmLocalSessionEnergy({
      env: { OPENAGENTS_APPLE_FM_MODELED_POWER_KW: "0.04" },
      startedAt: "2026-06-15T00:00:00.000Z",
      completedAt: "2026-06-15T00:00:30.000Z",
    })

    expect(estimate).toMatchObject({
      evidenceState: "modeled",
      methodRef: "method.apple_fm.power.modeled_configured_kw_wall_clock",
      modeledPowerKw: 0.04,
      wallClockSeconds: 30,
      wallClockHours: 0.008333333,
      energyKwh: 0.000333333,
    })
    expect("measuredPowerKw" in estimate).toBe(false)
    expect(estimate.caveatRefs).toContain("caveat.apple_fm.power.modeled_not_measured")
  })

  test("energy estimate is unavailable when the model is disabled", () => {
    const estimate = estimateAppleFmLocalSessionEnergy({
      env: { OPENAGENTS_APPLE_FM_POWER_ESTIMATE_MODE: "disabled" },
      startedAt: "2026-06-15T00:00:00.000Z",
      completedAt: "2026-06-15T00:00:30.000Z",
    })

    expect(estimate).toMatchObject({
      evidenceState: "unavailable",
      methodRef: "method.apple_fm.power.unavailable",
      wallClockSeconds: 30,
      energyKwh: null,
      blockerRefs: ["blocker.apple_fm.energy.estimate_disabled"],
    })
    expect("modeledPowerKw" in estimate).toBe(false)
    expect("measuredPowerKw" in estimate).toBe(false)
  })

  test("not-ready Apple FM refuses before a session is created", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const bridge = fakeNotReadyBridge()
      const actions = createControlSessionActions({
        env: { PROBE_APPLE_FM_BASE_URL: String(bridge.url) },
        proofsDir: proofDir,
        summary,
      })
      const started = await actions.startAppleFm({
        type: "apple_fm.session.start",
        prompt: "try local Apple FM",
        worktreePath: worktree,
      })
      expect(started).toMatchObject({
        ok: false,
        sessionRef: "",
      })
      if (started.ok) throw new Error("expected refusal")
      expect(started.blockerRefs).toContain("blocker.pylon.apple_fm.apple_intelligence_disabled")
      expect(await actions.list()).toEqual([])
    })
  })

  test("unsupported tool callback is retained as a redacted refusal event", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const bridge = fakeReadyBridge({
        toolName: "shell",
        toolInput: { cmd: "cat README.md" },
      })
      const actions = createControlSessionActions({
        env: { PROBE_APPLE_FM_BASE_URL: String(bridge.url) },
        proofsDir: proofDir,
        summary,
      })
      const started = await actions.startAppleFm({
        type: "apple_fm.session.start",
        prompt: "Try to use an unsupported tool.",
        worktreePath: worktree,
      })
      expect(started.ok).toBe(true)
      if (!started.ok) throw new Error(started.error)

      await waitForTerminal(actions, started.sessionRef)
      const events = await actions.events(started.sessionRef)
      const text = JSON.stringify(events.recentEvents)
      expect(text).toContain("Apple FM tool shell: unknown_tool")
      expect(text).not.toContain("cat README.md")
    })
  })

  test("workspace tools refuse symlink escapes", async () => {
    await withFixture(async ({ pylonHome, worktree }) => {
      const outsidePath = join(pylonHome, "outside-secret.txt")
      const symlinkPath = join(worktree, "outside-secret.txt")
      await writeFile(outsidePath, "outside secret body", "utf8")
      await symlink(outsidePath, symlinkPath)

      const tool = makeAppleFmWorkspaceTools({ cwd: worktree }).find(
        (entry) => entry.name === "read_file",
      )
      expect(tool).toBeDefined()
      if (tool === undefined) throw new Error("read_file tool missing")

      const output = await Effect.runPromise(tool.execute({ path: "outside-secret.txt" }))
      expect(output).toMatchObject({
        ok: false,
        status: "refused",
        blockerRefs: ["blocker.pylon.apple_fm.tool.symlink_refused"],
      })
      expect(JSON.stringify(output)).not.toContain("outside secret body")
    })
  })
})
