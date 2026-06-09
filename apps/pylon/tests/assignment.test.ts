import { mkdtemp, rm, writeFile } from "node:fs/promises"
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
  type PylonAssignmentLease,
} from "../src/assignment"
import { sendHeartbeat, sha256Base64Url } from "../src/presence"
import { assertPublicProjectionSafe, ensurePylonLocalState, writePresenceState } from "../src/state"
import { PSIONIC_QWEN_MODEL_REFS, type PsionicQwenModelAdmission } from "../packages/runtime/src/index"

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

function fakeAssignmentServer(input: { leases?: PylonAssignmentLease[]; rejectAccept?: boolean; cancelOnProgress?: boolean } = {}) {
  const requests: { path: string; body: any; headers: Headers }[] = []
  const accepted = new Set<string>()
  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url)
      const text = await request.text()
      const body = text ? JSON.parse(text) : {}
      requests.push({ path: url.pathname, body, headers: request.headers })

      expect(request.headers.get("x-nip98-body-sha256")).toBe(sha256Base64Url(text))
      expect(request.headers.get("x-nip98-signature")).toBeTruthy()
      if (body.pylonRef) {
        expect(request.headers.get("x-pylon-ref")).toBe(body.pylonRef)
      } else {
        expect(request.headers.get("x-pylon-ref")).toBeTruthy()
      }

      if (url.pathname.includes("/heartbeat")) {
        return Response.json({ heartbeatRef: `heartbeat.${body.pylonRef}.${body.sequence}` })
      }
      if (url.pathname.endsWith("/assignments/poll")) {
        return Response.json({
          schema: "openagents.pylon.assignment_poll_response.v0.3",
          leases: input.leases ?? [lease()],
        })
      }
      if (url.pathname.endsWith("/accept")) {
        if (input.rejectAccept) {
          return Response.json({ statusRef: "assignment.rejected.fake", reasonRef: "reject.fake" }, { status: 409 })
        }
        if (accepted.has(body.leaseRef)) {
          return Response.json({ statusRef: "assignment.duplicate.fake" }, { status: 409 })
        }
        accepted.add(body.leaseRef)
        return Response.json({ statusRef: `assignment.accepted.${body.leaseRef}` })
      }
      if (url.pathname.endsWith("/progress")) {
        if (input.cancelOnProgress) {
          return Response.json({ progressRef: "assignment.cancelled.fake" }, { status: 410 })
        }
        return Response.json({ progressRef: `assignment.progress.${body.leaseRef}.${body.sequence}` })
      }
      if (url.pathname.endsWith("/closeout")) {
        expect(body.paymentMode).toBe("no-spend")
        expect(body.settlementState).toBe("not_applicable")
        expect(body.payoutClaimAllowed).toBe(false)
        expect(body.redacted).toBe(true)
        return Response.json({ closeoutRef: `assignment.closeout.${body.leaseRef}` })
      }
      return Response.json({ errorRef: "error.not_found" }, { status: 404 })
    },
  })
  servers.push(server)
  return { baseUrl: `http://127.0.0.1:${server.port}`, requests }
}

async function readySummary(home: string) {
  const summary = createBootstrapSummary(
    parseBootstrapArgs(["--display-name", "Assignment Test", "--capability-ref", "cap.gepa.retained.v1"]),
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
      capabilityRefs: ["cap.gepa.retained.v1"],
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
        `/api/pylons/${encodeURIComponent(pylonRef)}/assignments/poll`,
        `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/accept`,
        `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/progress`,
        `/api/pylons/${encodeURIComponent(fake.requests[0].body.pylonRef)}/assignments/${encodeURIComponent(result.lease.leaseRef)}/closeout`,
      ])
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

  test("denies paused, stale, wrong-capability, unsupported-backend, paid-wallet-blocked, and expired leases", async () => {
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
          walletRunner: async () => ({ exitCode: 1, stdout: "", stderr: "offline" }),
        },
      )

      expect(admission.admissible).toBe(false)
      expect(admission.blockerRefs).toContain("blocker.assignment.lifecycle_paused")
      expect(admission.blockerRefs).toContain("blocker.assignment.presence_stale")
      expect(admission.blockerRefs).toContain("blocker.assignment.wrong_capability")
      expect(admission.blockerRefs).toContain("blocker.assignment.unsupported_backend")
      expect(admission.blockerRefs).toContain("blocker.assignment.wallet_blocked")
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
        proofRefs: ["assignment.proof.timeout"],
        receiptRefs: [],
        redacted: true,
        completedAt: "2026-06-09T00:01:00.000Z",
      } as const

      assertPublicProjectionSafe(closeout)
      const result = await submitAssignmentCloseout(summary, closeout, { baseUrl: fake.baseUrl })
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
