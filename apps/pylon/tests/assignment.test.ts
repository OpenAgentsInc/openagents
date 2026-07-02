import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { afterEach, describe, expect, test } from "bun:test"
import { decodePylonLifecycleWireEventJson } from "@openagentsinc/agent-runtime-schema"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  acceptAssignment,
  boundedAssignmentProgressMessage,
  computeAssignmentAdmission,
  pollAssignments,
  runNoSpendAssignment,
  submitAssignmentCloseout,
  submitAssignmentProgress,
  trainingWorkerReceiptsPathForHome,
  type AssignmentRunLifecycleEvent,
  type PylonAssignmentLease,
} from "../src/assignment"
import { sendHeartbeat } from "../src/presence"
import { verifyNip98Authorization } from "../src/nostr-identity"
import { assertPublicProjectionSafe, ensurePylonLocalState, writePresenceState } from "../src/state"
import { PSIONIC_QWEN_MODEL_REFS, type PsionicQwenModelAdmission } from "../packages/runtime/src/index"
import { CLAUDE_AGENT_SDK_PACKAGE } from "../src/claude-agent"
import {
  CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
  CLAUDE_AGENT_TASK_SCHEMA,
  type ClaudeAgentCheckoutRunner,
  type ClaudeAgentRunner,
} from "../src/claude-agent-executor"
import {
  CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
  CODEX_AGENT_TASK_SCHEMA,
  type CodexAgentRunner,
} from "../src/codex-agent-executor"
import { hashPylonAccountRef } from "../src/account-registry"
import { loadCodexAccountHealthRecord } from "../src/codex-account-health-ledger"
import { activeCodingRunCounts } from "../src/active-assignment-runs"

const INDEX = join(import.meta.dir, "..", "src", "index.ts")
const CWD = join(import.meta.dir, "..")
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
  failHeartbeats?: number
  rejectAccept?: boolean
  rejectAcceptRefs?: ReadonlyArray<string>
  cancelOnProgress?: boolean
  rejectLongProgressMessage?: boolean
  rejectUnsafeProgressMessage?: boolean
  rejectLocalProgressShape?: boolean
  rejectProgressStatus?: number
  authNow?: Date
  maxSkewSeconds?: number
} = {}) {
  const requests: { path: string; body: any; headers: Headers }[] = []
  const accepted = new Set<string>()
  let heartbeatFailures = input.failHeartbeats ?? 0
  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url)
      const text = await request.text()
      const body = text ? JSON.parse(text) : {}
      requests.push({ path: url.pathname, body, headers: request.headers })

      if (request.headers.get("authorization")?.startsWith("Bearer ")) {
        if (request.method === "POST" && url.pathname !== "/api/operator/pro/status") {
          expect(request.headers.get("Idempotency-Key")).toContain(
            url.pathname.includes("/heartbeat")
              ? "pylon-presence:"
              : url.pathname === "/api/pylon/claude/turns"
                ? "pylon.claude.turn"
              : "pylon.assignment.",
          )
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
          now: input.authNow ?? new Date("2026-06-09T00:00:30.000Z"),
          maxSkewSeconds: input.maxSkewSeconds ?? 300,
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
        if (heartbeatFailures > 0) {
          heartbeatFailures -= 1
          return Response.json({ errorRef: "error.presence.temporarily_unavailable" }, { status: 503 })
        }
        return Response.json({ heartbeatRef: `heartbeat.${body.pylonRef}.${body.sequence}` })
      }
      if (url.pathname === "/api/pylon/claude/turns") {
        expect(body.schemaVersion).toBe("openagents.pylon.claude_turn.v1")
        expect(body.usage).toEqual({
          inputTokens: 100,
          cachedInputTokens: 25,
          outputTokens: 40,
        })
        return Response.json({ ok: true, turnRef: `turn.claude.${body.assignmentRef}.${body.turnIndex}` })
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
        if (
          input.rejectLongProgressMessage &&
          typeof body.message === "string" &&
          body.message.length > 240
        ) {
          return Response.json({ errorRef: "error.assignment_progress.message_too_long" }, { status: 400 })
        }
        if (
          input.rejectUnsafeProgressMessage &&
          typeof body.message === "string" &&
          /(\/Users\/|token|oauth|secret|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})/i.test(body.message)
        ) {
          return Response.json({ errorRef: "error.assignment_progress.unsafe_message" }, { status: 400 })
        }
        if (
          input.rejectLocalProgressShape &&
          (
            "assignmentRef" in body ||
            "leaseRef" in body ||
            "observedAt" in body ||
            "proofRefs" in body ||
            "schema" in body ||
            "sequence" in body
          )
        ) {
          return Response.json({ errorRef: "error.assignment_progress.local_shape" }, { status: 400 })
        }
        if (input.rejectProgressStatus !== undefined) {
          return Response.json(
            { errorRef: `error.assignment_progress.http_${input.rejectProgressStatus}` },
            { status: input.rejectProgressStatus },
          )
        }
        const assignmentRef = decodeURIComponent(url.pathname.split("/").at(-2) ?? "")
        const progressRefSuffix =
          typeof body.sequence === "number"
            ? String(body.sequence)
            : typeof body.status === "string"
              ? body.status
              : "event"
        return Response.json({ progressRef: `assignment.progress.${body.leaseRef ?? assignmentRef}.${progressRefSuffix}` })
      }
      if (url.pathname.endsWith("/artifacts")) {
        expect(body.artifactRefs.length).toBeGreaterThan(0)
        expect(body.proofRefs.length).toBeGreaterThan(0)
        return Response.json({ artifactRef: `assignment.artifacts.${url.pathname.split("/").at(-2)}` })
      }
      if (url.pathname.endsWith("/closeout")) {
        expect(body.closeoutRefs.length).toBeGreaterThan(0)
        expect(body.status).toBeTruthy()
        expect(body).not.toHaveProperty("assignmentRef")
        expect(body).not.toHaveProperty("completedAt")
        expect(body).not.toHaveProperty("leaseRef")
        expect(body).not.toHaveProperty("paymentMode")
        expect(body).not.toHaveProperty("payoutClaimAllowed")
        expect(body).not.toHaveProperty("receiptRefs")
        expect(body).not.toHaveProperty("redacted")
        expect(body).not.toHaveProperty("schema")
        expect(body).not.toHaveProperty("settlementState")
        const assignmentRef = decodeURIComponent(url.pathname.split("/").at(-2) ?? "")
        return Response.json({ closeoutRef: `assignment.closeout.${assignmentRef}` })
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
  test("admission blocks ambiguous mixed-runner coding assignments", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer()
      const summary = await readySummary(home, [])
      await sendHeartbeat(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:00.000Z"),
      })
      const state = await ensurePylonLocalState(summary)
      const admission = await computeAssignmentAdmission(
        state,
        lease({
          capabilityRefs: [],
          codingAssignment: {
            claudeAgent: {
              schema: CLAUDE_AGENT_TASK_SCHEMA,
              agentKind: "claude_agent_sdk",
              fixtureRef: CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
            },
            codex: {
              schema: CODEX_AGENT_TASK_SCHEMA,
              agentKind: "codex_sdk",
              fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
            },
          },
          expiresAt: "2026-06-09T01:00:00.000Z",
        }),
        { now: () => new Date("2026-06-09T00:00:30.000Z") },
      )

      expect(admission.admissible).toBe(false)
      expect(admission.blockerRefs).toContain("blocker.assignment.agent_runner_ambiguous")
    })
  })

  test("bounds assignment progress messages before posting", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer({ rejectLongProgressMessage: true })
      const summary = await readySummary(home)
      const longMessage = `Proof-ready closeout ${"with extra public context ".repeat(16)}`

      const result = await submitAssignmentProgress(
        summary,
        {
          schema: "openagents.pylon.assignment_progress.v0.3",
          assignmentRef: "assignment.public.no_spend.long_progress",
          leaseRef: "lease.public.no_spend.long_progress",
          sequence: 1,
          status: "proof-ready",
          message: longMessage,
          artifactRefs: [],
          proofRefs: [],
          observedAt: "2026-06-09T00:00:30.000Z",
        },
        {
          baseUrl: fake.baseUrl,
          now: () => new Date("2026-06-09T00:00:30.000Z"),
        },
      )

      const progressRequest = fake.requests.find((request) => request.path.endsWith("/progress"))
      expect(result.progressRef).toBe("assignment.progress.lease.public.no_spend.long_progress.proof-ready")
      expect(progressRequest?.body.message).toBe(boundedAssignmentProgressMessage(longMessage))
      expect(progressRequest?.body.message.length).toBeLessThanOrEqual(240)
      expect(progressRequest?.body.message.endsWith("...")).toBe(true)
      expect(progressRequest?.body.progressRefs).toEqual([])
      expect(progressRequest?.body.blockerRefs).toEqual([])
      expect(progressRequest?.body).not.toHaveProperty("assignmentRef")
      expect(progressRequest?.body).not.toHaveProperty("leaseRef")
      expect(progressRequest?.body).not.toHaveProperty("proofRefs")
    })
  })

  test("polls, accepts, submits progress/proof refs, and closes a no-spend assignment", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer({ rejectLocalProgressShape: true })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })
      const lifecycleEvents: AssignmentRunLifecycleEvent[] = []

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
        onLifecycleEvent: (event) => lifecycleEvents.push(event),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected no-spend assignment to run")
      expect(result.closeout.paymentMode).toBe("no-spend")
      expect(result.closeout.settlementState).toBe("not_applicable")
      expect(result.closeout.payoutClaimAllowed).toBe(false)
      expect(result.closeout.artifactRefs[0].startsWith("assignment.artifact.")).toBe(true)
      expect(result.closeout.proofRefs[0].startsWith("assignment.proof.")).toBe(true)
      const pylonRef = fake.requests[0].body.pylonRef
      const assignmentPaths = fake.requests.map((request) => request.path).filter((path) => path.includes("/assignments/"))
      const expectedAcceptPath = `/api/pylons/${encodeURIComponent(pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/accept`
      const expectedProgressPath = `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/progress`
      const expectedArtifactsPath = `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/artifacts`
      const expectedCloseoutPath = `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/closeout`
      expect(assignmentPaths[0]).toBe(expectedAcceptPath)
      expect(assignmentPaths.at(-2)).toBe(expectedArtifactsPath)
      expect(assignmentPaths.at(-1)).toBe(expectedCloseoutPath)
      expect(assignmentPaths.filter((path) => path === expectedProgressPath).length).toBeGreaterThanOrEqual(2)
      expect(fake.requests.some((request) =>
        request.path === expectedProgressPath &&
        request.body.status === "running" &&
        request.body.phase === "running"
      )).toBe(true)
      const finalProgress = fake.requests
        .filter((request) => request.path === expectedProgressPath)
        .map((request) => request.body)
        .find((body) => body.status === "proof-ready")
      expect(finalProgress).toMatchObject({
        artifactRefs: result.progress.artifactRefs,
        blockerRefs: [],
        progressRefs: result.progress.proofRefs,
        status: "proof-ready",
      })
      expect(finalProgress).not.toHaveProperty("assignmentRef")
      expect(finalProgress).not.toHaveProperty("leaseRef")
      expect(finalProgress).not.toHaveProperty("proofRefs")
      expect(finalProgress).not.toHaveProperty("schema")
      expect(finalProgress).not.toHaveProperty("sequence")
      const closeoutRequest = fake.requests.find((request) => request.path === expectedCloseoutPath)
      expect(closeoutRequest?.body).toMatchObject({
        artifactRefs: result.closeout.artifactRefs,
        authorityReceiptRefs: result.closeout.receiptRefs,
        blockerRefs: [],
        buildRefs: result.closeout.buildRefs,
        closeoutRefs: result.closeout.closeoutRefs,
        proofRefs: result.closeout.proofRefs,
        resultRefs: result.closeout.resultRefs,
        status: "closeout_submitted",
        summaryRefs: result.closeout.summaryRefs,
        testRefs: result.closeout.testRefs,
      })
      expect(closeoutRequest?.body).not.toHaveProperty("assignmentRef")
      expect(closeoutRequest?.body).not.toHaveProperty("leaseRef")
      expect(closeoutRequest?.body).not.toHaveProperty("paymentMode")
      expect(closeoutRequest?.body).not.toHaveProperty("settlementState")
      expect(closeoutRequest?.body).not.toHaveProperty("payoutClaimAllowed")
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
      expect(lifecycleEvents.map((event) => event.event)).toEqual([
        "assignment_run.poll_complete",
        "assignment_run.accepted",
        "assignment_run.runtime_started",
        "assignment_run.runtime_progress",
        "assignment_run.progress_submitted",
        "assignment_run.artifacts_submitted",
        "assignment_run.closeout_submitted",
        "assignment_run.completed",
      ])
      expect(lifecycleEvents[0]).toMatchObject({
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        leaseCount: 1,
        candidateCount: 1,
      })
      expect(lifecycleEvents.at(-1)).toMatchObject({
        assignmentRef: result.lease.assignmentRef,
        leaseRef: result.lease.leaseRef,
        status: "accepted",
        closeoutRef: result.closeoutReceipt.closeoutRef,
      })
      const lifecycleJson = JSON.stringify(lifecycleEvents)
      expect(lifecycleJson).not.toContain(home)
      expect(lifecycleJson).not.toContain("/Users/")
      expect(lifecycleJson).not.toContain("accountHome")
      expect(lifecycleJson).not.toContain("provider")
    })
  })

  test("lifecycle reporter failures do not fail no-spend assignment execution", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer()
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
        onLifecycleEvent: () => {
          throw new Error("lifecycle sink unavailable")
        },
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected no-spend assignment to run")
      expect(result.closeout.status).toBe("accepted")
    })
  })

  test("run-no-spend refreshes stale presence before claiming a lease (#6354)", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer()
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
        heartbeatSequence: 7,
        blockerRefs: ["blocker.assignment.presence_stale"],
        sparkPayoutTargetRef: null,
        updatedAt: "2026-06-09T00:00:00.000Z",
      })

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:02:30.000Z"),
      })

      expect(result.ok).toBe(true)
      const heartbeatIndex = fake.requests.findIndex((request) => request.path.includes("/heartbeat"))
      const acceptIndex = fake.requests.findIndex((request) => request.path.endsWith("/accept"))
      expect(heartbeatIndex).toBeGreaterThanOrEqual(0)
      expect(acceptIndex).toBeGreaterThan(heartbeatIndex)
      expect(fake.requests[heartbeatIndex].body.sequence).toBe(8)
    })
  })

  test("run-no-spend emits actionable stale-presence recovery when heartbeat refresh fails (#6354)", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer({ failHeartbeats: 1 })
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
        heartbeatSequence: 7,
        blockerRefs: ["blocker.assignment.presence_stale"],
        sparkPayoutTargetRef: null,
        updatedAt: "2026-06-09T00:00:00.000Z",
      })

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:02:30.000Z"),
      })

      expect(result.ok).toBe(false)
      if (result.ok) throw new Error("expected stale-presence diagnostic")
      expect(result.acceptance.blockerRefs).toContain("blocker.assignment.presence_stale")
      expect(result.diagnostic).toMatchObject({
        schema: "openagents.pylon.assignment_recovery_diagnostic.v0.1",
        blockerRefs: ["blocker.assignment.presence_stale"],
        diagnosticRef: "diagnostic.assignment.presence_heartbeat_required",
        heartbeatStatus: 503,
        recoveryCommand: `pylon presence heartbeat --base-url ${fake.baseUrl}`,
      })
      assertPublicProjectionSafe(result.diagnostic)
    })
  })

  test("CLI run-no-spend --json streams lifecycle JSONL on stderr and final result JSON on stdout", async () => {
    await withTempHome(async (home) => {
      const cliLease = lease({
        assignmentRef: "assignment.public.no_spend.cli_status",
        leaseRef: "lease.public.no_spend.cli_status",
        expiresAt: "2026-07-09T01:00:00.000Z",
      })
      const fake = fakeAssignmentServer({
        leases: [cliLease],
        authNow: new Date(),
        maxSkewSeconds: 300,
      })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl })

      const proc = Bun.spawn(
        [
          "bun",
          INDEX,
          "assignment",
          "run-no-spend",
          "--base-url",
          fake.baseUrl,
          "--assignment-ref",
          cliLease.assignmentRef,
          "--json",
        ],
        {
          cwd: CWD,
          env: {
            ...Bun.env,
            PYLON_HOME: home,
          },
          stdout: "pipe",
          stderr: "pipe",
        },
      )
      const [stdout, stderr, exitCode] = await Promise.all([
        new Response(proc.stdout).text(),
        new Response(proc.stderr).text(),
        proc.exited,
      ])

      expect(exitCode, stderr || stdout).toBe(0)
      const result = JSON.parse(stdout)
      expect(result.ok).toBe(true)
      expect(result.lease.assignmentRef).toBe(cliLease.assignmentRef)
      expect(stdout).not.toContain("assignment_run.")

      const events = stderr.trim().split("\n").map((line) => decodePylonLifecycleWireEventJson(line))
      expect(events.map((event) => event.event)).toEqual([
        "assignment_run.poll_complete",
        "assignment_run.accepted",
        "assignment_run.runtime_started",
        "assignment_run.runtime_progress",
        "assignment_run.progress_submitted",
        "assignment_run.artifacts_submitted",
        "assignment_run.closeout_submitted",
        "assignment_run.completed",
      ])
      expect(events.every((event) => event.schema === "openagents.pylon.assignment_run_lifecycle_event.v0.1")).toBe(true)
      expect(events.at(-1)).toMatchObject({
        assignmentRef: cliLease.assignmentRef,
        leaseRef: cliLease.leaseRef,
        status: "accepted",
        closeoutRef: `assignment.closeout.${cliLease.leaseRef}`,
      })
      expect(stderr).not.toContain(home)
      expect(stderr).not.toContain("/Users/")
      expect(stderr).not.toContain("accountHome")
      expect(stderr).not.toContain("provider")
    })
  })

  test("routes Codex no-spend assignments through the selected Codex account", async () => {
    await withTempHome(async (home) => {
      const codexLease = lease({
        capabilityRefs: ["capability.pylon.local_codex"],
        codingAssignment: {
          codex: {
            agentKind: "codex_sdk",
            fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
            schema: CODEX_AGENT_TASK_SCHEMA,
          },
        },
      })
      const fake = fakeAssignmentServer({
        leases: [codexLease],
        rejectUnsafeProgressMessage: true,
      })
      const summary = await readySummary(home, ["capability.pylon.local_codex"])
      const state = await ensurePylonLocalState(summary)
      const codexHome = join(home, "accounts/codex/codex-a")
      await mkdir(codexHome, { recursive: true })
      await writeFile(join(codexHome, "auth.json"), "{}\n")
      await writeFile(
        state.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [{ provider: "codex", ref: "codex-a", home: codexHome }],
            },
          },
          null,
          2,
        )}\n`,
      )
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      let seenAccountRef: string | null = null
      let seenCodexHome: string | null = null
      const fixingCodexRunner: CodexAgentRunner = async (input) => {
        seenAccountRef = input.account?.accountRef ?? null
        seenCodexHome = input.env?.CODEX_HOME ?? null
        await writeFile(
          join(input.cwd, "sum.ts"),
          "export const sum = (left: number, right: number) => left + right\n",
        )
        return { commandCount: 1, editedFileCount: 1, outcome: "completed", sessionRef: null, turnCount: 1 }
      }

      const result = await runNoSpendAssignment(summary, {
        accountRef: "codex-a",
        baseUrl: fake.baseUrl,
        codexAgentProbe: {
          env: {},
          importer: async () => ({}),
          platform: "darwin",
        },
        codexAuthValidityProbe: async () => ({ valid: true }),
        codexAgentRunner: fixingCodexRunner,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected Codex assignment to run")
      expect(seenAccountRef).toBe("codex-a")
      expect(seenCodexHome).toBe(codexHome)
      expect(result.closeout.resultRefs).toContain(
        "result.public.pylon.codex_agent_task.fixture_repair_passed",
      )
    })
  })

  test("skips revoked Codex accounts during no-spend account selection", async () => {
    await withTempHome(async (home) => {
      const codexLease = lease({
        assignmentRef: "assignment.public.no_spend.codex_health_gate",
        leaseRef: "lease.public.no_spend.codex_health_gate",
        capabilityRefs: ["capability.pylon.local_codex"],
        codingAssignment: {
          codex: {
            agentKind: "codex_sdk",
            fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
            schema: CODEX_AGENT_TASK_SCHEMA,
          },
        },
      })
      const fake = fakeAssignmentServer({
        leases: [codexLease],
        rejectUnsafeProgressMessage: true,
      })
      const summary = await readySummary(home, ["capability.pylon.local_codex"])
      const state = await ensurePylonLocalState(summary)
      const revokedHome = join(home, "accounts/codex/codex-revoked")
      const goodHome = join(home, "accounts/codex/codex-good")
      await mkdir(revokedHome, { recursive: true })
      await mkdir(goodHome, { recursive: true })
      await writeFile(join(revokedHome, "auth.json"), "{}\n")
      await writeFile(join(goodHome, "auth.json"), "{}\n")
      await writeFile(
        state.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [
                { provider: "codex", ref: "codex-revoked", home: revokedHome },
                { provider: "codex", ref: "codex-good", home: goodHome },
              ],
            },
          },
          null,
          2,
        )}\n`,
      )
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      let seenAccountRef: string | null = null
      const fixingCodexRunner: CodexAgentRunner = async (input) => {
        seenAccountRef = input.account?.accountRef ?? null
        await writeFile(
          join(input.cwd, "sum.ts"),
          "export const sum = (left: number, right: number) => left + right\n",
        )
        return { commandCount: 1, editedFileCount: 1, outcome: "completed", sessionRef: null, turnCount: 1 }
      }

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        codexAgentProbe: {
          env: {},
          importer: async () => ({}),
          platform: "darwin",
        },
        codexAuthValidityProbe: async input =>
          input.home === revokedHome
            ? { valid: false, reason: "credentials_revoked" }
            : { valid: true },
        codexAgentRunner: fixingCodexRunner,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected Codex assignment to run")
      expect(seenAccountRef).toBe("codex-good")
      expect(
        (await loadCodexAccountHealthRecord(
          summary,
          hashPylonAccountRef("codex", "codex-revoked"),
        ))?.reason,
      ).toBe("credentials_revoked")
    })
  })

  test("emits public-safe live runtime progress while Codex assignment is active", async () => {
    await withTempHome(async (home) => {
      const codexLease = lease({
        assignmentRef: "assignment.public.no_spend.codex_live_progress",
        leaseRef: "lease.public.no_spend.codex_live_progress",
        capabilityRefs: ["capability.pylon.local_codex"],
        codingAssignment: {
          codex: {
            agentKind: "codex_sdk",
            fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
            schema: CODEX_AGENT_TASK_SCHEMA,
          },
        },
      })
      const fake = fakeAssignmentServer({
        leases: [codexLease],
        rejectUnsafeProgressMessage: true,
      })
      const summary = await readySummary(home, ["capability.pylon.local_codex"])
      const state = await ensurePylonLocalState(summary)
      const codexHome = join(home, "accounts/codex/codex-live")
      await mkdir(codexHome, { recursive: true })
      await writeFile(join(codexHome, "auth.json"), "{}\n")
      await writeFile(
        state.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [{ provider: "codex", ref: "codex-live", home: codexHome }],
            },
          },
          null,
          2,
        )}\n`,
      )
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      let runnerCompleted = false
      let progressObservedWhileActive = false
      const lifecycleEvents: AssignmentRunLifecycleEvent[] = []
      const slowFixingCodexRunner: CodexAgentRunner = async (input) => {
        await new Promise((resolve) => setTimeout(resolve, 30))
        await writeFile(
          join(input.cwd, "sum.ts"),
          "export const sum = (left: number, right: number) => left + right\n",
        )
        runnerCompleted = true
        return { commandCount: 1, editedFileCount: 1, outcome: "completed", sessionRef: null, turnCount: 1 }
      }

      const result = await runNoSpendAssignment(summary, {
        accountRef: "codex-live",
        baseUrl: fake.baseUrl,
        codexAgentProbe: {
          env: {},
          importer: async () => ({}),
          platform: "darwin",
        },
        codexAuthValidityProbe: async () => ({ valid: true }),
        codexAgentRunner: slowFixingCodexRunner,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
        runtimeProgressIntervalMs: 1,
        onLifecycleEvent: (event) => {
          lifecycleEvents.push(event)
          if (event.event === "assignment_run.runtime_progress" && !runnerCompleted) {
            progressObservedWhileActive = true
          }
        },
      })

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected Codex assignment to run")
      expect(progressObservedWhileActive).toBe(true)
      const progressEvents = lifecycleEvents.filter(
        (event) => event.event === "assignment_run.runtime_progress",
      )
      expect(progressEvents.length).toBeGreaterThan(0)
      const runtimeActiveProgress = progressEvents.find(
        (event) => event.phase === "runtime_active",
      )
      expect(runtimeActiveProgress).toMatchObject({
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        assignmentRef: codexLease.assignmentRef,
        accountRefHash: expect.stringMatching(/^account\.pylon\.codex\.[a-f0-9]{24}$/),
        leaseRef: codexLease.leaseRef,
        phase: "runtime_active",
        lastProgressEvent: "assignment_run.runtime_started",
      })
      expect(typeof runtimeActiveProgress?.elapsedMs).toBe("number")
      expect(runtimeActiveProgress?.elapsedMs).toBeGreaterThanOrEqual(0)
      const postedRuntimeProgress = fake.requests
        .filter((request) => request.path.endsWith("/progress"))
        .map((request) => request.body)
        .find((body) => body.status === "running" && body.phase === "proof")
      expect(postedRuntimeProgress).toMatchObject({
        artifactRefs: [],
        blockerRefs: [],
        message: "Runtime phase: proof.",
        phase: "proof",
        progressRefs: [],
        status: "running",
      })
      expect(postedRuntimeProgress).not.toHaveProperty("assignmentRef")
      expect(postedRuntimeProgress).not.toHaveProperty("leaseRef")
      expect(postedRuntimeProgress).not.toHaveProperty("proofRefs")
      expect(typeof postedRuntimeProgress.elapsedMs).toBe("number")

      const lifecycleJson = JSON.stringify(lifecycleEvents)
      expect(lifecycleJson).not.toContain(home)
      expect(lifecycleJson).not.toContain(codexHome)
      expect(lifecycleJson).not.toContain("codex-live")
      expect(lifecycleJson).not.toContain("/Users/")
      expect(lifecycleJson).not.toContain("accountHome")
      expect(lifecycleJson).not.toContain("provider")
      expect(lifecycleJson).not.toContain("prompt")
      expect(lifecycleJson).not.toContain("rawEvents")
      expect(lifecycleJson).not.toContain("shell output")
      expect(lifecycleJson).not.toContain("token")
      for (const event of lifecycleEvents) assertPublicProjectionSafe(event)
    })
  })

  test("closes out Codex runtime failures and clears active run markers", async () => {
    await withTempHome(async (home) => {
      const codexLease = lease({
        assignmentRef: "assignment.public.no_spend.codex_runtime_timeout",
        leaseRef: "lease.public.no_spend.codex_runtime_timeout",
        capabilityRefs: ["capability.pylon.local_codex"],
        codingAssignment: {
          codex: {
            agentKind: "codex_sdk",
            fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
            schema: CODEX_AGENT_TASK_SCHEMA,
          },
        },
      })
      const fake = fakeAssignmentServer({
        leases: [codexLease],
        rejectUnsafeProgressMessage: true,
      })
      const summary = await readySummary(home, ["capability.pylon.local_codex"])
      const state = await ensurePylonLocalState(summary)
      const codexHome = join(home, "accounts/codex/codex-timeout")
      await mkdir(codexHome, { recursive: true })
      await writeFile(join(codexHome, "auth.json"), "{}\n")
      await writeFile(
        state.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [{ provider: "codex", ref: "codex-timeout", home: codexHome }],
            },
          },
          null,
          2,
        )}\n`,
      )
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const lifecycleEvents: AssignmentRunLifecycleEvent[] = []
      const timingOutCodexRunner: CodexAgentRunner = async () => {
        throw new Error(
          "command timed out at /Users/christopherdavid/.codex/auth.json token 2026-06-09T00:00:30",
        )
      }

      const result = await runNoSpendAssignment(summary, {
        accountRef: "codex-timeout",
        baseUrl: fake.baseUrl,
        codexAgentProbe: {
          env: {},
          importer: async () => ({}),
          platform: "darwin",
        },
        codexAuthValidityProbe: async () => ({ valid: true }),
        codexAgentRunner: timingOutCodexRunner,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
        onLifecycleEvent: (event) => lifecycleEvents.push(event),
      })

      expect(result.ok).toBe(false)
      if (result.ok) throw new Error("expected Codex assignment to fail closed")
      expect(result.closeout.status).toBe("timed-out")
      expect(result.closeout.blockerRefs).toContain("blocker.assignment.codex_agent_execution_timed_out")
      expect(await activeCodingRunCounts(state.paths)).toEqual({})
      expect(lifecycleEvents).toContainEqual(
        expect.objectContaining({
          event: "assignment_run.completed",
          assignmentRef: codexLease.assignmentRef,
          leaseRef: codexLease.leaseRef,
          status: "timed-out",
          blockerRefs: expect.arrayContaining(["blocker.assignment.codex_agent_execution_timed_out"]),
        }),
      )
      const finalProgress = fake.requests
        .filter((request) => request.path.endsWith("/progress"))
        .map((request) => request.body)
        .find((body) => body.status === "proof-ready")
      expect(finalProgress?.message).toBe(
        "Assignment runtime completed; public-safe closeout evidence is ready.",
      )
      expect(fake.requests.some((request) => request.path.endsWith("/closeout"))).toBe(true)
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
        return {
          commandCount: 1,
          editedFileCount: 1,
          outcome: "completed",
          sessionRef: "session.pylon.claude_agent.test",
          turnCount: 3,
          usage: {
            inputTokens: 100,
            cachedInputTokens: 25,
            outputTokens: 40,
          },
        }
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
        agentToken: "oa_agent_test",
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
      const claudeTurnRequest = fake.requests.find(request => request.path === "/api/pylon/claude/turns")
      expect(claudeTurnRequest?.headers.get("Idempotency-Key")).toContain("pylon.claude.turn")
      expect(claudeTurnRequest?.body).toMatchObject({
        assignmentRef: "pylon_assignment.autopilot_work_order.test_1.task.public_sum_repair",
        leaseRef: "pylon_assignment.autopilot_work_order.test_1.task.public_sum_repair",
        pylonRef: expect.stringContaining("pylon."),
        sessionRef: "session.pylon.claude_agent.test",
        turnIndex: 1,
      })
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

  test("stales expired local accepted leases so interrupted runs do not block fresh work", async () => {
    await withTempHome(async (home) => {
      const expiredLease = lease({
        assignmentRef: "assignment.public.local_stale_expired",
        leaseRef: "lease.public.local_stale_expired",
        expiresAt: "2026-06-09T00:01:00.000Z",
      })
      const freshLease = lease({
        assignmentRef: "assignment.public.local_stale_fresh",
        leaseRef: "lease.public.local_stale_fresh",
        expiresAt: "2026-06-09T01:00:00.000Z",
      })
      const fake = fakeAssignmentServer({ leases: [expiredLease, freshLease] })
      const summary = await readySummary(home)
      const state = await ensurePylonLocalState(summary)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const acceptedExpired = await acceptAssignment(summary, expiredLease, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })
      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:02:00.000Z"),
      })
      const assignmentState = JSON.parse(await readFile(state.paths.assignmentState, "utf8"))

      expect(acceptedExpired.accepted).toBe(true)
      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected fresh assignment to run")
      expect(result.lease.leaseRef).toBe(freshLease.leaseRef)
      expect(assignmentState.leases[expiredLease.leaseRef]).toMatchObject({
        assignmentRef: expiredLease.assignmentRef,
        status: "stale",
        leaseExpiresAt: expiredLease.expiresAt,
      })
      expect(assignmentState.leases[freshLease.leaseRef]).toMatchObject({
        assignmentRef: freshLease.assignmentRef,
        status: "closed",
        leaseExpiresAt: freshLease.expiresAt,
      })
    })
  })

  test("keeps an expired local lease active when its owner heartbeat is fresh", async () => {
    await withTempHome(async (home) => {
      const renewedLease = lease({
        assignmentRef: "assignment.public.local_server_renewed",
        leaseRef: "lease.public.local_server_renewed",
        expiresAt: "2026-06-09T00:01:00.000Z",
      })
      const freshLease = lease({
        assignmentRef: "assignment.public.local_after_server_renewed",
        leaseRef: "lease.public.local_after_server_renewed",
        expiresAt: "2026-06-09T01:00:00.000Z",
      })
      const fake = fakeAssignmentServer({ leases: [renewedLease, freshLease] })
      const summary = await readySummary(home)
      const state = await ensurePylonLocalState(summary)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })
      await writeFile(
        state.paths.assignmentState,
        `${JSON.stringify({
          schema: "openagents.pylon.assignment_state.v0.3",
          leases: {
            [renewedLease.leaseRef]: {
              assignmentRef: renewedLease.assignmentRef,
              status: "running",
              acceptedAt: "2026-06-09T00:00:00.000Z",
              leaseExpiresAt: renewedLease.expiresAt,
              ownerHeartbeatAt: "2026-06-09T00:02:00.000Z",
              ownerHeartbeatSequence: 24,
              ownerProcessId: 424242,
              ownerStartedAt: "2026-06-09T00:00:00.000Z",
              paymentMode: "no-spend",
            },
          },
        }, null, 2)}\n`,
      )

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        localProcessIsAlive: () => true,
        now: () => new Date("2026-06-09T00:02:30.000Z"),
      })
      const closeouts = fake.requests.filter((request) => request.path.endsWith("/closeout"))
      const assignmentState = JSON.parse(await readFile(state.paths.assignmentState, "utf8"))

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected fresh assignment to run")
      expect(result.lease.leaseRef).toBe(freshLease.leaseRef)
      expect(closeouts.map((request) => decodeURIComponent(request.path.split("/").at(-2) ?? ""))).toEqual([
        freshLease.leaseRef,
      ])
      expect(assignmentState.leases[renewedLease.leaseRef]).toMatchObject({
        assignmentRef: renewedLease.assignmentRef,
        status: "running",
        leaseExpiresAt: renewedLease.expiresAt,
        ownerHeartbeatAt: "2026-06-09T00:02:00.000Z",
      })
      expect(assignmentState.leases[freshLease.leaseRef]).toMatchObject({
        assignmentRef: freshLease.assignmentRef,
        status: "closed",
        leaseExpiresAt: freshLease.expiresAt,
      })
    })
  })

  test("closeouts dead local no-spend owners so parallel runners can claim fresh work", async () => {
    await withTempHome(async (home) => {
      const interruptedLease = lease({
        assignmentRef: "assignment.public.local_interrupted_owner",
        leaseRef: "lease.public.local_interrupted_owner",
        expiresAt: "2026-06-09T00:30:00.000Z",
      })
      const freshLease = lease({
        assignmentRef: "assignment.public.local_after_interruption",
        leaseRef: "lease.public.local_after_interruption",
        expiresAt: "2026-06-09T00:30:00.000Z",
      })
      const fake = fakeAssignmentServer({ leases: [interruptedLease, freshLease] })
      const summary = await readySummary(home)
      const state = await ensurePylonLocalState(summary)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })
      await writeFile(
        state.paths.assignmentState,
        `${JSON.stringify({
          schema: "openagents.pylon.assignment_state.v0.3",
          leases: {
            [interruptedLease.leaseRef]: {
              assignmentRef: interruptedLease.assignmentRef,
              status: "running",
              acceptedAt: "2026-06-09T00:00:00.000Z",
              leaseExpiresAt: interruptedLease.expiresAt,
              ownerHeartbeatAt: "2026-06-09T00:01:00.000Z",
              ownerHeartbeatSequence: 3,
              ownerProcessId: 424242,
              ownerStartedAt: "2026-06-09T00:00:00.000Z",
              paymentMode: "no-spend",
            },
          },
        }, null, 2)}\n`,
      )

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        localProcessIsAlive: () => false,
        now: () => new Date("2026-06-09T00:02:00.000Z"),
      })
      const closeouts = fake.requests.filter((request) => request.path.endsWith("/closeout"))
      const assignmentState = JSON.parse(await readFile(state.paths.assignmentState, "utf8"))

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected fresh assignment to run")
      expect(result.lease.leaseRef).toBe(freshLease.leaseRef)
      expect(closeouts.map((request) => decodeURIComponent(request.path.split("/").at(-2) ?? ""))).toEqual([
        interruptedLease.leaseRef,
        freshLease.leaseRef,
      ])
      expect(closeouts[0]?.body).toMatchObject({
        status: "stale",
        blockerRefs: ["blocker.assignment.local_run_interrupted"],
      })
      expect(closeouts[0]?.body).not.toHaveProperty("leaseRef")
      expect(assignmentState.leases[interruptedLease.leaseRef]).toMatchObject({
        assignmentRef: interruptedLease.assignmentRef,
        status: "stale",
        leaseExpiresAt: interruptedLease.expiresAt,
      })
      expect(assignmentState.leases[freshLease.leaseRef]).toMatchObject({
        assignmentRef: freshLease.assignmentRef,
        status: "closed",
        leaseExpiresAt: freshLease.expiresAt,
      })
    })
  })

  test("retries local stale no-spend closeout until the server closeout is recorded", async () => {
    await withTempHome(async (home) => {
      const staleLease = lease({
        assignmentRef: "assignment.public.local_stale_unsubmitted",
        leaseRef: "lease.public.local_stale_unsubmitted",
        expiresAt: "2026-06-09T00:30:00.000Z",
      })
      const freshLease = lease({
        assignmentRef: "assignment.public.local_after_stale_retry",
        leaseRef: "lease.public.local_after_stale_retry",
        expiresAt: "2026-06-09T00:30:00.000Z",
      })
      const fake = fakeAssignmentServer({
        authNow: new Date("2026-06-09T00:06:00.000Z"),
        leases: [staleLease, freshLease],
        maxSkewSeconds: 600,
      })
      const summary = await readySummary(home)
      const state = await ensurePylonLocalState(summary)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })
      await writeFile(
        state.paths.assignmentState,
        `${JSON.stringify({
          schema: "openagents.pylon.assignment_state.v0.3",
          leases: {
            [staleLease.leaseRef]: {
              assignmentRef: staleLease.assignmentRef,
              status: "stale",
              acceptedAt: "2026-06-09T00:00:00.000Z",
              closedAt: "2026-06-09T00:05:00.000Z",
              leaseExpiresAt: staleLease.expiresAt,
              paymentMode: "no-spend",
            },
          },
        }, null, 2)}\n`,
      )

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:06:00.000Z"),
      })
      const closeouts = fake.requests.filter((request) => request.path.endsWith("/closeout"))
      const assignmentState = JSON.parse(await readFile(state.paths.assignmentState, "utf8"))

      expect(result.ok).toBe(true)
      if (!result.ok) throw new Error("expected fresh assignment to run")
      expect(result.lease.leaseRef).toBe(freshLease.leaseRef)
      expect(closeouts.map((request) => decodeURIComponent(request.path.split("/").at(-2) ?? ""))).toEqual([
        staleLease.leaseRef,
        freshLease.leaseRef,
      ])
      expect(assignmentState.leases[staleLease.leaseRef]).toMatchObject({
        assignmentRef: staleLease.assignmentRef,
        status: "stale",
        serverCloseoutSubmittedAt: "2026-06-09T00:06:00.000Z",
      })
      expect(assignmentState.leases[freshLease.leaseRef]).toMatchObject({
        assignmentRef: freshLease.assignmentRef,
        status: "closed",
        serverCloseoutSubmittedAt: "2026-06-09T00:06:00.000Z",
      })
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

  test("blocks assignments with revoked delegated capabilities before acceptance", async () => {
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

      const admission = await computeAssignmentAdmission(
        state,
        lease({
          capabilityRefs: ["cap.gepa.retained.v1"],
          delegation: {
            schema: "openagents.pylon.capability_delegation_chain.v0.1",
            rootIssuerRef: "agent.owner.primary",
            subjectRef: "agent.community.worker",
            audienceRef: state.identity.pylonRef,
            issuedAt: "2026-06-09T00:00:00.000Z",
            expiresAt: "2026-06-09T00:10:00.000Z",
            invocationRef: "invocation.assignment.revoked",
            capabilities: [
              {
                capabilityRef: "cap.gepa.retained.v1",
                action: "assignment.runtime_gate",
                resourceRef: "assignment.public.no_spend.test",
              },
            ],
            revocation: {
              revokedRefs: ["invocation.assignment.revoked"],
              revokedAt: "2026-06-09T00:00:10.000Z",
              reasonRef: "revocation.owner.stop",
            },
          },
        }),
        {
          now: () => new Date("2026-06-09T00:00:30.000Z"),
        },
      )

      expect(admission.admissible).toBe(false)
      expect(admission.blockerRefs).toContain("blocker.delegation.revoked")
    })
  })

  test("blocks assignments with malformed delegated capability chains", async () => {
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

      const admission = await computeAssignmentAdmission(
        state,
        lease({
          capabilityRefs: ["cap.gepa.retained.v1"],
          delegationInvalid: true,
        }),
        {
          now: () => new Date("2026-06-09T00:00:30.000Z"),
        },
      )

      expect(admission.admissible).toBe(false)
      expect(admission.blockerRefs).toContain("blocker.delegation.invalid_chain")
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
      const closeoutRequest = fake.requests.find((request) => request.path.endsWith("/closeout"))
      expect(closeoutRequest?.body).toMatchObject({
        artifactRefs: [],
        blockerRefs: ["blocker.assignment.timeout"],
        closeoutRefs: ["assignment.closeout.timeout"],
        proofRefs: ["assignment.proof.timeout"],
        status: "timed-out",
        summaryRefs: ["assignment.summary.timeout"],
      })
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

  test("labels progress submission HTTP rejections in the failure closeout", async () => {
    await withTempHome(async (home) => {
      const fake = fakeAssignmentServer({ rejectProgressStatus: 400 })
      const summary = await readySummary(home)
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
        now: () => new Date("2026-06-09T00:00:30.000Z"),
      })

      expect(result.ok).toBe(false)
      if (result.ok) throw new Error("expected rejected closeout")
      expect(result.closeout.status).toBe("rejected")
      expect(result.closeout.blockerRefs).toEqual([
        "blocker.assignment.progress_or_artifact_rejected",
        "blocker.assignment.progress_or_artifact_http_400",
      ])
      const closeoutRequest = fake.requests.find((request) => request.path.endsWith("/closeout"))
      expect(closeoutRequest?.body).toMatchObject({
        blockerRefs: result.closeout.blockerRefs,
        closeoutRefs: result.closeout.closeoutRefs,
        proofRefs: result.closeout.proofRefs,
        status: "rejected",
      })
      expect(closeoutRequest?.body).not.toHaveProperty("assignmentRef")
      expect(closeoutRequest?.body).not.toHaveProperty("leaseRef")
      expect(closeoutRequest?.body).not.toHaveProperty("paymentMode")
      assertPublicProjectionSafe(result.closeout)
    })
  })

  test("auto-selects the Claude account pinned by the assignment hash", async () => {
    await withTempHome(async (home) => {
      const claudeHomeA = join(home, "accounts/claude/claude-a")
      const claudeHomeB = join(home, "accounts/claude/claude-b")
      await mkdir(claudeHomeA, { recursive: true })
      await mkdir(claudeHomeB, { recursive: true })
      const claudeBHash = hashPylonAccountRef("claude_agent", "claude-b")
      const codingAssignment = {
        claudeAgent: {
          accountRefHash: claudeBHash,
          agentKind: "claude_agent_sdk",
          fixtureRef: "fixture.public.pylon.claude_agent.sum_repair.v1",
          schema: CLAUDE_AGENT_TASK_SCHEMA,
        },
        requiredCapabilityRefs: ["capability.pylon.local_claude_agent"],
        schema: "openagents.autopilot_coding_assignment.v1",
      }
      const fake = fakeAssignmentServer({
        leases: [
          lease({
            assignmentRef: "assignment.public.no_spend.claude_pinned_account",
            capabilityRefs: ["capability.pylon.local_claude_agent"],
            codingAssignment,
            jobKind: "claude_agent_task",
            leaseRef: "lease.public.no_spend.claude_pinned_account",
          } as Partial<PylonAssignmentLease>),
        ],
      })
      const summary = await readySummary(home, ["capability.pylon.local_claude_agent"])
      const state = await ensurePylonLocalState(summary)
      await writeFile(
        state.paths.config,
        `${JSON.stringify(
          {
            dev: {
              accounts: [
                { provider: "claude_agent", ref: "claude-a", home: claudeHomeA },
                { provider: "claude_agent", ref: "claude-b", home: claudeHomeB },
              ],
            },
          },
          null,
          2,
        )}\n`,
      )
      await sendHeartbeat(summary, { baseUrl: fake.baseUrl, now: () => new Date("2026-06-09T00:00:00.000Z") })

      let seenAccountRef: string | null = null
      let seenClaudeConfigDir: string | undefined
      const claudeAgentRunner: ClaudeAgentRunner = async (input) => {
        seenAccountRef = input.account?.accountRef ?? null
        seenClaudeConfigDir = input.env?.CLAUDE_CONFIG_DIR
        await writeFile(
          join(input.cwd, "sum.ts"),
          "export const sum = (left: number, right: number) => left + right\n",
        )
        return { commandCount: 1, editedFileCount: 1, outcome: "completed", sessionRef: null, turnCount: 2 }
      }

      const result = await runNoSpendAssignment(summary, {
        baseUrl: fake.baseUrl,
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
      if (!result.ok) throw new Error("expected pinned Claude assignment to run")
      expect(seenAccountRef).toBe("claude-b")
      expect(seenClaudeConfigDir).toBe(claudeHomeB)
      expect(result.closeout.resultRefs).toContain(
        "result.public.pylon.claude_agent_task.fixture_repair_passed",
      )
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
