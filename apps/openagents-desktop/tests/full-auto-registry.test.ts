import { describe, expect, test } from "vite-plus/test"
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  FULL_AUTO_BLOCKED_REASON_LIMIT,
  FULL_AUTO_RECORD_LIMIT,
  FULL_AUTO_REGISTRY_SCHEMA,
  FULL_AUTO_ROTATION_HISTORY_LIMIT,
  FULL_AUTO_ROUTING_POLICY_LIMIT,
  openFullAutoRegistry,
  projectFullAutoRotationHistory,
} from "../src/full-auto-registry.ts"

const record = (registry: ReturnType<typeof openFullAutoRegistry>, threadRef: string) =>
  registry.list().find(entry => entry.threadRef === threadRef)

describe("Full Auto registry cap semantics (FA-H7 #8880)", () => {
  test("continuationCount resets ONLY on toggle-off: a manual send leaves it unchanged; off-then-on zeroes it", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-cap-semantics-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registry = openFullAutoRegistry(registryFile)

      registry.set("thread-cap-semantics", true)
      for (let index = 0; index < 3; index += 1) registry.incrementContinuation("thread-cap-semantics")
      expect(record(registry, "thread-cap-semantics")?.continuationCount).toBe(3)

      // A manual send while the toggle stays on touches nothing in the
      // registry -- that IS the pinned semantic. The renderer sends the turn;
      // no registry API is invoked, so the count must be exactly where it was.
      expect(record(registry, "thread-cap-semantics")?.continuationCount).toBe(3)

      // Re-enabling while already enabled also preserves the count (set with
      // enabled: true keeps the existing counter).
      registry.set("thread-cap-semantics", true)
      expect(record(registry, "thread-cap-semantics")?.continuationCount).toBe(3)

      // Toggling off then on is the ONLY reset path.
      registry.set("thread-cap-semantics", false, { disabledBy: "ui_toggle" })
      registry.set("thread-cap-semantics", true)
      expect(record(registry, "thread-cap-semantics")?.continuationCount).toBe(0)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("Full Auto durable record extensions (FA-H2 #8875, FA-H3 #8876, FA-H5 #8878, FA-H6 #8879)", () => {
  test("an existing v1 registry file (no wave-2 fields) still decodes -- the schema upgrade never quarantines a user's state", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-v1-compat-"))
    try {
      const registryDir = path.join(root, "full-auto")
      const registryFile = path.join(registryDir, "registry.json")
      mkdirSync(registryDir, { recursive: true })
      // The exact pre-wave-2 on-disk shape: enabled-only records.
      writeFileSync(registryFile, JSON.stringify({
        schema: FULL_AUTO_REGISTRY_SCHEMA,
        records: [
          { threadRef: "thread-v1-enabled", enabled: true, continuationCount: 7, updatedAt: "2026-07-10T00:00:00.000Z" },
          { threadRef: "thread-v1-disabled", enabled: false, continuationCount: 0, updatedAt: "2026-07-09T00:00:00.000Z" },
        ],
      }), "utf8")

      const registry = openFullAutoRegistry(registryFile)
      expect(readdirSync(registryDir).filter(name => name.includes("quarantined"))).toEqual([])
      expect(registry.get("thread-v1-enabled")).toBe(true)
      const record = registry.record("thread-v1-enabled")
      expect(record?.continuationCount).toBe(7)
      expect(record?.workspaceRef).toBeUndefined()
      expect(record?.profile).toBeUndefined()
      expect(record?.pendingTurnRef ?? null).toBe(null)
      expect(record?.consecutiveFailures ?? 0).toBe(0)
      expect(record?.blockedReason ?? null).toBe(null)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("claimPending holds the lease exactly once until cleared; a missing record can never be claimed", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-lease-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registry = openFullAutoRegistry(registryFile)
      registry.set("thread-lease", true)

      expect(registry.claimPending("thread-lease", "turn.full-auto.one")).toBe(true)
      expect(registry.claimPending("thread-lease", "turn.full-auto.two")).toBe(false)
      const held = registry.record("thread-lease")
      expect(held?.pendingTurnRef).toBe("turn.full-auto.one")
      expect(held?.pendingStartedAt).toBeDefined()

      // The lease is durable: a fresh open of the same file still holds it.
      expect(openFullAutoRegistry(registryFile).record("thread-lease")?.pendingTurnRef).toBe("turn.full-auto.one")

      registry.clearPending("thread-lease")
      const cleared = registry.record("thread-lease")
      expect(cleared?.pendingTurnRef ?? null).toBe(null)
      expect(cleared?.pendingStartedAt).toBeUndefined()
      expect(registry.claimPending("thread-lease", "turn.full-auto.two")).toBe(true)

      expect(registry.claimPending("thread-missing", "turn.full-auto.x")).toBe(false)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("recordFailure increments and stamps typed failure state (releasing the lease); recordSuccess clears all of it", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-failure-state-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-f", true)
      registry.claimPending("thread-f", "turn.full-auto.pending")

      expect(registry.recordFailure("thread-f", "account_exhausted")).toBe(1)
      expect(registry.recordFailure("thread-f", "x".repeat(FULL_AUTO_BLOCKED_REASON_LIMIT + 50))).toBe(2)
      const failed = registry.record("thread-f")
      expect(failed?.consecutiveFailures).toBe(2)
      expect(failed?.lastFailureAt).toBeDefined()
      expect(failed?.blockedReason).toHaveLength(FULL_AUTO_BLOCKED_REASON_LIMIT)
      expect(failed?.pendingTurnRef ?? null).toBe(null)
      expect(failed?.enabled).toBe(true)

      registry.recordSuccess("thread-f")
      const cleared = registry.record("thread-f")
      expect(cleared?.consecutiveFailures ?? 0).toBe(0)
      expect(cleared?.lastFailureAt).toBeUndefined()
      expect(cleared?.blockedReason ?? null).toBe(null)

      expect(registry.recordFailure("thread-missing", "whatever")).toBe(0)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("bindWorkspace and bindProfile persist durably across a fresh open; binding a missing record is a null no-op", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-bind-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registry = openFullAutoRegistry(registryFile)
      registry.set("thread-bind", true)
      expect(registry.bindWorkspace("thread-bind", "/repo/granted")?.workspaceRef).toBe("/repo/granted")
      expect(registry.bindProfile("thread-bind", { accountRef: "codex-2", model: "gpt-5.5", reasoningEffort: "high" })?.profile)
        .toEqual({ accountRef: "codex-2", model: "gpt-5.5", reasoningEffort: "high" })

      const reopened = openFullAutoRegistry(registryFile).record("thread-bind")
      expect(reopened?.workspaceRef).toBe("/repo/granted")
      expect(reopened?.profile).toEqual({ accountRef: "codex-2", model: "gpt-5.5", reasoningEffort: "high" })

      expect(registry.bindWorkspace("thread-missing", "/repo/x")).toBe(null)
      expect(registry.bindProfile("thread-missing", { model: "gpt-5.5" })).toBe(null)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("set semantics with the wave-2 fields: enable binds workspace and clears failure state; disable records blockedReason, releases the lease, and preserves bindings", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-set-options-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      // Enable with a workspace binding (the FA-H2 handler path).
      registry.set("thread-s", true, { workspaceRef: "/repo/a" })
      expect(registry.record("thread-s")?.workspaceRef).toBe("/repo/a")

      registry.recordFailure("thread-s", "transient")
      registry.claimPending("thread-s", "turn.full-auto.claimed")

      // Disabling for a typed policy stop records the reason and releases
      // the lease, and keeps the workspace binding for diagnosis.
      registry.set("thread-s", false, { blockedReason: "workspace_mismatch", disabledBy: "workspace_guard" })
      const disabled = registry.record("thread-s")
      expect(disabled?.enabled).toBe(false)
      expect(disabled?.blockedReason).toBe("workspace_mismatch")
      expect(disabled?.disabledBy).toBe("workspace_guard")
      expect(disabled?.disabledAt).toBeDefined()
      expect(disabled?.pendingTurnRef ?? null).toBe(null)
      expect(disabled?.continuationCount).toBe(0)
      expect(disabled?.workspaceRef).toBe("/repo/a")

      // Re-enabling is a fresh grant: failure/blocked state clears, and the
      // enable-time options rebind the (possibly different) workspace.
      registry.set("thread-s", true, { workspaceRef: "/repo/b" })
      const reenabled = registry.record("thread-s")
      expect(reenabled?.blockedReason ?? null).toBe(null)
      expect(reenabled?.disabledBy).toBeUndefined()
      expect(reenabled?.disabledAt).toBeUndefined()
      expect(reenabled?.consecutiveFailures ?? 0).toBe(0)
      expect(reenabled?.lastFailureAt).toBeUndefined()
      expect(reenabled?.workspaceRef).toBe("/repo/b")

      // An owner toggle-off carries provenance but no blockedReason.
      registry.set("thread-s", false, { disabledBy: "ui_toggle" })
      expect(registry.record("thread-s")?.blockedReason ?? null).toBe(null)
      expect(registry.record("thread-s")?.disabledBy).toBe("ui_toggle")

      expect(() => registry.set("thread-s", false)).toThrow(
        "refusing to disable Full Auto without durable disable attribution",
      )
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("Full Auto multi-lane routing-policy fields (FA-RT-01 #8987)", () => {
  test("a rev-9/rev-10-era registry file WITHOUT routingPolicy/rotationHistory still decodes and behaves exactly as single-lane (legacy fixture)", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-rt-legacy-"))
    try {
      const registryDir = path.join(root, "full-auto")
      const registryFile = path.join(registryDir, "registry.json")
      mkdirSync(registryDir, { recursive: true })
      // The exact pre-#8987 on-disk shape: wave-2 fields present, no routing
      // fields at all.
      writeFileSync(registryFile, JSON.stringify({
        schema: FULL_AUTO_REGISTRY_SCHEMA,
        records: [{
          threadRef: "thread-legacy-single-lane",
          enabled: true,
          continuationCount: 3,
          updatedAt: "2026-07-16T00:00:00.000Z",
          workspaceRef: "/repo/granted",
          profile: { lane: "codex-local", accountRef: "codex-1", model: "gpt-5.5" },
        }],
      }), "utf8")

      const registry = openFullAutoRegistry(registryFile)
      expect(readdirSync(registryDir).filter(name => name.includes("quarantined"))).toEqual([])
      const decoded = registry.record("thread-legacy-single-lane")
      expect(decoded?.enabled).toBe(true)
      expect(decoded?.routingPolicy).toBeUndefined()
      expect(decoded?.rotationHistory).toBeUndefined()
      expect(projectFullAutoRotationHistory(decoded!)).toEqual([])
      // Mutating through every existing path keeps the fields absent -- the
      // upgrade never invents a policy for a legacy record.
      registry.incrementContinuation("thread-legacy-single-lane")
      registry.recordFailure("thread-legacy-single-lane", "transient")
      registry.recordSuccess("thread-legacy-single-lane")
      const touched = openFullAutoRegistry(registryFile).record("thread-legacy-single-lane")
      expect(touched?.routingPolicy).toBeUndefined()
      expect(touched?.rotationHistory).toBeUndefined()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("bindRoutingPolicy persists the ordered candidate list durably, survives enable/disable transitions, and null clears it", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-rt-bind-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      const registry = openFullAutoRegistry(registryFile)
      registry.set("thread-rt", true, { workspaceRef: "/repo/a" })
      const policy = [
        { lane: "codex-local", accountRef: "codex-1" },
        { lane: "fable-local" },
      ]
      expect(registry.bindRoutingPolicy("thread-rt", policy)?.routingPolicy).toEqual(policy)

      // Durable across a fresh open.
      expect(openFullAutoRegistry(registryFile).record("thread-rt")?.routingPolicy).toEqual(policy)

      // The policy survives a disable (diagnosis-preserving, like
      // workspaceRef) and a re-enable.
      registry.set("thread-rt", false, { disabledBy: "ui_toggle" })
      expect(registry.record("thread-rt")?.routingPolicy).toEqual(policy)
      registry.set("thread-rt", true, { workspaceRef: "/repo/a" })
      expect(registry.record("thread-rt")?.routingPolicy).toEqual(policy)

      // An enable-time option rebinds it atomically.
      const rebound = [{ lane: "fable-local" }]
      registry.set("thread-rt", true, { workspaceRef: "/repo/a", routingPolicy: rebound })
      expect(registry.record("thread-rt")?.routingPolicy).toEqual(rebound)

      // null clears back to legacy single-lane behavior.
      expect(registry.bindRoutingPolicy("thread-rt", null)?.routingPolicy).toBeUndefined()
      expect(openFullAutoRegistry(registryFile).record("thread-rt")?.routingPolicy).toBeUndefined()

      // Missing record is a null no-op; bounds fail closed.
      expect(registry.bindRoutingPolicy("thread-missing", policy)).toBe(null)
      expect(() => registry.bindRoutingPolicy("thread-rt", [])).toThrow()
      expect(() => registry.bindRoutingPolicy(
        "thread-rt",
        Array.from({ length: FULL_AUTO_ROUTING_POLICY_LIMIT + 1 }, (_, index) => ({ lane: `lane-${index}` })),
      )).toThrow()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test(`recordRotation appends typed rotation facts, caps the history at ${FULL_AUTO_ROTATION_HISTORY_LIMIT} with the OLDEST evicted, and survives a fresh open`, () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-rt-history-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      let tick = 0
      const registry = openFullAutoRegistry(
        registryFile,
        () => new Date(Date.UTC(2026, 6, 17, 0, 0, 0, tick++)),
      )
      registry.set("thread-rot", true)
      expect(registry.recordRotation("thread-rot", {
        fromLane: "codex-local",
        toLane: "fable-local",
        reason: "account_exhausted",
      })?.rotationHistory).toEqual([{
        fromLane: "codex-local",
        toLane: "fable-local",
        reason: "account_exhausted",
        at: expect.stringContaining("2026-07-17T"),
      }])

      for (let index = 0; index < FULL_AUTO_ROTATION_HISTORY_LIMIT + 4; index += 1) {
        registry.recordRotation("thread-rot", {
          fromLane: `lane-${index}`,
          toLane: `lane-${index + 1}`,
          reason: "provider_error",
        })
      }
      const history = registry.record("thread-rot")?.rotationHistory ?? []
      expect(history).toHaveLength(FULL_AUTO_ROTATION_HISTORY_LIMIT)
      // The first (account_exhausted) entry and the oldest provider_error
      // entries were evicted; the most recent entry is last.
      expect(history.every(entry => entry.reason === "provider_error")).toBe(true)
      expect(history.at(-1)?.fromLane).toBe(`lane-${FULL_AUTO_ROTATION_HISTORY_LIMIT + 3}`)

      // Durable, and the public-safe projection returns exactly the typed
      // fields (never anything extra).
      const reopened = openFullAutoRegistry(registryFile).record("thread-rot")!
      expect(reopened.rotationHistory).toEqual(history)
      const projected = projectFullAutoRotationHistory(reopened)
      expect(projected).toHaveLength(FULL_AUTO_ROTATION_HISTORY_LIMIT)
      for (const entry of projected) {
        expect(Object.keys(entry).sort()).toEqual(["at", "fromLane", "reason", "toLane"])
      }

      // Missing record is a null no-op.
      expect(registry.recordRotation("thread-missing", {
        fromLane: "a",
        toLane: "b",
        reason: "rate_limited",
      })).toBe(null)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("rotation history is preserved across disable/enable (evidence, like blockedReason) and recordFailure/recordSuccess leave it untouched", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-rt-preserve-"))
    try {
      const registry = openFullAutoRegistry(path.join(root, "full-auto", "registry.json"))
      registry.set("thread-keep", true)
      registry.recordRotation("thread-keep", { fromLane: "codex-local", toLane: "fable-local", reason: "rate_limited" })

      registry.recordFailure("thread-keep", "transient")
      registry.recordSuccess("thread-keep")
      registry.set("thread-keep", false, { disabledBy: "ui_toggle" })
      registry.set("thread-keep", true)

      expect(registry.record("thread-keep")?.rotationHistory).toEqual([{
        fromLane: "codex-local",
        toLane: "fable-local",
        reason: "rate_limited",
        at: expect.any(String),
      }])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("Full Auto registry robustness (FA-H10 #8883)", () => {
  test("a corrupt registry file is quarantined and the registry opens empty instead of throwing (fail closed for the feature, open for the app)", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-quarantine-"))
    try {
      const registryDir = path.join(root, "full-auto")
      const registryFile = path.join(registryDir, "registry.json")
      const seeded = openFullAutoRegistry(registryFile)
      seeded.set("thread-before-corruption", true)
      writeFileSync(registryFile, "{ this is not json", "utf8")

      // Must not throw -- a corrupt automation preference never blocks main
      // initialization.
      const registry = openFullAutoRegistry(registryFile)
      expect(registry.list()).toEqual([])
      expect(registry.get("thread-before-corruption")).toBe(false)
      expect(registry.enabledThreads()).toEqual([])

      const quarantined = readdirSync(registryDir).filter(name =>
        name.startsWith("registry.json.quarantined-"),
      )
      expect(quarantined).toHaveLength(1)
      expect(readFileSync(path.join(registryDir, quarantined[0]!), "utf8")).toBe("{ this is not json")

      // A subsequent set persists cleanly and survives a fresh open.
      registry.set("thread-after-quarantine", true)
      const reopened = openFullAutoRegistry(registryFile)
      expect(reopened.get("thread-after-quarantine")).toBe(true)
      expect(reopened.list()).toHaveLength(1)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a schema-invalid (but valid JSON) registry file is also quarantined rather than thrown", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-quarantine-schema-"))
    try {
      const registryDir = path.join(root, "full-auto")
      const registryFile = path.join(registryDir, "registry.json")
      const seeded = openFullAutoRegistry(registryFile)
      seeded.set("thread-seed", true)
      writeFileSync(registryFile, JSON.stringify({ schema: "wrong.schema.v9", records: "nope" }), "utf8")

      const registry = openFullAutoRegistry(registryFile)
      expect(registry.list()).toEqual([])
      expect(
        readdirSync(registryDir).filter(name => name.startsWith("registry.json.quarantined-")),
      ).toHaveLength(1)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("eviction never drops an enabled record: the oldest enabled thread survives while old disabled records are evicted", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-full-auto-eviction-"))
    try {
      const registryFile = path.join(root, "full-auto", "registry.json")
      // Deterministic strictly-increasing clock so "old-enabled" is the
      // oldest record by updatedAt from then on.
      let tick = 0
      const registry = openFullAutoRegistry(
        registryFile,
        () => new Date(Date.UTC(2026, 0, 1, 0, 0, 0, tick++)),
      )

      registry.set("old-enabled", true)
      const disabledTotal = FULL_AUTO_RECORD_LIMIT + 12
      for (let index = 0; index < disabledTotal; index += 1) {
        registry.set(`disabled-${index}`, false, { disabledBy: "ui_toggle" })
      }

      // The enabled record survives even though 140 records were touched more
      // recently; only the disabled tail is bounded.
      expect(registry.get("old-enabled")).toBe(true)
      expect(registry.enabledThreads()).toEqual(["old-enabled"])
      expect(registry.list()).toHaveLength(FULL_AUTO_RECORD_LIMIT)

      // The oldest disabled records were the ones evicted; the most recent
      // disabled records remain.
      expect(record(registry, "disabled-0")).toBeUndefined()
      expect(record(registry, `disabled-${disabledTotal - 1}`)).toBeDefined()

      // The durable file agrees after a fresh open: restart still resumes the
      // enabled thread.
      const reopened = openFullAutoRegistry(registryFile)
      expect(reopened.get("old-enabled")).toBe(true)
      expect(reopened.list()).toHaveLength(FULL_AUTO_RECORD_LIMIT)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
