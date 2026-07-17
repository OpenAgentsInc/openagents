// Oracle for FA-RUN-04 (#8972): the control-API surface for the private
// FullAutoRunReport and its derived public-safe FullAutoRunReceipt --
// GET /v1/full-auto/runs/{runRef}/report and .../receipt. A separate file
// from full-auto-run-report.test.ts (the pure aggregator/redaction oracle)
// so the HTTP/auth/OpenAPI-parity lane stays independently evolvable,
// matching the existing per-lane split (liveness, handoff) in this surface.
import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Schema } from "effect"

import {
  FullAutoControlRunReceiptResponseSchema,
  FullAutoControlRunReportResponseSchema,
} from "./full-auto-control-contract.ts"
import { openFullAutoRegistry } from "./full-auto-registry.ts"
import { openFullAutoRunRegistry, type FullAutoRunRegistry } from "./full-auto-run-registry.ts"
import { openFullAutoRunReportStore, type FullAutoRunReportStore } from "./full-auto-run-report.ts"
import {
  openProviderHandoffRegistry,
  type ProviderHandoffRegistry,
} from "./full-auto-provider-handoff.ts"
import { LOCAL_TURN_RECORD_SCHEMA, type LocalTurnRecord } from "./local-turn-journal.ts"
import {
  startFullAutoControlServer,
  type FullAutoControlServer,
} from "./full-auto-control-server.ts"

// Hoisted so the (repeated) response-decode calls below never recompile the
// same schema per call (root INVARIANTS.md: Effect Workspace Boundary).
const decodeReportResponse = Schema.decodeUnknownSync(FullAutoControlRunReportResponseSchema)
const decodeReceiptResponse = Schema.decodeUnknownSync(FullAutoControlRunReceiptResponseSchema)

const GRANTED_WORKSPACE = "/Users/secret-owner/full-auto-report-control/workspace"

const makeTurn = (
  input: Readonly<{
    threadRef: string
    turnRef: string
    updatedAt: string
    disposition: LocalTurnRecord["disposition"]
  }>,
): LocalTurnRecord => ({
  schema: LOCAL_TURN_RECORD_SCHEMA,
  threadRef: input.threadRef,
  turnRef: input.turnRef,
  lane: "codex-local",
  userMessageKey: `${input.turnRef}.user`,
  assistantMessageKey: `${input.turnRef}.assistant`,
  accountRef: "codex-primary",
  providerSessionRef: null,
  model: "gpt-codex",
  phase: input.disposition === "completed" ? "completed" : "failed",
  persistedCursor: 0,
  assistantText: "SECRET_RAW_TRANSCRIPT_MUST_NEVER_APPEAR_IN_ANY_RESPONSE",
  assistantSegments: [],
  recoveryGeneration: 0,
  disposition: input.disposition,
  createdAt: input.updatedAt,
  updatedAt: input.updatedAt,
})

type Harness = Readonly<{
  root: string
  runRegistry: FullAutoRunRegistry
  reportStore: FullAutoRunReportStore
  providerHandoffRegistry: ProviderHandoffRegistry
  turns: Array<LocalTurnRecord>
  server: FullAutoControlServer
  request: (
    method: "GET" | "POST",
    pathname: string,
    options?: Readonly<{ token?: string | null; body?: unknown }>,
  ) => Promise<Readonly<{ status: number; body: any }>>
  dispose: () => Promise<void>
}>

const startHarness = async (): Promise<Harness> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-report-control-"))
  const registry = openFullAutoRegistry(path.join(root, "registry.json"))
  const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
  const reportStore = openFullAutoRunReportStore(path.join(root, "reports.json"))
  const providerHandoffRegistry = openProviderHandoffRegistry(
    path.join(root, "provider-handoffs.json"),
  )
  const turns: Array<LocalTurnRecord> = []
  let mintedThreadCount = 0
  const server = await startFullAutoControlServer({
    capabilities: {
      registry,
      runRegistry,
      reportStore,
      providerHandoffRegistry,
      resolveWorkspaceRef: () => GRANTED_WORKSPACE,
      triggerReconciliation: async () => {},
      liveState: () => null,
      listTurns: (threadRef) => turns.filter((record) => record.threadRef === threadRef),
      appendSystemNote: () => {},
      createThread: () => {
        mintedThreadCount += 1
        return `thread.report-control.${mintedThreadCount}`
      },
      isLaneEligible: (laneRef) => laneRef === "codex-local",
      interruptLiveTurn: () => true,
    },
    controlFilePath: path.join(root, "full-auto", "control.json"),
  })
  const request: Harness["request"] = async (method, pathname, options) => {
    const token = options?.token === undefined ? server.credential.token : options.token
    const response = await fetch(`${server.url}${pathname}`, {
      method,
      headers: {
        ...(token === null ? {} : { authorization: `Bearer ${token}` }),
        ...(options?.body === undefined ? {} : { "content-type": "application/json" }),
      },
      ...(options?.body === undefined ? {} : { body: JSON.stringify(options.body) }),
    })
    return { status: response.status, body: await response.json() }
  }
  return {
    root,
    runRegistry,
    reportStore,
    providerHandoffRegistry,
    turns,
    server,
    request,
    dispose: async () => {
      await server.stop()
      rmSync(root, { recursive: true, force: true })
    },
  }
}

const START_BODY = {
  workspaceRef: GRANTED_WORKSPACE,
  title: "Report control surface",
  objective: "SECRET_OBJECTIVE_never_appears_in_receipt",
  doneCondition: "SECRET_DONE_CONDITION_never_appears_in_receipt",
}

describe("GET /v1/full-auto/runs/{runRef}/report (FA-RUN-04 #8972)", () => {
  test("requires the bearer credential", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: START_BODY,
      })
      const runRef = started.body.run.runRef
      const response = await harness.request("GET", `/v1/full-auto/runs/${runRef}/report`, {
        token: null,
      })
      expect(response.status).toBe(401)
    } finally {
      await harness.dispose()
    }
  })

  test("404s for an unknown runRef", async () => {
    const harness = await startHarness()
    try {
      const response = await harness.request("GET", "/v1/full-auto/runs/run.does-not-exist/report")
      expect(response.status).toBe(404)
      expect(response.body.error).toBe("not_found")
    } finally {
      await harness.dispose()
    }
  })

  test("a freshly started run gets one report decodable against the contract schema, with a lifecycle transition already present", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: START_BODY,
      })
      expect(started.status).toBe(200)
      const runRef = started.body.run.runRef

      const result = await harness.request("GET", `/v1/full-auto/runs/${runRef}/report`)
      expect(result.status).toBe(200)
      const decoded = decodeReportResponse(result.body)
      expect(decoded.report.runRef).toBe(runRef)
      expect(decoded.report.state).toBe("running")
      expect(decoded.report.lifecycleTransitions.length).toBeGreaterThanOrEqual(1)
      expect(decoded.report.lifecycleTransitions[0]!.to).toBe("running")
      // Never the raw objective/doneCondition text -- digests only.
      expect(JSON.stringify(decoded.report)).not.toContain(
        "SECRET_OBJECTIVE_never_appears_in_receipt",
      )
      expect(JSON.stringify(decoded.report)).not.toContain(
        "SECRET_DONE_CONDITION_never_appears_in_receipt",
      )
    } finally {
      await harness.dispose()
    }
  })

  test("aggregates a turn and a mutation-driven lifecycle transition (pause -> resume -> stop) across separate control-API calls, never duplicating turns", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: START_BODY,
      })
      const runRef = started.body.run.runRef
      const threadRef = started.body.run.threadRef

      harness.turns.push(
        makeTurn({
          threadRef,
          turnRef: "turn.full-auto.1",
          updatedAt: new Date().toISOString(),
          disposition: "completed",
        }),
      )

      const paused = await harness.request("POST", `/v1/full-auto/runs/${runRef}/pause`)
      expect(paused.status).toBe(200)
      const resumed = await harness.request("POST", `/v1/full-auto/runs/${runRef}/resume`)
      expect(resumed.status).toBe(200)
      const stopped = await harness.request("POST", `/v1/full-auto/runs/${runRef}/stop`)
      expect(stopped.status).toBe(200)

      const result = await harness.request("GET", `/v1/full-auto/runs/${runRef}/report`)
      const decoded = decodeReportResponse(result.body)
      expect(decoded.report.state).toBe("stopped")
      expect(decoded.report.endedAt).toBeDefined()
      expect(decoded.report.turns).toHaveLength(1)
      expect(decoded.report.turns[0]!.turnRef).toBe("turn.full-auto.1")
      const toStates = decoded.report.lifecycleTransitions.map((transition) => transition.to)
      expect(toStates).toContain("running")
      expect(toStates).toContain("paused")
      expect(toStates).toContain("stopped")

      // A second GET must never duplicate the already-captured turn.
      const again = await harness.request("GET", `/v1/full-auto/runs/${runRef}/report`)
      const decodedAgain = decodeReportResponse(again.body)
      expect(decodedAgain.report.turns).toHaveLength(1)
      expect(decodedAgain.report.reportRevision).toBeGreaterThan(decoded.report.reportRevision)
    } finally {
      await harness.dispose()
    }
  })
})

describe("GET /v1/full-auto/runs/{runRef}/receipt (FA-RUN-04 #8972)", () => {
  test("requires the bearer credential", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: START_BODY,
      })
      const runRef = started.body.run.runRef
      const response = await harness.request("GET", `/v1/full-auto/runs/${runRef}/receipt`, {
        token: null,
      })
      expect(response.status).toBe(401)
    } finally {
      await harness.dispose()
    }
  })

  test("404s for an unknown runRef", async () => {
    const harness = await startHarness()
    try {
      const response = await harness.request(
        "GET",
        "/v1/full-auto/runs/run.does-not-exist/receipt",
      )
      expect(response.status).toBe(404)
      expect(response.body.error).toBe("not_found")
    } finally {
      await harness.dispose()
    }
  })

  test("the receipt decodes against the contract schema and never carries the run's raw objective/doneCondition/workspace text over HTTP", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: START_BODY,
      })
      const runRef = started.body.run.runRef

      const result = await harness.request("GET", `/v1/full-auto/runs/${runRef}/receipt`)
      expect(result.status).toBe(200)
      const decoded = decodeReceiptResponse(result.body)
      expect(decoded.receipt.runRef).toBe(runRef)
      expect(decoded.receipt.state).toBe("running")
      expect(decoded.receipt.workspaceRefDigest).toMatch(/^[0-9a-f]{64}$/)

      const rawBody = JSON.stringify(result.body)
      for (const secret of [
        "SECRET_OBJECTIVE_never_appears_in_receipt",
        "SECRET_DONE_CONDITION_never_appears_in_receipt",
        GRANTED_WORKSPACE,
        "Report control surface", // the run's title
      ]) {
        expect(rawBody, `receipt HTTP response must never contain: ${secret}`).not.toContain(
          secret,
        )
      }
    } finally {
      await harness.dispose()
    }
  })

  test("a receipt read after a provider handoff reflects the handoff count and disposition without leaking the handoff reason text", async () => {
    const harness = await startHarness()
    try {
      const started = await harness.request("POST", "/v1/full-auto/runs/start", {
        body: START_BODY,
      })
      const runRef = started.body.run.runRef
      harness.providerHandoffRegistry.record({
        runRef,
        threadRef: started.body.run.threadRef,
        from: "codex-local",
        to: "claude-local",
        actor: "control_api",
        at: new Date().toISOString(),
        reason: "SECRET_HANDOFF_REASON_never_in_receipt",
        disposition: "complete_within_bounds",
        truncated: false,
      })
      const result = await harness.request("GET", `/v1/full-auto/runs/${runRef}/receipt`)
      const decoded = decodeReceiptResponse(result.body)
      expect(decoded.receipt.providerTransitionCount).toBe(1)
      expect(decoded.receipt.providerTransitionDispositions).toEqual(["complete_within_bounds"])
      expect(JSON.stringify(result.body)).not.toContain("SECRET_HANDOFF_REASON_never_in_receipt")
    } finally {
      await harness.dispose()
    }
  })
})
