import { describe, expect, test } from "bun:test"

import {
  KhalaMobileStraightLineE2eSchemaId,
  STRAIGHT_LINE_LEGS,
  STRAIGHT_LINE_SEED_ACCOUNT,
  STRAIGHT_LINE_SEED_REPO,
  blockedStraightLineLegIds,
  runnableStraightLineLegIds,
} from "../src/qa/straight-line-e2e"

const mobileRoot = new URL("../", import.meta.url)
const read = (path: string) => Bun.file(new URL(path, mobileRoot)).text()

// Oracle for khala_mobile.platform.straight_line_repo_pick_writeback.v1 —
// the typed-skip discipline: blocked legs stay blocked WITH named blockers,
// runnable legs each have a real committed Maestro flow, and the runner
// script knows every leg id (so it can never silently drop one).
describe("Khala Mobile P0.8 straight-line E2E leg registry", () => {
  test("straight_line_legs_are_typed_and_honest.unit — blocked legs carry blockers; runnable legs carry flows", () => {
    expect(KhalaMobileStraightLineE2eSchemaId).toBe(
      "openagents.khala_mobile.straight_line_e2e_receipt.v1",
    )
    expect(STRAIGHT_LINE_SEED_ACCOUNT).toBe("AgentFlampy")
    expect(STRAIGHT_LINE_SEED_REPO).toBe("AgentFlampy/openagents")

    expect(STRAIGHT_LINE_LEGS.length).toBeGreaterThanOrEqual(6)
    for (const leg of STRAIGHT_LINE_LEGS) {
      if (leg.mode === "blocked") {
        expect(leg.blockerRefs.length).toBeGreaterThan(0)
      } else {
        expect(leg.blockerRefs).toEqual([])
        expect(leg.flow.length).toBeGreaterThan(0)
      }
    }

    // The CX-3 cloud-execution wall (#8547) must stay a typed blocker on the
    // writeback leg — never enforced, never faked — until that lane exists.
    const writeback = STRAIGHT_LINE_LEGS.find(leg => leg.id === "push_writeback")
    expect(writeback?.mode).toBe("blocked")
    expect(writeback?.blockerRefs.join(" ")).toContain("8547")

    // The repo-pick leg is gated on the mobile USER session invariant, and
    // its (gated) flow targets the seeded fork.
    const repoPick = STRAIGHT_LINE_LEGS.find(leg => leg.id === "ios_repo_pick_fork_bind")
    expect(repoPick?.mode).toBe("blocked")
    expect(repoPick?.blockerRefs.join(" ")).toContain("mobile_session")
    expect(repoPick?.flow).toBe("StraightLineRepoPick.yaml")
  })

  test("straight_line_flows_exist_and_stay_credential_clean.unit — every referenced flow is committed and holds no secrets", async () => {
    for (const leg of STRAIGHT_LINE_LEGS) {
      if (leg.flow === "") continue
      const flow = await read(`.maestro/flows/${leg.flow}`)
      expect(flow.length).toBeGreaterThan(0)
      expect(flow).not.toMatch(/oa_agent_[A-Za-z0-9_-]{8,}/)
      expect(flow).not.toMatch(/Bearer\s+[A-Za-z0-9._-]+/)
    }

    // The fork-bind flow is FAIL-CLOSED: it must assert the degraded state is
    // absent and target the seeded fork through the env parameter.
    const forkBind = await read(".maestro/flows/StraightLineRepoPick.yaml")
    expect(forkBind).toContain('assertNotVisible: "Repositories unavailable"')
    expect(forkBind).toContain("${KHALA_MAESTRO_REPO_FULL_NAME}")
    expect(forkBind).toContain('KHALA_MAESTRO_REPO_FULL_NAME: "AgentFlampy/openagents"')
  })

  test("straight_line_runner_covers_every_leg.unit — the runner script names each leg id exactly once as run or typed skip", async () => {
    const runner = await read("scripts/straight-line-e2e-run.sh")
    for (const leg of STRAIGHT_LINE_LEGS) {
      expect(runner).toContain(`"${leg.id}"`)
    }
    // Blocked legs must be recorded through the typed-skip path.
    expect(runner).toContain("skip_leg \"push_writeback\"")
    expect(runner).toContain("8547")
    // The runner probes the real user-session gate instead of assuming it.
    expect(runner).toContain("/api/mobile/repos?page=1&perPage=1")
    // Receipt is typed to the registry schema.
    expect(runner).toContain("openagents.khala_mobile.straight_line_e2e_receipt.v1")
    // Never a hosted-CI hook or a committed credential.
    expect(runner).not.toContain("eas ")
    expect(runner).not.toMatch(/oa_agent_[A-Za-z0-9_-]{8,}/)
  })

  test("straight_line_leg_partition_is_total.unit", () => {
    const all = [...runnableStraightLineLegIds(), ...blockedStraightLineLegIds()].sort()
    expect(all).toEqual(STRAIGHT_LINE_LEGS.map(leg => leg.id).sort())
  })
})
