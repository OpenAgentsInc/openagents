import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { afterEach, describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  acceptAssignment,
  computeAssignmentAdmission,
  pollAssignments,
  runNoSpendAssignment,
  submitAssignmentCloseout,
  trainingWorkerReceiptsPathForHome,
  type PylonAssignmentLease,
} from "../src/assignment"
import { sendHeartbeat } from "../src/presence"
import { verifyNip98Authorization } from "../src/nostr-identity"
import { assertPublicProjectionSafe, ensurePylonLocalState, writePresenceState } from "../src/state"
import { PSIONIC_QWEN_MODEL_REFS, type PsionicQwenModelAdmission } from "../packages/runtime/src/index"
import { CLAUDE_AGENT_SDK_PACKAGE } from "../src/claude-agent"
import {
  CLAUDE_AGENT_TASK_SCHEMA,
  type ClaudeAgentCheckoutRunner,
  type ClaudeAgentRunner,
} from "../src/claude-agent-executor"

const servers: ReturnType<typeof Bun.serve>[] = []

afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true)
})

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-assignment-test-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

const lease = (overrides: Partial<PylonAssignmentLease> = {}): PylonAssignmentLease => ({
  schema: "openagents.pylon.assignment_lease.v0.3",
  assignmentRef: "assignment.public.no_spend.test",
  leaseRef: "lease.public.no_spend.test",
  goal: "Return a public-safe proof ref for this fake no-spend assignment.",
  paymentMode: "no-spend",
  capabilityRefs: ["cap.gepa.retained.v1"],
  expiresAt: "2026-06-09T01:00:00.000Z",
  ...overrides,
})

function fakeAssignmentServer(input: {
  leases?: PylonAssignmentLease[]
  rejectAccept?: boolean
  rejectAcceptRefs?: ReadonlyArray<string>
  cancelOnProgress?: boolean
} = {}) {
  const requests: { path: string; body: any; headers: Headers }[] = []
  const accepted = new Set<string>()
  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url)
      const text = await request.text()
      const body = text ? JSON.parse(text) : {}
      requests.push({ path: url.pathname, body, headers: request.headers })

      if (request.headers.get("authorization")?.startsWith("Bearer ")) {
        if (request.method === "POST") {
          expect(request.headers.get("Idempotency-Key")).toContain("pylon.assignment.")
        }
      } else {
        verifyNip98Authorization(request.headers.get("authorization"), {
          method: request.method,
          url: request.url,
          body: text,
          // Verify against the same fixed epoch the tests inject into the
          // client (`now: () => new Date("2026-06-09T...")`). A wall-clock
          // `now` with a wide maxSkewSeconds was a date bomb: it expired
          // 300,000s after the fixed epoch and failed every NIP-98 test.
          now: new Date("2026-06-09T00:00:30.000Z"),
          maxSkewSeconds: 300,
        })
        expect(request.headers.get("x-nip98-body-sha256")).toBeNull()
        expect(request.headers.get("x-nip98-signature")).toBeNull()
        expect(request.headers.get("x-nip98-pubkey")).toBeNull()
        if (body.pylonRef) {
          expect(request.headers.get("x-pylon-ref")).toBe(body.pylonRef)
        } else {
          expect(request.headers.get("x-pylon-ref")).toBeTruthy()
        }
      }

      if (url.pathname.includes("/heartbeat")) {
        return Response.json({ heartbeatRef: `heartbeat.${body.pylonRef}.${body.sequence}` })
      }
      if (url.pathname.endsWith("/assignments")) {
        return Response.json({
          schema: "openagents.pylon.assignment_poll_response.v0.3",
          assignments: input.leases ?? [lease()],
        })
      }
      if (url.pathname.endsWith("/accept")) {
        const assignmentRef = decodeURIComponent(url.pathname.split("/").at(-2) ?? "")
        if (
          input.rejectAccept ||
          input.rejectAcceptRefs?.includes(assignmentRef) === true
        ) {
          return Response.json({ statusRef: "assignment.rejected.fake", reasonRef: "reject.fake" }, { status: 409 })
        }
        if (accepted.has(assignmentRef)) {
          return Response.json({ statusRef: "assignment.duplicate.fake" }, { status: 409 })
        }
        accepted.add(assignmentRef)
        return Response.json({ statusRef: `assignment.accepted.${assignmentRef}` })
      }
      if (url.pathname.endsWith("/progress")) {
        if (input.cancelOnProgress) {
          return Response.json({ progressRef: "assignment.cancelled.fake" }, { status: 410 })
        }
        return Response.json({ progressRef: `assignment.progress.${body.leaseRef}.${body.sequence}` })
      }
      if (url.pathname.endsWith("/artifacts")) {
        expect(body.artifactRefs.length).toBeGreaterThan(0)
        expect(body.proofRefs.length).toBeGreaterThan(0)
        return Response.json({ artifactRef: `assignment.artifacts.${url.pathname.split("/").at(-2)}` })
      }
      if (url.pathname.endsWith("/closeout")) {
        expect(body.paymentMode).toBe("no-spend")
        expect(body.settlementState).toBe("not_applicable")
        expect(body.payoutClaimAllowed).toBe(false)
        expect(body.redacted).toBe(true)
        expect(body.closeoutRefs.length).toBeGreaterThan(0)
        return Response.json({ closeoutRef: `assignment.closeout.${body.leaseRef}` })
      }
      return Response.json({ errorRef: "error.not_found" }, { status: 404 })
    },
  })
  servers.push(server)
  return { baseUrl: `http://127.0.0.1:${server.port}`, requests }
}

async function readySummary(home: string, capabilityRefs: string[] = ["cap.gepa.retained.v1"]) {
  const summary = createBootstrapSummary(
    parseBootstrapArgs(["--display-name", "Assignment Test", ...capabilityRefs.flatMap(ref => ["--capability-ref", ref])]),
    { PYLON_HOME: home },
    "darwin",
  )
  const state = await ensurePylonLocalState(summary)
  await writeFile(
    state.paths.runtimeState,
      `${JSON.stringify({
        lifecycle: "assignment-ready",
        displayName: "Assignment Test",
        resourceMode: "background_20",
        capabilityRefs,
        blockerRefs: [],
        updatedAt: "2026-06-09T00:00:00.000Z",
      })}\n`,
  )
  return summary
}

describe("Pylon assignment lease flow", () => {
  test("polls, accepts, submits progress/proof refs, and closes a no-spend assignment", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer()
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected no-spend assignment to run")
      expect(result.closeout.paymentMode).toBe("no-spend")
      expect(result.closeout.settlementState).toBe("not_applicable")
      expect(result.closeout.payoutClaimAllowed).toBe(false)
      expect(result.closeout.artifactRefs[0].startsWith("assignment.artifact.")).toBe(true)
      expect(result.closeout.proofRefs[0].startsWith("assignment.proof.")).toBe(true)
      const pylonRef = fake.requests[0].body.pylonRef
      expect(fake.requests.map((request) => request.path).filter((path) => path.includes("/assignments/"))).toEqual([
        `/api/pylons/${encodeURIComponent(pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/accept`,
        `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/progress`,
        `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/artifacts`,
        `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/closeout`,
      ])
      expect(fake.requests.map((request) => request.path).filter((path) => path.endsWith("/assignments"))).toEqual([
        `/api/pylons/${encodeURIComponent(pylonRef)}/assignments`,
      ])

      const bundle = JSON.parse(
        await readFile(trainingWorkerReceiptsPathForHome(home), "utf8"),
      )
      expect(bundle.schema).toBe(
        "openagents.pylon.training_worker_receipts_bundle.v0.3",
      )
      expect(bundle.sourceRefs).toContain("source.pylon.assignment_closeout")
      expect(bundle.workerReceipts).toHaveLength(1)
      expect(bundle.workerReceipts[0]).toMatchObject({
        schema: "openagents.psionic.training_worker_receipt.v0.3",
        assignmentRef: result.lease.assignmentRef,
        workerRef: pylonRef,
      })
      expect(bundle.workerReceipts[0].receiptRef).toStartWith(
        "receipt.pylon.training_worker.",
      )
      expect(bundle.workerReceipts[0].checkpointRefs).toContain(
        result.closeoutReceipt.closeoutRef,
      )
      expect(bundle.workerReceipts[0].proofRefs).toEqual(
        expect.arrayContaining(result.closeout.proofRefs),
      )
      expect(JSON.stringify(bundle)).not.toContain(home)
      expect(JSON.stringify(bundle)).not.toContain("/Users/")
      assertPublicProjectionSafe(bundle)
    })
  })

  test("skips locally terminal leases that are still offered by the server", async () => {
    await withTempHome(async (home) => {
      const firstLease = lease({
        assignmentRef: "assignment.public.no_spend.rejected",
        leaseRef: "lease.public.no_spend.rejected",
      })
      const secondLease = lease({
        assignmentRef: "assignment.public.no_spend.next",
        leaseRef: "lease.public.no_spend.next",
      })
      const fake = fakeAssignmentServer({ leases: [firstLease, secondLease] })
      const summary = await readySummary(home)
      const state = await ensurePylonLocalState(summary)
      await writeFile(
        state.paths.assignmentState,
        `${JSON.stringify({
          schema: "openagents.pylon.assignment_state.v0.3",
          leases: {
            [firstLease.leaseRef]: {
              assignmentRef: firstLease.assignmentRef,
              status: "rejected",
              closedAt: "2026-06-09T00:00:10.000Z",
            },
          },
        }, null, 2)}\n`,
      )
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected runner to skip terminal lease")
      expect(result.lease.leaseRef).toBe(secondLease.leaseRef)
      const acceptPaths = fake.requests
        .map((request) => request.path)
        .filter((path) => path.endsWith("/accept"))
      expect(acceptPaths).toEqual([
        `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(secondLease.leaseRef)}/accept`,
      ])
    })
  })

  test("tries the next no-spend lease after a race-lost server acceptance", async () => {
    await withTempHome(async (home) => {
      const firstLease = lease({
        assignmentRef: "assignment.public.no_spend.race_lost",
        leaseRef: "lease.public.no_spend.race_lost",
      })
      const secondLease = lease({
        assignmentRef: "assignment.public.no_spend.parallel_slot",
        leaseRef: "lease.public.no_spend.parallel_slot",
      })
      const fake = fakeAssignmentServer({
        leases: [firstLease, secondLease],
        rejectAcceptRefs: [firstLease.leaseRef],
      })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected runner to claim the second lease")
      expect(result.lease.leaseRef).toBe(secondLease.leaseRef)
      const acceptPaths = fake.requests
        .map((request) => request.path)
        .filter((path) => path.endsWith("/accept"))
      expect(acceptPaths).toEqual([
        `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(firstLease.leaseRef)}/accept`,
        `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(secondLease.leaseRef)}/accept`,
      ])
    })
  })

  test("can target a single no-spend lease when older offers are still visible", async () => {
    await withTempHome(async (home) => {
      const oldLease = lease({
        assignmentRef: "assignment.public.no_spend.old_visible",
        leaseRef: "lease.public.no_spend.old_visible",
      })
      const targetLease = lease({
        assignmentRef: "assignment.public.no_spend.target",
        leaseRef: "lease.public.no_spend.target",
      })
      const fake = fakeAssignmentServer({ leases: [oldLease, targetLease] })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:00.000Z"),
      })

      const result = await runNoSpendAssignment(summary, {
        assignmentRef: targetLease.assignmentRef,
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected runner to claim target lease")
      expect(result.lease.leaseRef).toBe(targetLease.leaseRef)
      const acceptPaths = fake.requests
        .map((request) => request.path)
        .filter((path) => path.endsWith("/accept"))
      expect(acceptPaths).toEqual([
        `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(targetLease.leaseRef)}/accept`,
      ])
    })
  })

  test("executes runtime-gate coding assignment and reports only public-safe refs", async () => {
    await withTempHome(async (home) => {
      const codingAssignment = {
        assignmentRef: "pylon_assignment.public.runtime_gate.fixture_repair",
        budget: {
          paymentMode: "unpaid_smoke",
        },
        objective: {
          objectiveRef: "objective.public.pylon_runtime_gate.fixture_repair",
        },
        publicSafe: true,
        requiredCapabilityRefs: ["cap.gepa.retained.v1"],
        runtimeGate: {
          agentKind: "codex_cli_or_fixture",
          fixtureRef: "fixture.public.pylon.codex_runtime.sum_repair.v1",
          schema: "openagents.pylon.runtime_gate.v0.3",
        },
        schema: "openagents.autopilot_coding_assignment.v1",
      }
      const fake = fakeAssignmentServer({
        leases: [
          lease({
            assignmentRef: "pylon_assignment.public.runtime_gate.fixture_repair",
            codingAssignment,
            goal: "objective.public.pylon_runtime_gate.fixture_repair",
            leaseRef: "lease.public.runtime_gate.fixture_repair",
          }),
        ],
      })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected runtime gate assignment to run")
      expect(result.progress.artifactRefs[0].startsWith("artifact.pylon.runtime_gate.fixture_patch.")).toBe(true)
      expect(result.progress.proofRefs[0].startsWith("proof.pylon.runtime_gate.test_passed.")).toBe(true)
      expect(result.closeout.artifactRefs).toEqual(result.progress.artifactRefs)
      expect(result.closeout.proofRefs).toEqual(result.progress.proofRefs)
      expect(result.closeout.buildRefs[0].startsWith("command.pylon.runtime_gate.bun_test.")).toBe(true)
      expect(result.closeout.receiptRefs.some((ref) => ref.startsWith("run.pylon.runtime_gate."))).toBe(true)
      expect(result.closeout.resultRefs).toEqual(["result.public.pylon_runtime_gate.fixture_repair_passed"])
      expect(result.closeout.summaryRefs).toEqual(["summary.public.pylon_runtime_gate.fixture_repair_passed"])
      expect(result.closeout.testRefs).toEqual(result.closeout.buildRefs)
      const serverBodies = JSON.stringify(fake.requests.map((request) => request.body))
      expect(serverBodies).not.toContain(home)
      expect(serverBodies).not.toContain("/Users/")
      expect(serverBodies).not.toContain(".cache")
      expect(serverBodies).not.toContain("sum.ts")
      expect(serverBodies).not.toContain("left + right")
      expect(serverBodies).not.toContain("oa_agent")
      assertPublicProjectionSafe(result.closeout)
    })
  })

  test("normalizes current OpenAgents Autopilot coding assignment projections", async () => {
    await withTempHome(async (home) => {
      const codingAssignment = {
        assignmentRef: "pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract",
        budget: {
          paymentMode: "unpaid_smoke",
        },
        objective: {
          objectiveRef: "objective.autopilot_work_order.test_1.task.autopilot_coder.docs_contract",
        },
        publicSafe: true,
        requiredCapabilityRefs: ["cap.gepa.retained.v1"],
        schema: "openagents.autopilot_coding_assignment.v1",
      }
      const fake = fakeAssignmentServer({
        leases: [
          {
            assignmentRef: "pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract",
            codingAssignment,
            jobKind: "validation",
            leaseExpiresInSeconds: 600,
            state: "offered",
            taskRefs: ["autopilot_work_order.test_1", "task.autopilot_coder.docs_contract"],
          } as unknown as PylonAssignmentLease,
        ],
      })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const leases = await pollAssignments(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(leases).toEqual([
        expect.objectContaining({
          assignmentRef: "pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract",
          capabilityRefs: ["cap.gepa.retained.v1"],
          codingAssignment,
          goal: "objective.autopilot_work_order.test_1.task.autopilot_coder.docs_contract",
          leaseRef: "pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract",
          paymentMode: "no-spend",
        }),
      ])
    })
  })

  test("runs a projected claude_agent_task git_checkout assignment through no-spend closeout", async () => {
    await withTempHome(async (home) => {
      const codingAssignment = {
        assignmentRef: "pylon_assignment.autopilot_work_order.test_1.task.public_sum_repair",
        budget: {
          paymentMode: "unpaid_smoke",
        },
        objective: {
          objectiveRef: "objective.autopilot_work_order.test_1.task.public_sum_repair",
          publicSummary: "Repair the public sum fixture.",
        },
        publicSafe: true,
        requiredCapabilityRefs: ["capability.pylon.local_claude_agent"],
        claudeAgent: {
          agentKind: "claude_agent_sdk",
          allowedToolKinds: ["edit", "file", "git", "shell", "test_runner"],
          maxTurns: 8,
          schema: CLAUDE_AGENT_TASK_SCHEMA,
          timeoutSeconds: 120,
        },
        schema: "openagents.autopilot_coding_assignment.v1",
        workspace: {
          kind: "git_checkout",
          repository: {
            branch: "main",
            commitSha: "4444444444444444444444444444444444444444",
            fullName: "OpenAgentsInc/public-sum-fixture",
            provider: "github",
            visibility: "public",
          },
          verificationCommand: {
            args: ["bun", "test", "sum.test.ts"],
            commandRef: "command.public.autopilot_coder.bun_test_sum",
          },
        },
      }
      const checkoutRunner: ClaudeAgentCheckoutRunner = async (workspace) => {
        await mkdir(workspace, { recursive: true })
        await writeFile(
          join(workspace, "package.json"),
          `${JSON.stringify({ private: true, scripts: { test: "bun test sum.test.ts" }, type: "module" }, null, 2)}\n`,
        )
        await writeFile(join(workspace, "sum.ts"), "export const sum = (left: number, right: number) => left - right\n")
        await writeFile(
          join(workspace, "sum.test.ts"),
          [
            'import { describe, expect, test } from "bun:test"',
            'import { sum } from "./sum"',
            "",
            'describe("sum checkout", () => {',
            '  test("adds two numbers", () => {',
            "    expect(sum(2, 3)).toBe(5)",
            "  })",
            "})",
            "",
          ].join("\n"),
        )
      }
      const claudeAgentRunner: ClaudeAgentRunner = async (input) => {
        expect(input.cwd).toContain("claude-agent-tasks")
        expect(input.instructions).toContain("command.public.autopilot_coder.bun_test_sum")
        await writeFile(
          join(input.cwd, "sum.ts"),
          "export const sum = (left: number, right: number) => left + right\n",
        )
        return { commandCount: 1, editedFileCount: 1, outcome: "completed", sessionRef: null, turnCount: 3 }
      }
      const fake = fakeAssignmentServer({
        leases: [
          {
            assignmentRef: "pylon_assignment.autopilot_work_order.test_1.task.public_sum_repair",
            codingAssignment,
            jobKind: "claude_agent_task",
            leaseExpiresInSeconds: 600,
            state: "offered",
            taskRefs: ["autopilot_work_order.test_1", "task.public_sum_repair"],
          } as unknown as PylonAssignmentLease,
        ],
      })
      const summary = await readySummary(home, ["capability.pylon.local_claude_agent"])
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        claudeAgentCheckoutRunner: checkoutRunner,
        claudeAgentProbe: {
          env: { ANTHROPIC_API_KEY: "test-key-shape" },
          importer: async (specifier: string) => {
            if (specifier !== CLAUDE_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
            return {}
          },
          platform: "darwin",
        },
        claudeAgentRunner,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected claude_agent_task git checkout assignment to run")
      expect(result.closeout.resultRefs).toContain("result.public.pylon.claude_agent_task.git_checkout_verified_passed")
      expect(result.closeout.blockerRefs).toEqual([])
      expect(result.closeout.artifactRefs[0]).toStartWith("artifact.pylon.claude_agent_task.patch.")
      expect(result.closeout.testRefs[0]).toStartWith("command.pylon.claude_agent_task.verification.")
      expect(result.closeout.previewRefs[0]).toStartWith("workspace.pylon.claude_agent_task.")
      const serverBodies = JSON.stringify(fake.requests.map((request) => request.body))
      expect(serverBodies).not.toContain(home)
      expect(serverBodies).not.toContain("/Users/")
      expect(serverBodies).not.toContain("OpenAgentsInc/public-sum-fixture")
      expect(serverBodies).not.toContain("Repair the public sum fixture.")
      expect(serverBodies).not.toContain("left + right")
      assertPublicProjectionSafe(result.closeout)
    })
  })

  test("polls, accepts, submits progress/proof refs, and closes a no-spend assignment with bearer auth", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer()
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const result = await runNoSpendAssignment(summary, {
        agentToken: "oa_agent_test",
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected bearer no-spend assignment to run")
      expect(fake.requests.find((request) => request.path.endsWith("/assignments"))?.headers.get("authorization")).toBe(
        "Bearer oa_agent_test",
      )
      expect(fake.requests.find((request) => request.path.endsWith("/accept"))?.headers.get("Idempotency-Key")).toContain(
        "pylon.assignment.",
      )
      expect(result.closeout.receiptRefs.some((ref) => ref.startsWith("assignment.artifacts."))).toBe(true)
    })
  })

  test("NIP-98 assignment polling works for local harnesses without legacy custom signature headers", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer()
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      await pollAssignments(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      const poll = fake.requests.find((request) => request.path.endsWith("/assignments"))

      expect(poll?.headers.get("authorization")?.startsWith("Nostr ")).toBe(true)
      expect(poll?.headers.get("x-nip98-body-sha256")).toBeNull()
      expect(poll?.headers.get("x-nip98-signature")).toBeNull()
      expect(poll?.headers.get("x-nip98-pubkey")).toBeNull()
    })
  })

  test("blocks duplicate local lease acceptance before contacting the server", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer()
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })
      const candidate = lease()

      const first = await acceptAssignment(summary, candidate, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })
      const second = await acceptAssignment(summary, candidate, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:31.000Z"),
      })

      expect(first.accepted).toBe(true)
      expect(second.accepted).toBe(false)
      expect(second.blockerRefs).toContain("blocker.assignment.duplicate_lease")
      expect(fake.requests.filter((request) => request.path.endsWith("/accept"))).toHaveLength(1)
    })
  })

  test("denies paused, stale, wrong-capability, unsupported-backend, and expired leases without wallet gating", async () => {
    await withTempHome(async (home) => {
      const summary = await readySummary(home)
      const state = await ensurePylonLocalState(summary)
      await writePresenceState(state.paths, {
        registered: true,
        linked: false,
        stale: true,
        pylonRef: state.identity.pylonRef,
        registrationRef: "registration.test",
        linkRef: null,
        lastHeartbeatAt: "2026-06-09T00:00:00.000Z",
        heartbeatSequence: 1,
        blockerRefs: [],
        updatedAt: "2026-06-09T00:00:00.000Z",
      })
      await writeFile(
        state.paths.runtimeState,
        `${JSON.stringify({
          ...state.runtime,
          lifecycle: "paused",
          capabilityRefs: ["cap.other"],
          blockerRefs: [],
          updatedAt: "2026-06-09T00:00:00.000Z",
        })}\n`,
      )
      const refreshed = {
        ...(await ensurePylonLocalState(summary)),
        runtime: {
          ...state.runtime,
          lifecycle: "paused" as const,
          capabilityRefs: ["cap.other"],
          blockerRefs: [],
          updatedAt: "2026-06-09T00:00:00.000Z",
        },
      }
      const admission = await computeAssignmentAdmission(
        refreshed,
        lease({
          paymentMode: "paid",
          backendRef: "backend.gemini",
          expiresAt: "2026-06-09T00:00:01.000Z",
        }),
        {
          now: () => new Date("2026-06-09T00:05:00.000Z"),
        },
      )

      expect(admission.admissible).toBe(false)
      expect(admission.blockerRefs).toContain("blocker.assignment.lifecycle_paused")
      expect(admission.blockerRefs).toContain("blocker.assignment.presence_stale")
      expect(admission.blockerRefs).toContain("blocker.assignment.wrong_capability")
      expect(admission.blockerRefs).toContain("blocker.assignment.unsupported_backend")
      expect(admission.blockerRefs).not.toContain("blocker.assignment.wallet_blocked")
      expect(admission.blockerRefs).toContain("blocker.assignment.lease_expired")
    })
  })

  test("distinguishes Psionic 0.8B fallback from 2B-required assignment admission", async () => {
    await withTempHome(async (home) => {
      const summary = await readySummary(home)
      const state = await ensurePylonLocalState(summary)
      await writePresenceState(state.paths, {
        registered: true,
        linked: false,
        stale: false,
        pylonRef: state.identity.pylonRef,
        registrationRef: "registration.test",
        linkRef: null,
        lastHeartbeatAt: "2026-06-09T00:00:00.000Z",
        heartbeatSequence: 1,
        blockerRefs: [],
        updatedAt: "2026-06-09T00:00:00.000Z",
      })
      const only08b = psionicAdmission([PSIONIC_QWEN_MODEL_REFS.qwen35_0_8b])
      const fallback = await computeAssignmentAdmission(
        state,
        lease({
          capabilityRefs: ["cap.gepa.retained.v1"],
          backendRef: undefined,
          psionicQwenRequirements: {
            workClass: "local_inference",
            mode: "coding_agent",
          },
        }),
        {
          now: () => new Date("2026-06-09T00:00:30.000Z"),
          psionicQwenAdmission: only08b,
        },
      )
      const requires2b = await computeAssignmentAdmission(
        state,
        lease({
          capabilityRefs: ["cap.gepa.retained.v1"],
          psionicQwenRequirements: {
            workClass: "local_inference",
            mode: "requires_2b",
            requiredModelRef: PSIONIC_QWEN_MODEL_REFS.qwen35_2b,
          },
        }),
        {
          now: () => new Date("2026-06-09T00:00:30.000Z"),
          psionicQwenAdmission: only08b,
        },
      )

      expect(fallback.admissible).toBe(true)
      expect(requires2b.admissible).toBe(false)
      expect(requires2b.blockerRefs).toContain("blocker.psionic_qwen35.model_2b_missing")
    })
  })

  test("closeout receipts include Psionic backend and model refs without raw artifacts", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer({
        leases: [
          lease({
            psionicQwenRequirements: {
              workClass: "local_inference",
              mode: "requires_2b",
              requiredModelRef: PSIONIC_QWEN_MODEL_REFS.qwen35_2b,
              receiptRefs: ["receipt.psionic.backend.redacted"],
            },
          }),
        ],
      })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })
      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
        psionicQwenAdmission: psionicAdmission([PSIONIC_QWEN_MODEL_REFS.qwen35_2b]),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected Psionic assignment to run")
      expect(result.closeout.receiptRefs).toContain("backend.psionic.qwen35")
      expect(result.closeout.receiptRefs).toContain("model.psionic.qwen35.2b.q8_0")
      expect(result.closeout.receiptRefs).toContain("receipt.psionic.backend.redacted")
      expect(JSON.stringify(result.closeout)).not.toContain("/Users/")
      expect(JSON.stringify(result.closeout)).not.toContain(".gguf")
    })
  })

  test("poll handles server cancellation/rejection and explicit timeout closeout shapes", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer({ leases: [lease({ leaseRef: "lease.public.timeout" })] })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })
      const leases = await pollAssignments(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:30.000Z") })
      const closeout = {
        schema: "openagents.pylon.assignment_closeout.v0.3",
        assignmentRef: leases[0].assignmentRef,
        leaseRef: leases[0].leaseRef,
        status: "timed-out",
        paymentMode: "no-spend",
        settlementState: "not_applicable",
        payoutClaimAllowed: false,
        artifactRefs: [],
        blockerRefs: ["blocker.assignment.timeout"],
        buildRefs: [],
        closeoutRefs: ["assignment.closeout.timeout"],
        previewRefs: [],
        proofRefs: ["assignment.proof.timeout"],
        receiptRefs: [],
        resultRefs: [],
        summaryRefs: ["assignment.summary.timeout"],
        testRefs: [],
        redacted: true,
        completedAt: "2026-06-09T00:01:00.000Z",
      } as const

      assertPublicProjectionSafe(closeout)
      const result = await submitAssignmentCloseout(summary, closeout, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })
      expect(result.closeoutRef).toBe("assignment.closeout.lease.public.timeout")
    })
  })

  test("returns public-safe denial refs for server-side accept rejection", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer({ rejectAccept: true })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const result = await acceptAssignment(summary, lease(), {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.accepted).toBe(false)
      expect(result.denialRef).toBe("denial.assignment.server_rejected")
      expect(result.blockerRefs).toContain("blocker.assignment.server_rejected")
      assertPublicProjectionSafe(result)
    })
  })

  test("converts mid-run cancellation into a no-spend cancelled closeout", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer({ cancelOnProgress: true })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(false)
      if (result.ok) throw new Error("expected cancellation closeout")
      expect(result.closeout.status).toBe("cancelled")
      expect(result.closeout.paymentMode).toBe("no-spend")
      expect(result.closeout.settlementState).toBe("not_applicable")
      expect(result.closeout.payoutClaimAllowed).toBe(false)
      expect(result.closeout.proofRefs[0].startsWith("assignment.proof.failure.")).toBe(true)
      assertPublicProjectionSafe(result.closeout)
    })
  })
})

function psionicAdmission(modelRefs: PsionicQwenModelAdmission["admittedModelRefs"]): PsionicQwenModelAdmission {
  return {
    rows: [],
    admittedModelRefs: modelRefs,
    observedModelRefs: modelRefs,
    blockerRefs: [],
  }
}
