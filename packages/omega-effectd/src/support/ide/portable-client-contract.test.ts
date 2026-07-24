import { describe, expect, test } from "vite-plus/test"

import {
  decodeIdePortableClientCommand,
  decodeIdePortableClientSnapshot,
  emptyIdePortableClientSnapshot,
} from "./portable-client-contract.ts"

describe("Desktop IDE portable client contract", () => {
  test("keeps an unavailable boundary explicit and bounded", () => {
    const snapshot = emptyIdePortableClientSnapshot()
    expect(decodeIdePortableClientSnapshot(snapshot)).toEqual(snapshot)
    expect(snapshot.status.phase).toBe("unavailable")
  })

  test("rejects host paths and malformed portable commands before IPC", () => {
    expect(decodeIdePortableClientCommand({
      schema: "openagents.portable_session_command.v1",
      commandRef: "/Users/owner/private",
    })).toBeNull()
  })
})
