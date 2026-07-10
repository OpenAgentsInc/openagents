import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import type {
  FleetSteeringOutcome,
  FleetSteeringOutcomeAck,
  FleetSteeringPage,
  KhalaFleetIntent,
} from "@openagentsinc/khala-fleet-intents"

import {
  makePylonFleetRunSteeringHttpTransport,
  PylonFleetRunSteeringTransportError,
  tickPylonFleetRunSteeringConsumer,
  type PylonFleetRunSteeringTransport,
} from "./fleet-run-steering-consumer.js"
import { openPylonFleetRunRuntime } from "./fleet-run-runtime.js"
import { openPylonOwnedStandingFleetRunExecutor } from "./fleet-run-owned-standing-executor.js"
import { openPylonStandingFleetRunExecutor } from "./fleet-run-standing-executor.js"
import { planFixtureWork } from "./work-planner.js"

const now = new Date("2026-07-10T02:00:00.000Z")
const runRef = "fleet_run.sarah.0123456789abcdef0123"
const claimRef = "claim.sarah_fleet_run.0123456789abcdef01234567"
const pylonRef = "pylon.public.fc3.steering"
const workUnitRef = "fixture:fc3.steering.unit"
const workClaimRef = "claim.public.fc3.steering.unit"
const assignmentRef = "assignment.public.fc3.steering.unit"

type FleetIntentInput<T> = T extends unknown
  ? Omit<T, "schema" | "createdAt" | "origin" | "idempotencyKey" | "runRef"> & {
      intentId: string
    }
  : never

const fleetIntent = (
  input: FleetIntentInput<KhalaFleetIntent>,
): KhalaFleetIntent => ({
  schema: "khala.fleet_intent.v1",
  createdAt: now.toISOString(),
  origin: { surface: "test_fixture" },
  idempotencyKey: `idempotency.${input.intentId}`,
  runRef,
  ...input,
} as KhalaFleetIntent)

const page = (
  intents: FleetSteeringPage["intents"],
  nextAfter = intents.at(-1)?.seq ?? 0,
): FleetSteeringPage => ({
  ok: true,
  runRef,
  claimRef,
  intents,
  nextAfter,
  upToDate: true,
})

const delivery = (
  seq: number,
  intent: KhalaFleetIntent,
): FleetSteeringPage["intents"][number] => ({
  seq,
  intentId: intent.intentId,
  intent,
  createdAt: now.toISOString(),
})

const ack = (outcomes: ReadonlyArray<FleetSteeringOutcome>): FleetSteeringOutcomeAck => ({
  ok: true,
  runRef,
  claimRef,
  outcomes: [...outcomes],
  storedOutcomeCount: outcomes.length,
  duplicateOutcomeCount: 0,
})

const seed = async (env: NodeJS.ProcessEnv) => {
  const runtime = await openPylonFleetRunRuntime({ env, now: () => now })
  runtime.store.createFleetRun({
    runRef,
    objective: "Exercise accepted Sarah steering delivery.",
    workSource: "fixture",
    authorityBinding: {
      schema: "openagents.pylon.fleet_run_authority_binding.v1",
      source: "sarah_authority",
      authorityFingerprint: "a".repeat(64),
      claimRef,
      pylonRef,
      targetPreference: "owner_local",
      phase: "accepted",
    },
    targetConcurrency: 1,
    workerKind: "auto",
    state: "running",
    now,
  })
  runtime.store.tryClaimWorkUnit({
    claimRef: workClaimRef,
    workUnitRef,
    runRef,
    assignmentRef,
    workerAccountRef: "account.public.fc3.codex",
    ttl: 60_000,
    now,
  })
  runtime.store.updateWorkClaimState(workClaimRef, "in_progress", now)
  return runtime
}

describe("Pylon FleetRun steering transport", () => {
  test("uses the exact bearer route, rejects excess response fields, and sends body-free ACKs", async () => {
    const calls: Array<{ url: string; init: RequestInit }> = []
    const outcome: FleetSteeringOutcome = {
      seq: 7,
      intentId: "intent.fc3.transport",
      outcome: "queued_follow_up",
      outcomeRef: "outcome.pylon.fleet_steering.0123456789abcdef01234567",
      observedAt: now.toISOString(),
    }
    const transport = makePylonFleetRunSteeringHttpTransport({
      agentToken: "oa_agent_fixture_private",
      baseUrl: "https://openagents.test",
      fetchImpl: (async (url, init) => {
        calls.push({ url: String(url), init: init ?? {} })
        return calls.length === 1
          ? Response.json(page([]))
          : Response.json(ack([outcome]))
      }) as typeof fetch,
    })
    await transport.read({ pylonRef, runRef, claimRef, after: 0, limit: 64 })
    await transport.postOutcomes({ pylonRef, runRef, claimRef, outcomes: [outcome] })

    const readUrl = new URL(calls[0]!.url)
    expect(readUrl.pathname).toBe(
      `/api/pylons/${pylonRef}/fleet-runs/${runRef}/steering`,
    )
    expect(readUrl.searchParams.get("claimRef")).toBe(claimRef)
    expect(readUrl.searchParams.get("after")).toBe("0")
    expect(calls[0]!.init.headers).toMatchObject({
      Authorization: "Bearer oa_agent_fixture_private",
    })
    expect(new URL(calls[1]!.url).pathname).toEndWith("/steering/outcomes")
    const posted = JSON.parse(String(calls[1]!.init.body)) as Record<string, unknown>
    expect(posted).toEqual({ claimRef, outcomes: [outcome] })
    expect(JSON.stringify(posted)).not.toContain("body")
    expect(JSON.stringify(posted)).not.toContain("detail")

    const strict = makePylonFleetRunSteeringHttpTransport({
      agentToken: "oa_agent_fixture_private",
      baseUrl: "https://openagents.test",
      fetchImpl: (() => Promise.resolve(Response.json({ ...page([]), privateDetail: "no" }))) as unknown as typeof fetch,
    })
    await expect(strict.read({ pylonRef, runRef, claimRef, after: 0, limit: 64 }))
      .rejects.toMatchObject({ failure: "bad_response" })

    const unavailable = makePylonFleetRunSteeringHttpTransport({
      agentToken: "oa_agent_fixture_private",
      baseUrl: "https://openagents.test",
      fetchImpl: (() => Promise.resolve(new Response("private server detail", { status: 403 }))) as unknown as typeof fetch,
    })
    const error = await unavailable.read({ pylonRef, runRef, claimRef, after: 0, limit: 64 })
      .catch(value => value)
    expect(error).toBeInstanceOf(PylonFleetRunSteeringTransportError)
    expect(error).toMatchObject({ failure: "not_authorized" })
    expect(String(error)).not.toContain("private server detail")
    expect(String(error)).not.toContain("oa_agent_fixture_private")
  })
})

describe("Pylon FleetRun steering consumer", () => {
  test("atomically applies pause/resume, queues exact targets, and retries byte-identical ACKs after reopen", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-reopen-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    const pause = fleetIntent({
      intentId: "intent.fc3.pause",
      kind: "fleet_run_control",
      action: "pause",
    })
    const resume = fleetIntent({
      intentId: "intent.fc3.resume",
      kind: "fleet_run_control",
      action: "resume",
    })
    const steer = fleetIntent({
      intentId: "intent.fc3.steer",
      kind: "steer_message",
      targetRef: assignmentRef,
      body: "owner-private follow-up body",
    })
    const delivered = page([
      delivery(5, pause),
      delivery(11, resume),
      delivery(23, steer),
    ], 23)
    const attemptedBatches: FleetSteeringOutcome[][] = []
    let readCalls = 0

    try {
      const first = await seed(env)
      const offline: PylonFleetRunSteeringTransport = {
        read: () => {
          readCalls += 1
          return Promise.resolve(delivered)
        },
        postOutcomes: ({ outcomes }) => {
          attemptedBatches.push([...outcomes])
          return Promise.reject(new PylonFleetRunSteeringTransportError("network_failed"))
        },
      }
      const firstTick = await tickPylonFleetRunSteeringConsumer({
        store: first.store,
        transport: offline,
        pylonRef,
        runRef,
        claimRef,
        now: () => now,
      })
      expect(firstTick).toMatchObject({
        ok: false,
        applied: 3,
        pendingAcknowledgements: 3,
        watermark: 23,
      })
      expect(first.store.getFleetRun(runRef)?.state).toBe("running")
      expect(first.store.listFleetRunSteeringQueuedFollowUps({
        pylonRef,
        runRef,
        claimRef,
      })).toEqual([
        expect.objectContaining({
          intentId: "intent.fc3.steer",
          intentKind: "steer_message",
          workUnitRef,
          workClaimRef,
          assignmentRef,
          body: "owner-private follow-up body",
        }),
      ])
      expect(first.store.getWorkClaim(workClaimRef)?.state).toBe("in_progress")
      expect(attemptedBatches).toHaveLength(1)
      const firstBytes = JSON.stringify(attemptedBatches[0])
      expect(firstBytes).not.toContain("owner-private follow-up body")
      expect(attemptedBatches[0]?.[0]?.outcomeRef).toBe(
        "outcome.pylon.fleet_steering.28827659208bb87c76a27ce4",
      )
      const backpressured = await tickPylonFleetRunSteeringConsumer({
        store: first.store,
        transport: offline,
        pylonRef,
        runRef,
        claimRef,
        now: () => now,
      })
      expect(backpressured).toMatchObject({
        ok: false,
        applied: 0,
        pendingAcknowledgements: 3,
        failure: "backpressure",
      })
      expect(readCalls).toBe(1)
      await first.close()

      const reopened = await openPylonFleetRunRuntime({ env, now: () => now })
      const retried: FleetSteeringOutcome[][] = []
      const online: PylonFleetRunSteeringTransport = {
        read: ({ after }) => {
          expect(after).toBe(23)
          return Promise.resolve(page([], 23))
        },
        postOutcomes: ({ outcomes }) => {
          retried.push([...outcomes])
          return Promise.resolve(ack(outcomes))
        },
      }
      try {
        const retry = await tickPylonFleetRunSteeringConsumer({
          store: reopened.store,
          transport: online,
          pylonRef,
          runRef,
          claimRef,
          now: () => new Date("2026-07-10T03:00:00.000Z"),
        })
        expect(retry).toMatchObject({
          ok: true,
          applied: 0,
          acknowledged: 3,
          pendingAcknowledgements: 0,
          watermark: 23,
        })
        expect(retried).toHaveLength(1)
        expect(JSON.stringify(retried[0])).toBe(firstBytes)
        expect(retried[0]!.map(item => item.seq)).toEqual([5, 11, 23])
        expect(retried[0]!.every(item =>
          /^outcome\.pylon\.fleet_steering\.[a-f0-9]{24}$/u.test(item.outcomeRef)
        )).toBe(true)
        expect(reopened.store.listFleetRunSteeringQueuedFollowUps({
          pylonRef,
          runRef,
          claimRef,
        })).toHaveLength(1)
      } finally {
        await reopened.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("rejects ambiguous/incomplete targets, skips stale targets, and never chooses latest", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-targets-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seed(env)
      runtime.store.releaseWorkClaim(workClaimRef, now)
      const replacementClaim = "claim.public.fc3.steering.replacement"
      runtime.store.tryClaimWorkUnit({
        claimRef: replacementClaim,
        workUnitRef,
        runRef,
        assignmentRef: "assignment.public.fc3.steering.replacement",
        workerAccountRef: "account.public.fc3.claude",
        ttl: 60_000,
        now,
      })
      runtime.store.updateWorkClaimState(replacementClaim, "in_progress", now)
      const incompleteClaim = "claim.public.fc3.steering.incomplete"
      runtime.store.tryClaimWorkUnit({
        claimRef: incompleteClaim,
        workUnitRef: "fixture:fc3.incomplete",
        runRef,
        workerAccountRef: "account.public.fc3.grok",
        ttl: 60_000,
        now,
      })

      const intents = [
        fleetIntent({
          intentId: "intent.fc3.ambiguous",
          kind: "steer_message",
          targetRef: workUnitRef,
          body: "must not pick latest",
        }),
        fleetIntent({
          intentId: "intent.fc3.stale",
          kind: "steer_message",
          targetRef: workClaimRef,
          body: "stale",
        }),
        fleetIntent({
          intentId: "intent.fc3.missing",
          kind: "steer_message",
          targetRef: "assignment.public.fc3.missing",
          body: "missing",
        }),
        fleetIntent({
          intentId: "intent.fc3.incomplete",
          kind: "steer_message",
          targetRef: incompleteClaim,
          body: "incomplete",
        }),
        fleetIntent({
          intentId: "intent.fc3.approval",
          kind: "approval_decision",
          approvalRef: "assignment.public.fc3.steering.replacement",
          decision: "allow",
        }),
      ]
      const posted: FleetSteeringOutcome[][] = []
      const transport: PylonFleetRunSteeringTransport = {
        read: () => Promise.resolve(page(intents.map((intent, index) =>
          delivery((index + 1) * 10, intent)
        ), 50)),
        postOutcomes: ({ outcomes }) => {
          posted.push([...outcomes])
          return Promise.resolve(ack(outcomes))
        },
      }
      const result = await tickPylonFleetRunSteeringConsumer({
        store: runtime.store,
        transport,
        pylonRef,
        runRef,
        claimRef,
        now: () => now,
      })
      expect(result).toMatchObject({ ok: true, applied: 5, acknowledged: 5 })
      expect(posted[0]!.map(item => item.outcome)).toEqual([
        "rejected",
        "skipped_stale",
        "rejected",
        "rejected",
        "rejected",
      ])
      expect(JSON.stringify(posted)).not.toContain("must not pick latest")
      expect(runtime.store.listFleetRunSteeringQueuedFollowUps({
        pylonRef,
        runRef,
        claimRef,
      })).toEqual([])
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("queues only approval refs durably bound to the exact live attempt", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-approval-binding-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seed(env)
      const approvalRef = "approval.public.fc3.steering.unit"
      runtime.store.bindFleetRunSteeringApproval({
        approvalRef,
        pylonRef,
        runRef,
        claimRef,
        workUnitRef,
        workClaimRef,
        assignmentRef,
        now,
      })
      const known = fleetIntent({
        intentId: "intent.fc3.approval.known",
        kind: "approval_decision",
        approvalRef,
        decision: "allow",
      })
      const unknown = fleetIntent({
        intentId: "intent.fc3.approval.unknown",
        kind: "approval_decision",
        approvalRef: "approval.public.fc3.steering.unknown",
        decision: "deny",
      })
      const outcomes: FleetSteeringOutcome[] = []
      const transport: PylonFleetRunSteeringTransport = {
        read: () => Promise.resolve(page([delivery(1, known), delivery(2, unknown)], 2)),
        postOutcomes: ({ outcomes: batch }) => {
          outcomes.push(...batch)
          return Promise.resolve(ack(batch))
        },
      }
      const result = await tickPylonFleetRunSteeringConsumer({
        store: runtime.store,
        transport,
        pylonRef,
        runRef,
        claimRef,
        now: () => now,
      })
      expect(result).toMatchObject({ ok: true, applied: 2, acknowledged: 2 })
      expect(outcomes.map(outcome => outcome.outcome)).toEqual(["queued_follow_up", "rejected"])
      expect(runtime.store.listFleetRunSteeringQueuedFollowUps({
        pylonRef,
        runRef,
        claimRef,
      })).toEqual([expect.objectContaining({
        approvalRef,
        workUnitRef,
        workClaimRef,
        assignmentRef,
        decision: "allow",
      })])
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("rejects unordered, duplicate, mismatched, oversized, and stalled pages before mutation", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-invalid-pages-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seed(env)
      const one = fleetIntent({
        intentId: "intent.fc3.page.one",
        kind: "fleet_run_control",
        action: "pause",
      })
      const two = fleetIntent({
        intentId: "intent.fc3.page.two",
        kind: "fleet_run_control",
        action: "pause",
      })
      const cases: Array<{ value: FleetSteeringPage; limit: number }> = [
        { value: page([delivery(2, two), delivery(1, one)], 1), limit: 100 },
        { value: page([delivery(1, one), delivery(2, one)], 2), limit: 100 },
        { value: page([delivery(1, one), delivery(2, two)], 2), limit: 1 },
        { value: { ...page([], 0), upToDate: false }, limit: 100 },
      ]
      let posted = 0
      for (const testCase of cases) {
        const result = await tickPylonFleetRunSteeringConsumer({
          store: runtime.store,
          transport: {
            read: () => Promise.resolve(testCase.value),
            postOutcomes: () => {
              posted += 1
              return Promise.reject(new Error("invalid page must not ACK"))
            },
          },
          pylonRef,
          runRef,
          claimRef,
          limit: testCase.limit,
          now: () => now,
        })
        expect(result).toMatchObject({
          ok: false,
          applied: 0,
          watermark: 0,
          failure: "invalid_page",
        })
      }
      expect(posted).toBe(0)
      expect(runtime.store.getFleetRun(runRef)?.state).toBe("running")
      expect(runtime.store.listFleetRunSteeringOutcomeOutbox({
        pylonRef,
        runRef,
        claimRef,
      })).toEqual([])
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("accepts distinct server delivery and client intent timestamps", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-clock-domains-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seed(env)
      const pause = fleetIntent({
        intentId: "intent.fc3.clock-domains",
        kind: "fleet_run_control",
        action: "pause",
      })
      const serverDelivery = {
        ...delivery(1, pause),
        createdAt: "2026-07-10T02:00:05.000Z",
      }
      const transport: PylonFleetRunSteeringTransport = {
        read: () => Promise.resolve(page([serverDelivery], 1)),
        postOutcomes: ({ outcomes }) => Promise.resolve(ack(outcomes)),
      }
      const result = await tickPylonFleetRunSteeringConsumer({
        store: runtime.store,
        transport,
        pylonRef,
        runRef,
        claimRef,
        now: () => now,
      })
      expect(pause.createdAt).toBe("2026-07-10T02:00:00.000Z")
      expect(serverDelivery.createdAt).toBe("2026-07-10T02:00:05.000Z")
      expect(result).toMatchObject({ ok: true, applied: 1, acknowledged: 1 })
      expect(runtime.store.getFleetRun(runRef)?.state).toBe("paused")
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("bounds the private follow-up queue and stops reading at backpressure", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-follow-up-cap-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seed(env)
      const intents = Array.from({ length: 128 }, (_, index) => fleetIntent({
        intentId: `intent.fc3.cap.${String(index + 1).padStart(3, "0")}`,
        kind: "steer_message",
        targetRef: assignmentRef,
        bodyRef: `body.public.fc3.cap.${String(index + 1).padStart(3, "0")}`,
      }))
      const afters: number[] = []
      const transport: PylonFleetRunSteeringTransport = {
        read: ({ after }) => {
          afters.push(after)
          return Promise.resolve(after === 0
            ? page(intents.slice(0, 100).map((intent, index) =>
                delivery(index + 1, intent)
              ), 100)
            : page(intents.slice(100).map((intent, index) =>
                delivery(index + 101, intent)
              ), 128))
        },
        postOutcomes: ({ outcomes }) => Promise.resolve(ack(outcomes)),
      }
      const first = await tickPylonFleetRunSteeringConsumer({
        store: runtime.store,
        transport,
        pylonRef,
        runRef,
        claimRef,
        now: () => now,
      })
      expect(first).toMatchObject({
        ok: false,
        applied: 100,
        acknowledged: 64,
        pendingAcknowledgements: 36,
        failure: "backpressure",
      })
      const second = await tickPylonFleetRunSteeringConsumer({
        store: runtime.store,
        transport,
        pylonRef,
        runRef,
        claimRef,
        now: () => now,
      })
      expect(second).toMatchObject({
        ok: true,
        applied: 28,
        acknowledged: 64,
        pendingAcknowledgements: 0,
        watermark: 128,
      })
      const third = await tickPylonFleetRunSteeringConsumer({
        store: runtime.store,
        transport,
        pylonRef,
        runRef,
        claimRef,
        now: () => now,
      })
      expect(third).toMatchObject({
        ok: false,
        applied: 0,
        pendingAcknowledgements: 0,
        watermark: 128,
        failure: "backpressure",
      })
      expect(afters).toEqual([0, 100])
      expect(runtime.store.listFleetRunSteeringQueuedFollowUps({
        pylonRef,
        runRef,
        claimRef,
      })).toHaveLength(128)
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("rolls state, outcome, watermark, and ACK outbox back together when application throws", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-atomic-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seed(env)
      expect(() => runtime.store.applyFleetRunSteeringIntent({
        pylonRef,
        runRef,
        claimRef,
        seq: 1,
        intentId: "intent.fc3.atomic",
        intentKind: "fleet_run_control",
        intentDigest: "b".repeat(64),
        observedAt: now,
        outcomeRefFor: () => "outcome.pylon.fleet_steering.aaaaaaaaaaaaaaaaaaaaaaaa",
      }, () => {
        runtime.store.updateFleetRunState(runRef, "paused", now, "operator")
        throw new Error("fixture failure after state mutation")
      })).toThrow("fixture failure")
      expect(runtime.store.getFleetRun(runRef)?.state).toBe("running")
      expect(runtime.store.getFleetRunSteeringWatermark(pylonRef, runRef, claimRef)).toBe(0)
      expect(runtime.store.getFleetRunSteeringOutcome({
        pylonRef,
        runRef,
        claimRef,
        seq: 1,
        intentId: "intent.fc3.atomic",
      })).toBeNull()
      expect(runtime.store.listFleetRunSteeringOutcomeOutbox({
        pylonRef,
        runRef,
        claimRef,
      })).toEqual([])
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("stops refill but reports active stop as queued without releasing running claims", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-stop-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seed(env)
      const secondClaimRef = "claim.public.fc3.steering.second"
      runtime.store.tryClaimWorkUnit({
        claimRef: secondClaimRef,
        workUnitRef: "fixture:fc3.steering.second",
        runRef,
        assignmentRef: "assignment.public.fc3.steering.second",
        workerAccountRef: "account.public.fc3.claude",
        ttl: 60_000,
        now,
      })
      const stop = fleetIntent({
        intentId: "intent.fc3.stop",
        kind: "fleet_run_control",
        action: "stop",
      })
      let observedBeforeAck = false
      let stoppedOutcome: FleetSteeringOutcome | undefined
      const transport: PylonFleetRunSteeringTransport = {
        read: () => Promise.resolve(page([delivery(9, stop)], 9)),
        postOutcomes: ({ outcomes }) => {
          observedBeforeAck =
            runtime.store.getFleetRun(runRef)?.state === "stopped" &&
            runtime.store.listLiveWorkClaims(now)
              .filter(claim => claim.runRef === runRef).length === 2
          stoppedOutcome = outcomes[0]
          return Promise.resolve(ack(outcomes))
        },
      }
      const result = await tickPylonFleetRunSteeringConsumer({
        store: runtime.store,
        transport,
        pylonRef,
        runRef,
        claimRef,
        now: () => now,
      })
      expect(result).toMatchObject({ ok: true, applied: 1, acknowledged: 1 })
      expect(observedBeforeAck).toBe(true)
      expect(stoppedOutcome?.outcome).toBe("queued_follow_up")
      expect(runtime.store.getFleetRun(runRef)?.state).toBe("stopped")
      expect(runtime.store.getWorkClaim(workClaimRef)?.state).toBe("in_progress")
      expect(runtime.store.getWorkClaim(secondClaimRef)?.state).toBe("claimed")
      expect(runtime.store.listFleetRunSteeringQueuedFollowUps({
        pylonRef,
        runRef,
        claimRef,
      })).toEqual([
        expect.objectContaining({
          intentKind: "fleet_run_control",
          residualRefs: expect.arrayContaining([
            workClaimRef,
            assignmentRef,
            secondClaimRef,
            "assignment.public.fc3.steering.second",
          ]),
        }),
      ])
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("reports stop applied when the refill loop has no live work left", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-idle-stop-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const runtime = await seed(env)
      runtime.store.releaseWorkClaim(workClaimRef, now)
      const stop = fleetIntent({
        intentId: "intent.fc3.idle-stop",
        kind: "fleet_run_control",
        action: "stop",
      })
      let outcome: FleetSteeringOutcome | undefined
      const transport: PylonFleetRunSteeringTransport = {
        read: () => Promise.resolve(page([delivery(9, stop)], 9)),
        postOutcomes: ({ outcomes }) => {
          outcome = outcomes[0]
          return Promise.resolve(ack(outcomes))
        },
      }
      const result = await tickPylonFleetRunSteeringConsumer({
        store: runtime.store,
        transport,
        pylonRef,
        runRef,
        claimRef,
        now: () => now,
      })
      expect(result).toMatchObject({ ok: true, applied: 1, acknowledged: 1 })
      expect(outcome?.outcome).toBe("applied")
      expect(runtime.store.getFleetRun(runRef)?.state).toBe("stopped")
      expect(runtime.store.listFleetRunSteeringQueuedFollowUps({
        pylonRef,
        runRef,
        claimRef,
      })).toEqual([])
      await runtime.close()
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("mounts the optional consumer only from the standing run's exact accepted claim", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-standing-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const seeded = await seed(env)
      await seeded.close()
      const factoryInputs: unknown[] = []
      const followUpFactoryInputs: unknown[] = []
      let consumerClosed = 0
      let followUpClosed = 0
      const standing = await openPylonStandingFleetRunExecutor({
        env,
        now: () => now,
        pylonRef,
        runRef,
        livenessProbe: () => "live",
        planner: {
          plan: ({ now: at }) => Promise.resolve(planFixtureWork({
            kind: "fixture",
            units: [{ ref: "fc3.steering.unit", title: "Steering unit" }],
          }, { now: at })),
        },
        capacity: { accounts: () => Promise.resolve([]) },
        runner: {
          dispatch: () => Promise.reject(new Error("no capacity must dispatch nothing")),
        },
        clock: {
          now: () => now,
          sleep: () => new Promise<void>(() => {}),
        },
        startImmediately: false,
        steeringConsumerFactory: input => {
          factoryInputs.push(input)
          return {
            tick: () => Promise.resolve({
              ok: true,
              applied: 0,
              acknowledged: 0,
              pendingAcknowledgements: 0,
              watermark: 0,
              failure: null,
            }),
            close: () => {
              consumerClosed += 1
              return Promise.resolve()
            },
          }
        },
        steeringFollowUpDispatcherFactory: input => {
          followUpFactoryInputs.push(input)
          return {
            tick: () => Promise.resolve({
              ok: true,
              dispatched: 0,
              completionsDelivered: 0,
              pending: 0,
              failure: null,
            }),
            close: () => {
              followUpClosed += 1
              return Promise.resolve()
            },
          }
        },
      })
      expect(factoryInputs).toEqual([{
        store: standing.runtime.store,
        pylonRef,
        runRef,
        claimRef,
      }])
      expect(followUpFactoryInputs).toEqual(factoryInputs)
      await standing.close()
      expect(consumerClosed).toBe(1)
      expect(followUpClosed).toBe(1)
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("the canonical owner-local node composition polls with its agent bearer", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-fc3-steering-owned-standing-"))
    const env = { PYLON_HOME: join(root, "pylon-home") } as NodeJS.ProcessEnv
    try {
      const seeded = await seed(env)
      seeded.store.releaseWorkClaim(workClaimRef, now)
      await seeded.close()
      const calls: Array<{ url: string; init: RequestInit }> = []
      const fetchImpl = (async (url: string | URL | Request, init?: RequestInit) => {
        calls.push({ url: String(url), init: init ?? {} })
        return Response.json(page([], 0))
      }) as unknown as typeof fetch
      const standing = await openPylonOwnedStandingFleetRunExecutor({
        env,
        now: () => now,
        pylonRef,
        runRef,
        baseUrl: "https://openagents.test",
        agentToken: "oa_agent_owned_standing_fixture",
        fetch: fetchImpl,
        clock: {
          now: () => now,
          sleep: () => new Promise<void>(() => {}),
        },
        startImmediately: false,
        options: {
          grok: false,
          loadRegistry: () => Promise.resolve([]),
        },
      })
      try {
        for (let attempt = 0; attempt < 50 && calls.length === 0; attempt += 1) {
          await Bun.sleep(2)
        }
        expect(calls).toHaveLength(1)
        expect(new URL(calls[0]!.url).pathname).toBe(
          `/api/pylons/${pylonRef}/fleet-runs/${runRef}/steering`,
        )
        expect(calls[0]!.init.headers).toMatchObject({
          Authorization: "Bearer oa_agent_owned_standing_fixture",
        })
      } finally {
        await standing.close()
      }
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})
