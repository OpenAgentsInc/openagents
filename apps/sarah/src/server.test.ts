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
      )
      const body = await res.json()
      expect(body.toolResults[0].ok).toBe(false)
      expect(body.toolResults[0].output.error).toEqual({
        code: "store_unavailable",
        message:
          "coding_fleet_start does not have an enabled durable fleet run store.",
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
    }
  })

  test("coding_fleet_start rejects unusable plan DAGs", async () => {
    const savedTestMode = process.env.SARAH_ACCOUNT_LINK_TEST_MODE
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
