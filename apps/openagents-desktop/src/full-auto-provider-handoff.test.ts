import { describe, expect, test } from "vite-plus/test"
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Schema } from "effect"

import type { DesktopThread } from "./chat-contract.ts"
import {
  PROVIDER_HANDOFF_ENVELOPE_SCHEMA,
  PROVIDER_HANDOFF_TRANSITION_LIMIT,
  ProviderHandoffEnvelopeSchema,
  ProviderHandoffTransitionRecordSchema,
  buildProviderHandoffEnvelope,
  openProviderHandoffRegistry,
  providerHandoffDispositionForEnvelope,
} from "./full-auto-provider-handoff.ts"
import type { FullAutoRun } from "./full-auto-run-registry.ts"

const makeThread = (notes: ReadonlyArray<Readonly<{ role: "user" | "assistant" | "system"; text: string }>>): DesktopThread => ({
  id: "thread.handoff",
  title: "Handoff thread",
  updatedAt: "2026-07-17T00:00:00.000Z",
  notes: notes.map((note, index) => ({
    key: `note-${index}`,
    role: note.role,
    text: note.text,
    timestamp: `2026-07-17T00:00:0${index}.000Z`,
  })),
})

const makeRun = (overrides?: Partial<FullAutoRun>): FullAutoRun => ({
  runRef: "run.full-auto.handoff-1",
  threadRef: "thread.handoff",
  title: "Fix the flaky test",
  objective: "Make tests/flaky.test.ts stop flaking.",
  objectiveSource: "user",
  doneCondition: "The test passes 20 consecutive local runs.",
  objectiveHistory: [],
  turnCap: 20,
  successfulAttempts: 0,
  failedAttempts: 0,
  state: "paused",
  stateRevision: 3,
  createdAt: "2026-07-17T00:00:00.000Z",
  transitions: [],
  ...overrides,
})

describe("buildProviderHandoffEnvelope (FA-HO-01 #8975)", () => {
  test("assembles a host-owned envelope with the run's objective/doneCondition as a dedicated priority channel", () => {
    const thread = makeThread([
      { role: "user", text: "Please fix the flaky test." },
      { role: "assistant", text: "On it." },
    ])
    const envelope = buildProviderHandoffEnvelope({
      run: makeRun(),
      sourceLaneRef: "codex-local",
      targetLaneRef: "claude-local",
      thread,
      reason: "Owner requested a switch.",
      actor: "control_api",
      at: "2026-07-17T01:00:00.000Z",
    })
    const decoded = Schema.decodeUnknownSync(ProviderHandoffEnvelopeSchema)(envelope)
    expect(decoded.schema).toBe(PROVIDER_HANDOFF_ENVELOPE_SCHEMA)
    expect(decoded.runRef).toBe("run.full-auto.handoff-1")
    expect(decoded.threadRef).toBe("thread.handoff")
    expect(decoded.sourceLaneRef).toBe("codex-local")
    expect(decoded.targetLaneRef).toBe("claude-local")
    expect(decoded.objective).toBe("Make tests/flaky.test.ts stop flaking.")
    expect(decoded.doneCondition).toBe("The test passes 20 consecutive local runs.")
    expect(decoded.runStateRevision).toBe(3)
    expect(decoded.recentContext).toEqual([
      { role: "user", text: "Please fix the flaky test." },
      { role: "assistant", text: "On it." },
    ])
    expect(decoded.contextTruncated).toBe(false)
  })

  test("separates bounded tool/system outcome summary from authored user/assistant context", () => {
    const thread = makeThread([
      { role: "user", text: "Run the tests." },
      { role: "system", text: "Tool ran: pnpm test -- ok" },
      { role: "assistant", text: "Tests passed." },
    ])
    const envelope = buildProviderHandoffEnvelope({
      run: makeRun(),
      sourceLaneRef: "codex-local",
      targetLaneRef: "claude-local",
      thread,
      reason: "Owner requested a switch.",
      actor: "control_api",
      at: "2026-07-17T01:00:00.000Z",
    })
    expect(envelope.recentContext.every(message => message.role === "user" || message.role === "assistant")).toBe(true)
    expect(envelope.toolNoteSummary).toEqual(["Tool ran: pnpm test -- ok"])
  })

  test("always carries an explicit provider_private_never_transferred omission -- never implies session transfer", () => {
    const envelope = buildProviderHandoffEnvelope({
      run: makeRun(),
      sourceLaneRef: "codex-local",
      targetLaneRef: "claude-local",
      thread: null,
      reason: "Owner requested a switch.",
      actor: "control_api",
      at: "2026-07-17T01:00:00.000Z",
    })
    expect(envelope.omissions.some(omission => omission.reason === "provider_private_never_transferred")).toBe(true)
  })

  test("no run bound: objective/doneCondition are absent (never invented) and an explicit no_run_bound omission is recorded", () => {
    const thread = makeThread([{ role: "user", text: "Hi" }])
    const envelope = buildProviderHandoffEnvelope({
      run: null,
      sourceLaneRef: "codex-local",
      targetLaneRef: "claude-local",
      thread,
      reason: "Manual provider switch requested from the composer.",
      actor: "owner_ui",
      at: "2026-07-17T01:00:00.000Z",
    })
    expect(envelope.runRef).toBeUndefined()
    expect(envelope.objective).toBeUndefined()
    expect(envelope.doneCondition).toBeUndefined()
    expect(envelope.omissions.some(omission => omission.reason === "no_run_bound")).toBe(true)
  })

  test("truncation: exceeding the shared 32-message bound sets contextTruncated and records a bounded_truncation omission -- objective survives regardless", () => {
    const notes: Array<Readonly<{ role: "user" | "assistant"; text: string }>> = Array.from(
      { length: 40 },
      (_, index) => ({
        role: index % 2 === 0 ? "user" : "assistant",
        text: `message ${index}`,
      }),
    )
    const thread = makeThread(notes)
    const envelope = buildProviderHandoffEnvelope({
      run: makeRun(),
      sourceLaneRef: "codex-local",
      targetLaneRef: "claude-local",
      thread,
      reason: "Owner requested a switch.",
      actor: "control_api",
      at: "2026-07-17T01:00:00.000Z",
    })
    expect(envelope.contextTruncated).toBe(true)
    expect(envelope.contextSourceMessageCount).toBe(40)
    expect(envelope.contextIncludedMessageCount).toBeLessThan(40)
    expect(envelope.omissions.some(omission => omission.reason === "bounded_truncation")).toBe(true)
    // FA-AC-49: objective/done-condition are never crowded out by transcript truncation.
    expect(envelope.objective).toBe("Make tests/flaky.test.ts stop flaking.")
    expect(envelope.doneCondition).toBe("The test passes 20 consecutive local runs.")
    expect(providerHandoffDispositionForEnvelope(envelope)).toBe("truncated_with_confirmation")
  })

  test("no truncation -> disposition is complete_within_bounds", () => {
    const thread = makeThread([{ role: "user", text: "Hi" }])
    const envelope = buildProviderHandoffEnvelope({
      run: makeRun(),
      sourceLaneRef: "codex-local",
      targetLaneRef: "claude-local",
      thread,
      reason: "Owner requested a switch.",
      actor: "control_api",
      at: "2026-07-17T01:00:00.000Z",
    })
    expect(providerHandoffDispositionForEnvelope(envelope)).toBe("complete_within_bounds")
  })

  test("Unicode and malformed/blank notes do not crash the builder and blank notes are excluded", () => {
    const thread = makeThread([
      { role: "user", text: "こんにちは 🚀 — multi-byte and emoji content" },
      { role: "assistant", text: "   " },
      { role: "assistant", text: "" },
      { role: "user", text: "Second real message" },
    ])
    const envelope = buildProviderHandoffEnvelope({
      run: makeRun(),
      sourceLaneRef: "codex-local",
      targetLaneRef: "claude-local",
      thread,
      reason: "Owner requested a switch.",
      actor: "control_api",
      at: "2026-07-17T01:00:00.000Z",
    })
    expect(envelope.recentContext.map(message => message.text)).toEqual([
      "こんにちは 🚀 — multi-byte and emoji content",
      "Second real message",
    ])
  })

  test("artifactRefs and pendingQuestions are honestly empty with a not_modeled_yet omission -- never fabricated", () => {
    const envelope = buildProviderHandoffEnvelope({
      run: makeRun(),
      sourceLaneRef: "codex-local",
      targetLaneRef: "claude-local",
      thread: null,
      reason: "Owner requested a switch.",
      actor: "control_api",
      at: "2026-07-17T01:00:00.000Z",
    })
    expect(envelope.artifactRefs).toEqual([])
    expect(envelope.pendingQuestions).toEqual([])
    expect(envelope.omissions.filter(omission => omission.reason === "not_modeled_yet").length).toBeGreaterThanOrEqual(2)
  })
})

describe("openProviderHandoffRegistry (durable receipt store, FA-HO-01 #8975)", () => {
  test("records a transition, persists it mode 0600, and survives a reopen (restart durability)", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-handoff-"))
    try {
      const file = path.join(root, "full-auto", "provider-handoffs.json")
      const registryA = openProviderHandoffRegistry(file, () => new Date("2026-07-17T02:00:00.000Z"))
      const recorded = registryA.record({
        runRef: "run.full-auto.handoff-1",
        threadRef: "thread.handoff",
        from: "codex-local",
        to: "claude-local",
        actor: "control_api",
        at: "2026-07-17T02:00:00.000Z",
        reason: "Owner requested a switch.",
        disposition: "complete_within_bounds",
        truncated: false,
      })
      const decoded = Schema.decodeUnknownSync(ProviderHandoffTransitionRecordSchema)(recorded)
      expect(decoded.handoffRef.length).toBeGreaterThan(0)
      expect(decoded.from).toBe("codex-local")
      expect(decoded.to).toBe("claude-local")
      if (process.platform !== "win32") {
        expect(statSync(file).mode & 0o777).toBe(0o600)
      }
      expect(JSON.parse(readFileSync(file, "utf8")).transitions).toHaveLength(1)

      // Restart: a fresh registry instance over the same file sees the record.
      const registryB = openProviderHandoffRegistry(file)
      expect(registryB.list()).toEqual([recorded])
      expect(registryB.list({ runRef: "run.full-auto.handoff-1" })).toEqual([recorded])
      expect(registryB.list({ runRef: "run.full-auto.unrelated" })).toEqual([])
      expect(registryB.list({ threadRef: "thread.handoff" })).toEqual([recorded])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("bounds the durable store to PROVIDER_HANDOFF_TRANSITION_LIMIT, dropping the oldest first", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-handoff-bound-"))
    try {
      const file = path.join(root, "full-auto", "provider-handoffs.json")
      const registry = openProviderHandoffRegistry(file)
      for (let index = 0; index < PROVIDER_HANDOFF_TRANSITION_LIMIT + 5; index++) {
        registry.record({
          runRef: "run.full-auto.bound",
          threadRef: "thread.bound",
          from: "codex-local",
          to: "claude-local",
          actor: "control_api",
          at: `2026-07-17T02:00:${String(index % 60).padStart(2, "0")}.000Z`,
          reason: `switch ${index}`,
          disposition: "complete_within_bounds",
          truncated: false,
        })
      }
      const all = registry.list()
      expect(all).toHaveLength(PROVIDER_HANDOFF_TRANSITION_LIMIT)
      expect(all[0]!.reason).toBe("switch 5")
      expect(all.at(-1)!.reason).toBe(`switch ${PROVIDER_HANDOFF_TRANSITION_LIMIT + 4}`)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("a corrupt registry file is quarantined and the registry starts empty rather than crash-looping", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-provider-handoff-corrupt-"))
    try {
      const file = path.join(root, "full-auto", "provider-handoffs.json")
      mkdirSync(path.dirname(file), { recursive: true })
      writeFileSync(file, "not valid json{{{")
      const registry = openProviderHandoffRegistry(file)
      expect(registry.list()).toEqual([])
      const quarantined = readdirSync(path.dirname(file)).filter(name => name.includes("quarantined"))
      expect(quarantined.length).toBeGreaterThan(0)
      expect(existsSync(file)).toBe(false)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
