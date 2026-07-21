import { existsSync, mkdtempSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { afterEach, beforeEach, describe, expect, test } from "vite-plus/test"

import { openDesktopWorkspaceConsentStore } from "./desktop-workspace-consent-host.ts"

let root = ""
const consentPath = (): string => path.join(root, "workspace-consent.json")

beforeEach(() => {
  root = mkdtempSync(path.join(tmpdir(), "oa-workspace-consent-"))
})
afterEach(() => {
  rmSync(root, { recursive: true, force: true })
})

describe("openDesktopWorkspaceConsentStore", () => {
  test("returns null before anything is decided", () => {
    expect(openDesktopWorkspaceConsentStore(consentPath()).snapshot()).toBeNull()
  })

  test("persists a grant once and reads it back on the next launch (asked once)", () => {
    const decidedAt = "2026-07-21T12:00:00.000Z"
    const first = openDesktopWorkspaceConsentStore(consentPath())
    const stored = first.record({ status: "granted", workspaceRoot: "/work/openagents", decidedAt })
    expect(stored.status).toBe("granted")
    expect(stored.workspaceRoot).toBe("/work/openagents")

    // A fresh store (relaunch) sees the durable decision — no re-prompt.
    const relaunch = openDesktopWorkspaceConsentStore(consentPath())
    const snapshot = relaunch.snapshot()
    expect(snapshot?.status).toBe("granted")
    expect(snapshot?.workspaceRoot).toBe("/work/openagents")
    expect(snapshot?.decidedAt).toBe(decidedAt)
  })

  test("a decline persists with no workspace root and survives relaunch", () => {
    openDesktopWorkspaceConsentStore(consentPath()).record({
      status: "declined",
      workspaceRoot: "/work/ignored",
      decidedAt: "2026-07-21T12:00:00.000Z",
    })
    const snapshot = openDesktopWorkspaceConsentStore(consentPath()).snapshot()
    expect(snapshot?.status).toBe("declined")
    expect(snapshot?.workspaceRoot).toBeNull()
  })

  test("writes the record owner-only (mode 0600)", () => {
    openDesktopWorkspaceConsentStore(consentPath()).record({
      status: "granted",
      workspaceRoot: "/work/openagents",
      decidedAt: "2026-07-21T12:00:00.000Z",
    })
    expect(existsSync(consentPath())).toBe(true)
    expect(statSync(consentPath()).mode & 0o777).toBe(0o600)
  })

  test("a corrupt file is treated as not-yet-decided, never throws", () => {
    writeFileSync(consentPath(), "{ not json", "utf8")
    expect(openDesktopWorkspaceConsentStore(consentPath()).snapshot()).toBeNull()
  })
})
