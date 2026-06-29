import { describe, expect, test } from "bun:test"
import {
  CONFORMANCE_CASE_NAMES,
  runConformanceMatrix,
} from "@openagentsinc/autopilot-control-protocol"

// CL-33: the desktop client runs the shared cross-client conformance matrix,
// proving the protocol behaves identically on this runtime.
describe("cross-client conformance (desktop)", () => {
  test("every conformance case passes", () => {
    const results = runConformanceMatrix()
    expect(results.length).toBe(CONFORMANCE_CASE_NAMES.length)
    for (const result of results) {
      expect(result.ok, `${result.name}: ${result.detail ?? ""}`).toBe(true)
    }
  })
})
