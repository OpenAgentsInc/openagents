import { Schema } from "effect"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { describe, expect, test } from "vite-plus/test"

import {
  FULL_AUTO_DEFAULT_LANE,
  FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS,
  FULL_AUTO_RUN_ACTIVE_LIMIT,
  FULL_AUTO_RUN_LEGAL_TRANSITIONS,
  FULL_AUTO_RUN_RECEIPT_SCHEMA,
  createOmegaEffectdService,
} from "./index.ts"
import { FullAutoGuardrailsSchema } from "./engine/full-auto-registry.ts"
import {
  classifyFullAutoDispatchFailureReason,
  recoveryActionForCause,
} from "./engine/full-auto-liveness.ts"

describe("omega-effectd freeze constants", () => {
  test("keeps the eight-run active limit", () => {
    expect(FULL_AUTO_RUN_ACTIVE_LIMIT).toBe(8)
  })

  test("keeps the three non-overridable guardrails", () => {
    expect([...FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS]).toEqual([
      "workspace_binding",
      "own_capacity_only",
      "no_rate_limit_reset_triggering",
    ])
  })

  test("keeps the ten-state legal transition graph", () => {
    expect(FULL_AUTO_RUN_LEGAL_TRANSITIONS.size).toBe(10)
    expect(FULL_AUTO_RUN_LEGAL_TRANSITIONS.get("draft")).toEqual(
      new Set(["running", "stopped"]),
    )
  })

  test("keeps default lane and receipt schema id", () => {
    expect(FULL_AUTO_DEFAULT_LANE).toBe("codex-local")
    expect(FULL_AUTO_RUN_RECEIPT_SCHEMA).toBe("openagents.desktop.full_auto_run_receipt.v1")
  })
})

describe("omega-effectd FA-04 guardrail immunity and missing-thread recovery", () => {
  test("FullAutoGuardrailsSchema has no escape hatch for non-overridable laws", () => {
    const decoded = Schema.decodeUnknownSync(FullAutoGuardrailsSchema)({
      maxTurns: 12,
      workspace_binding: false,
      own_capacity_only: false,
      no_rate_limit_reset_triggering: true,
      rateLimitReset: true,
    })
    expect(decoded).toEqual({ maxTurns: 12 })
    expect(Object.keys(decoded).sort()).toEqual(["maxTurns"])
  })

  test("missing host thread classifies to stop_only stall", () => {
    expect(classifyFullAutoDispatchFailureReason("host_thread_missing")).toBe("host_thread_missing")
    expect(classifyFullAutoDispatchFailureReason("That conversation no longer exists.")).toBe(
      "host_thread_missing",
    )
    expect(recoveryActionForCause("host_thread_missing")).toBe("stop_only")
    expect(recoveryActionForCause("rate_limited")).toBe("retry_now")
  })
})

describe("omega-effectd service lifecycle", () => {
  test("starts and stops with an injected data root", async () => {
    const dataRoot = mkdtempSync(join(tmpdir(), "omega-effectd-"))
    const service = createOmegaEffectdService({ paths: { dataRoot } })
    await service.start()
    expect(service.health().status).toBe("running")
    expect(service.health().dataRoot).toBe(dataRoot)
    expect(service.activeRunLimit).toBe(8)
    await service.stop()
    expect(service.health().status).toBe("stopped")
    rmSync(dataRoot, { recursive: true, force: true })
  })
})
