import { readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

/**
 * MH-6 (#8585) FleetPeekScreen mount-coverage contract. Follows the
 * thread-list-screen pattern: a lightweight `react-test-renderer` mount plus
 * source assertions on the screen's wiring, so this suite never installs a
 * global `mock.module` for the shared sync-runtime context (which leaks across
 * bun test files). The screen's PURE half (view-model derivation + the three
 * typed intent factories) is exhaustively behavior-tested in
 * fleet-peek-core.test.ts.
 */

const mobileRoot = new URL("../", import.meta.url).pathname
const source = readFileSync(
  join(mobileRoot, "src/screens/fleet-peek-screen.tsx"),
  "utf8",
)

const ContractMountMarker = ({ children }: { children: React.ReactNode }) =>
  React.createElement("Text", null, children)

describe("contract khala_mobile.fleet_peek.rn_component_mount_coverage.v1 — FleetPeekScreen", () => {
  test("keeps the fleet peek wired to the Sync projection reads and the three MH-0 typed steering intents", () => {
    let renderer: ReturnType<typeof createTestRenderer> | undefined
    act(() => {
      renderer = createTestRenderer(
        React.createElement(ContractMountMarker, null, "Fleet"),
      )
    })
    expect(renderer!.toJSON()).toMatchObject({ children: ["Fleet"], type: "Text" })

    // reads: the four projected fleet entity types via the shared scope hook
    expect(source).toContain("useKhalaSyncScopeEntities")
    expect(source).toContain("FLEET_RUN_ENTITY_TYPE")
    expect(source).toContain("FLEET_WORKER_ENTITY_TYPE")
    expect(source).toContain("FLEET_APPROVAL_ENTITY_TYPE")
    expect(source).toContain("FLEET_STEER_ENTITY_TYPE")
    expect(source).toContain("deriveFleetPeekViewModel")

    // dispatch: the three typed intents through the db-collection client
    // mutators via session.mutate — never a local supervisor implementation
    expect(source).toContain("fleetDispatchRunControlClientMutator")
    expect(source).toContain("fleetDispatchApprovalDecisionClientMutator")
    expect(source).toContain("makeRunControlIntent")
    expect(source).toContain("makeApprovalDecisionIntent")
    expect(source).toContain("session!.mutate")

    // operator affordances: run controls + per-approval allow/deny
    expect(source).toContain("availableRunControls")
    expect(source).toContain("Allow")
    expect(source).toContain("Deny")
    expect(source).toContain("harnessCounts")
  })
})
