import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"

import {
  decideFullAutoInitiative,
  isFullAutoCandidateHostVerifiable,
  makeFullAutoSelfClaim,
} from "./full-auto-initiative.ts"
import type { FullAutoCandidateWorkItem } from "./full-auto-objective-selection.ts"
import { rankFullAutoObjectiveCandidates } from "./full-auto-objective-selection.ts"
import { openFullAutoRunRegistry } from "./full-auto-run-registry.ts"

/** A realistic, schema-valid self-selected candidate produced by HANDS-1
 * owner-priority selection -- a desktop-surface item with a named host-runnable
 * verification and a real citation. */
const selectDesktopCandidate = (): FullAutoCandidateWorkItem => {
  const selection = rankFullAutoObjectiveCandidates({
    signals: [
      {
        title: "Close the Full Auto MemoHarness gap",
        readTarget: "docs/sol/MASTER_ROADMAP.md",
        deliverable: "Implement the MemoHarness seam and its bounded oracle.",
        verification: "pnpm exec vp test --cwd apps/openagents-desktop full-auto-initiative.test.ts",
        rationale: "Desktop is the top owner-priority surface; the gap is cited in the roadmap.",
        surface: "desktop",
        citedRefs: ["github:OpenAgentsInc/openagents#9184", "docs/sol/MASTER_ROADMAP.md"],
      },
    ],
    now: () => new Date("2026-07-22T00:00:00.000Z"),
  })
  const candidate = selection.candidates[0]
  expect(candidate).toBeDefined()
  return candidate!
}

const fixedNow = () => new Date("2026-07-22T12:00:00.000Z")

describe("Full Auto autonomy initiative (HANDS-6 #9184)", () => {
  test("BEFORE: the old 'require an open GitHub claim' rule stops on a valuable self-selected action; AFTER: autonomy initiative proceeds and self-claims", () => {
    const candidate = selectDesktopCandidate()

    // BEFORE (the weakness surfaced live): the run believed it needed an OPEN
    // GitHub claim before acting. With none open, that rule stops -- exactly the
    // passive default this issue fixes.
    const oldRequireOpenGithubClaimToProceed = (hasOpenGithubClaim: boolean): boolean => hasOpenGithubClaim
    expect(oldRequireOpenGithubClaimToProceed(false)).toBe(false)

    // AFTER: with autonomy enabled, no owner halt, a host-verifiable candidate,
    // and NO conflicting active claim, the run PROCEEDS despite there being no
    // open GitHub issue -- and records an honest self-claim.
    const decision = decideFullAutoInitiative({
      autonomyEnabled: true,
      ownerHalt: false,
      candidate,
      conflictingActiveClaim: false,
      runRef: "run.full-auto.abc",
      now: fixedNow,
      mintClaimRef: () => "claim.self.fixed",
    })
    expect(decision.action).toBe("proceed")
    if (decision.action !== "proceed") return
    expect(decision.selfClaim).toEqual({
      schema: "openagents.desktop.full_auto_self_claim.v1",
      claimRef: "claim.self.fixed",
      runRef: "run.full-auto.abc",
      scope: "Close the Full Auto MemoHarness gap",
      basis: "self_selected",
      verification: "pnpm exec vp test --cwd apps/openagents-desktop full-auto-initiative.test.ts",
      citedRefs: ["github:OpenAgentsInc/openagents#9184", "docs/sol/MASTER_ROADMAP.md"],
      ledger: "local",
      claimedAt: "2026-07-22T12:00:00.000Z",
    })
    // The self-claim is NOT a GitHub issue: its basis is the self-selected work
    // packet, and it is relay-ready (ledger local today, movable to relay #9185).
    expect(decision.selfClaim.basis).toBe("self_selected")
    expect(decision.selfClaim.ledger).toBe("local")
  })

  test("initiative is host-verification-gated: a candidate with no runnable verification holds", () => {
    const candidate = selectDesktopCandidate()
    // A malformed/non-verifiable candidate (its named verification is not a
    // runnable check) is NOT host-verifiable, so initiative holds -- initiative
    // and the HANDS-2 completion gate stay on one truth.
    const notVerifiable: FullAutoCandidateWorkItem = { ...candidate, verification: " " }
    expect(isFullAutoCandidateHostVerifiable(candidate)).toBe(true)
    expect(isFullAutoCandidateHostVerifiable(notVerifiable)).toBe(false)

    const decision = decideFullAutoInitiative({
      autonomyEnabled: true,
      ownerHalt: false,
      candidate: notVerifiable,
      conflictingActiveClaim: false,
      runRef: "run.full-auto.abc",
    })
    expect(decision).toMatchObject({ action: "hold", reason: "not_host_verifiable" })
  })

  test("owner Stop/override always holds initiative", () => {
    const candidate = selectDesktopCandidate()
    const decision = decideFullAutoInitiative({
      autonomyEnabled: true,
      ownerHalt: true,
      candidate,
      conflictingActiveClaim: false,
      runRef: "run.full-auto.abc",
    })
    expect(decision).toMatchObject({ action: "hold", reason: "owner_halt" })
  })

  test("a conflicting active claim holds initiative (yield, do not collide)", () => {
    const candidate = selectDesktopCandidate()
    const decision = decideFullAutoInitiative({
      autonomyEnabled: true,
      ownerHalt: false,
      candidate,
      conflictingActiveClaim: true,
      runRef: "run.full-auto.abc",
    })
    expect(decision).toMatchObject({ action: "hold", reason: "conflicting_active_claim" })
  })

  test("no self-selected candidate holds initiative", () => {
    const decision = decideFullAutoInitiative({
      autonomyEnabled: true,
      ownerHalt: false,
      candidate: null,
      conflictingActiveClaim: false,
      runRef: "run.full-auto.abc",
    })
    expect(decision).toMatchObject({ action: "hold", reason: "no_candidate" })
  })

  test("a non-autonomy run never takes initiative: default Full Auto behavior is unchanged", () => {
    const candidate = selectDesktopCandidate()
    const decision = decideFullAutoInitiative({
      autonomyEnabled: false,
      ownerHalt: false,
      candidate,
      conflictingActiveClaim: false,
      runRef: "run.full-auto.abc",
    })
    expect(decision).toMatchObject({ action: "hold", reason: "autonomy_disabled" })
  })

  test("makeFullAutoSelfClaim mints a stable local, relay-movable claim from a candidate", () => {
    const candidate = selectDesktopCandidate()
    const relay = makeFullAutoSelfClaim(candidate, {
      runRef: "run.full-auto.xyz",
      now: fixedNow,
      mintClaimRef: () => "claim.self.relay",
      ledger: "relay",
    })
    expect(relay.ledger).toBe("relay")
    expect(relay.runRef).toBe("run.full-auto.xyz")
    expect(relay.claimRef).toBe("claim.self.relay")
  })

  test("the self-claim persists on an autonomy run and is a null no-op on a non-autonomy run", () => {
    const root = mkdtempSync(path.join(tmpdir(), "full-auto-initiative-"))
    try {
      const registry = openFullAutoRunRegistry(path.join(root, "runs.json"))
      const candidate = selectDesktopCandidate()
      const claim = makeFullAutoSelfClaim(candidate, { runRef: "placeholder", now: fixedNow })

      // Non-autonomy run: recordSelfClaim is a null no-op (a self-claim is
      // meaningless without the autonomy gate).
      const plain = registry.startNew({
        title: "Plain run",
        objective: "Do the plain thing.",
        doneCondition: "It is done.",
        objectiveSource: "user",
        actor: "owner_ui",
        reason: "test",
      })
      expect(plain.ok).toBe(true)
      if (!plain.ok) return
      expect(registry.recordSelfClaim(plain.run.runRef, { ...claim, runRef: plain.run.runRef })).toBeNull()

      // Autonomy run: the self-claim is durably recorded on the autonomy block.
      const auto = registry.startNew({
        title: "Autonomy run",
        objective: "Take initiative.",
        doneCondition: "verify: echo ok",
        objectiveSource: "system_selected",
        actor: "control_api",
        reason: "test",
      })
      expect(auto.ok).toBe(true)
      if (!auto.ok) return
      registry.setAutonomy(auto.run.runRef, { enabled: true })
      const recorded = registry.recordSelfClaim(auto.run.runRef, { ...claim, runRef: auto.run.runRef })
      expect(recorded?.autonomy?.selfClaim?.basis).toBe("self_selected")
      expect(recorded?.autonomy?.selfClaim?.runRef).toBe(auto.run.runRef)
      // Reloading from disk proves the durable write round-trips.
      const reopened = openFullAutoRunRegistry(path.join(root, "runs.json"))
      expect(reopened.get(auto.run.runRef)?.autonomy?.selfClaim?.claimRef).toBe(claim.claimRef)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
