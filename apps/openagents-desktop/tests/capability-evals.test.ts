/**
 * EP250 capability-eval suite (#8712).
 *
 * Iterates the typed capability registry (`src/capability-registry.ts`, which
 * encodes the audit's §4 taxonomy) and:
 *   1. Meta-tests the registry itself: every capability present, the
 *      table-derived status distribution locked (drift = red), oracle/blocker
 *      invariants enforced, and every wired oracle reference points at a file
 *      that exists on disk (ref-rot guard).
 *   2. Drives the REAL headless programmatic oracles for the capabilities the
 *      audit marks ui_available / partial-with-a-real-path — the app's typed
 *      surfaces exercised in-process (no Electron window): local-lane event
 *      streams (A1/A4), the interrupt seam (A2), the workspace save/git-diff
 *      seams (C3/E1), and the usage ledger (K2).
 *   3. Documents every missing/blocked capability with a skipped-with-reason
 *      test that names its blocker, so the suite fails loudly if someone
 *      claims one done without wiring it.
 *
 * Behavior-contract coverage note: this file is the programmatic oracle for
 * `openagents_desktop.chat.composer_stop_button.v1` (the interrupt seam test
 * below references that contract id).
 */
import { describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { execFileSync } from "node:child_process"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  AUDIT_PROSE_SUMMARY,
  CAPABILITY_TABLE_DISTRIBUTION,
  capabilityRegistry,
  capabilityStatusCounts,
  isWired,
  type CapabilityRow,
} from "../src/capability-registry.ts"
import {
  makeLocalHarnessChatHost,
  type FableLocalRendererBridge,
} from "../src/renderer/local-harness.ts"
import type { DesktopThread } from "../src/chat-contract.ts"
import type {
  FableLocalEvent,
  FableLocalEventEnvelope,
} from "../src/fable-local-contract.ts"
import {
  decodeFableLocalInterruptRequest,
  fableLocalFailureMessage,
} from "../src/fable-local-contract.ts"
import {
  readWorkspaceFile,
  saveWorkspaceFile,
  workspaceGitDiff,
  workspaceGitStatus,
} from "../src/workspace-service.ts"
import { makeUsageLedger } from "../src/usage-ledger.ts"

// The audit's §4 taxonomy row ids, in order. The meta-test asserts the
// registry matches this set exactly (no drift in either direction).
const AUDIT_CAPABILITY_IDS = [
  "A1", "A2", "A3", "A4",
  "B1", "B2", "B3",
  "C1", "C2", "C3",
  "D1", "D2", "D3",
  "E1", "E2", "E3", "E4", "E5",
  "F1", "F2",
  "G1", "G2", "G3", "G4", "G5",
  "H1", "H2", "H3", "H4", "H5",
  "I1", "I2", "I3", "I4",
  "J1", "J2", "J3", "J4",
  "K1", "K2",
] as const

const repoRoot = path.resolve(import.meta.dir, "..", "..", "..")
const resolveRef = (ref: string): string => path.join(repoRoot, ref)

// ---------------------------------------------------------------------------
// 1. Registry meta-tests
// ---------------------------------------------------------------------------

describe("capability registry (EP250 audit §4 taxonomy)", () => {
  test("every audit capability has exactly one registry row and no extras", () => {
    const ids = capabilityRegistry.map((row) => row.id)
    expect(new Set(ids).size).toBe(ids.length) // unique
    expect([...ids].sort()).toEqual([...AUDIT_CAPABILITY_IDS].sort())
  })

  test("the status distribution matches the audit §4 tables exactly (drift = red)", () => {
    const counts = capabilityStatusCounts()
    expect(counts.ui_available).toBe(CAPABILITY_TABLE_DISTRIBUTION.ui_available)
    expect(counts.programmatic_only).toBe(CAPABILITY_TABLE_DISTRIBUTION.programmatic_only)
    expect(counts.partial).toBe(CAPABILITY_TABLE_DISTRIBUTION.partial)
    expect(counts.missing).toBe(CAPABILITY_TABLE_DISTRIBUTION.missing)
    expect(capabilityRegistry.length).toBe(CAPABILITY_TABLE_DISTRIBUTION.total)
  })

  test("the audit's prose 'Totals' line is a KNOWN inconsistency, superseded by the tables", () => {
    // The audit summary prose says 33 / 13-4-10-6; its own §4 tables sum to
    // 40 / 15-4-13-8. We follow the tables (the per-capability authority) and
    // record the prose figure here so the discrepancy is explicit, not hidden.
    expect(AUDIT_PROSE_SUMMARY.total).not.toBe(CAPABILITY_TABLE_DISTRIBUTION.total)
    expect(AUDIT_PROSE_SUMMARY.ui_available).not.toBe(CAPABILITY_TABLE_DISTRIBUTION.ui_available)
    // programmatic_only happens to agree (4); the rest diverge.
    expect(AUDIT_PROSE_SUMMARY.programmatic_only).toBe(CAPABILITY_TABLE_DISTRIBUTION.programmatic_only)
  })

  test("ui_available capabilities name BOTH oracles and neither is pending", () => {
    for (const row of capabilityRegistry.filter((value) => value.status === "ui_available")) {
      expect(row.uiOracleRef, `${row.id} ui oracle ref`).not.toBe("")
      expect(row.programmaticOracleRef, `${row.id} programmatic oracle ref`).not.toBe("")
      expect(isWired(row.uiOracleWiring), `${row.id} ui oracle wired`).toBe(true)
      expect(isWired(row.programmaticOracleWiring), `${row.id} programmatic oracle wired`).toBe(true)
    }
  })

  test("programmatic_only capabilities name a wired programmatic oracle", () => {
    for (const row of capabilityRegistry.filter((value) => value.status === "programmatic_only")) {
      expect(row.programmaticOracleRef, `${row.id} programmatic oracle ref`).not.toBe("")
      expect(isWired(row.programmaticOracleWiring), `${row.id} programmatic oracle wired`).toBe(true)
    }
  })

  test("missing capabilities carry a blocker and no wired oracle (documented gap)", () => {
    for (const row of capabilityRegistry.filter((value) => value.status === "missing")) {
      expect(row.blocker, `${row.id} blocker`).toBeTruthy()
      expect(isWired(row.uiOracleWiring), `${row.id} ui oracle`).toBe(false)
      expect(isWired(row.programmaticOracleWiring), `${row.id} programmatic oracle`).toBe(false)
    }
  })

  test("partial capabilities either wire a real oracle or carry a blocker", () => {
    for (const row of capabilityRegistry.filter((value) => value.status === "partial")) {
      const anyWired = isWired(row.uiOracleWiring) || isWired(row.programmaticOracleWiring)
      expect(anyWired || Boolean(row.blocker), `${row.id} has a path or a blocker`).toBe(true)
    }
  })

  test("every pending oracle carries a blocker (no silent gaps)", () => {
    for (const row of capabilityRegistry) {
      const hasPending = row.uiOracleWiring === "pending" || row.programmaticOracleWiring === "pending"
      if (hasPending && row.status !== "ui_available") {
        expect(row.blocker, `${row.id} pending oracle needs a blocker`).toBeTruthy()
      }
    }
  })

  test("every WIRED oracle reference points at a file that exists (ref-rot guard)", () => {
    const missing: string[] = []
    for (const row of capabilityRegistry) {
      if (isWired(row.uiOracleWiring) && !existsSync(resolveRef(row.uiOracleRef))) {
        missing.push(`${row.id} ui -> ${row.uiOracleRef}`)
      }
      if (isWired(row.programmaticOracleWiring) && !existsSync(resolveRef(row.programmaticOracleRef))) {
        missing.push(`${row.id} programmatic -> ${row.programmaticOracleRef}`)
      }
    }
    expect(missing).toEqual([])
  })
})

// ---------------------------------------------------------------------------
// Local-lane test harness (drives makeLocalHarnessChatHost headlessly)
// ---------------------------------------------------------------------------

const evalThread: DesktopThread = {
  id: "thread.capability-eval",
  title: "Capability eval",
  updatedAt: "2026-07-11T18:00:00.000Z",
  notes: [{ key: "u1", role: "user", text: "hi", timestamp: "18:00" }],
}

/** A scripted local-lane bridge: `start` replays events then resolves. */
const makeScriptedFableBridge = (
  events: (turnRef: string) => ReadonlyArray<FableLocalEvent>,
  result: Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }>,
): FableLocalRendererBridge => {
  let listener: ((envelope: FableLocalEventEnvelope) => void) | null = null
  return {
    availability: async () => ({ state: "available", accountRef: "acct.capability-eval" }),
    start: async (value: unknown) => {
      const turnRef = (value as { turnRef: string }).turnRef
      for (const event of events(turnRef)) listener?.({ turnRef, event })
      return result
    },
    interrupt: async () => true,
    onEvent: (l) => { listener = l; return () => { listener = null } },
  }
}

const buildFableHost = (bridge: FableLocalRendererBridge) =>
  makeLocalHarnessChatHost({
    base: {
      listThreads: async () => [],
      newThread: async () => null,
      openThread: async () => null,
      sendMessage: async () => ({ ok: false, error: "base host not exercised" }),
    },
    fable: bridge,
    fableAvailability: () => ({ state: "available", accountRef: "acct.capability-eval" }),
  })

// ---------------------------------------------------------------------------
// 2. Real headless programmatic oracles
// ---------------------------------------------------------------------------

describe("A1 multi-turn streaming chat + A4 effective model (programmatic oracle)", () => {
  test("turn_started -> text_delta+ assemble the assistant reply; model_effective renders a caption", async () => {
    const finalThread: DesktopThread = {
      ...evalThread,
      notes: [...evalThread.notes, { key: "a1", role: "assistant", text: "Hello world", timestamp: "18:00" }],
    }
    const bridge = makeScriptedFableBridge(
      () => [
        { kind: "turn_started", thread: evalThread },
        { kind: "model_effective", model: "claude-fable-5" },
        { kind: "text_delta", text: "Hello " },
        { kind: "text_delta", text: "world" },
      ],
      { ok: true, thread: finalThread },
    )
    const updates: DesktopThread[] = []
    const result = await buildFableHost(bridge).sendMessage({
      id: evalThread.id,
      message: "hi",
      harness: "fable",
      onUpdate: (thread) => updates.push(thread),
    })

    expect(result.ok).toBe(true)
    expect(updates.length).toBeGreaterThan(0)
    const last = updates[updates.length - 1]!
    // A1: streamed deltas assembled into the assistant bubble.
    expect(last.notes.some((note) => note.role === "assistant" && note.text === "Hello world")).toBe(true)
    // A4: the SDK-reported effective model surfaces as a caption ("Fable · <model>").
    expect(last.notes.some((note) => note.role === "system" && note.text === "Fable · claude-fable-5")).toBe(true)
  })

  test("A4: a substituted model is a typed failure, never streamed as Fable", () => {
    expect(fableLocalFailureMessage("model_substituted", "requested claude-fable-5, got other")).toContain(
      "refused a substituted model",
    )
  })
})

describe("A2 mid-turn interrupt (programmatic oracle) — openagents_desktop.chat.composer_stop_button.v1", () => {
  test("interruptActive signals the active turn's exact turnRef on the frozen interrupt channel", async () => {
    let startedTurnRef: string | null = null
    let interruptedTurnRef: string | null = null
    let resolveStart: (() => void) | null = null
    let subscribed = false
    const bridge: FableLocalRendererBridge = {
      availability: async () => ({ state: "available", accountRef: "acct.capability-eval" }),
      start: (value: unknown) => new Promise((resolve) => {
        startedTurnRef = (value as { turnRef: string }).turnRef
        // Interrupt resolves the pending turn with the typed interrupted failure.
        resolveStart = () => resolve({ ok: false, error: fableLocalFailureMessage("interrupted", "") })
      }),
      interrupt: async (value: unknown) => {
        interruptedTurnRef = (value as { turnRef: string }).turnRef
        resolveStart?.()
        return true
      },
      onEvent: () => { subscribed = true; return () => {} },
    }
    const host = buildFableHost(bridge)

    const pending = host.sendMessage({ id: evalThread.id, message: "hi", harness: "fable" })
    // Let sendMessage reach the awaiting bridge.start (activeTurn is now set).
    await new Promise((resolve) => setTimeout(resolve, 0))
    const interrupted = await host.interruptActive?.()
    const result = await pending

    expect(interrupted).toBe(true)
    expect(startedTurnRef).not.toBeNull()
    expect(interruptedTurnRef).toBe(startedTurnRef) // exact-ref interrupt
    expect(result.ok).toBe(false)
    expect(result.error).toBe("The local Fable turn was interrupted.")
    expect(subscribed).toBe(true) // the lane subscribed to events before starting
  })

  test("interruptActive is a no-op (false) when no turn is active", async () => {
    const bridge = makeScriptedFableBridge(() => [], { ok: true, thread: evalThread })
    const host = buildFableHost(bridge)
    expect(await host.interruptActive?.()).toBe(false)
  })

  test("the frozen interrupt request shape decodes exactly { turnRef }", () => {
    expect(decodeFableLocalInterruptRequest({ turnRef: "turn.fable.abc" })).toEqual({ turnRef: "turn.fable.abc" })
    expect(decodeFableLocalInterruptRequest({})).toBeNull()
  })
})

describe("C3 human file edit + save (programmatic oracle)", () => {
  const roots: string[] = []
  const makeRoot = (): string => {
    const root = mkdtempSync(path.join(tmpdir(), "capability-eval-save-"))
    roots.push(root)
    return root
  }

  test("stale expectedRevision -> typed conflict; fresh revision -> saved and reread", () => {
    const root = makeRoot()
    const file = path.join(root, "notes.md")
    writeFileSync(file, "before\n")
    const initial = readWorkspaceFile(root, file)
    expect(initial).not.toBeNull()
    if (initial === null) throw new Error("expected workspace file")

    // Stale revision (the SHA-256 expectedRevision guard) refuses the write.
    const stale = saveWorkspaceFile(root, { path: file, content: "clobber\n", expectedRevision: "sha256:stale" })
    expect(stale.state).toBe("conflict")

    // Fresh revision saves atomically and reads back the new content.
    const saved = saveWorkspaceFile(root, { path: file, content: "after\n", expectedRevision: initial.revision })
    expect(saved.state).toBe("saved")
    expect(readWorkspaceFile(root, file)?.content).toBe("after\n")

    while (roots.length > 0) rmSync(roots.pop()!, { recursive: true, force: true })
  })
})

describe("E1 repo inspection: status/diff (programmatic oracle)", () => {
  test("workspaceGitStatus reports the dirty file and workspaceGitDiff projects a bounded hunk", () => {
    const root = mkdtempSync(path.join(tmpdir(), "capability-eval-git-"))
    try {
      execFileSync("git", ["init", "--quiet"], { cwd: root })
      execFileSync("git", ["config", "user.email", "fixture@example.test"], { cwd: root })
      execFileSync("git", ["config", "user.name", "Fixture"], { cwd: root })
      const file = path.join(root, "README.md")
      writeFileSync(file, "before\n")
      execFileSync("git", ["add", "README.md"], { cwd: root })
      execFileSync("git", ["commit", "--quiet", "-m", "initial"], { cwd: root })
      writeFileSync(file, "after\n")

      const status = workspaceGitStatus(root)
      expect(status.state).toBe("available")
      if (status.state !== "available") throw new Error("expected git status")
      expect(status.changes.some((change) => change.path === "README.md" && change.kind === "modified")).toBe(true)

      const diff = workspaceGitDiff(root, file)
      expect(diff.state).toBe("available")
      if (diff.state !== "available") throw new Error("expected git diff")
      expect(diff.content).toContain("-before")
      expect(diff.content).toContain("+after")
      expect(diff.content).not.toContain(root) // path-redacted
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("K2 usage / token observability (programmatic oracle)", () => {
  test("the usage ledger accumulates exact per-account token totals across turns and children", () => {
    const ledger = makeUsageLedger(() => new Date("2026-07-11T18:00:00.000Z"))
    ledger.record({
      provider: "claude_agent",
      accountRef: "claude-pylon-1",
      requestedModel: "claude-fable-5",
      kind: "turn",
      usage: { inputTokens: 40, cachedInputTokens: 0, outputTokens: 9, reasoningTokens: 0, totalTokens: 49 },
    })
    ledger.record({
      provider: "codex",
      accountRef: "codex-2",
      requestedModel: "gpt-5.6-sol",
      kind: "child",
      usage: { inputTokens: 1000, cachedInputTokens: 0, outputTokens: 400, reasoningTokens: 40, totalTokens: 1440 },
    })
    const snapshot = ledger.snapshot()
    const claude = snapshot.rows.find((row) => row.accountRef === "claude-pylon-1")
    const codex = snapshot.rows.find((row) => row.accountRef === "codex-2")
    expect(claude?.totalTokens).toBe(49)
    expect(claude?.turns).toBe(1)
    expect(codex?.totalTokens).toBe(1440)
    expect(codex?.children).toBe(1)
    expect(codex?.requestedModel).toBe("gpt-5.6-sol")
    ledger.dispose()
  })
})

// ---------------------------------------------------------------------------
// 3. Documented gaps: every pending/blocked capability fails loudly if claimed
// ---------------------------------------------------------------------------

describe("capability gaps (skipped-with-reason: blocked until wired)", () => {
  const blockedRows: ReadonlyArray<CapabilityRow> = capabilityRegistry.filter(
    (row) =>
      row.status === "missing" ||
      (row.uiOracleWiring === "pending" && row.programmaticOracleWiring === "pending"),
  )

  test("the blocked set matches the audit's ranked gaps (documented, not silently absent)", () => {
    // These are the capabilities with NO wired oracle on either side today.
    expect(blockedRows.map((row) => row.id).sort()).toEqual(
      ["A3", "D3", "G4", "H2", "H5", "I1", "I2", "I3", "I4", "J2", "J4"].sort(),
    )
  })

  for (const row of blockedRows) {
    test.skip(`${row.id} ${row.capability} — BLOCKED: ${row.blocker ?? "no path"}`, () => {
      // Intentionally skipped: this capability has no wired oracle. The row's
      // blocker documents why; when the capability lands, replace this skip
      // with a real oracle and flip the registry wiring off "pending".
      expect(row.blocker).toBeTruthy()
    })
  }
})
