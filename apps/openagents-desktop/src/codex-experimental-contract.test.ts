import { describe, expect, test } from "vite-plus/test"
import { decodeCodexExperimentalRequest } from "./codex-experimental-contract.ts"

describe("Codex experimental IPC contract", () => {
  test("requires explicit confirmation and exposes no raw protocol operation", () => {
    expect(decodeCodexExperimentalRequest({ operation: "process_spawn", command: ["git", "status"], cwd: ".", confirmed: true })).not.toBeNull()
    expect(decodeCodexExperimentalRequest({ operation: "process_spawn", command: ["git"], cwd: "." })).toBeNull()
    expect(decodeCodexExperimentalRequest({ operation: "memory_reset", confirmation: "RESET", confirmed: true })).not.toBeNull()
    expect(decodeCodexExperimentalRequest({ operation: "memory_reset", confirmation: "yes", confirmed: true })).toBeNull()
    expect(decodeCodexExperimentalRequest({ operation: "raw_rpc", method: "process/spawn" })).toBeNull()
  })
})
