import { mkdtempSync, readFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import type { DesktopThread } from "./chat-contract.ts"
import type { ProviderLaneComposerProjection } from "./provider-lane-capabilities.ts"
import {
  makeProviderLaneRegistry,
  PROVIDER_SWITCH_HISTORY_MESSAGES,
  type ProviderLaneAuthentication,
  type ProviderLaneRegistryEntry,
} from "./provider-lane-registry.ts"

const capability = (laneRef: string, overrides: Partial<ProviderLaneComposerProjection> = {}): ProviderLaneComposerProjection => ({
  laneRef,
  provider: laneRef,
  displayName: laneRef,
  admission: "admitted",
  reason: null,
  models: ["fixture"],
  reasoningEfforts: [],
  permissionModes: ["owner_full"],
  approvals: "host_mediated",
  questions: true,
  skills: false,
  images: false,
  fullAuto: false,
  interrupt: true,
  queueFollowup: false,
  steerTurn: false,
  extensions: [],
  evidence: "conformant",
  ...overrides,
})

const lane = (
  laneRef: string,
  authentication: ProviderLaneAuthentication = "ready",
  overrides: Partial<ProviderLaneRegistryEntry> = {},
): ProviderLaneRegistryEntry => ({
  laneRef,
  provider: laneRef,
  profileRef: `profile:${laneRef}`,
  configuration: "configured",
  authentication,
  admission: "admitted",
  reason: null,
  capabilities: capability(laneRef),
  ...overrides,
})

const thread = (count = 3): DesktopThread => ({
  id: "thread.one",
  title: "One",
  updatedAt: "2026-07-16T00:00:00.000Z",
  notes: Array.from({ length: count }, (_, index) => ({
    key: `note.${index}`,
    role: index % 2 === 0 ? "user" as const : "assistant" as const,
    text: `message ${index}`,
    timestamp: "2026-07-16T00:00:00.000Z",
  })),
})

const setup = () => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-lane-registry-"))
  const file = path.join(root, "provider-lanes.json")
  return { file, registry: makeProviderLaneRegistry({ file, now: () => new Date("2026-07-16T12:00:00.000Z") }) }
}

describe("provider lane registry", () => {
  test("persists a per-thread selection atomically and survives reconstruction", () => {
    const { file, registry } = setup()
    expect(registry.selection("thread.one")).toBe("codex-local")
    registry.bind("thread.one", "fable-local")
    expect(makeProviderLaneRegistry({ file }).selection("thread.one")).toBe("fable-local")
    expect(JSON.parse(readFileSync(file, "utf8"))).toMatchObject({
      version: 1,
      selections: [{ threadRef: "thread.one", laneRef: "fable-local" }],
    })
  })

  test("shows missing authentication and unadmitted peers as typed refusals", () => {
    const { registry } = setup()
    expect(registry.switchThread({
      threadRef: "thread.one", laneRef: "claude", lanes: [lane("claude", "missing")], thread: thread(),
    })).toMatchObject({ ok: false, reason: "missing_auth" })
    expect(registry.switchThread({
      threadRef: "thread.one",
      laneRef: "peer.cursor",
      lanes: [lane("peer.cursor", "ready", { admission: "quarantined", reason: "Peer profile is unadmitted." })],
      thread: thread(),
    })).toMatchObject({ ok: false, reason: "unadmitted_peer", message: "Peer profile is unadmitted." })
    expect(registry.listSelections()).toEqual([])
  })

  test("refuses capability-incompatible switches without changing durable selection", () => {
    const { registry } = setup()
    const result = registry.switchThread({
      threadRef: "thread.one",
      laneRef: "peer.grok",
      lanes: [lane("peer.grok")],
      thread: thread(),
      requiredCapabilities: ["images", "fullAuto"],
    })
    expect(result).toMatchObject({
      ok: false,
      reason: "capability_mismatch",
      missingCapabilities: ["images", "fullAuto"],
    })
    expect(registry.selection("thread.one")).toBe("codex-local")
  })

  test("compatible switching carries bounded host-owned history and commits only after admission", () => {
    const { registry } = setup()
    const result = registry.switchThread({
      threadRef: "thread.one",
      laneRef: "fable-local",
      lanes: [lane("fable-local")],
      thread: thread(PROVIDER_SWITCH_HISTORY_MESSAGES + 5),
      requiredCapabilities: ["interrupt"],
    })
    expect(result).toMatchObject({
      ok: true,
      laneRef: "fable-local",
      previousLaneRef: "codex-local",
      truncated: true,
    })
    if (result.ok) {
      expect(result.history).toHaveLength(PROVIDER_SWITCH_HISTORY_MESSAGES)
      expect(result.history[0]?.text).toBe("message 5")
      expect(result.history.at(-1)?.text).toBe(`message ${PROVIDER_SWITCH_HISTORY_MESSAGES + 4}`)
    }
    expect(registry.selection("thread.one")).toBe("fable-local")
  })
})
