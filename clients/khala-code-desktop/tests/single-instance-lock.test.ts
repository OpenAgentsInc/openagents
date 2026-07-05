import { afterEach, describe, expect, test } from "bun:test"
import { rm } from "node:fs/promises"
import { randomBytes } from "node:crypto"

import {
  KHALA_CODE_DESKTOP_SINGLE_INSTANCE_ENV,
  KHALA_CODE_DESKTOP_SINGLE_INSTANCE_SOCKET_ENV,
  acquireKhalaCodeDesktopSingleInstanceLock,
  khalaCodeDesktopSingleInstanceEnabled,
  resolveKhalaCodeDesktopSingleInstanceSocketPath,
} from "../src/bun/single-instance-lock"

const tempSockets: string[] = []
afterEach(async () => {
  for (const path of tempSockets.splice(0)) {
    await rm(path, { force: true })
  }
})

// AF_UNIX `sun_path` is kernel-limited to ~100-108 bytes (see
// MAX_SAFE_UNIX_SOCKET_PATH_BYTES in single-instance-lock.ts). `os.tmpdir()`
// on macOS resolves to a long per-user `/var/folders/...` path that alone can
// exceed that limit, so tests use a short path directly under `/tmp` rather
// than the usual `mkdtemp(tmpdir(), ...)` pattern.
const tempSocketPath = (): string => {
  const path = `/tmp/khala-si-${randomBytes(4).toString("hex")}.sock`
  tempSockets.push(path)
  return path
}

describe("Khala Code desktop single-instance flag resolution", () => {
  test("is enabled by default and disabled only by an explicit 0/false/off", () => {
    expect(khalaCodeDesktopSingleInstanceEnabled({})).toBe(true)
    expect(khalaCodeDesktopSingleInstanceEnabled({
      [KHALA_CODE_DESKTOP_SINGLE_INSTANCE_ENV]: "0",
    })).toBe(false)
    expect(khalaCodeDesktopSingleInstanceEnabled({
      [KHALA_CODE_DESKTOP_SINGLE_INSTANCE_ENV]: "false",
    })).toBe(false)
    expect(khalaCodeDesktopSingleInstanceEnabled({
      [KHALA_CODE_DESKTOP_SINGLE_INSTANCE_ENV]: "off",
    })).toBe(false)
    expect(khalaCodeDesktopSingleInstanceEnabled({
      [KHALA_CODE_DESKTOP_SINGLE_INSTANCE_ENV]: "1",
    })).toBe(true)
  })

  test("resolves the socket path from HOME by default, or the explicit override", () => {
    expect(resolveKhalaCodeDesktopSingleInstanceSocketPath({ HOME: "/home/owner" }))
      .toBe("/home/owner/.khala-code/desktop-single-instance.sock")
    expect(resolveKhalaCodeDesktopSingleInstanceSocketPath({
      HOME: "/home/owner",
      [KHALA_CODE_DESKTOP_SINGLE_INSTANCE_SOCKET_ENV]: "/tmp/custom.sock",
    })).toBe("/tmp/custom.sock")
  })
})

describe("Khala Code desktop single-instance lock (real socket I/O)", () => {
  test("the first launch becomes primary; a second launch forwards and does not become primary", async () => {
    const socketPath = tempSocketPath()
    const received: string[] = []

    const primary = await acquireKhalaCodeDesktopSingleInstanceLock({
      onIncomingPayload: payload => received.push(payload),
      socketPath,
    })
    expect(primary.role).toBe("primary")

    const secondary = await acquireKhalaCodeDesktopSingleInstanceLock({
      forwardPayload: "khala-code://thread/abc-123",
      onIncomingPayload: () => {
        throw new Error("secondary should never receive incoming payloads")
      },
      socketPath,
    })
    expect(secondary).toEqual({ forwarded: true, role: "secondary" })

    // The forward is a real async socket write; give it a moment to land.
    await new Promise(resolve => setTimeout(resolve, 50))
    expect(received).toEqual(["khala-code://thread/abc-123"])

    if (primary.role === "primary") primary.close()
  })

  test("forwards multiple secondary launches to the same primary", async () => {
    const socketPath = tempSocketPath()
    const received: string[] = []

    const primary = await acquireKhalaCodeDesktopSingleInstanceLock({
      onIncomingPayload: payload => received.push(payload),
      socketPath,
    })
    expect(primary.role).toBe("primary")

    for (const url of ["khala-code://thread/1", "khala-code://thread/2", "khala-code://thread/3"]) {
      const secondary = await acquireKhalaCodeDesktopSingleInstanceLock({
        forwardPayload: url,
        onIncomingPayload: () => undefined,
        socketPath,
      })
      expect(secondary).toEqual({ forwarded: true, role: "secondary" })
    }

    await new Promise(resolve => setTimeout(resolve, 100))
    expect(received.sort()).toEqual([
      "khala-code://thread/1",
      "khala-code://thread/2",
      "khala-code://thread/3",
    ])

    if (primary.role === "primary") primary.close()
  })

  test("a secondary launch with no payload still forwards cleanly (no forwardPayload)", async () => {
    const socketPath = tempSocketPath()
    const received: string[] = []

    const primary = await acquireKhalaCodeDesktopSingleInstanceLock({
      onIncomingPayload: payload => received.push(payload),
      socketPath,
    })
    expect(primary.role).toBe("primary")

    const secondary = await acquireKhalaCodeDesktopSingleInstanceLock({
      onIncomingPayload: () => undefined,
      socketPath,
    })
    expect(secondary).toEqual({ forwarded: true, role: "secondary" })

    await new Promise(resolve => setTimeout(resolve, 50))
    expect(received).toEqual([])

    if (primary.role === "primary") primary.close()
  })

  test("after the primary closes, a new launch can become primary at the same path", async () => {
    const socketPath = tempSocketPath()

    const first = await acquireKhalaCodeDesktopSingleInstanceLock({
      onIncomingPayload: () => undefined,
      socketPath,
    })
    expect(first.role).toBe("primary")
    if (first.role === "primary") first.close()

    // Give the OS a beat to release the socket file/listener.
    await new Promise(resolve => setTimeout(resolve, 50))

    const second = await acquireKhalaCodeDesktopSingleInstanceLock({
      onIncomingPayload: () => undefined,
      socketPath,
    })
    expect(second.role).toBe("primary")
    if (second.role === "primary") second.close()
  })

  test("refuses an overlong socket path instead of risking a truncated AF_UNIX bind/connect", async () => {
    // AF_UNIX sun_path is kernel-limited (~100-108 bytes). A path at or past
    // that limit must fail loudly via onListenError, never silently connect
    // to (or bind) a truncated/unrelated path.
    const overlong = `/tmp/${"x".repeat(120)}.sock`
    const errors: string[] = []

    const result = await acquireKhalaCodeDesktopSingleInstanceLock({
      onIncomingPayload: () => undefined,
      onListenError: error => errors.push(error instanceof Error ? error.message : String(error)),
      socketPath: overlong,
    })

    expect(result).toEqual({ forwarded: false, role: "secondary" })
    expect(errors).toHaveLength(1)
    expect(errors[0]).toContain("too long")
  })
})
