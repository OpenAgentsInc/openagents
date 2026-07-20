import { mkdtempSync, readFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import type { DesktopThread } from "./chat-contract.ts"
import type { ProviderLaneComposerProjection } from "./provider-lane-capabilities.ts"
import {
  makeProviderLaneRegistry,
  nativeLaneAuthenticationFromAvailability,
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
    registry.bind("thread.one", "claude-local")
    expect(makeProviderLaneRegistry({ file }).selection("thread.one")).toBe("claude-local")
    expect(JSON.parse(readFileSync(file, "utf8"))).toMatchObject({
      version: 1,
      selections: [{ threadRef: "thread.one", laneRef: "claude-local" }],
    })
  })

  test("shows missing authentication and unadmitted peers as typed refusals", () => {
    const { registry } = setup()
    expect(registry.switchThread({
      threadRef: "thread.one", laneRef: "claude", lanes: [lane("claude", "missing")], thread: thread(),
    })).toMatchObject({ ok: false, reason: "missing_auth", message: "claude has no verified authentication." })
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
      laneRef: "claude-local",
      lanes: [lane("claude-local")],
      thread: thread(PROVIDER_SWITCH_HISTORY_MESSAGES + 5),
      requiredCapabilities: ["interrupt"],
    })
    expect(result).toMatchObject({
      ok: true,
      laneRef: "claude-local",
      previousLaneRef: "codex-local",
      truncated: true,
    })
    if (result.ok) {
      expect(result.history).toHaveLength(PROVIDER_SWITCH_HISTORY_MESSAGES)
      expect(result.history[0]?.text).toBe("message 5")
      expect(result.history.at(-1)?.text).toBe(`message ${PROVIDER_SWITCH_HISTORY_MESSAGES + 4}`)
    }
    expect(registry.selection("thread.one")).toBe("claude-local")
  })

  // Bug #8998: a live-P0 regression where every ordinary (non-Full-Auto)
  // provider switch to a native lane (codex-local, claude-local) was refused
  // with `missing_auth` regardless of real login state. Root cause: the
  // caller (main.ts's `providerLaneEntries()`) sourced `authentication` from
  // a passive cache that nothing populated after #8974 removed the last
  // renderer caller of the availability IPC channels, so it stayed stuck at
  // the "unknown" default forever. `switchThread` itself was never at fault
  // (it is a pure function of whatever `lanes` the caller passes in) -- these
  // cases pin the contract the fix now relies on: a caller MUST derive
  // `authentication` from a live availability probe, never from a stale
  // default, or a genuinely-authenticated account gets permanently locked out.
  test("nativeLaneAuthenticationFromAvailability maps a live probe to ready/missing, never unknown", () => {
    expect(nativeLaneAuthenticationFromAvailability({ state: "available" })).toBe("ready")
    expect(nativeLaneAuthenticationFromAvailability({ state: "unavailable" })).toBe("missing")
  })

  test("a native lane whose authentication reflects a live-probed available account is never refused for missing_auth", () => {
    const { registry } = setup()
    // Simulates the FIXED `providerLaneEntries()`: authentication computed
    // fresh from `codexLocal.availability()` on every call via
    // `nativeLaneAuthenticationFromAvailability`, never from a passive cache.
    const liveAuthentication = nativeLaneAuthenticationFromAvailability({ state: "available" })
    const result = registry.switchThread({
      threadRef: "thread.one",
      laneRef: "codex-local",
      lanes: [lane("codex-local", liveAuthentication)],
      thread: thread(),
    })
    expect(result).toMatchObject({ ok: true, laneRef: "codex-local" })
    // Repeated calls (as would happen across repeated `providerLaneEntries()`
    // invocations from repeated composer opens) must never regress to a
    // stale-cache refusal as long as the live probe keeps reporting available.
    expect(registry.switchThread({
      threadRef: "thread.one",
      laneRef: "claude-local",
      lanes: [lane("claude-local", nativeLaneAuthenticationFromAvailability({ state: "available" }))],
      thread: thread(),
    })).toMatchObject({ ok: true, laneRef: "claude-local" })
  })

  test("a native lane still stuck at the pre-#8998-fix stale 'unknown' default is correctly refused (proves the bug shape)", () => {
    const { registry } = setup()
    // This is exactly the pre-fix bug: a lane reported with the permanent
    // "unknown" default (the passive `providerLaneAuthentication` Map's
    // seed value, never updated because nothing calls the availability IPC
    // channel anymore) is refused even though the account is genuinely
    // authenticated. The fix's job is ensuring production callers never
    // actually construct lanes this way for a native lane -- see
    // `nativeLaneAuthenticationFromAvailability` above and its use in
    // `providerLaneEntries()` (apps/openagents-desktop/src/main.ts).
    const result = registry.switchThread({
      threadRef: "thread.one",
      laneRef: "codex-local",
      lanes: [lane("codex-local", "unknown")],
      thread: thread(),
    })
    expect(result).toMatchObject({ ok: false, reason: "missing_auth" })
  })
})
