import { describe, expect, test } from "bun:test"
import { IsolatedAppProofEnvironment, isIsolatedAppProof } from "../src/isolated-app-proof.ts"

describe("isIsolatedAppProof", () => {
  test("requires the explicit flag and a user-data directory strictly below temp", () => {
    const enabled = { [IsolatedAppProofEnvironment]: "1" }
    expect(isIsolatedAppProof({ env: enabled, userDataPath: "/tmp/cut27-user", temporaryDirectory: "/tmp" })).toBe(true)
    expect(isIsolatedAppProof({ env: {}, userDataPath: "/tmp/cut27-user", temporaryDirectory: "/tmp" })).toBe(false)
    expect(isIsolatedAppProof({ env: enabled, userDataPath: "/tmp", temporaryDirectory: "/tmp" })).toBe(false)
    expect(isIsolatedAppProof({ env: enabled, userDataPath: "/Users/owner/Library/Application Support/OpenAgents", temporaryDirectory: "/tmp" })).toBe(false)
    expect(isIsolatedAppProof({ env: enabled, userDataPath: "/tmp/../Users/owner", temporaryDirectory: "/tmp" })).toBe(false)
  })
})
