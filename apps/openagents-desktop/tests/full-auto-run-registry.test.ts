// Oracle for behavior contract openagents_desktop.full_auto_play_pause_stop_lifecycle.v1
// (packages/behavior-contracts/src/openagents-apps.ts) -- the Draft/Running/
// Pausing/Paused/Retrying/Stalled/Completed/Failed/Stopped/Cap-reached
// lifecycle state machine this file exercises.
import { describe, expect, test } from "vite-plus/test"
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { openFullAutoRegistry, type FullAutoRecord } from "../src/full-auto-registry.ts"
import {
  FULL_AUTO_LEGACY_MIGRATION_DONE_CONDITION,
  FULL_AUTO_LEGACY_MIGRATION_OBJECTIVE,
  FULL_AUTO_RUN_ACTIVE_STATES,
  FULL_AUTO_RUN_ACTIVE_LIMIT,
  FULL_AUTO_RUN_LEGAL_TRANSITIONS,
  FULL_AUTO_RUN_RECORD_LIMIT,
  FULL_AUTO_RUN_REGISTRY_SCHEMA,
  FullAutoRunStateSchema,
  applyFullAutoRunTransition,
  isFullAutoRunActive,
  isFullAutoRunTerminal,
  isLegalFullAutoRunTransition,
  migrateLegacyFullAutoRegistry,
  openFullAutoRunRegistry,
  settleFullAutoRunFromThreadState,
  type FullAutoRun,
  type FullAutoRunState,
} from "../src/full-auto-run-registry.ts"

const withTempDir = <A>(prefix: string, fn: (root: string) => A): A => {
  const root = mkdtempSync(path.join(tmpdir(), prefix))
  try {
    return fn(root)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

const draftInput = (overrides?: Partial<Parameters<ReturnType<typeof openFullAutoRunRegistry>["createDraft"]>[0]>) => ({
  title: "Fix the flaky test",
  objective: "Make tests/flaky.test.ts stop flaking under repeated runs.",
  doneCondition: "The test passes 20 consecutive local runs and CI is green.",
  objectiveSource: "user" as const,
  workspaceRef: "/granted/workspace",
  ...overrides,
})

const ALL_STATES: ReadonlyArray<FullAutoRunState> = [
  "draft",
  "running",
  "pausing",
  "paused",
  "retrying",
  "stalled",
  "completed",
  "failed",
  "stopped",
  "cap_reached",
]

describe("FullAutoRun lifecycle state machine (FA-RUN-01 #8969, FA-AC-43/44/45/46/50)", () => {
  test("the lifecycle enumeration is exactly Draft/Running/Pausing/Paused/Retrying/Stalled/Completed/Failed/Stopped/Cap-reached", () => {
    expect([...FullAutoRunStateSchema.literals]).toEqual([
      "draft",
      "running",
      "pausing",
      "paused",
      "retrying",
      "stalled",
      "completed",
      "failed",
      "stopped",
      "cap_reached",
    ])
  })

  test("terminal states have no outgoing edges and are never resumed in place", () => {
    for (const state of ["completed", "failed", "stopped", "cap_reached"] as const) {
      expect(isFullAutoRunTerminal(state)).toBe(true)
      expect(FULL_AUTO_RUN_LEGAL_TRANSITIONS.get(state)?.size ?? 0).toBe(0)
      for (const to of ALL_STATES) expect(isLegalFullAutoRunTransition(state, to)).toBe(false)
    }
  })

  test("Resume (draft/running/pausing/retrying/stalled -> running) is legal ONLY from paused, matching the behavior contract's illegal-transition example", () => {
    for (const from of ALL_STATES) {
      if (from === "paused") {
        expect(isLegalFullAutoRunTransition(from, "running")).toBe(true)
      } else if (from === "draft" || from === "retrying") {
        // draft->running is Start, retrying->running is a settled retry --
        // both legal edges into running, but neither is "Resume" (Resume is
        // specifically the paused->running edge exercised below).
        continue
      } else {
        expect(isLegalFullAutoRunTransition(from, "running")).toBe(false)
      }
    }
  })

  test("Stop is legal from every non-terminal state and never legal from a terminal state", () => {
    for (const from of ALL_STATES) {
      if (isFullAutoRunTerminal(from)) {
        expect(isLegalFullAutoRunTransition(from, "stopped")).toBe(false)
      } else {
        expect(isLegalFullAutoRunTransition(from, "stopped")).toBe(true)
      }
    }
  })

  test("the v1 concurrency-active state set is exactly running/pausing/paused/retrying/stalled -- draft and every terminal state are excluded", () => {
    expect([...FULL_AUTO_RUN_ACTIVE_STATES].sort()).toEqual(
      ["running", "pausing", "paused", "retrying", "stalled"].sort(),
    )
    expect(isFullAutoRunActive("draft")).toBe(false)
    expect(isFullAutoRunActive("completed")).toBe(false)
  })

  test("applyFullAutoRunTransition refuses an illegal transition with a typed error and never silently coerces state", () => {
    withTempDir("oa-full-auto-run-pure-transition-", root => {
      const registry = openFullAutoRunRegistry(
        path.join(root, "runs.json"),
        () => new Date("2026-07-17T00:00:00.000Z"),
      )
      const run = registry.createDraft(draftInput())
      expect(() =>
        applyFullAutoRunTransition(run, { to: "completed", actor: "owner_ui", reason: "skip ahead" }),
      ).toThrow(/illegal Full Auto run transition draft -> completed/)
      // The run itself (returned separately, not mutated by the throwing call)
      // is untouched.
      expect(run.state).toBe("draft")
    })
  })

  test("every legal transition persists actor/time/reason and increments stateRevision", () => {
    let run: FullAutoRun = {
      runRef: "run.test.1",
      title: "t",
      objective: "o",
      objectiveSource: "user",
      doneCondition: "d",
      objectiveHistory: [],
      turnCap: 20,
      successfulAttempts: 0,
      failedAttempts: 0,
      state: "draft",
      stateRevision: 0,
      createdAt: "2026-07-17T00:00:00.000Z",
      transitions: [],
    }
    run = applyFullAutoRunTransition(
      run,
      { to: "running", actor: "control_api", reason: "Start" },
      () => new Date("2026-07-17T00:01:00.000Z"),
    )
    expect(run.state).toBe("running")
    expect(run.stateRevision).toBe(1)
    expect(run.startedAt).toBe("2026-07-17T00:01:00.000Z")
    expect(run.transitions).toEqual([
      { from: "draft", to: "running", actor: "control_api", reason: "Start", at: "2026-07-17T00:01:00.000Z" },
    ])
  })

  test("Pause with an active turn goes Running -> Pausing -> Paused only once the turn resolves; Pause with no turn goes directly to Paused", () => {
    expect(isLegalFullAutoRunTransition("running", "pausing")).toBe(true)
    expect(isLegalFullAutoRunTransition("running", "paused")).toBe(true)
    expect(isLegalFullAutoRunTransition("pausing", "paused")).toBe(true)
    // Pausing cannot short-circuit straight back to running -- Resume is the
    // only path back, and only from Paused.
    expect(isLegalFullAutoRunTransition("pausing", "running")).toBe(false)
  })
})

describe("FullAutoRun registry: concurrent runs, draft/start, rerun, eviction (FA-AC-38/39/40/50)", () => {
  test("a run created via startNew carries runRef, title, objective, doneCondition, workspace, profile, cap, and state", () => {
    withTempDir("oa-full-auto-run-create-", root => {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"), () => new Date("2026-07-17T00:00:00.000Z"))
      const result = registry.startNew({
        ...draftInput(),
        profile: { lane: "codex-local" },
        turnCap: 12,
        actor: "control_api",
        reason: "control API start",
      })
      expect(result.ok).toBe(true)
      if (!result.ok) return
      expect(result.run.runRef).toMatch(/^run\.full-auto\./)
      expect(result.run.title).toBe("Fix the flaky test")
      expect(result.run.objective).toContain("flaky")
      expect(result.run.doneCondition).toContain("20 consecutive")
      expect(result.run.workspaceRef).toBe("/granted/workspace")
      expect(result.run.profile?.lane).toBe("codex-local")
      expect(result.run.turnCap).toBe(12)
      expect(result.run.state).toBe("running")
    })
  })

  test("starts multiple active runs with distinct runRef/threadRef-scoped lifecycle state", () => {
    withTempDir("oa-full-auto-run-concurrency-", root => {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const first = registry.startNew({ ...draftInput(), actor: "control_api", reason: "first" })
      expect(first.ok).toBe(true)
      if (!first.ok) return

      // A second draft and start are independent of the first run.
      const secondDraft = registry.createDraft(draftInput({ title: "Second mission" }))
      expect(secondDraft.state).toBe("draft")

      const secondStart = registry.start(secondDraft.runRef, { actor: "control_api", reason: "second" })
      expect(secondStart.ok).toBe(true)
      if (!secondStart.ok) return
      expect(registry.activeRuns().map(run => run.runRef)).toEqual(expect.arrayContaining([
        first.run.runRef,
        secondStart.run.runRef,
      ]))

      // Stopping one run does not mutate or stop the other.
      const stopped = registry.transition(first.run.runRef, { to: "stopped", actor: "owner_ui", reason: "done" })
      expect(stopped.ok).toBe(true)
      expect(registry.get(secondStart.run.runRef)?.state).toBe("running")
      expect(registry.activeRuns().map(run => run.runRef)).toEqual([secondStart.run.runRef])
    })
  })

  test("a successful fallback attempt atomically projects the lane that actually accepted the dispatch", () => {
    withTempDir("oa-full-auto-run-success-profile-", root => {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const started = registry.startNew({
        ...draftInput(),
        threadRef: "thread-rotated",
        profile: { lane: "fable-local", model: "claude-opus-4-8" },
        actor: "control_api",
        reason: "start on Claude",
      })
      expect(started.ok).toBe(true)
      if (!started.ok) return

      registry.recordAttempt(started.run.runRef, "failure", { reason: "provider_error" })
      const accepted = registry.recordAttempt(started.run.runRef, "success", {
        turnRef: "turn.full-auto.codex-fallback",
        profile: { lane: "codex-local", model: "gpt-5.6-sol" },
      })

      expect(accepted?.profile).toEqual({ lane: "codex-local", model: "gpt-5.6-sol" })
      expect(accepted?.successfulAttempts).toBe(1)
      expect(accepted?.failedAttempts).toBe(1)
      expect(openFullAutoRunRegistry(path.join(root, "runs.json")).get(started.run.runRef)?.profile)
        .toEqual({ lane: "codex-local", model: "gpt-5.6-sol" })
    })
  })

  test("bounds local concurrent admission without mutating an existing run or minting an extra draft", () => {
    withTempDir("oa-full-auto-run-capacity-", root => {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      for (let index = 0; index < FULL_AUTO_RUN_ACTIVE_LIMIT; index += 1) {
        expect(registry.startNew({
          ...draftInput({ title: `Mission ${index}` }),
          threadRef: `thread-${index}`,
          actor: "control_api",
          reason: "capacity fixture",
        }).ok).toBe(true)
      }
      const before = registry.list().map(run => ({ runRef: run.runRef, state: run.state }))
      const refused = registry.startNew({
        ...draftInput({ title: "Mission over limit" }),
        threadRef: "thread-over-limit",
        actor: "control_api",
        reason: "capacity fixture",
      })
      expect(refused).toEqual({
        ok: false,
        reason: "active_run_limit_reached",
        activeRunCount: FULL_AUTO_RUN_ACTIVE_LIMIT,
        activeRunLimit: FULL_AUTO_RUN_ACTIVE_LIMIT,
      })
      expect(registry.list().map(run => ({ runRef: run.runRef, state: run.state }))).toEqual(before)
    })
  })

  test("starting a new run from a terminal run mints a new distinct runRef, never mutates the terminal record, and carries predecessorRunRef for context only", () => {
    withTempDir("oa-full-auto-run-rerun-", root => {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const first = registry.startNew({ ...draftInput(), actor: "control_api", reason: "first" })
      expect(first.ok).toBe(true)
      if (!first.ok) return
      registry.transition(first.run.runRef, { to: "completed", actor: "owner_ui", reason: "shipped" })
      const beforeRerunSnapshot = registry.get(first.run.runRef)

      const rerunResult = registry.rerun(first.run.runRef, {
        ...draftInput({ title: "Follow-up mission" }),
        actor: "owner_ui",
        reason: "rerun after completion",
      })
      expect(rerunResult.ok).toBe(true)
      if (!rerunResult.ok) return
      expect(rerunResult.run.runRef).not.toBe(first.run.runRef)
      expect(rerunResult.run.predecessorRunRef).toBe(first.run.runRef)
      expect(rerunResult.run.state).toBe("running")

      // The terminal predecessor is byte-for-byte unchanged.
      expect(registry.get(first.run.runRef)).toEqual(beforeRerunSnapshot)
    })
  })

  test("rerun refuses a non-terminal or unknown predecessor", () => {
    withTempDir("oa-full-auto-run-rerun-guard-", root => {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const first = registry.startNew({ ...draftInput(), actor: "control_api", reason: "first" })
      expect(first.ok).toBe(true)
      if (!first.ok) return
      const notTerminal = registry.rerun(first.run.runRef, { ...draftInput(), actor: "owner_ui", reason: "x" })
      expect(notTerminal).toEqual({ ok: false, reason: "predecessor_not_terminal" })
      const unknown = registry.rerun("run.does-not-exist", { ...draftInput(), actor: "owner_ui", reason: "x" })
      expect(unknown).toEqual({ ok: false, reason: "predecessor_not_found" })
    })
  })

  test("illegal transition attempts return a typed result rather than throwing at the registry boundary", () => {
    withTempDir("oa-full-auto-run-illegal-", root => {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const draft = registry.createDraft(draftInput())
      const result = registry.transition(draft.runRef, { to: "completed", actor: "owner_ui", reason: "skip" })
      expect(result).toEqual({ ok: false, reason: "illegal_transition", from: "draft", to: "completed" })
      expect(registry.get(draft.runRef)?.state).toBe("draft")
    })
  })

  test("eviction never drops a non-terminal run; the disabled/terminal/draft tail is bounded", () => {
    withTempDir("oa-full-auto-run-eviction-", root => {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const active = registry.startNew({ ...draftInput(), actor: "control_api", reason: "keep me" })
      expect(active.ok).toBe(true)
      if (!active.ok) return
      registry.transition(active.run.runRef, { to: "paused", actor: "owner_ui", reason: "pause to free the slot for drafts" })

      for (let index = 0; index < FULL_AUTO_RUN_RECORD_LIMIT + 20; index += 1) {
        registry.createDraft(draftInput({ title: `filler ${index}` }))
      }

      const all = registry.list()
      expect(all.some(run => run.runRef === active.run.runRef)).toBe(true)
      expect(all.length).toBeLessThanOrEqual(FULL_AUTO_RUN_RECORD_LIMIT)
    })
  })

  test("a corrupt run registry file is quarantined and the registry opens empty instead of throwing", () => {
    withTempDir("oa-full-auto-run-corrupt-", root => {
      const registryDir = path.join(root, "full-auto")
      const registryFile = path.join(registryDir, "runs.json")
      mkdirSync(registryDir, { recursive: true })
      writeFileSync(registryFile, "{ not json", "utf8")
      const registry = openFullAutoRunRegistry(registryFile)
      expect(registry.list()).toEqual([])
      expect(readdirSync(registryDir).some(name => name.includes("quarantined"))).toBe(true)
    })
  })

  test("a schema-invalid (but valid JSON) run registry file is also quarantined rather than thrown", () => {
    withTempDir("oa-full-auto-run-schema-invalid-", root => {
      const registryDir = path.join(root, "full-auto")
      const registryFile = path.join(registryDir, "runs.json")
      mkdirSync(registryDir, { recursive: true })
      writeFileSync(
        registryFile,
        JSON.stringify({ schema: FULL_AUTO_RUN_REGISTRY_SCHEMA, runs: [{ runRef: "x" }] }),
        "utf8",
      )
      const registry = openFullAutoRunRegistry(registryFile)
      expect(registry.list()).toEqual([])
      expect(readdirSync(registryDir).some(name => name.includes("quarantined"))).toBe(true)
    })
  })

  test("objective and done condition persist across a fresh registry open against the same file (restart survival)", () => {
    withTempDir("oa-full-auto-run-restart-", root => {
      const file = path.join(root, "runs.json")
      const first = openFullAutoRunRegistry(file)
      const started = first.startNew({ ...draftInput(), actor: "control_api", reason: "start" })
      expect(started.ok).toBe(true)
      if (!started.ok) return

      const reopened = openFullAutoRunRegistry(file)
      const restored = reopened.get(started.run.runRef)
      expect(restored?.objective).toBe(started.run.objective)
      expect(restored?.doneCondition).toBe(started.run.doneCondition)
      expect(restored?.state).toBe("running")
      const raw = JSON.parse(readFileSync(file, "utf8"))
      expect(raw.schema).toBe(FULL_AUTO_RUN_REGISTRY_SCHEMA)
    })
  })
})

describe("Legacy registry migration (FA-AC-41)", () => {
  const legacyRecord = (overrides: Partial<FullAutoRecord> = {}): FullAutoRecord => ({
    threadRef: "thread-legacy",
    enabled: true,
    continuationCount: 3,
    updatedAt: "2026-07-16T00:00:00.000Z",
    ...overrides,
  })

  test("an enabled legacy row migrates to exactly one Running FullAutoRun with the exact prior generic instruction as a marked legacy_migration objective", () => {
    withTempDir("oa-full-auto-migration-basic-", root => {
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const outcome = migrateLegacyFullAutoRegistry({
        legacyRecords: [legacyRecord({ workspaceRef: "/w", profile: { lane: "codex-local" } })],
        runRegistry,
      })
      expect(outcome.migrated).toHaveLength(1)
      expect(outcome.skippedDisabled).toEqual([])
      expect(outcome.migrated[0]!.objective).toBe(FULL_AUTO_LEGACY_MIGRATION_OBJECTIVE)
      expect(outcome.migrated[0]!.doneCondition).toBe(FULL_AUTO_LEGACY_MIGRATION_DONE_CONDITION)
      expect(outcome.migrated[0]!.objectiveSource).toBe("legacy_migration")
      expect(outcome.migrated[0]!.threadRef).toBe("thread-legacy")
      expect(outcome.migrated[0]!.workspaceRef).toBe("/w")
      expect(outcome.migrated[0]!.state).toBe("running")
      expect(outcome.migrated[0]!.migratedFrom).toBe("legacy_registry")
    })
  })

  test("a disabled legacy row does NOT migrate to an active run and cannot start merely because Desktop relaunches", () => {
    withTempDir("oa-full-auto-migration-disabled-", root => {
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const outcome = migrateLegacyFullAutoRegistry({
        legacyRecords: [legacyRecord({ threadRef: "thread-disabled", enabled: false })],
        runRegistry,
      })
      expect(outcome.migrated).toEqual([])
      expect(outcome.skippedDisabled).toEqual(["thread-disabled"])
      expect(runRegistry.list()).toEqual([])
    })
  })

  test("migration is idempotent: a second migration pass over the same legacy rows performs no duplicate migration", () => {
    withTempDir("oa-full-auto-migration-idempotent-", root => {
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const legacyRecords = [legacyRecord()]
      const first = migrateLegacyFullAutoRegistry({ legacyRecords, runRegistry })
      expect(first.migrated).toHaveLength(1)
      const second = migrateLegacyFullAutoRegistry({ legacyRecords, runRegistry })
      expect(second.migrated).toEqual([])
      expect(second.skippedAlreadyMigrated).toEqual(["thread-legacy"])
      // Exactly one run exists for that threadRef, not two.
      expect(runRegistry.list().filter(run => run.threadRef === "thread-legacy")).toHaveLength(1)
    })
  })

  test("concurrently-enabled legacy rows migrate as independently active runs without data loss", () => {
    withTempDir("oa-full-auto-migration-concurrency-", root => {
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const outcome = migrateLegacyFullAutoRegistry({
        legacyRecords: [
          legacyRecord({ threadRef: "thread-a", workspaceRef: "/a" }),
          legacyRecord({ threadRef: "thread-b", workspaceRef: "/b", continuationCount: 9 }),
        ],
        runRegistry,
      })
      expect(outcome.migrated).toHaveLength(2)
      expect(outcome.migrated.map(run => run.threadRef)).toEqual(["thread-a", "thread-b"])
      expect(outcome.preservedAsDraft).toEqual([])
      expect(runRegistry.activeRuns()).toHaveLength(2)
      expect(runRegistry.findByThreadRef("thread-b")?.workspaceRef).toBe("/b")
    })
  })

  test("migration composes with a real legacy full-auto-registry.ts file", () => {
    withTempDir("oa-full-auto-migration-real-registry-", root => {
      const legacyRegistry = openFullAutoRegistry(path.join(root, "legacy.json"))
      legacyRegistry.set("thread-real", true, { workspaceRef: "/real", profile: { lane: "codex-local" } })
      legacyRegistry.set("thread-real-disabled", false, { disabledBy: "ui_toggle" })

      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const outcome = migrateLegacyFullAutoRegistry({ legacyRecords: legacyRegistry.list(), runRegistry })
      expect(outcome.migrated).toHaveLength(1)
      expect(outcome.migrated[0]!.threadRef).toBe("thread-real")
      expect(outcome.skippedDisabled).toEqual(["thread-real-disabled"])
    })
  })
})

describe("Missing/orphaned thread and provider-session disposition (FA-AC-42 stub)", () => {
  test("a Running run whose bound thread record is missing transitions to Stalled with a typed, owner-visible reason -- never silent reattachment", () => {
    withTempDir("oa-full-auto-orphan-", root => {
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const started = runRegistry.startNew({ ...draftInput(), threadRef: "thread-orphan", actor: "control_api", reason: "start" })
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const settled = settleFullAutoRunFromThreadState(runRegistry, started.run, {
        threadRecord: null,
        turnRunning: false,
      })
      expect(settled.state).toBe("stalled")
      expect(settled.transitions.at(-1)?.actor).toBe("thread_state_sync")
      expect(settled.transitions.at(-1)?.reason).toContain("thread-orphan")
    })
  })

  test("a Running run whose thread was disabled by continuation_cap settles to cap_reached; by dispatch_failure_limit settles to failed", () => {
    withTempDir("oa-full-auto-settle-", root => {
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const capRun = runRegistry.startNew({ ...draftInput(), threadRef: "thread-cap", actor: "control_api", reason: "start" })
      expect(capRun.ok).toBe(true)
      if (!capRun.ok) return
      const capSettled = settleFullAutoRunFromThreadState(runRegistry, capRun.run, {
        threadRecord: {
          threadRef: "thread-cap",
          enabled: false,
          continuationCount: 20,
          updatedAt: "2026-07-17T00:00:00.000Z",
          disabledBy: "continuation_cap",
          blockedReason: "continuation_cap_reached",
        },
        turnRunning: false,
      })
      expect(capSettled.state).toBe("cap_reached")

      const failRun = runRegistry.startNew({
        ...draftInput({ title: "another mission" }),
        threadRef: "thread-fail",
        actor: "control_api",
        reason: "start",
      })
      // Concurrency will refuse this since capRun's cap_reached is terminal
      // now, freeing the slot -- but startNew above already checked before
      // the settle call above ran in this test, so retry once explicitly.
      const failStart = failRun.ok ? failRun : runRegistry.start(
        runRegistry.list().find(run => run.threadRef === "thread-fail")?.runRef ?? "",
        { actor: "control_api", reason: "retry" },
      )
      expect(failStart.ok).toBe(true)
      if (!failStart.ok) return
      const failSettled = settleFullAutoRunFromThreadState(runRegistry, failStart.run, {
        threadRecord: {
          threadRef: "thread-fail",
          enabled: false,
          continuationCount: 4,
          updatedAt: "2026-07-17T00:00:00.000Z",
          disabledBy: "dispatch_failure_limit",
          blockedReason: "5 consecutive dispatch failures",
        },
        turnRunning: false,
      })
      expect(failSettled.state).toBe("failed")
      expect(failSettled.terminalReason).toBe("5 consecutive dispatch failures")
    })
  })

  test("a Pausing run settles to Paused only once the turn stops running, never while turnRunning is true", () => {
    withTempDir("oa-full-auto-settle-pausing-", root => {
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const started = runRegistry.startNew({ ...draftInput(), threadRef: "thread-pausing", actor: "control_api", reason: "start" })
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const pausing = runRegistry.transition(started.run.runRef, { to: "pausing", actor: "owner_ui", reason: "pause requested" })
      expect(pausing.ok).toBe(true)
      if (!pausing.ok) return

      const stillRunning = settleFullAutoRunFromThreadState(runRegistry, pausing.run, {
        threadRecord: { threadRef: "thread-pausing", enabled: true, continuationCount: 1, updatedAt: "x" },
        turnRunning: true,
      })
      expect(stillRunning.state).toBe("pausing")

      const resolved = settleFullAutoRunFromThreadState(runRegistry, stillRunning, {
        threadRecord: { threadRef: "thread-pausing", enabled: true, continuationCount: 1, updatedAt: "x" },
        turnRunning: false,
      })
      expect(resolved.state).toBe("paused")
      expect(resolved.transitions.at(-1)?.actor).toBe("turn_resolution")
    })
  })

  test("settle is a no-op for a terminal run", () => {
    withTempDir("oa-full-auto-settle-terminal-", root => {
      const runRegistry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const started = runRegistry.startNew({ ...draftInput(), threadRef: "thread-terminal", actor: "control_api", reason: "start" })
      expect(started.ok).toBe(true)
      if (!started.ok) return
      const stopped = runRegistry.transition(started.run.runRef, { to: "stopped", actor: "owner_ui", reason: "done" })
      expect(stopped.ok).toBe(true)
      if (!stopped.ok) return
      const settled = settleFullAutoRunFromThreadState(runRegistry, stopped.run, { threadRecord: null, turnRunning: false })
      expect(settled).toEqual(stopped.run)
    })
  })
})
