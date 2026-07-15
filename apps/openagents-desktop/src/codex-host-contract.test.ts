import { describe, expect, test } from "vite-plus/test"

import { decodeCodexHostRequest } from "./codex-host-contract.ts"

describe("Codex host IPC contract", () => {
  test("admits only the closed capability operation set", () => {
    expect(decodeCodexHostRequest({ operation: "fs_read", path: "src/main.ts" })).toEqual({ operation: "fs_read", path: "src/main.ts" })
    expect(decodeCodexHostRequest({ operation: "command_exec", command: ["git", "status"], cwd: "." })).not.toBeNull()
    expect(decodeCodexHostRequest({ operation: "raw_rpc", method: "fs/readFile", params: {} })).toBeNull()
    expect(decodeCodexHostRequest({ operation: "command_exec", command: Array.from({ length: 129 }, () => "x") })).toBeNull()
    expect(decodeCodexHostRequest({ operation: "feedback_upload", classification: "bug", reason: null, attachments: Array.from({ length: 9 }, () => "x"), includeLogs: false })).toBeNull()
  })
})
