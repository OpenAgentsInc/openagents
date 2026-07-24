import { createHash } from "node:crypto"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import { resolveAgentComputerSessionsPath } from "../paths.ts"
import { createOmegaEffectdService } from "../service.ts"
import { OMEGA_EFFECTD_PROTOCOL_SCHEMA } from "./framed.ts"
import { createOmegaEffectdFramedServer } from "./server.ts"

const withRoot = async (fn: (root: string) => Promise<void>): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-effectd-ac01-"))
  try {
    await fn(root)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

const request = (id: string, generation: number, method: string, params?: unknown) =>
  JSON.stringify({
    schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
    kind: "request",
    id,
    generation,
    method,
    ...(params === undefined ? {} : { params }),
  })

const sessionProjection = (input: {
  readonly id: string
  readonly state: "queued" | "running" | "completed" | "failed" | "cancelled"
  readonly artifact_ref?: string | null
}) => ({
  object: "cloud.coding_session",
  product_object: "agent.computer_session",
  id: input.id,
  lane: "cloud-gcp",
  adapter: "codex",
  repo_ref: "OpenAgentsInc/openagents",
  state: input.state,
  placement_ref: "placement.cloud-coding.ac01",
  lease_refs: ["lease.ac01.01"],
  agent_computer_ref: "agentcomputer.ac01.01",
  agent_computer_state: input.state === "completed" ? "reclaimed" : "active",
  lifecycle_receipt_refs: [],
  resource_usage_receipt_refs: [],
  artifact_ref: input.artifact_ref ?? null,
  created_at: "2026-07-24T00:00:00.000Z",
})

const jsonResponse = (status: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  })

describe("omega-effectd Agent Computer runner (AC-01)", () => {
  test("start, refresh, list, and durable store never keep bearer or objective", async () => {
    await withRoot(async root => {
      let getCount = 0
      const fetchImpl: typeof fetch = async (_input, init) => {
        const method = init?.method ?? "GET"
        if (method === "POST") {
          const body = JSON.parse(String(init?.body ?? "{}")) as {
            repoRef?: string
            objective?: string
          }
          expect(body.repoRef).toBe("OpenAgentsInc/openagents")
          expect(body.objective).toBe("Prove AC-01 launch")
          return jsonResponse(200, sessionProjection({ id: "ccs_ac01_start", state: "queued" }))
        }
        getCount += 1
        if (getCount === 1) {
          return jsonResponse(200, sessionProjection({ id: "ccs_ac01_start", state: "running" }))
        }
        return jsonResponse(
          200,
          sessionProjection({
            id: "ccs_ac01_start",
            state: "completed",
            artifact_ref: "artifact.ac01.01",
          }),
        )
      }

      const service = createOmegaEffectdService({ paths: { dataRoot: root } })
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        { agentComputerFetch: fetchImpl, agentComputerSleep: () => Effect.void },
      )
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }))

      const started = await server.handleLine(
        request("2", 1, "start_agent_computer_session", {
          bearerToken: "secret-bearer-token-ac01",
          controlPlaneBaseUrl: "https://openagents.com",
          repoRef: "OpenAgentsInc/openagents",
          objective: "Prove AC-01 launch",
          adapter: "codex",
          lane: "cloud-gcp",
        }),
      )
      expect(started?.ok).toBe(true)
      const session = (
        started?.result as {
          session: {
            sessionRef: string
            environment: string
            objectiveDigest: string
            state: string
          }
        }
      ).session
      expect(session.sessionRef).toBe("ccs_ac01_start")
      expect(session.environment).toBe("openagents_cloud")
      expect(session.state).toBe("queued")
      expect(session.objectiveDigest).toBe(
        createHash("sha256").update("Prove AC-01 launch").digest("hex"),
      )
      expect(JSON.stringify(started)).not.toContain("secret-bearer-token-ac01")
      expect(JSON.stringify(started)).not.toContain("Prove AC-01 launch")

      const refreshed = await server.handleLine(
        request("3", 1, "refresh_agent_computer_session", {
          bearerToken: "secret-bearer-token-ac01",
          sessionRef: session.sessionRef,
        }),
      )
      expect(refreshed?.ok).toBe(true)
      expect((refreshed?.result as { session: { state: string } }).session.state).toBe("running")

      const listed = await server.handleLine(request("4", 1, "list_agent_computer_sessions"))
      expect(listed?.ok).toBe(true)
      const sessions = (listed?.result as { sessions: Array<{ sessionRef: string }> }).sessions
      expect(sessions).toHaveLength(1)
      expect(sessions[0]?.sessionRef).toBe("ccs_ac01_start")

      const disk = readFileSync(resolveAgentComputerSessionsPath({ dataRoot: root }), "utf8")
      expect(disk).toContain("ccs_ac01_start")
      expect(disk).not.toContain("secret-bearer-token-ac01")
      expect(disk).not.toContain("Prove AC-01 launch")
    })
  })

  test("run_agent_computer_turn uses openagents_cloud runner and projects terminal state", async () => {
    await withRoot(async root => {
      let getCount = 0
      const fetchImpl: typeof fetch = async (_input, init) => {
        const method = init?.method ?? "GET"
        if (method === "POST") {
          return jsonResponse(
            200,
            sessionProjection({
              id: "ccs_ac01_turn",
              state: "completed",
              artifact_ref: "artifact.ac01.turn",
            }),
          )
        }
        getCount += 1
        return jsonResponse(502, { error: "lifecycle_store_unavailable" })
      }

      const service = createOmegaEffectdService({ paths: { dataRoot: root } })
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        { agentComputerFetch: fetchImpl, agentComputerSleep: () => Effect.void },
      )
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }))

      const turn = await server.handleLine(
        request("2", 1, "run_agent_computer_turn", {
          bearerToken: "secret-bearer-token-ac01-turn",
          controlPlaneBaseUrl: "https://openagents.com",
          repoRef: "OpenAgentsInc/openagents",
          objective: "Observe one Agent Computer turn",
        }),
      )
      expect(turn?.ok).toBe(true)
      const result = turn?.result as {
        session: { sessionRef: string; state: string; artifactRef: string | null }
        finishReason: string
        eventKinds: string[]
      }
      expect(result.session.sessionRef).toBe("ccs_ac01_turn")
      expect(result.session.state).toBe("completed")
      expect(result.session.artifactRef).toBe("artifact.ac01.turn")
      expect(result.finishReason).toBe("stop")
      expect(result.eventKinds[0]).toBe("turn.started")
      expect(result.eventKinds.at(-1)).toBe("turn.finished")
      expect(getCount).toBe(0)
      expect(JSON.stringify(turn)).not.toContain("secret-bearer-token-ac01-turn")
      expect(JSON.stringify(turn)).not.toContain("Observe one Agent Computer turn")

      const got = await server.handleLine(
        request("3", 1, "get_agent_computer_session", { sessionRef: "ccs_ac01_turn" }),
      )
      expect((got?.result as { session: { state: string } }).session.state).toBe("completed")
    })
  })

  test("initialize advertises Agent Computer capabilities", async () => {
    await withRoot(async root => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } })
      const server = createOmegaEffectdFramedServer(service, { dataRoot: root })
      const init = await server.handleLine(request("1", 0, "initialize", { generation: 3 }))
      const caps = (init?.result as { capabilities: string[] }).capabilities
      expect(caps).toContain("start_agent_computer_session")
      expect(caps).toContain("refresh_agent_computer_session")
      expect(caps).toContain("run_agent_computer_turn")
      expect(caps).toContain("get_agent_computer_session")
      expect(caps).toContain("list_agent_computer_sessions")
    })
  })
})
