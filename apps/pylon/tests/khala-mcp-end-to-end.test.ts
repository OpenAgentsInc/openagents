import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { MemoryStreamStore } from "@openagentsinc/durable-stream"
import { afterEach, describe, expect, test } from "bun:test"

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { CODEX_AGENT_CAPABILITY_REF } from "../src/codex-agent"
import { CODEX_AGENT_SDK_PACKAGE } from "../src/codex-agent"
import { runNoSpendAssignment } from "../src/assignment"
import {
  handlePylonKhalaMcpRequest,
  pylonKhalaMcpConfig,
} from "../src/khala-mcp"
import { sendHeartbeat } from "../src/presence"
import { assertPublicProjectionSafe, ensurePylonLocalState } from "../src/state"

import type {
  AgentRegistrationStore,
  ProgrammaticAgentSession,
} from "../../openagents.com/workers/api/src/agent-registration"
import {
  khalaDurableRequestIsLinkedToPrincipal,
  khalaMcpAgentPrincipal,
  makeKhalaMcpCatalog,
} from "../../openagents.com/workers/api/src/khala-mcp"
import {
  routeDurableInferenceReadRequest,
} from "../../openagents.com/workers/api/src/inference/durable-inference-read-routes"
import {
  seedDurableInferenceStream,
} from "../../openagents.com/workers/api/src/inference/durable-inference-proxy"
import type {
  PylonApiAssignmentRecord,
  PylonApiEventRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from "../../openagents.com/workers/api/src/pylon-api"
import {
  publicPylonApiAssignmentProjection,
} from "../../openagents.com/workers/api/src/pylon-api"

const NOW_ISO = "2026-06-25T12:00:00.000Z"
const NOW_MS = Date.parse(NOW_ISO)
const OWNER_TOKEN = "oa_agent_owner_fixture"
const OTHER_TOKEN = "oa_agent_other_fixture"

const servers: ReturnType<typeof Bun.serve>[] = []

afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true)
})

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-khala-mcp-e2e-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

async function readySummary(home: string) {
  const summary = createBootstrapSummary(
    parseBootstrapArgs([
      "--display-name",
      "Khala MCP E2E",
      "--capability-ref",
      CODEX_AGENT_CAPABILITY_REF,
    ]),
    { PYLON_HOME: home },
    "darwin",
  )
  const state = await ensurePylonLocalState(summary)
  await writeFile(
    state.paths.runtimeState,
    `${JSON.stringify({
      blockerRefs: [],
      capabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
      displayName: "Khala MCP E2E",
      lifecycle: "assignment-ready",
      resourceMode: "background_20",
      updatedAt: NOW_ISO,
    })}\n`,
  )
  return summary
}

const session = (
  agentUserId: string,
  openauthUserId: string,
  tokenPrefix: string,
): ProgrammaticAgentSession => ({
  credential: {
    id: `credential_${agentUserId}`,
    lastUsedAt: NOW_ISO,
    openauthUserId,
    profileMetadataJson: "{}",
    tokenPrefix,
  },
  user: {
    avatarUrl: null,
    createdAt: NOW_ISO,
    displayName: agentUserId,
    id: agentUserId,
    kind: "agent",
    primaryEmail: null,
    status: "active",
    updatedAt: NOW_ISO,
  },
})

const ownerSession = session("agent_owner", "user_owner", "oa_agent_owner")
const otherSession = session("agent_other", "user_other", "oa_agent_other")

const bearerToken = (request: Request): string | undefined => {
  const authorization = request.headers.get("authorization")
  if (authorization === null) return undefined
  const [scheme, token] = authorization.split(" ")
  return scheme?.toLowerCase() === "bearer" ? token : undefined
}

const sessionForToken = (
  token: string | undefined,
): ProgrammaticAgentSession | undefined => {
  if (token === OWNER_TOKEN) return ownerSession
  if (token === OTHER_TOKEN) return otherSession
  return undefined
}

const makeAgentStore = (): AgentRegistrationStore => ({
  createAgentRegistration: async () => {},
  findAgentByTokenHash: async () => undefined,
  listLinkedAgentsForOpenAuthUser: async openauthUserId =>
    openauthUserId === "user_owner"
      ? [
          {
            agentUserId: "agent_owner",
            credentialId: "credential_agent_owner",
            displayName: "Owner Agent",
            linkKind: "credential_anchor",
            openauthUserId: "user_owner",
            tokenPrefix: "oa_agent_owner",
          },
        ]
      : openauthUserId === "user_other"
        ? [
            {
              agentUserId: "agent_other",
              credentialId: "credential_agent_other",
              displayName: "Other Agent",
              linkKind: "credential_anchor",
              openauthUserId: "user_other",
              tokenPrefix: "oa_agent_other",
            },
          ]
        : [],
  touchAgentCredential: async () => {},
  updateAgentDisplayName: async () => 1,
})

const registration = (
  pylonRef: string,
  overrides: Partial<PylonApiRegistrationRecord> = {},
): PylonApiRegistrationRecord => ({
  capabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
  clientProtocolVersion: "0.3.0",
  clientVersion: "0.3.0",
  createdAt: NOW_ISO,
  displayName: "Linked Codex Pylon",
  id: "pylon_api_registration_owner",
  latestCapacityRefs: [
    "capacity.coding.codex.ready=1",
    "capacity.coding.codex.available=1",
  ],
  latestHeartbeatAt: NOW_ISO,
  latestHeartbeatStatus: "online",
  latestHealthRefs: ["health.public.pylon_cli.ok"],
  latestLoadRefs: ["load.coding.codex.busy=0", "load.coding.codex.queued=0"],
  latestResourceMode: "background_20",
  ownerAgentCredentialId: "credential_agent_owner",
  ownerAgentTokenPrefix: "oa_agent_owner",
  ownerAgentUserId: "agent_owner",
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: "{}",
  pylonRef,
  resourceMode: "background_20",
  status: "active",
  updatedAt: NOW_ISO,
  walletReady: true,
  walletRef: null,
  ...overrides,
})

type HarnessPylonStore = PylonApiStore & {
  assignments: () => PylonApiAssignmentRecord[]
}

const makePylonStore = (
  registrations: ReadonlyArray<PylonApiRegistrationRecord>,
): HarnessPylonStore => {
  const assignments: PylonApiAssignmentRecord[] = []
  const replaceAssignment = (record: PylonApiAssignmentRecord) => {
    const index = assignments.findIndex(item => item.id === record.id)
    if (index < 0) assignments.push(record)
    else assignments[index] = record
    return record
  }
  return {
    assignments: () => assignments,
    createAssignment: async record => {
      assignments.push(record)
      return { idempotent: false, record }
    },
    createEvent: async () => {
      throw new Error("not used")
    },
    listAssignmentsForPylon: async pylonRef =>
      assignments.filter(item => item.pylonRef === pylonRef),
    listAssignmentsForPylons: async pylonRefs =>
      assignments.filter(item => pylonRefs.includes(item.pylonRef)),
    listEventsForAssignment: async () => [],
    listEventsForPylon: async (): Promise<ReadonlyArray<PylonApiEventRecord>> =>
      [],
    listProviderJobLifecycleForPylons: async () => [],
    listRegistrations: async () => registrations,
    listRegistrationsForOwnerAgentUserIds: async ownerAgentUserIds =>
      registrations.filter(item =>
        ownerAgentUserIds.includes(item.ownerAgentUserId),
      ),
    readAssignment: async assignmentRef =>
      assignments.find(item => item.assignmentRef === assignmentRef),
    readAssignmentByIdempotencyKeyHash: async hash =>
      assignments.find(item => item.idempotencyKeyHash === hash),
    readEventByIdempotencyKeyHash: async () => undefined,
    readRegistration: async pylonRef =>
      registrations.find(item => item.pylonRef === pylonRef),
    updateAssignment: async record => replaceAssignment(record),
    upsertProviderJobLifecycle: async record => record,
    upsertRegistration: async record => record,
  }
}

const json = (payload: unknown, init: ResponseInit = {}) =>
  Response.json(payload, {
    headers: { "cache-control": "no-store", ...init.headers },
    status: init.status,
  })

const makeOpenAgentsFixtureServer = (input: {
  agentStore: AgentRegistrationStore
  durableStores: Map<string, MemoryStreamStore>
  ids?: string[]
  pylonStore: HarnessPylonStore
}) => {
  const requests: Array<{ body: unknown; path: string }> = []
  let durableReplayCount = 0
  const idQueue = [...(input.ids ?? ["chatcmpl_e2e", "assignment_e2e"])]
  const catalog = makeKhalaMcpCatalog({
    agentStore: () => input.agentStore,
    makeId: () => idQueue.shift() ?? "id_more",
    nowIso: () => NOW_ISO,
    pylonStore: () => input.pylonStore,
    recordTokensServed: () => async () => {},
  })
  const storeFor = (requestId: string) => {
    let store = input.durableStores.get(requestId)
    if (store === undefined) {
      store = new MemoryStreamStore()
      input.durableStores.set(requestId, store)
    }
    return store
  }
  const requireSession = (request: Request) => sessionForToken(bearerToken(request))

  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url)
      const text = await request.text()
      const body = text.trim() === "" ? {} : JSON.parse(text)
      requests.push({ body, path: url.pathname })

      if (url.pathname === "/api/mcp" && request.method === "POST") {
        const session = requireSession(request)
        if (session === undefined) return json({ error: "unauthorized" }, { status: 401 })
        const rpc = body as {
          id?: string | number | null
          method?: unknown
          params?: { arguments?: unknown; name?: unknown }
        }
        if (rpc.method !== "tools/call" || typeof rpc.params?.name !== "string") {
          return json({
            error: {
              code: -32601,
              message: "unsupported MCP fixture method",
            },
            id: rpc.id ?? null,
            jsonrpc: "2.0",
          })
        }
        const outcome = await catalog.callTool(
          {},
          request,
          khalaMcpAgentPrincipal(session, NOW_ISO),
          rpc.params.name,
          rpc.params.arguments,
        )
        return json({
          id: rpc.id ?? null,
          jsonrpc: "2.0",
          result: outcome,
        })
      }

      if (url.pathname === "/v1/chat/completions" && request.method === "POST") {
        const session = requireSession(request)
        if (session === undefined) return json({ error: "unauthorized" }, { status: 401 })
        const prompt = (body as { messages?: Array<{ content?: unknown }> })
          .messages?.[0]?.content
        const openagents = (body as { openagents?: unknown }).openagents as
          | Record<string, unknown>
          | undefined
        const coding = openagents?.coding as Record<string, unknown> | undefined
        const workflow = openagents?.workflowClass
        const outcome = await catalog.callTool(
          {},
          request,
          khalaMcpAgentPrincipal(session, NOW_ISO),
          "khala.request",
          {
            prompt,
            targetPylonRef: coding?.targetPylonRef,
            workflow,
          },
        )
        const payload = outcome.structuredContent as Record<string, unknown>
        if (outcome.isError) {
          return json(payload, {
            status:
              typeof payload.statusCode === "number" ? payload.statusCode : 400,
          })
        }
        return new Response(
          `data: ${JSON.stringify({
            choices: [
              {
                delta: { content: "Khala assignment accepted." },
                index: 0,
              },
            ],
          })}\n\n`,
          {
            headers: {
              "cache-control": "no-store",
              "content-type": "text/event-stream",
              "openagents-coding-assignment-ref": String(payload.assignmentRef),
              "openagents-durable-stream-url": String(payload.durableStreamUrl),
              "stream-next-offset": "0",
              "stream-up-to-date": "true",
            },
          },
        )
      }

      if (
        url.pathname.startsWith("/v1/chat/completions/durable/") &&
        request.method === "GET"
      ) {
        const durableRequestId = decodeURIComponent(
          url.pathname.slice("/v1/chat/completions/durable/".length),
        )
        const assignment =
          await input.pylonStore.readAssignmentByIdempotencyKeyHash(
            `khala-coding:${durableRequestId}`,
          )
        if (assignment !== undefined) {
          const session = requireSession(request)
          if (session === undefined) {
            return json({ error: "unauthorized" }, { status: 401 })
          }
          const authorized = await khalaDurableRequestIsLinkedToPrincipal({
            agentStore: input.agentStore,
            durableRequestId,
            principal: khalaMcpAgentPrincipal(session, NOW_ISO),
            pylonStore: input.pylonStore,
          })
          if (!authorized) {
            return json(
              {
                error: "durable_request_not_authorized",
                reason:
                  "The durable Khala stream is outside this caller-owned linked Pylon set.",
              },
              { status: 403 },
            )
          }
        }
        durableReplayCount += 1
        return (
          routeDurableInferenceReadRequest(request, {
            durableStream: requestId => input.durableStores.get(requestId),
            enabled: true,
            nowEpochMillis: () => NOW_MS,
          }) ?? json({ error: "not_found" }, { status: 404 })
        )
      }

      if (url.pathname.includes("/heartbeat")) {
        return json({ heartbeatRef: `heartbeat.${(body as { pylonRef?: string }).pylonRef}.1` })
      }

      if (url.pathname.endsWith("/assignments") && request.method === "GET") {
        const pylonRef = decodeURIComponent(url.pathname.split("/").at(-2) ?? "")
        return json({
          assignments: input.pylonStore
            .assignments()
            .filter(assignment => assignment.pylonRef === pylonRef)
            .map(assignment =>
              publicPylonApiAssignmentProjection(assignment, NOW_ISO),
            ),
          schema: "openagents.pylon.assignment_poll_response.v0.3",
        })
      }

      if (url.pathname.endsWith("/accept")) {
        return json({ statusRef: `assignment.accepted.${url.pathname.split("/").at(-2)}` })
      }

      if (url.pathname.endsWith("/progress")) {
        return json({ progressRef: `assignment.progress.${(body as { leaseRef?: string }).leaseRef}.1` })
      }

      if (url.pathname.endsWith("/artifacts")) {
        expect((body as { artifactRefs?: unknown[] }).artifactRefs?.length).toBeGreaterThan(0)
        expect((body as { proofRefs?: unknown[] }).proofRefs?.length).toBeGreaterThan(0)
        return json({ artifactRef: `assignment.artifacts.${url.pathname.split("/").at(-2)}` })
      }

      if (url.pathname.endsWith("/closeout")) {
        const closeout = body as {
          leaseRef: string
          resultRefs: string[]
          status: string
        }
        expect(closeout.status).toBe("accepted")
        expect(closeout.resultRefs).toContain(
          "result.public.pylon.codex_agent_task.fixture_repair_passed",
        )
        const assignment = input.pylonStore
          .assignments()
          .find(item => item.assignmentRef === closeout.leaseRef)
        expect(assignment).toBeDefined()
        const durableRequestRef = assignment?.taskRefs.find(ref =>
          ref.startsWith("request.public.khala_coding."),
        )
        const durableRequestId = durableRequestRef?.replace(
          "request.public.khala_coding.",
          "",
        )
        expect(durableRequestId).toBe("chatcmpl_e2e")
        const store = storeFor("chatcmpl_e2e")
        seedDurableInferenceStream({
          close: true,
          frames: [
            `data: ${JSON.stringify({
              choices: [
                {
                  delta: {
                    content:
                      "Pylon closeout: result.public.pylon.codex_agent_task.fixture_repair_passed",
                  },
                  index: 0,
                },
              ],
            })}\n\n`,
            "data: [DONE]\n\n",
          ],
          nowMs: NOW_MS,
          requestId: "chatcmpl_e2e",
          store,
        })
        return json({ closeoutRef: `assignment.closeout.${closeout.leaseRef}` })
      }

      return json({ error: "not_found" }, { status: 404 })
    },
  })
  servers.push(server)
  return {
    baseUrl: `http://127.0.0.1:${server.port}`,
    durableReplayCount: () => durableReplayCount,
    requests,
  }
}

const callToolText = async (
  input: Readonly<{
    args: Record<string, unknown>
    baseUrl: string
    id: string
    name: string
    token: string
  }>,
) => {
  const response = await handlePylonKhalaMcpRequest(
    {
      id: input.id,
      jsonrpc: "2.0",
      method: "tools/call",
      params: {
        arguments: input.args,
        name: input.name,
      },
    },
    {
      network: {
        agentToken: input.token,
        baseUrl: input.baseUrl,
      },
    },
  )
  const result = response.result as
    | {
        content?: Array<{ text?: string }>
        isError?: boolean
      }
    | undefined
  return {
    isError: result?.isError === true,
    text: result?.content?.[0]?.text ?? "",
  }
}

describe("Khala MCP end-to-end smoke", () => {
  test("bare MCP call issues, Pylon runs, durable resume replays, and a second account is denied", async () => {
    await withTempHome(async home => {
      const summary = await readySummary(home)
      const state = await ensurePylonLocalState(summary)
      const agentStore = makeAgentStore()
      const pylonStore = makePylonStore([registration(state.identity.pylonRef)])
      const durableStores = new Map<string, MemoryStreamStore>()
      const fixture = makeOpenAgentsFixtureServer({
        agentStore,
        durableStores,
        pylonStore,
      })

      const config = pylonKhalaMcpConfig({
        baseUrl: fixture.baseUrl,
        command: "pylon",
      })
      expect(config.mcpServers["openagents-khala-local"].args).toEqual(["mcp"])
      expect(config.mcpServers["openagents-khala-remote"].url).toBe(
        `${fixture.baseUrl}/api/mcp`,
      )

      const issuedCall = await callToolText({
        args: {
          prompt: "Repair the fixture through my linked Pylon.",
          targetPylonRef: state.identity.pylonRef,
          workflow: "codex_agent_task",
        },
        baseUrl: fixture.baseUrl,
        id: "issue",
        name: "khala.request",
        token: OWNER_TOKEN,
      })
      expect(issuedCall.isError).toBe(false)
      const issued = JSON.parse(issuedCall.text) as {
        assignmentRef: string | null
        durableRequestId: string | null
        durableStreamUrl: string | null
        ok: true
      }
      expect(issued.ok).toBe(true)
      expect(issued.assignmentRef).toBe(
        "assignment.public.khala_coding.assignment_e2e",
      )
      expect(issued.durableRequestId).toBe("chatcmpl_e2e")
      expect(issued.durableStreamUrl).toBe(
        "/v1/chat/completions/durable/chatcmpl_e2e",
      )
      expect(pylonStore.assignments()).toHaveLength(1)
      expect(pylonStore.assignments()[0]?.taskRefs).toContain(
        "request.public.khala_coding.chatcmpl_e2e",
      )

      await sendHeartbeat(summary, {
        agentToken: OWNER_TOKEN,
        baseUrl: fixture.baseUrl,
        now: () => new Date(NOW_ISO),
      })

      const run = await runNoSpendAssignment(summary, {
        agentToken: OWNER_TOKEN,
        baseUrl: fixture.baseUrl,
        codexAgentProbe: {
          codexCliLoginPresent: false,
          env: { CODEX_API_KEY: "test-key-shape" },
          importer: async specifier => {
            if (specifier !== CODEX_AGENT_SDK_PACKAGE) {
              throw new Error(`unexpected import: ${specifier}`)
            }
            return {}
          },
          platform: "darwin",
        },
        codexAuthValidityProbe: async () => ({ valid: true }),
        codexAgentRunner: async input => {
          expect(input.cwd).toContain("codex-agent-tasks")
          expect(input.instructions).toContain("bounded fixture workspace")
          await writeFile(
            join(input.cwd, "sum.ts"),
            "export const sum = (left: number, right: number) => left + right\n",
          )
          return {
            commandCount: 1,
            editedFileCount: 1,
            outcome: "completed",
            sessionRef: "session.pylon.codex_agent.fixture",
            turnCount: 2,
          }
        },
        now: () => new Date(NOW_ISO),
      })
      expect(run.ok).toBe(true)
      if (!run.ok) throw new Error("expected no-spend Khala assignment to run")
      expect(run.closeout.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.fixture_repair_passed",
      )
      assertPublicProjectionSafe(run.closeout)

      const resumedCall = await callToolText({
        args: { durableRequestId: "chatcmpl_e2e", offset: 0 },
        baseUrl: fixture.baseUrl,
        id: "resume-owner",
        name: "khala.resume",
        token: OWNER_TOKEN,
      })
      expect(resumedCall.isError).toBe(false)
      const resumed = JSON.parse(resumedCall.text) as {
        durableRequestId: string | null
        ok: true
        streamClosed: boolean
        text: string
      }
      expect(resumed).toMatchObject({
        durableRequestId: "chatcmpl_e2e",
        ok: true,
        streamClosed: true,
      })
      expect(resumed.text).toContain(
        "result.public.pylon.codex_agent_task.fixture_repair_passed",
      )
      expect(fixture.durableReplayCount()).toBe(1)

      const deniedCall = await callToolText({
        args: { durableRequestId: "chatcmpl_e2e", offset: 0 },
        baseUrl: fixture.baseUrl,
        id: "resume-other",
        name: "khala.resume",
        token: OTHER_TOKEN,
      })
      expect(deniedCall.isError).toBe(true)
      expect(deniedCall.text).toContain("durable_request_not_authorized")
      expect(deniedCall.text).toContain("caller-owned linked Pylon assignment")
      expect(fixture.durableReplayCount()).toBe(1)
    })
  })

  test("local MCP khala.spawn creates a two-child fixture run and protects spawn status", async () => {
    await withTempHome(async home => {
      const summary = await readySummary(home)
      const state = await ensurePylonLocalState(summary)
      const agentStore = makeAgentStore()
      const pylonStore = makePylonStore([
        registration(state.identity.pylonRef, {
          latestCapacityRefs: [
            "capacity.coding.codex.ready=2",
            "capacity.coding.codex.available=2",
          ],
        }),
      ])
      const fixture = makeOpenAgentsFixtureServer({
        agentStore,
        durableStores: new Map<string, MemoryStreamStore>(),
        ids: [
          "spawn_e2e",
          "chatcmpl_spawn_one",
          "assignment_spawn_one",
          "record_one",
          "row_one",
          "chatcmpl_spawn_two",
          "assignment_spawn_two",
          "record_two",
          "row_two",
        ],
        pylonStore,
      })

      const spawnedCall = await callToolText({
        args: {
          count: 2,
          fixture: true,
          objective: "Run two fixture workers through my linked Pylon.",
          targetPylonRef: state.identity.pylonRef,
        },
        baseUrl: fixture.baseUrl,
        id: "spawn",
        name: "khala.spawn",
        token: OWNER_TOKEN,
      })
      expect(spawnedCall.isError).toBe(false)
      const spawned = JSON.parse(spawnedCall.text) as {
        assignedCount: number
        children: Array<{ assignmentRef: string; durableRequestId: string }>
        ok: true
        spawnRef: string
      }
      expect(spawned).toMatchObject({
        assignedCount: 2,
        ok: true,
        spawnRef: "spawn.public.khala_coding.spawn_e2e",
      })
      expect(spawned.children).toMatchObject([
        {
          assignmentRef: "assignment.public.khala_coding.assignment_spawn_one",
          durableRequestId: "chatcmpl_spawn_one",
        },
        {
          assignmentRef: "assignment.public.khala_coding.assignment_spawn_two",
          durableRequestId: "chatcmpl_spawn_two",
        },
      ])
      expect(pylonStore.assignments()).toHaveLength(2)
      expect(pylonStore.assignments()[0]?.taskRefs).toContain(
        "spawn.public.khala_coding.spawn_e2e",
      )

      const statusCall = await callToolText({
        args: { spawnRef: spawned.spawnRef },
        baseUrl: fixture.baseUrl,
        id: "spawn-status",
        name: "khala.spawnStatus",
        token: OWNER_TOKEN,
      })
      expect(statusCall.isError).toBe(false)
      const status = JSON.parse(statusCall.text) as {
        childCount: number
        children: Array<{ durableRequestId: string; state: string }>
        ok: true
      }
      expect(status).toMatchObject({
        childCount: 2,
        ok: true,
      })
      expect(JSON.stringify(status)).not.toContain("rawEvents")

      const deniedStatus = await callToolText({
        args: { spawnRef: spawned.spawnRef },
        baseUrl: fixture.baseUrl,
        id: "spawn-status-other",
        name: "khala.spawnStatus",
        token: OTHER_TOKEN,
      })
      expect(deniedStatus.isError).toBe(true)
      expect(deniedStatus.text).toContain("spawn_not_found_or_not_authorized")
    })
  })
})
