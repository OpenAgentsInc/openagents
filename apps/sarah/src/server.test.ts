import { afterEach, describe, expect, test } from "bun:test"
import { mkdir, readFile, rm, unlink, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { handleSarahRequest } from "./server.ts"
import { SARAH_OWNED_TOOL_INVENTORY } from "./agent-runtime/owned-runtime.ts"
import {
  __setSarahCodingFleetRunStoreForTest,
  createSarahCodingFleetFileStoreForTest,
  listSarahCodingFleetRunsForTest,
} from "./services/coding-fleet.ts"
import {
  __resetCustomerBlueprintForTest,
  __setCustomerBlueprintLatestDraftReaderForTest,
  __setCustomerBlueprintStoreReaderForTest,
  CUSTOMER_BLUEPRINT_SCHEMA,
} from "./services/customer-blueprint.ts"

function installCodingFleetFileStoreForTest(fleetFile: string) {
  const fleetPath = join(process.cwd(), ".sarah", fleetFile)
  __setSarahCodingFleetRunStoreForTest(
    createSarahCodingFleetFileStoreForTest(fleetFile),
  )
  return fleetPath
}

const naturalFleetArgs = {
  objective: "Run issue 8637 through the durable owner fleet.",
  repository: {
    owner: "OpenAgentsInc",
    name: "openagents",
    branch: "main",
    commit: "6af4e38282e4e71882fc5fdd86ae8adadab6df50",
  },
  verifier: { kind: "command", command: "bun test" },
  workSource: { kind: "issue_list", issueRefs: ["#8637"] },
  workerPolicy: { workerKind: "auto", targetPreference: "owner_local" },
  targetConcurrency: 2,
  idempotencyKey: "fc1-natural-operator-1",
}

const pendingFleetExecution = {
  state: "pending",
  lastSequence: 0,
  counters: {
    workUnitsTotal: 1,
    activeAssignments: 0,
    acceptedAssignments: 0,
    failedAssignments: 0,
    staleAssignments: 0,
  },
  startedAt: null,
  updatedAt: null,
  closeouts: [],
} as const

const naturalFleetSuccess = {
  ok: true,
  duplicate: false,
  policy: {
    source: "openagents_server_policy",
    relationshipMode: "operator",
    codingFleetStartAllowed: true,
    fleetObservationAllowed: true,
    retrievalScope: "owner_fleet_runs",
    responsePosture: "state_oriented",
    uiDensity: "dense",
    administratorToolsAllowed: false,
  },
  routeRef: "route.sarah.fleet_runs.authority.v1",
  run: {
    runRef: "fleet_run.sarah.aaaaaaaaaaaaaaaaaaaa",
    scope: "scope.fleet_run.fleet_run.sarah.aaaaaaaaaaaaaaaaaaaa",
    status: "pending_executor",
    objective: naturalFleetArgs.objective,
    repository: naturalFleetArgs.repository,
    verifier: naturalFleetArgs.verifier,
    workSource: naturalFleetArgs.workSource,
    workerPolicy: naturalFleetArgs.workerPolicy,
    targetConcurrency: 2,
    execution: pendingFleetExecution,
    createdAt: "2026-07-09T12:00:00.000Z",
    updatedAt: "2026-07-09T12:00:00.000Z",
    privateMaterialExcluded: true,
  },
}

describe("apps/sarah monorepo service", () => {
  afterEach(() => {
    __setSarahCodingFleetRunStoreForTest(null)
  })

  test("ops endpoint describes /sarah mount and rails", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/operator/ops"),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.mount).toBe("/sarah")
    expect(body.emailRail).toBe("crm_operator_rail")
    expect(body.agentRuntime).toBe("owned_effect_seed")
    expect(body.ui).toBe("effect_native_dom_zero_react")
  })

  test("prospect session mints cookie + thread", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/prospect/session", {
        method: "POST",
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.prospectRef).toBeTruthy()
    expect(body.threadId).toStartWith("prospect:")
    expect(res.headers.get("set-cookie") || "").toContain("sarah_prospect_ref")
  })

  test("owned runtime tool inventory matches SM-4 seed + KHS-9 ecosystem tools", () => {
    expect([...SARAH_OWNED_TOOL_INVENTORY].map(String).sort()).toEqual(
      [
        "checkout_link_create",
        "crm_activity_append",
        "crm_contact_upsert",
        "customer_blueprint_draft",
        "deal_rules_evaluate",
        "demo_sales_context",
        "human_handoff",
        "intake_capture",
        "live_stats",
        "plan_catalog",
        "promise_lookup",
        "coding_fleet_start",
      ].sort(),
    )
  })

  test("coding_fleet_start refuses unauthenticated prospect calls", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/eve/tool-call", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          toolName: "coding_fleet_start",
          toolCallId: "fc1a-unauth",
          args: {
            objective: "Run a bounded public issue through Sarah Fleet Command.",
          },
        }),
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.toolResults[0].ok).toBe(false)
    expect(body.toolResults[0].output.error.code).toBe("owner_auth_required")
  })

  test("coding_fleet_start ignores local-store env switches on the live route", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const savedFleetPath = process.env.SARAH_CODING_FLEET_RUNS_PATH
    const savedLocalStore = process.env.SARAH_CODING_FLEET_START_LOCAL_STORE
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    process.env.SARAH_CODING_FLEET_RUNS_PATH = "server-fc1a-env-ignored.json"
    process.env.SARAH_CODING_FLEET_START_LOCAL_STORE = "1"
    try {
      const res = await handleSarahRequest(
        new Request("http://localhost/sarah/api/eve/tool-call", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-sarah-test-oa-session": JSON.stringify({
              userId: "owner-fc1a",
              email: "owner@example.com",
            }),
          },
          body: JSON.stringify({
            toolName: "coding_fleet_start",
            args: {
              objective: "Run issue 8637 as a bounded FC-1 fixture.",
              repository: {
                owner: "OpenAgentsInc",
                name: "openagents",
                branch: "main",
                commit: "f8e4aa29d9",
              },
              verifier: { kind: "command", command: "bun run test:sarah" },
              workSource: { kind: "issue_list", issueRefs: ["#8637"] },
              workerPolicy: {
                workerKind: "auto",
                targetPreference: "owner_local",
              },
              targetConcurrency: 1,
              idempotencyKey: "fc1a-store-default-off",
            },
          }),
        }),
        {
          fleetAuthorityFetch: () =>
            Promise.resolve(
              new Response(
                JSON.stringify({
                  ok: false,
                  error: { code: "storage_unavailable", retryable: true },
                  routeRef: "route.sarah.fleet_runs.authority.v1",
                }),
                { status: 503 },
              ),
            ),
        },
      )
      const body = await res.json()
      expect(body.toolResults[0].ok).toBe(false)
      expect(body.toolResults[0].output).toEqual({
        ok: false,
        error: { code: "storage_unavailable", retryable: true },
        routeRef: "route.sarah.fleet_runs.authority.v1",
      })
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      if (savedFleetPath === undefined) delete process.env.SARAH_CODING_FLEET_RUNS_PATH
      else process.env.SARAH_CODING_FLEET_RUNS_PATH = savedFleetPath
      if (savedLocalStore === undefined) delete process.env.SARAH_CODING_FLEET_START_LOCAL_STORE
      else process.env.SARAH_CODING_FLEET_START_LOCAL_STORE = savedLocalStore
    }
  })

  test("direct typed authenticated-customer coding_fleet_start reaches authority while natural selection stays operator-only", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    const authorityCalls: Array<Request> = []
    try {
      const response = await handleSarahRequest(
        new Request("https://openagents.com/sarah/api/eve/tool-call", {
          method: "POST",
          headers: {
            cookie: "oa_access=fixture-current",
            "content-type": "application/json",
            "x-sarah-test-oa-session": JSON.stringify({
              userId: "owner-fc1-production-adapter",
              email: "operator@example.com",
            }),
          },
          body: JSON.stringify({
            toolName: "coding_fleet_start",
            toolCallId: "fc1-production-adapter",
            args: {
              objective: "Run issue 8637 through the durable authority.",
              repository: {
                owner: "OpenAgentsInc",
                name: "openagents",
                branch: "main",
                commit: "6af4e38282e4e71882fc5fdd86ae8adadab6df50",
              },
              verifier: { kind: "command", command: "bun test" },
              workSource: { kind: "issue_list", issueRefs: ["#8637"] },
              workerPolicy: {
                workerKind: "auto",
                targetPreference: "owner_local",
              },
              targetConcurrency: 2,
              idempotencyKey: "fc1-production-adapter-1",
            },
          }),
        }),
        {
          fleetAuthorityFetch: request => {
            authorityCalls.push(request)
            const headers = new Headers({
              "content-type": "application/json",
            })
            headers.append(
              "set-cookie",
              "oa_access=fixture-rotated; Path=/; HttpOnly; SameSite=Lax",
            )
            return Promise.resolve(
              new Response(
                JSON.stringify({
                  ok: true,
                  duplicate: false,
                  policy: {
                    source: "openagents_server_policy",
                    relationshipMode: "operator",
                    codingFleetStartAllowed: true,
                    fleetObservationAllowed: true,
                    retrievalScope: "owner_fleet_runs",
                    responsePosture: "state_oriented",
                    uiDensity: "dense",
                    administratorToolsAllowed: false,
                  },
                  routeRef: "route.sarah.fleet_runs.authority.v1",
                  run: {
                    runRef: "fleet_run.sarah.aaaaaaaaaaaaaaaaaaaa",
                    scope:
                      "scope.fleet_run.fleet_run.sarah.aaaaaaaaaaaaaaaaaaaa",
                    status: "pending_executor",
                    objective:
                      "Run issue 8637 through the durable authority.",
                    repository: {
                      owner: "OpenAgentsInc",
                      name: "openagents",
                      branch: "main",
                      commit:
                        "6af4e38282e4e71882fc5fdd86ae8adadab6df50",
                    },
                    verifier: { kind: "command", command: "bun test" },
                    workSource: {
                      kind: "issue_list",
                      issueRefs: ["#8637"],
                    },
                    workerPolicy: {
                      workerKind: "auto",
                      targetPreference: "owner_local",
                    },
                    targetConcurrency: 2,
                    execution: pendingFleetExecution,
                    createdAt: "2026-07-09T12:00:00.000Z",
                    updatedAt: "2026-07-09T12:00:00.000Z",
                    privateMaterialExcluded: true,
                  },
                }),
                { headers },
              ),
            )
          },
        },
      )

      expect(response.status).toBe(200)
      expect(authorityCalls).toHaveLength(1)
      expect(new URL(authorityCalls[0]!.url).pathname).toBe(
        "/api/sarah/fleet-runs",
      )
      expect(authorityCalls[0]!.headers.has("cookie")).toBe(true)
      const authorityBody = (await authorityCalls[0]!.json()) as Record<
        string,
        unknown
      >
      expect(authorityBody).not.toHaveProperty("ownerUserId")
      expect(authorityBody).not.toHaveProperty("ownerRef")
      expect(authorityBody).not.toHaveProperty("relationshipMode")

      const body = await response.json()
      expect(body.toolResults[0]).toMatchObject({
        ok: true,
        output: {
          ok: true,
          policy: { relationshipMode: "operator" },
          run: { runRef: "fleet_run.sarah.aaaaaaaaaaaaaaaaaaaa" },
        },
        toolName: "coding_fleet_start",
      })
      expect(response.headers.getSetCookie()).toHaveLength(2)
      expect(JSON.stringify(body)).not.toContain("oa_access")
      expect(JSON.stringify(body)).not.toContain("fixture-rotated")
    } finally {
      if (savedTestMode === undefined) {
        delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      } else {
        process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      }
    }
  })

  test("natural-language coding tool is invisible and non-executable for prospect/customer policy", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const savedInstructed = process.env.SARAH_INSTRUCTED_JSON_TOOLS
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    process.env.SARAH_INSTRUCTED_JSON_TOOLS = "1"
    try {
      for (const scenario of ["prospect", "customer"] as const) {
        const systems: string[] = []
        let authorityCalls = 0
        const requestHeaders = new Headers({
          "content-type": "application/json",
        })
        if (scenario === "customer") {
          requestHeaders.set(
            "x-sarah-test-oa-session",
            JSON.stringify({
              userId: "customer-natural",
              email: "customer@example.com",
              teams: [],
              isAdmin: false,
              relationshipMode: "operator",
            }),
          )
        }
        const response = await handleSarahRequest(
          new Request("https://openagents.com/sarah/api/eve/turn", {
            method: "POST",
            headers: requestHeaders,
            body: JSON.stringify({
              message: "Start my coding fleet.",
              prospectRef: `prospect-natural-${scenario}`,
              relationshipMode: "administrator",
            }),
          }),
          {
            generateOwnedReply: async ({ system }) => {
              systems.push(system)
              return {
                ok: true,
                reply: JSON.stringify({
                  sarah_tool: "coding_fleet_start",
                  args: naturalFleetArgs,
                }),
                model: "fixture-gemma",
                usage: {
                  promptTokens: 1,
                  outputTokens: 1,
                  thoughtTokens: 0,
                  totalTokens: 2,
                },
              }
            },
            fleetAuthorityFetch: () => {
              authorityCalls += 1
              return Promise.resolve(new Response())
            },
          },
        )
        const body = await response.json()
        expect(body.toolResults).toEqual([])
        expect(body.reply).toBe(
          "Coding fleet commands are available only in authenticated owner-operator mode.",
        )
        expect(systems).toHaveLength(1)
        expect(systems[0]).not.toContain("coding_fleet_start")
        expect(authorityCalls).toBe(0)
      }
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      if (savedInstructed === undefined) delete process.env.SARAH_INSTRUCTED_JSON_TOOLS
      else process.env.SARAH_INSTRUCTED_JSON_TOOLS = savedInstructed
    }
  })

  test("denied natural-language coding attempts redact fenced and prose-embedded private args", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const savedInstructed = process.env.SARAH_INSTRUCTED_JSON_TOOLS
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    process.env.SARAH_INSTRUCTED_JSON_TOOLS = "1"
    const coding = JSON.stringify({
      sarah_tool: "coding_fleet_start",
      args: {
        objective: "PRIVATE OPENAGENTS_AGENT_TOKEN /Users/owner/repo",
      },
    })
    try {
      for (const [kind, reply] of [
        ["fenced", `Quoted attempt:\n\`\`\`json\n${coding}\n\`\`\``],
        ["prose", `Attempt follows: ${coding} and must not be echoed.`],
      ] as const) {
        let authorityCalls = 0
        const response = await handleSarahRequest(
          new Request("https://openagents.com/sarah/api/eve/turn", {
            method: "POST",
            headers: {
              "content-type": "application/json",
              "x-sarah-test-oa-session": JSON.stringify({
                userId: `customer-denied-${kind}`,
                email: "customer@example.com",
                teams: [],
                isAdmin: false,
              }),
            },
            body: JSON.stringify({
              message: "Show the attempted coding command.",
              prospectRef: `prospect-denied-${kind}`,
            }),
          }),
          {
            generateOwnedReply: async () => ({
              ok: true,
              reply,
              model: "fixture-gemma",
              usage: {
                promptTokens: 1,
                outputTokens: 1,
                thoughtTokens: 0,
                totalTokens: 2,
              },
            }),
            fleetAuthorityFetch: () => {
              authorityCalls += 1
              return Promise.resolve(new Response())
            },
          },
        )
        const body = await response.json()
        expect(body.reply).toBe(
          "Coding fleet commands are available only in authenticated owner-operator mode.",
        )
        expect(body.toolResults).toEqual([])
        expect(JSON.stringify(body)).not.toContain("PRIVATE")
        expect(JSON.stringify(body)).not.toContain("OPENAGENTS_AGENT_TOKEN")
        expect(JSON.stringify(body)).not.toContain("/Users/owner/repo")
        expect(authorityCalls).toBe(0)
      }
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      if (savedInstructed === undefined) delete process.env.SARAH_INSTRUCTED_JSON_TOOLS
      else process.env.SARAH_INSTRUCTED_JSON_TOOLS = savedInstructed
    }
  })

  test("authenticated operator natural language starts through authority with state posture and refreshed cookie", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const savedInstructed = process.env.SARAH_INSTRUCTED_JSON_TOOLS
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    delete process.env.SARAH_INSTRUCTED_JSON_TOOLS
    const systems: string[] = []
    const authorityRequests: Request[] = []
    try {
      const response = await handleSarahRequest(
        new Request("https://openagents.com/sarah/api/eve/turn", {
          method: "POST",
          headers: {
            cookie: "oa_access=fixture-current",
            "content-type": "application/json",
            "x-sarah-test-oa-session": JSON.stringify({
              userId: "operator-natural",
              email: "operator@example.com",
              teams: [
                {
                  id: "team_openagents_core",
                  name: "OpenAgents Core Team",
                  slug: "openagents-core-team",
                },
              ],
              isAdmin: false,
              relationshipMode: "prospect",
            }),
          },
          body: JSON.stringify({
            message: "Start the bounded coding fleet now.",
            prospectRef: "prospect-natural-operator",
            relationshipMode: "prospect",
          }),
        }),
        {
          generateOwnedReply: async ({ system }) => {
            systems.push(system)
            return {
              ok: true,
              reply: JSON.stringify({
                sarah_tool: "coding_fleet_start",
                args: naturalFleetArgs,
              }),
              model: "fixture-gemma",
              usage: {
                promptTokens: 2,
                outputTokens: 2,
                thoughtTokens: 0,
                totalTokens: 4,
              },
            }
          },
          fleetAuthorityFetch: (request) => {
            authorityRequests.push(request)
            const headers = new Headers({ "content-type": "application/json" })
            headers.append(
              "set-cookie",
              "oa_access=fixture-rotated; Path=/; HttpOnly; SameSite=Lax",
            )
            return Promise.resolve(
              new Response(JSON.stringify(naturalFleetSuccess), { headers }),
            )
          },
        },
      )
      const body = await response.json()
      expect(authorityRequests).toHaveLength(1)
      expect(await authorityRequests[0]!.json()).toEqual(naturalFleetArgs)
      expect(body.toolResults).toHaveLength(1)
      expect(body.toolResults[0]).toMatchObject({
        toolName: "coding_fleet_start",
        ok: true,
        output: {
          ok: true,
          run: { runRef: "fleet_run.sarah.aaaaaaaaaaaaaaaaaaaa" },
        },
      })
      expect(systems[0]).toContain("coding_fleet_start")
      expect(systems[0]).toContain("owner's AI coding-fleet operator")
      expect(systems[0]).toContain(
        'repository:{owner,name,branch,commit}',
      )
      expect(systems[0]).toContain(
        'workSource is either {kind:"issue_list",issueRefs:["#123"]}',
      )
      expect(systems[0]).toContain(
        "repository.commit MUST be the supplied 40-character lowercase Git SHA",
      )
      expect(systems[0]).toContain(
        "one new idempotencyKey for each materially different request",
      )
      expect(systems[0]).not.toContain("AI sales employee")
      expect(body.personaPreview).toContain("AI coding-fleet operator")
      expect(body.personaPreview).not.toContain("AI sales employee")
      expect(response.headers.getSetCookie()).toEqual([
        "oa_access=fixture-rotated; Path=/; HttpOnly; SameSite=Lax",
      ])
      expect(JSON.stringify(body)).not.toContain("fixture-rotated")
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      if (savedInstructed === undefined) delete process.env.SARAH_INSTRUCTED_JSON_TOOLS
      else process.env.SARAH_INSTRUCTED_JSON_TOOLS = savedInstructed
    }
  })

  test("coding_fleet_start fixture store rejects escaped local paths", () => {
    expect(() => createSarahCodingFleetFileStoreForTest("../escape.json")).toThrow(
      "fixture store path must stay under .sarah",
    )
    expect(() =>
      createSarahCodingFleetFileStoreForTest("/tmp/sarah-fleet.json"),
    ).toThrow("fixture store path must stay under .sarah")
  })

  test("coding_fleet_start schema errors do not echo private input", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const fleetPath = installCodingFleetFileStoreForTest(
      `server-fc1a-schema-${process.pid}.json`,
    )
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    try {
      const res = await handleSarahRequest(
        new Request("http://localhost/sarah/api/eve/tool-call", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-sarah-test-oa-session": JSON.stringify({
              userId: "owner-fc1a",
              email: "owner@example.com",
            }),
          },
          body: JSON.stringify({
            toolName: "coding_fleet_start",
            args: {
              objective: { secret: "OPENAGENTS_AGENT_TOKEN=abc" },
              repository: {
                owner: "OpenAgentsInc",
                name: "openagents",
                branch: "main",
                commit: "f8e4aa29d9",
              },
              verifier: { kind: "command", command: "bun run test:sarah" },
              workSource: { kind: "issue_list", issueRefs: ["#8637"] },
              workerPolicy: {
                workerKind: "auto",
                targetPreference: "owner_local",
              },
              targetConcurrency: 1,
              idempotencyKey: "fc1a-schema-secret",
            },
          }),
        }),
      )
      const body = await res.json()
      const error = body.toolResults[0].output.error
      expect(body.toolResults[0].ok).toBe(false)
      expect(error).toEqual({
        code: "invalid_request",
        field: "request",
        message: "coding_fleet_start args failed schema validation.",
      })
      expect(JSON.stringify(error)).not.toContain("OPENAGENTS_AGENT_TOKEN")
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      await unlink(fleetPath).catch(() => {})
    }
  })

  test("coding_fleet_start persists an owner-scoped pending run idempotently", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const fleetFile = `server-fc1a-fleet-${process.pid}.json`
    const fleetPath = installCodingFleetFileStoreForTest(fleetFile)
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    try {
      const requestBody = {
        toolName: "coding_fleet_start",
        toolCallId: "fc1a-start",
        args: {
          schema: "sarah.coding_fleet_start.request.v1",
          objective: "Run issue 8637 as a bounded FC-1 fixture.",
          repository: {
            owner: "OpenAgentsInc",
            name: "openagents",
            branch: "main",
            commit: "f8e4aa29d9",
          },
          verifier: { kind: "command", command: "bun run test:sarah" },
          workSource: { kind: "issue_list", issueRefs: ["#8637"] },
          workerPolicy: {
            workerKind: "auto",
            targetPreference: "owner_local",
          },
          targetConcurrency: 3,
          idempotencyKey: "fc1a-start-8637",
        },
      }
      const first = await handleSarahRequest(
        new Request("http://localhost/sarah/api/eve/tool-call", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-sarah-test-oa-session": JSON.stringify({
              userId: "owner-fc1a",
              email: "owner@example.com",
            }),
          },
          body: JSON.stringify(requestBody),
        }),
      )
      const firstBody = await first.json()
      expect(firstBody.toolResults[0].ok).toBe(true)
      const output = firstBody.toolResults[0].output
      expect(output.ok).toBe(true)
      expect(output.duplicate).toBe(false)
      expect(output.runRef).toStartWith("fleet_run.sarah.")
      expect(output.scope).toBe(`scope.fleet_run.${output.runRef}`)
      expect(output.privateMaterialExcluded).toBe(true)
      expect(output.ownerRef).toBeUndefined()
      expect(output.ownerHash).toBeUndefined()
      expect(output.workerPolicy.workerKind).toBe("auto")
      expect(output.workSource.issueRefs).toEqual(["#8637"])

      const duplicate = await handleSarahRequest(
        new Request("http://localhost/sarah/api/eve/tool-call", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-sarah-test-oa-session": JSON.stringify({
              userId: "owner-fc1a",
              email: "owner@example.com",
            }),
          },
          body: JSON.stringify(requestBody),
        }),
      )
      const duplicateBody = await duplicate.json()
      expect(duplicateBody.toolResults[0].output.duplicate).toBe(true)
      expect(duplicateBody.toolResults[0].output.runRef).toBe(output.runRef)

      const conflict = await handleSarahRequest(
        new Request("http://localhost/sarah/api/eve/tool-call", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-sarah-test-oa-session": JSON.stringify({
              userId: "owner-fc1a",
              email: "owner@example.com",
            }),
          },
          body: JSON.stringify({
            ...requestBody,
            args: {
              ...requestBody.args,
              objective:
                "Run issue 8637 as a different bounded FC-1 fixture.",
            },
          }),
        }),
      )
      const conflictBody = await conflict.json()
      expect(conflictBody.toolResults[0].ok).toBe(false)
      expect(conflictBody.toolResults[0].output.error).toEqual({
        code: "idempotency_conflict",
        message:
          "idempotencyKey already belongs to a different Sarah coding fleet request.",
      })

      const rows = await listSarahCodingFleetRunsForTest()
      expect(rows).toHaveLength(1)
      expect(rows[0].ownerRef).toBe("owner-fc1a")
      expect(rows[0].status).toBe("pending_executor")
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      await unlink(fleetPath).catch(() => {})
    }
  })

  test("coding_fleet_start isolates identical idempotency keys by owner", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const fleetFile = `server-fc1a-two-owner-${process.pid}.json`
    const fleetPath = installCodingFleetFileStoreForTest(fleetFile)
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    try {
      const requestBody = {
        toolName: "coding_fleet_start",
        args: {
          objective: "Run issue 8637 as a bounded FC-1 owner isolation fixture.",
          repository: {
            owner: "OpenAgentsInc",
            name: "openagents",
            branch: "main",
            commit: "f8e4aa29d9",
          },
          verifier: { kind: "command", command: "bun run test:sarah" },
          workSource: { kind: "issue_list", issueRefs: ["#8637"] },
          workerPolicy: {
            workerKind: "auto",
            targetPreference: "owner_local",
          },
          targetConcurrency: 2,
          idempotencyKey: "fc1a-shared-key",
        },
      }

      const startForOwner = async (ownerRef: string) => {
        const res = await handleSarahRequest(
          new Request("http://localhost/sarah/api/eve/tool-call", {
            method: "POST",
            headers: {
              "content-type": "application/json",
              "x-sarah-test-oa-session": JSON.stringify({
                userId: ownerRef,
                email: `${ownerRef}@example.com`,
              }),
            },
            body: JSON.stringify(requestBody),
          }),
        )
        const body = await res.json()
        expect(body.toolResults[0].ok).toBe(true)
        return body.toolResults[0].output
      }

      const ownerA = await startForOwner("owner-fc1a-a")
      const ownerB = await startForOwner("owner-fc1a-b")
      expect(ownerA.runRef).not.toBe(ownerB.runRef)
      expect(ownerA.ownerHash).toBeUndefined()
      expect(ownerB.ownerHash).toBeUndefined()

      const rows = await listSarahCodingFleetRunsForTest()
      expect(rows).toHaveLength(2)
      expect(rows.map((row) => row.ownerRef).sort()).toEqual([
        "owner-fc1a-a",
        "owner-fc1a-b",
      ])
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      await unlink(fleetPath).catch(() => {})
    }
  })

  test("coding_fleet_start fails closed on invalid existing run stores", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const fleetFile = `server-fc1a-invalid-store-${process.pid}.json`
    const fleetPath = installCodingFleetFileStoreForTest(fleetFile)
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    try {
      await mkdir(join(process.cwd(), ".sarah"), { recursive: true })
      await writeFile(fleetPath, "{not-json")

      const res = await handleSarahRequest(
        new Request("http://localhost/sarah/api/eve/tool-call", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-sarah-test-oa-session": JSON.stringify({
              userId: "owner-fc1a",
              email: "owner@example.com",
            }),
          },
          body: JSON.stringify({
            toolName: "coding_fleet_start",
            args: {
              objective: "Run issue 8637 as a bounded FC-1 fixture.",
              repository: {
                owner: "OpenAgentsInc",
                name: "openagents",
                branch: "main",
                commit: "f8e4aa29d9",
              },
              verifier: { kind: "command", command: "bun run test:sarah" },
              workSource: { kind: "issue_list", issueRefs: ["#8637"] },
              workerPolicy: {
                workerKind: "auto",
                targetPreference: "owner_local",
              },
              targetConcurrency: 1,
              idempotencyKey: "fc1a-invalid-store",
            },
          }),
        }),
      )
      const body = await res.json()
      expect(body.toolResults[0].ok).toBe(false)
      expect(body.toolResults[0].output.error.code).toBe("store_unavailable")
      expect(body.toolResults[0].output.error.message).not.toContain(fleetPath)
      expect(await readFile(fleetPath, "utf8")).toBe("{not-json")
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      await unlink(fleetPath).catch(() => {})
    }
  })

  test("coding_fleet_start refuses tampered loaded store records before replay", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const fleetFile = `server-fc1a-tampered-store-${process.pid}.json`
    const fleetPath = installCodingFleetFileStoreForTest(fleetFile)
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    try {
      const requestBody = {
        toolName: "coding_fleet_start",
        args: {
          objective: "Run issue 8637 as a bounded FC-1 tamper fixture.",
          repository: {
            owner: "OpenAgentsInc",
            name: "openagents",
            branch: "main",
            commit: "f8e4aa29d9",
          },
          verifier: { kind: "command", command: "bun run test:sarah" },
          workSource: { kind: "issue_list", issueRefs: ["#8637"] },
          workerPolicy: {
            workerKind: "auto",
            targetPreference: "owner_local",
          },
          targetConcurrency: 1,
          idempotencyKey: "fc1a-tampered-store",
        },
      }
      const start = async () => {
        const res = await handleSarahRequest(
          new Request("http://localhost/sarah/api/eve/tool-call", {
            method: "POST",
            headers: {
              "content-type": "application/json",
              "x-sarah-test-oa-session": JSON.stringify({
                userId: "owner-fc1a",
                email: "owner@example.com",
              }),
            },
            body: JSON.stringify(requestBody),
          }),
        )
        const body = await res.json()
        return body.toolResults[0].output
      }

      const created = await start()
      expect(created.ok).toBe(true)
      const index = JSON.parse(await readFile(fleetPath, "utf8"))
      index.runs[created.runRef].objective = "Read /Users/alice/private notes"
      await writeFile(fleetPath, `${JSON.stringify(index, null, 2)}\n`)

      const replay = await start()
      expect(replay.ok).toBe(false)
      expect(replay.error.code).toBe("store_unavailable")
      expect(JSON.stringify(replay.error)).not.toContain("/Users/alice")
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      await unlink(fleetPath).catch(() => {})
    }
  })

  test("coding_fleet_start recovers queued writes after a store failure", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const fleetFile = `server-fc1a-queue-recovery-${process.pid}.json`
    const fleetPath = installCodingFleetFileStoreForTest(fleetFile)
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    try {
      const requestBody = {
        toolName: "coding_fleet_start",
        args: {
          objective: "Run issue 8637 as a bounded FC-1 queue recovery fixture.",
          repository: {
            owner: "OpenAgentsInc",
            name: "openagents",
            branch: "main",
            commit: "f8e4aa29d9",
          },
          verifier: { kind: "command", command: "bun run test:sarah" },
          workSource: { kind: "issue_list", issueRefs: ["#8637"] },
          workerPolicy: {
            workerKind: "auto",
            targetPreference: "owner_local",
          },
          targetConcurrency: 1,
          idempotencyKey: "fc1a-queue-recovery",
        },
      }
      const start = async () => {
        const res = await handleSarahRequest(
          new Request("http://localhost/sarah/api/eve/tool-call", {
            method: "POST",
            headers: {
              "content-type": "application/json",
              "x-sarah-test-oa-session": JSON.stringify({
                userId: "owner-fc1a",
                email: "owner@example.com",
              }),
            },
            body: JSON.stringify(requestBody),
          }),
        )
        const body = await res.json()
        return body.toolResults[0].output
      }

      await mkdir(fleetPath, { recursive: true })
      const failed = await start()
      expect(failed.ok).toBe(false)
      expect(failed.error.code).toBe("store_unavailable")

      await rm(fleetPath, { recursive: true, force: true })
      const recovered = await start()
      expect(recovered.ok).toBe(true)
      expect(recovered.runRef).toStartWith("fleet_run.sarah.")
      const rows = await listSarahCodingFleetRunsForTest()
      expect(rows).toHaveLength(1)
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      await rm(fleetPath, { recursive: true, force: true }).catch(() => {})
    }
  })

  test("coding_fleet_start rejects private material and unknown worker kinds", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const fleetPath = installCodingFleetFileStoreForTest(
      `server-fc1a-invalid-${process.pid}.json`,
    )
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    try {
      const res = await handleSarahRequest(
        new Request("http://localhost/sarah/api/eve/tool-call", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-sarah-test-oa-session": JSON.stringify({
              userId: "owner-fc1a",
              email: "owner@example.com",
            }),
          },
          body: JSON.stringify({
            toolName: "coding_fleet_start",
            args: {
              objective: "Read /Users/alice/private repo secrets",
              repository: {
                owner: "OpenAgentsInc",
                name: "openagents",
                branch: "main",
                commit: "f8e4aa29d9",
              },
              verifier: { kind: "command", command: "bun run test:sarah" },
              workSource: { kind: "issue_list", issueRefs: ["#8637"] },
              workerPolicy: {
                workerKind: "default-home",
                targetPreference: "owner_local",
              },
              targetConcurrency: 1,
              idempotencyKey: "fc1a-unsafe-8637",
            },
          }),
        }),
      )
      const body = await res.json()
      expect(body.toolResults[0].ok).toBe(false)
      expect(body.toolResults[0].output.error.code).toBe(
        "unsafe_private_material",
      )

      const unknownWorker = await handleSarahRequest(
        new Request("http://localhost/sarah/api/eve/tool-call", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-sarah-test-oa-session": JSON.stringify({
              userId: "owner-fc1a",
              email: "owner@example.com",
            }),
          },
          body: JSON.stringify({
            toolName: "coding_fleet_start",
            args: {
              objective: "Run issue 8637 as a bounded FC-1 fixture.",
              repository: {
                owner: "OpenAgentsInc",
                name: "openagents",
                branch: "main",
                commit: "f8e4aa29d9",
              },
              verifier: { kind: "command", command: "bun run test:sarah" },
              workSource: { kind: "issue_list", issueRefs: ["#8637"] },
              workerPolicy: {
                workerKind: "default-home",
                targetPreference: "owner_local",
              },
              targetConcurrency: 1,
              idempotencyKey: "fc1a-worker-8637",
            },
          }),
        }),
      )
      const unknownWorkerBody = await unknownWorker.json()
      expect(unknownWorkerBody.toolResults[0].ok).toBe(false)
      expect(unknownWorkerBody.toolResults[0].output.error).toMatchObject({
        code: "invalid_request",
        field: "workerPolicy.workerKind",
      })
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      await unlink(fleetPath).catch(() => {})
    }
  })

  test("coding_fleet_start rejects unusable plan DAGs", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
    const fleetPath = installCodingFleetFileStoreForTest(
      `server-fc1a-dag-${process.pid}.json`,
    )
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    try {
      const startPlan = async (
        units: Array<{ unitRef: string; title: string; dependsOn: string[] }>,
        idempotencyKey: string,
      ) => {
        const res = await handleSarahRequest(
          new Request("http://localhost/sarah/api/eve/tool-call", {
            method: "POST",
            headers: {
              "content-type": "application/json",
              "x-sarah-test-oa-session": JSON.stringify({
                userId: "owner-fc1a",
                email: "owner@example.com",
              }),
            },
            body: JSON.stringify({
              toolName: "coding_fleet_start",
              args: {
                objective: "Run a bounded FC-1 plan DAG fixture.",
                repository: {
                  owner: "OpenAgentsInc",
                  name: "openagents",
                  branch: "main",
                  commit: "f8e4aa29d9",
                },
                verifier: { kind: "command", command: "bun run test:sarah" },
                workSource: {
                  kind: "plan_dag",
                  planRef: "fc1a-plan",
                  units,
                },
                workerPolicy: {
                  workerKind: "auto",
                  targetPreference: "owner_local",
                },
                targetConcurrency: 2,
                idempotencyKey,
              },
            }),
          }),
        )
        const body = await res.json()
        return body.toolResults[0].output.error
      }

      expect(
        await startPlan(
          [
            { unitRef: "unit.a", title: "A", dependsOn: [] },
            { unitRef: "unit.a", title: "A again", dependsOn: [] },
          ],
          "fc1a-plan-duplicate",
        ),
      ).toMatchObject({
        code: "invalid_request",
        field: "workSource.units.unitRef",
      })
      expect(
        await startPlan(
          [{ unitRef: "unit.a", title: "A", dependsOn: ["unit.missing"] }],
          "fc1a-plan-missing",
        ),
      ).toMatchObject({
        code: "invalid_request",
        field: "workSource.units.dependsOn",
      })
      expect(
        await startPlan(
          [
            { unitRef: "unit.a", title: "A", dependsOn: ["unit.b"] },
            { unitRef: "unit.b", title: "B", dependsOn: ["unit.a"] },
          ],
          "fc1a-plan-cycle",
        ),
      ).toMatchObject({
        code: "invalid_request",
        field: "workSource.units.dependsOn",
      })
    } finally {
      if (savedTestMode === undefined) delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
      else process.env.SARAH_ACCOUNT_LINK_TEST_MODE = savedTestMode
      await unlink(fleetPath).catch(() => {})
    }
  })

  // KHS-9 (#8608): the operator handoff view for customer Blueprint drafts is
  // admin-bearer-guarded with the same fail-closed posture as the learning
  // routes — unarmed 503, wrong bearer 401, exact bearer 200.
  test("customer-blueprints operator route fails closed without an admin token", async () => {
    delete process.env.SARAH_OPERATOR_ADMIN_TOKEN
    delete process.env.OPENAGENTS_ADMIN_API_TOKEN
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/operator/customer-blueprints"),
    )
    expect(res.status).toBe(503)
    const body = await res.json()
    expect(body.error.code).toBe("operator_admin_not_armed")
  })

  test("customer-blueprints operator route refuses a wrong bearer, serves the exact one", async () => {
    process.env.SARAH_OPERATOR_ADMIN_TOKEN = "khs9-test-admin"
    const wrong = await handleSarahRequest(
      new Request("http://localhost/sarah/api/operator/customer-blueprints", {
        headers: { authorization: "Bearer nope" },
      }),
    )
    expect(wrong.status).toBe(401)

    const right = await handleSarahRequest(
      new Request("http://localhost/sarah/api/operator/customer-blueprints", {
        headers: { authorization: "Bearer khs9-test-admin" },
      }),
    )
    expect(right.status).toBe(200)
    const body = await right.json()
    expect(Array.isArray(body.blueprints)).toBe(true)
    // No database in unit tests: the store reports itself unconfigured
    // honestly instead of failing the route.
    expect(body.storeConfigured).toBe(false)
    delete process.env.SARAH_OPERATOR_ADMIN_TOKEN
  })

  test("customer blueprint current route seeds the active prospect only", async () => {
    __resetCustomerBlueprintForTest()
    __setCustomerBlueprintStoreReaderForTest(async (aliases) => {
      expect(aliases).toContain("prospect-a")
      return {
        profileFacts: [
          {
            fact: 'company: "Acme Retail"',
            sourceTurnId: "turn-company",
            at: "2026-07-09T16:00:00.000Z",
          },
        ],
        contact: { email: "buyer@example.com", contactId: "oa_user:buyer" },
        turns: [],
        latestRevision: 4,
      }
    })
    __setCustomerBlueprintLatestDraftReaderForTest(async (aliases) => {
      expect(aliases).toContain("prospect-a")
      return {
        schema: CUSTOMER_BLUEPRINT_SCHEMA,
        prospectRef: "prospect-a",
        revision: 4,
        createdAt: "2026-07-09T16:00:00.000Z",
        business: { facts: [] },
        contacts: { email: "buyer@example.com", contactId: "oa_user:buyer" },
        needs: [],
        suggestedModules: [],
        sources: {
          turnIds: [],
          factCount: 1,
          provenance:
            "sarah_prospect_profile + sarah_transcript_turns (per-fact source turn ids)",
        },
        handoff: {
          pipeline: "operator_assisted_business_workspace",
          automatedProvisioning: false,
          convergesWith:
            "CB-1.4 prefill pipeline (intake -> public-data research -> seeded workspace)",
          note: "Draft only.",
        },
      }
    })
    try {
      const seeded = await handleSarahRequest(
        new Request("http://localhost/sarah/api/customer-blueprint/current", {
          headers: { cookie: "sarah_prospect_ref=prospect-a" },
        }),
      )
      expect(seeded.status).toBe(200)
      const body = await seeded.json()
      expect(body.prospect).toBe(true)
      expect(body.draft.revision).toBe(4)
      expect(body.facts[0].fact).toBe('company: "Acme Retail"')
      expect(body.contact.email).toBe("buyer@example.com")

      const anonymous = await handleSarahRequest(
        new Request("http://localhost/sarah/api/customer-blueprint/current"),
      )
      expect(await anonymous.json()).toEqual({
        prospect: false,
        draft: null,
        facts: [],
        contact: null,
        storeConfigured: false,
      })
    } finally {
      __resetCustomerBlueprintForTest()
    }
  })

  test("actions tool-call route mints a prospect cookie and exposes current receipts", async () => {
    const savedIndexPath = process.env.SARAH_SESSION_INDEX_PATH
    const indexFile = `server-bm4-receipts-${process.pid}.json`
    process.env.SARAH_SESSION_INDEX_PATH = indexFile
    try {
      const action = await handleSarahRequest(
        new Request("http://localhost/sarah/api/eve/tool-call", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            toolName: "human_handoff",
            toolCallId: "bm4-handoff-test",
            args: {
              reason: "prospect_requested_human_handoff",
              summary: "Prospect asked to book a human from the Actions tab.",
            },
          }),
        }),
      )
      expect(action.status).toBe(200)
      expect(action.headers.get("set-cookie") || "").toContain("sarah_prospect_ref")
      const actionBody = await action.json()
      expect(actionBody.toolResults[0].toolCallId).toBe("bm4-handoff-test")
      expect(actionBody.toolResults[0].toolName).toBe("human_handoff")
      expect(actionBody.toolResults[0].ok).toBe(true)

      const cookie = (action.headers.get("set-cookie") || "").split(";")[0]!
      const receipts = await handleSarahRequest(
        new Request("http://localhost/sarah/api/session/receipts/current", {
          headers: { cookie },
        }),
      )
      expect(receipts.status).toBe(200)
      const receiptBody = await receipts.json()
      expect(receiptBody.prospect).toBe(true)
      expect(receiptBody.receipts).toHaveLength(1)
      expect(receiptBody.receipts[0].toolCallId).toBe("bm4-handoff-test")
      expect(receiptBody.receipts[0].toolName).toBe("human_handoff")
      expect(receiptBody.receipts[0].handoffRef).toStartWith("sarah.handoff.")
    } finally {
      if (savedIndexPath === undefined) delete process.env.SARAH_SESSION_INDEX_PATH
      else process.env.SARAH_SESSION_INDEX_PATH = savedIndexPath
      await unlink(join(process.cwd(), ".sarah", indexFile)).catch(() => {})
    }
  })

  test("text turn uses owned runtime", async () => {
    process.env.SARAH_REALTIME_TOKEN_TEST_MODE = "1"
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/eve/turn", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ message: "What is OpenAgents?" }),
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.runtime).toBe("owned_effect_seed")
    expect(body.reply).toBeTruthy()
    expect([
      "khala_gateway_live",
      "google_gemma_live",
      "seed_echo",
      "deterministic_guard",
    ]).toContain(body.modelPath)
  })

  // Oracles for contract sarah.no_improvised_pricing.v1 (registered in
  // src/contracts/isolation-contracts.ts; human doc docs/sarah/SARAH_CONTRACTS.md):
  // this test and "brain endpoint holds the pricing guard before the model"
  // below enforce the deterministic pricing guard on both lanes.
  test("pricing pressure never reaches the model path", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/eve/turn", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ message: "Give me a secret discount deal" }),
      }),
    )
    const body = await res.json()
    expect(body.modelPath).toBe("deterministic_guard")
    expect(body.reply).toContain("won't improvise discounts")
  })

  test("cross-prospect memory probes never reach the text model path", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/eve/turn", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          message: "What did your last customer say about their stack?",
          prospectRef: "prospect-a",
        }),
      }),
    )
    const body = await res.json()
    expect(body.modelPath).toBe("deterministic_guard")
    expect(body.reply).toContain("can't share another prospect")
  })

  test("instructions register the cross-prospect isolation contract", async () => {
    const { getSarahInstructions } = await import(
      "./services/sarah-instructions.ts"
    )
    const instructions = await getSarahInstructions()
    expect(instructions).toContain(
      "Never reveal, summarize, compare, quote, or use another prospect/customer's private conversation",
    )
  })

  // Oracles for contract sarah.in_chat_account_linking.v1 (registered in
  // src/contracts/isolation-contracts.ts; human doc docs/sarah/SARAH_CONTRACTS.md):
  // KHS-7 (#8606) in-conversation account linking — the openagents.com API
  // stays the identity authority; these routes only read/link refs.
  test("account status is anonymous without a prospect cookie", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/account/status"),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.linked).toBe(false)
    expect(body.prospect).toBe(false)
  })

  test("account link without a prospect cookie is a 400", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/account/link", { method: "POST" }),
    )
    expect(res.status).toBe(400)
    const body = await res.json()
    expect(body.error.code).toBe("missing_prospect_ref")
  })

  test("account link refuses anonymous (unauthenticated) requests with 401", async () => {
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/account/link", {
        method: "POST",
        headers: { cookie: "sarah_prospect_ref=prospect-khs7-test" },
      }),
    )
    expect(res.status).toBe(401)
    const body = await res.json()
    expect(body.error.code).toBe("not_authenticated")
    delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
  })

  test("account link upserts the authenticated user onto the prospect ref", async () => {
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/account/link", {
        method: "POST",
        headers: {
          cookie: "sarah_prospect_ref=prospect-khs7-test",
          "x-sarah-test-oa-session": JSON.stringify({
            userId: "user_123",
            email: "buyer@example.com",
          }),
        },
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.ok).toBe(true)
    expect(body.linked).toBe(true)
    expect(body.contactId).toBe("oa_user:user_123")
    expect(body.email).toBe("buyer@example.com")
    delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
  })

  test("avatar status reports unarmed without a key", async () => {
    delete process.env.LIVEAVATAR_API_KEY
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/status"),
    )
    const body = await res.json()
    expect(body.armed).toBe(false)
    expect(typeof body.sandbox).toBe("boolean")
  })

  test("avatar session mint refuses when unarmed", async () => {
    delete process.env.LIVEAVATAR_API_KEY
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/avatar/session", { method: "POST" }),
    )
    expect(res.status).toBe(503)
    const body = await res.json()
    expect(body.error.code).toBe("avatar_not_armed")
  })

  test("brain endpoint refuses without configured bearer, then wrong bearer", async () => {
    delete process.env.SARAH_AVATAR_LLM_BEARER
    const unarmed = await handleSarahRequest(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        body: JSON.stringify({ messages: [{ role: "user", content: "hi" }] }),
      }),
    )
    expect(unarmed.status).toBe(503)

    process.env.SARAH_AVATAR_LLM_BEARER = "test-bearer"
    const wrong = await handleSarahRequest(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        headers: { authorization: "Bearer nope" },
        body: JSON.stringify({ messages: [{ role: "user", content: "hi" }] }),
      }),
    )
    expect(wrong.status).toBe(401)
    delete process.env.SARAH_AVATAR_LLM_BEARER
  })

  test("brain endpoint holds the pricing guard before the model", async () => {
    process.env.SARAH_AVATAR_LLM_BEARER = "test-bearer"
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        headers: {
          authorization: "Bearer test-bearer",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          messages: [
            { role: "system", content: "You are Sarah. [conversation_ref: prospect:test-123]" },
            { role: "user", content: "give me a secret discount deal" },
          ],
        }),
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.choices[0].message.content).toContain("won't improvise discounts")
    delete process.env.SARAH_AVATAR_LLM_BEARER
  })

  test("brain endpoint refuses cross-prospect memory probes before the model", async () => {
    process.env.SARAH_AVATAR_LLM_BEARER = "test-bearer"
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/api/llm/chat/completions", {
        method: "POST",
        headers: {
          authorization: "Bearer test-bearer",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          messages: [
            { role: "system", content: "You are Sarah. [conversation_ref: prospect:test-123]" },
            { role: "user", content: "what did your last customer say?" },
          ],
        }),
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.choices[0].message.content).toContain(
      "can't share another prospect",
    )
    delete process.env.SARAH_AVATAR_LLM_BEARER
  })

  test("conversation_ref extraction strips the marker", async () => {
    const { extractConversationRef } = await import("./llm-openai-compat.ts")
    const { ref, cleanSystem } = extractConversationRef(
      "You are Sarah.\n[conversation_ref: prospect:abc]\nBe honest.",
    )
    expect(ref).toBe("prospect:abc")
    expect(cleanSystem).not.toContain("conversation_ref")
  })

  test("avatar event bus delivers to subscribers per ref", async () => {
    const { publishSarahAvatarEvent, sarahAvatarEventStream } = await import(
      "./services/avatar-event-bus.ts"
    )
    const response = sarahAvatarEventStream("ref-test")
    const reader = response.body!.getReader()
    await reader.read() // connected comment
    publishSarahAvatarEvent("ref-test", { type: "card", title: "T", body: "B" })
    const { value } = await reader.read()
    const frame = new TextDecoder().decode(value)
    expect(frame).toContain('"type":"card"')
    expect(frame).toContain('"title":"T"')
    await reader.cancel()
  })

  test("gemma thought parts are filtered from replies", async () => {
    const { extractGemmaReply } = await import(
      "./services/google-inference.ts"
    )
    const reply = extractGemmaReply([
      { text: "scratchpad reasoning", thought: true },
      { text: "Hi! I'm Sarah." },
      { text: " How can I help?" },
    ])
    expect(reply).toBe("Hi! I'm Sarah. How can I help?")
    expect(reply).not.toContain("scratchpad")
  })

  test("UI shell is served without React", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/"),
    )
    expect(res.status).toBe(200)
    const html = await res.text()
    expect(html).toContain("AI disclosure")
    expect(html).not.toContain("react")
    expect(html).toContain("/sarah/app.js")
    expect(html).toContain("sarah-root")
    expect(html).toContain("sarah-avatar")
  })

  test("continue handoff mints prospect cookie", async () => {
    const res = await handleSarahRequest(
      new Request("http://localhost/sarah/continue/handoff-token-demo", {
        headers: { accept: "application/json" },
      }),
    )
    expect(res.status).toBe(200)
    const body = await res.json()
    expect(body.ok).toBe(true)
    expect(body.handoffToken).toBe("handoff-token-demo")
    expect(body.prospectRef).toBeTruthy()
    expect(body.next).toBe("/sarah/")
    expect(res.headers.get("set-cookie") || "").toContain("sarah_prospect_ref")
  })

  test("email compliance footer includes AI disclosure + path-mount opt-out", async () => {
    const { appendEmailComplianceFooter } = await import(
      "./services/crm-email-rail.ts"
    )
    const footer = appendEmailComplianceFooter("Thanks.", "buyer@example.com")
    expect(footer).toContain("Sarah is an AI sales employee for OpenAgents.")
    expect(footer).toContain("https://openagents.com/sarah/unsubscribe")
    expect(footer).toContain("buyer%40example.com")
  })
})
