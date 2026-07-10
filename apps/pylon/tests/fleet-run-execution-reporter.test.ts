import { describe, expect, test } from "bun:test"
import { createHash } from "node:crypto"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { canonicalJson } from "@openagentsinc/khala-sync"

import {
  makePylonFleetRunExecutionHttpPort,
  openPylonFleetRunExecutionReporter,
  PYLON_FLEET_RUN_EXECUTION_ACK_SCHEMA,
  PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
  type PylonFleetRunAnyExecutionBatch,
  type PylonFleetRunExecutionAck,
  type PylonFleetRunExecutionBatch,
  type PylonFleetRunExecutionEventInput,
} from "../src/orchestration/fleet-run-execution-reporter.js"
import { openPylonFleetRunRuntime } from "../src/orchestration/fleet-run-runtime.js"

const fixedNow = new Date("2026-07-10T00:10:00.000Z")
const runRef = "fleet_run.sarah.0123456789abcdef0123"
const claimRef = "claim.sarah_fleet_run.0123456789abcdef01234567"
const pylonRef = "pylon.public.fc2.execution"
const maxBatchBytes = 256 * 1_024

const eventWithSequence = (
  sequence: number,
  event: PylonFleetRunExecutionEventInput,
) => ({
  ...event,
  sequence,
  eventRef: `event.pylon.fleet_run.${createHash("sha256")
    .update(canonicalJson({ runRef, claimRef, event }))
    .digest("hex")
    .slice(0, 24)}`,
})

const startedEvent = (sequence = 1, observedAt = fixedNow.toISOString()) =>
  eventWithSequence(sequence, {
    schema: "openagents.pylon.fleet_run_execution_event.v1",
    kind: "run_started",
    observedAt,
  })

const ack = (acceptedThroughSequence: number): PylonFleetRunExecutionAck => ({
  schema: PYLON_FLEET_RUN_EXECUTION_ACK_SCHEMA,
  runRef,
  claimRef,
  acceptedThroughSequence,
  storedEventCount: acceptedThroughSequence,
  duplicateEventCount: 0,
  execution: {
    state: acceptedThroughSequence === 0 ? "pending" : "running",
    lastSequence: acceptedThroughSequence,
    counters: {
      workUnitsTotal: 1,
      activeAssignments: 0,
      acceptedAssignments: 0,
      failedAssignments: 0,
      staleAssignments: 0,
    },
    startedAt: acceptedThroughSequence === 0 ? null : fixedNow.toISOString(),
    updatedAt: fixedNow.toISOString(),
    closeouts: [],
  },
})

const seedAcceptedRun = async (env: NodeJS.ProcessEnv) => {
  const runtime = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
  runtime.store.createFleetRun({
    runRef,
    objective: "Project the exact accepted Sarah FleetRun execution lifecycle.",
    workSource: "fixture",
    authorityBinding: {
      schema: "openagents.pylon.fleet_run_authority_binding.v1",
      source: "sarah_authority",
      authorityFingerprint: "c".repeat(64),
      claimRef,
      pylonRef,
      targetPreference: "owner_local",
      phase: "accepted",
    },
    targetConcurrency: 1,
    workerKind: "auto",
    state: "running",
    now: fixedNow,
  })
  return runtime
}

describe("Pylon FleetRun execution reporter", () => {
  test("replays legacy v1 rows and splits a mixed outbox at the v2 schema boundary", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-run-mixed-schema-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seedAcceptedRun(env)
      let online = false
      const delivered: PylonFleetRunAnyExecutionBatch[] = []
      const reporter = openPylonFleetRunExecutionReporter({
        store: runtime.store,
        pylonRef,
        runRef,
        now: () => fixedNow,
        remote: {
          append: async ({ batch }) => {
            if (!online) throw new Error("offline")
            delivered.push(batch)
            return ack(batch.events.at(-1)?.sequence ?? 0)
          },
        },
      })
      await reporter.record({
        schema: "openagents.pylon.fleet_run_execution_event.v1",
        kind: "run_started",
        observedAt: fixedNow.toISOString(),
      })
      await reporter.record({
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "run_started",
        observedAt: new Date(fixedNow.getTime() + 1_000).toISOString(),
      })
      await reporter.record({
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "work_progress",
        // Parallel executor callbacks may legitimately reorder their remote
        // audit clocks; local sequence/receipt custody must still advance.
        observedAt: new Date(fixedNow.getTime() - 1_000).toISOString(),
        unitRef: "unit.fixture.mixed_schema",
        workClaimRef: "work_claim.fixture.mixed_schema",
        workerKind: "codex",
        marginalCostClass: "subscription",
        blockerRefs: [],
      })
      online = true
      await reporter.close()

      expect(delivered.map(batch => batch.schema)).toEqual([
        "openagents.pylon.fleet_run_execution_batch.v1",
        "openagents.pylon.fleet_run_execution_batch.v2",
      ])
      expect(delivered.map(batch => batch.events.map(event => event.sequence))).toEqual([
        [1],
        [2],
      ])
      expect(runtime.store.listFleetRunExecutionOutbox(runRef)).toEqual([])
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("durably replaces incoherent v2 account, usage, and blocker evidence with typed failures", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-run-v2-coherence-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seedAcceptedRun(env)
      const delivered: PylonFleetRunAnyExecutionBatch[] = []
      const reporter = openPylonFleetRunExecutionReporter({
        store: runtime.store,
        pylonRef,
        runRef,
        now: () => fixedNow,
        remote: {
          append: async ({ batch }) => {
            delivered.push(batch)
            return ack(batch.events.at(-1)?.sequence ?? 0)
          },
        },
      })
      await reporter.record({
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "run_started",
        observedAt: fixedNow.toISOString(),
      })
      await reporter.record({
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "work_progress",
        observedAt: fixedNow.toISOString(),
        unitRef: "unit.fixture.account_mismatch",
        workClaimRef: "work_claim.fixture.account_mismatch",
        assignmentRef: "assignment.public.suspect",
        workerKind: "codex",
        accountRefHash: "account.pylon.claude_agent.0123456789abcdef01234567",
        marginalCostClass: "subscription",
        blockerRefs: [],
      })
      await reporter.record({
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "work_terminal",
        observedAt: fixedNow.toISOString(),
        unitRef: "unit.fixture.usage_mismatch",
        workClaimRef: "work_claim.fixture.usage_mismatch",
        assignmentRef: "assignment.public.codex",
        workerKind: "codex",
        accountRefHash: "account.pylon.codex.0123456789abcdef01234567",
        marginalCostClass: "subscription",
        terminalState: "failed",
        usageEvidence: {
          schema: "openagents.pylon.fleet_run_usage_evidence.v1",
          truth: "not_measured",
          harnessKind: "grok",
          evidenceRef: "evidence.public.suspect",
          assignmentRef: "assignment.public.grok",
          receiptRef: "receipt.public.suspect",
          tokenUsageRefs: [],
          caveatRefs: ["caveat.public.suspect"],
        },
        blockerRefs: ["blocker.public.original"],
      })
      await reporter.record({
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "work_progress",
        observedAt: fixedNow.toISOString(),
        unitRef: "unit.fixture.blocker_overflow",
        workClaimRef: "work_claim.fixture.blocker_overflow",
        workerKind: "grok",
        marginalCostClass: "not_measured",
        blockerRefs: Array.from(
          { length: 33 },
          (_, index) => `blocker.public.overflow.${index}`,
        ),
      })
      await reporter.record({
        schema: "openagents.pylon.fleet_run_execution_event.v2",
        kind: "work_terminal",
        observedAt: fixedNow.toISOString(),
        unitRef: "unit.fixture.missing_accepted_identity",
        workClaimRef: "work_claim.fixture.missing_accepted_identity",
        workerKind: "claude",
        terminalState: "accepted",
        blockerRefs: [],
      } as unknown as PylonFleetRunExecutionEventInput)
      await reporter.close()

      const failures = delivered
        .flatMap(batch => batch.events)
        .filter(event => event.kind === "work_terminal")
      expect(failures).toHaveLength(4)
      expect(failures.map(event => event.blockerRefs)).toEqual([
        ["blocker.pylon.fleet_run.evidence_identity_invalid"],
        ["blocker.pylon.fleet_run.evidence_identity_invalid"],
        ["blocker.pylon.fleet_run.evidence_cardinality_invalid"],
        ["blocker.pylon.fleet_run.evidence_identity_invalid"],
      ])
      for (const failure of failures) {
        expect(failure).not.toHaveProperty("assignmentRef")
        expect(failure).not.toHaveProperty("accountRefHash")
        expect(failure).not.toHaveProperty("usageEvidence")
      }
      expect(runtime.store.listFleetRunExecutionOutbox(runRef)).toEqual([])
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("retains a failed append and replays its exact sequence after reopen", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-run-reporter-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const first = await seedAcceptedRun(env)
      const failed = openPylonFleetRunExecutionReporter({
        store: first.store,
        pylonRef,
        runRef,
        now: () => fixedNow,
        remote: { append: () => Promise.reject(new Error("offline")) },
      })
      await failed.record({
        schema: "openagents.pylon.fleet_run_execution_event.v1",
        kind: "run_started",
        observedAt: fixedNow.toISOString(),
      })
      expect(first.store.listFleetRunExecutionOutbox(runRef)).toEqual([
        expect.objectContaining({ sequence: 1, deliveredAt: null }),
      ])
      await failed.close()
      await first.close()

      const reopened = await openPylonFleetRunRuntime({ env, now: () => fixedNow })
      const seen: number[][] = []
      const reporter = openPylonFleetRunExecutionReporter({
        store: reopened.store,
        pylonRef,
        runRef,
        now: () => fixedNow,
        remote: {
          append: async ({ batch }) => {
            seen.push(batch.events.map(event => event.sequence))
            return ack(batch.events.at(-1)?.sequence ?? 0)
          },
        },
      })
      try {
        expect((await reporter.flush())?.acceptedThroughSequence).toBe(1)
        expect(seen).toEqual([[1]])
        expect(reopened.store.listFleetRunExecutionOutbox(runRef)).toEqual([])

        // The stable event ref makes a repeated start byte-identical and does
        // not generate another server call after the original ack.
        await reporter.record({
          schema: "openagents.pylon.fleet_run_execution_event.v1",
          kind: "run_started",
          observedAt: fixedNow.toISOString(),
        })
        expect(seen).toEqual([[1]])
        expect(reopened.store.listFleetRunExecutionOutbox(runRef, { pendingOnly: false })).toHaveLength(1)
      } finally {
        await reporter.close()
        await reopened.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("posts a strict bearer batch to the exact Pylon/run route", async () => {
    const calls: Array<{ url: string; init: RequestInit }> = []
    const port = makePylonFleetRunExecutionHttpPort({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.test",
      fetchImpl: async (url, init) => {
        calls.push({ url: String(url), init: init ?? {} })
        const baseAck = ack(1)
        return Response.json({
          ...baseAck,
          execution: {
            ...baseAck.execution,
            closeouts: [{
              unitRef: "plan_unit.sarah.0123456789abcdef01234567",
              workClaimRef: "claim.public.fixture",
              workerKind: "codex",
              blockerRefs: [],
              observedAt: fixedNow.toISOString(),
              eventRef: "event.pylon.fleet_run.0123456789abcdef01234567",
              terminalState: "accepted",
              assignmentRef: "assignment.public.fixture",
              accountRefHash: "account.pylon.codex.0123456789abcdef01234567",
              closeoutRef: "closeout.public.fixture",
              usageEvidence: {
                truth: "exact",
                tokenUsageRefs: ["token_usage.public.fixture"],
              },
            }],
          },
        })
      },
    })
    const result = await port.append({
      pylonRef,
      runRef,
      batch: {
        schema: PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
        claimRef,
        events: [startedEvent()],
      },
    })

    expect(result.acceptedThroughSequence).toBe(1)
    expect(result.execution.closeouts).toHaveLength(1)
    expect(calls).toHaveLength(1)
    expect(new URL(calls[0]!.url).pathname).toBe(
      `/api/pylons/${pylonRef}/fleet-runs/${runRef}/events`,
    )
    expect(calls[0]!.init.method).toBe("POST")
    expect(calls[0]!.init.redirect).toBe("error")
    expect((calls[0]!.init.headers as Record<string, string>).Authorization).toBe(
      "Bearer oa_agent_fixture",
    )
    expect(JSON.parse(String(calls[0]!.init.body))).toMatchObject({
      schema: PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
      claimRef,
      events: [{ sequence: 1, kind: "run_started" }],
    })
  })

  test("refuses bearer transport over non-loopback HTTP", () => {
    expect(() => makePylonFleetRunExecutionHttpPort({
      agentToken: "oa_agent_fixture",
      baseUrl: "http://openagents.test",
    })).toThrow("unavailable")
  })

  test("retries the exact frozen batch after a lost response even when a later event enqueues", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-run-idempotency-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seedAcceptedRun(env)
      const calls: Array<{ body: string; idempotencyKey: string }> = []
      let loseFirstResponse = true
      const remote = makePylonFleetRunExecutionHttpPort({
        agentToken: "oa_agent_fixture",
        baseUrl: "https://openagents.test",
        fetchImpl: async (_url, init) => {
          const body = String(init?.body)
          calls.push({
            body,
            idempotencyKey: (init?.headers as Record<string, string>)["Idempotency-Key"]!,
          })
          if (loseFirstResponse) {
            loseFirstResponse = false
            throw new Error("response lost after commit")
          }
          const batch = JSON.parse(body) as PylonFleetRunExecutionBatch
          return Response.json(ack(batch.events.at(-1)!.sequence))
        },
      })
      const reporter = openPylonFleetRunExecutionReporter({
        store: runtime.store,
        pylonRef,
        runRef,
        now: () => fixedNow,
        remote,
      })
      try {
        await reporter.record({
          schema: "openagents.pylon.fleet_run_execution_event.v1",
          kind: "run_started",
          observedAt: fixedNow.toISOString(),
        })
        await reporter.record({
          schema: "openagents.pylon.fleet_run_execution_event.v2",
          kind: "work_progress",
          observedAt: new Date(fixedNow.getTime() + 1_000).toISOString(),
          unitRef: "unit.fixture.frozen_batch",
          workClaimRef: "work_claim.fixture.frozen_batch",
          workerKind: "codex",
          marginalCostClass: "subscription",
          blockerRefs: [],
        })
        expect(calls).toHaveLength(3)
        expect(calls[1]).toEqual(calls[0])
        expect(JSON.parse(calls[0]!.body).events.map((event: { sequence: number }) => event.sequence))
          .toEqual([1])
        expect(JSON.parse(calls[2]!.body).events.map((event: { sequence: number }) => event.sequence))
          .toEqual([2])
        expect(calls[2]!.idempotencyKey).not.toBe(calls[0]!.idempotencyKey)
      } finally {
        await reporter.close()
        await runtime.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("close drains every largest byte-bounded prefix from a large valid queue", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fleet-run-drain-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seedAcceptedRun(env)
      let online = false
      const delivered: PylonFleetRunExecutionBatch[] = []
      const reporter = openPylonFleetRunExecutionReporter({
        store: runtime.store,
        pylonRef,
        runRef,
        now: () => fixedNow,
        remote: {
          append: async ({ batch }) => {
            if (!online) throw new Error("offline")
            delivered.push(batch)
            return ack(batch.events.at(-1)!.sequence)
          },
        },
      })
      const usageRefs = Array.from({ length: 100 }, (_, index) =>
        `usage.${String(index).padStart(3, "0")}.${"x".repeat(168)}`)
      const inputs = Array.from({ length: 20 }, (_, index): PylonFleetRunExecutionEventInput => ({
        schema: "openagents.pylon.fleet_run_execution_event.v1",
        kind: "work_terminal",
        observedAt: new Date(fixedNow.getTime() + index * 1_000).toISOString(),
        unitRef: `unit.fixture.${index}`,
        workClaimRef: `work-claim.fixture.${index}`,
        assignmentRef: `assignment.fixture.${index}`,
        workerKind: "codex",
        accountRefHash: "account.pylon.codex.0123456789abcdef01234567",
        terminalState: "accepted",
        closeoutRef: `closeout.fixture.${index}`,
        usageEvidence: { truth: "exact", tokenUsageRefs: usageRefs },
        blockerRefs: [],
      }))

      await Promise.all(inputs.map(event => reporter.record(event)))
      online = true
      await reporter.close()

      expect(delivered.length).toBeGreaterThan(1)
      expect(delivered.flatMap(batch => batch.events.map(event => event.sequence)))
        .toEqual(Array.from({ length: 20 }, (_, index) => index + 1))
      for (const batch of delivered) {
        expect(batch.events.length).toBeLessThanOrEqual(64)
        expect(new TextEncoder().encode(canonicalJson(batch)).byteLength)
          .toBeLessThanOrEqual(maxBatchBytes)
      }
      const nextEvent = delivered[1]!.events[0]!
      const expanded = {
        ...delivered[0]!,
        events: [...delivered[0]!.events, nextEvent],
      }
      expect(new TextEncoder().encode(canonicalJson(expanded)).byteLength)
        .toBeGreaterThan(maxBatchBytes)
      expect(runtime.store.listFleetRunExecutionOutbox(runRef)).toEqual([])
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("rejects non-contiguous, unsafe, or content-unbound events without leaking input", async () => {
    let fetchCalls = 0
    const port = makePylonFleetRunExecutionHttpPort({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.test",
      fetchImpl: async () => {
        fetchCalls += 1
        return Response.json(ack(1))
      },
    })
    const valid = startedEvent()
    const attempts: PylonFleetRunExecutionBatch[] = [
      {
        schema: PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
        claimRef,
        events: [valid, startedEvent(3, new Date(fixedNow.getTime() + 1_000).toISOString())],
      },
      {
        schema: PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
        claimRef,
        events: [{ ...valid, sequence: Number.MAX_SAFE_INTEGER + 1 }],
      },
      {
        schema: PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
        claimRef,
        events: [{ ...valid, observedAt: "2026-07-10T00:10:01.000Z" }],
      },
    ]
    for (const batch of attempts) {
      const error = await port.append({ pylonRef, runRef, batch }).catch(value => value as Error)
      expect(error.message).toBe("Pylon FleetRun execution projection is unavailable")
      expect(error.message).not.toContain("2026-07-10T00:10:01.000Z")
    }
    expect(fetchCalls).toBe(0)
  })

  test("requires an exact ack and bounds a streamed response before buffering it", async () => {
    const batch: PylonFleetRunExecutionBatch = {
      schema: PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
      claimRef,
      events: [startedEvent()],
    }
    const wrongAck = makePylonFleetRunExecutionHttpPort({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.test",
      fetchImpl: async () => Response.json(ack(2)),
    })
    await expect(wrongAck.append({ pylonRef, runRef, batch })).rejects.toThrow("unavailable")

    let cancelled = false
    const oversized = makePylonFleetRunExecutionHttpPort({
      agentToken: "oa_agent_fixture",
      baseUrl: "https://openagents.test",
      fetchImpl: async () => new Response(new ReadableStream<Uint8Array>({
        start(controller) {
          controller.enqueue(new Uint8Array(200_000))
          controller.enqueue(new Uint8Array(100_000))
        },
        cancel() {
          cancelled = true
        },
      }), { status: 200 }),
    })
    await expect(oversized.append({ pylonRef, runRef, batch })).rejects.toThrow("unavailable")
    expect(cancelled).toBe(true)
  })
})
