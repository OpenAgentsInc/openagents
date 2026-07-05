import { describe, expect, test } from "bun:test"

import type {
  KhalaCodeDesktopDiagnosticsSnapshotResult,
  KhalaCodeDesktopUpdaterStatus,
} from "../src/shared/rpc"
import {
  buildKhalaCodeSupportIssueMetadata,
  isKhalaCodeSupportUrlAllowed,
  projectKhalaCodeSupportEntrypoints,
  sanitizeKhalaCodeSupportMetadataText,
} from "../src/shared/support-entrypoints"

const updater = (releaseNotesUrl = "https://github.com/OpenAgentsInc/openagents/releases/tag/test"): KhalaCodeDesktopUpdaterStatus => ({
  app: "Khala Code Desktop",
  capability: "in_app_updater",
  channel: "stable",
  currentVersion: "0.1.0",
  enabled: true,
  observedAt: "2026-07-05T00:00:00.000Z",
  ok: true,
  releaseNotesUrl,
  state: { checkedAt: "2026-07-05T00:00:00.000Z", status: "up_to_date", version: "0.1.0" },
})

const diagnostics: KhalaCodeDesktopDiagnosticsSnapshotResult = {
  counts: { main: 2, "native-shell": 0, renderer: 3, service: 1 },
  ok: true,
  unresponsiveState: "responsive",
}

describe("Khala Code support entrypoints", () => {
  test("allowlists only OpenAgents support URLs", () => {
    expect(isKhalaCodeSupportUrlAllowed("https://openagents.com/docs")).toBe(true)
    expect(isKhalaCodeSupportUrlAllowed("https://github.com/OpenAgentsInc/openagents/issues/new")).toBe(true)
    expect(isKhalaCodeSupportUrlAllowed("http://openagents.com/docs")).toBe(false)
    expect(isKhalaCodeSupportUrlAllowed("https://evil.example/OpenAgentsInc/openagents")).toBe(false)
  })

  test("builds release-note and support links from current build metadata", () => {
    const projection = projectKhalaCodeSupportEntrypoints({
      activeThreadPresent: true,
      activeView: "settings",
      diagnostics,
      messageCount: 4,
      updater: updater("https://not-allowed.example/release"),
    })

    expect(projection.entries.map(entry => entry.id)).toEqual([
      "release_notes",
      "docs",
      "support",
      "feedback",
      "bug_report",
    ])
    expect(projection.entries[0]?.url).toBe("https://github.com/OpenAgentsInc/openagents/releases")
    expect(projection.issueMetadata).toContain("version=0.1.0")
    expect(projection.issueMetadata).toContain("activeThreadPresent=true")
  })

  test("redacts secrets and private local paths from issue metadata", () => {
    const redacted = sanitizeKhalaCodeSupportMetadataText(
      "token=sk-secretsecretsecret /Users/christopherdavid/work/openagents C:\\Users\\me\\secret",
    )
    expect(redacted).toContain("[REDACTED_SECRET]")
    expect(redacted).toContain("[REDACTED_LOCAL_PATH]")
    expect(redacted).not.toContain("christopherdavid")

    const metadata = buildKhalaCodeSupportIssueMetadata({
      activeThreadPresent: false,
      activeView: "/Users/christopherdavid/work/openagents",
      diagnostics,
      messageCount: 1,
      updater: {
        ...updater(),
        state: { message: "authorization: bearer-token /Users/me/work", retryable: true, status: "error" },
      },
    })
    expect(metadata).not.toContain("bearer-token")
    expect(metadata).not.toContain("/Users/")
  })
})
