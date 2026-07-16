import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { FULL_AUTO_RECORD_LIMIT, openFullAutoRegistry } from "../src/full-auto-registry.ts"

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
      registry.set("thread-cap-semantics", false)
      registry.set("thread-cap-semantics", true)
      expect(record(registry, "thread-cap-semantics")?.continuationCount).toBe(0)
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
        registry.set(`disabled-${index}`, false)
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
