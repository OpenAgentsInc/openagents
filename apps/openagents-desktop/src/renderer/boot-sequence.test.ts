import { describe, expect, test } from "vite-plus/test"

import { CODEX_CHIP_REASON_VERIFYING } from "../codex-local-contract.ts"
import type { ProviderLaneComposerProjection } from "../provider-lane-capabilities.ts"
import {
  bootSequenceReadyCount,
  bootSequenceScanning,
  projectBootSequenceAgents,
  projectBootSequenceIdentity,
} from "./boot-sequence.ts"
import { initialDesktopShellState, type DesktopShellState } from "./shell.ts"

const base = initialDesktopShellState("electron/darwin")

const lane = (over: Partial<ProviderLaneComposerProjection>): ProviderLaneComposerProjection => ({
  laneRef: "codex-local",
  provider: "codex",
  displayName: "Codex",
  admission: "admitted",
  reason: null,
  models: [],
  reasoningEfforts: [],
  permissionModes: ["owner_full"],
  approvals: "host_mediated",
  questions: true,
  skills: true,
  images: true,
  fullAuto: true,
  interrupt: true,
  queueFollowup: true,
  steerTurn: true,
  extensions: [],
  evidence: "conformant",
  ...over,
})

const withState = (over: Partial<DesktopShellState>): DesktopShellState => ({ ...base, ...over })

describe("boot sequence agent scan", () => {
  test("projects the curated agent order regardless of discovery state", () => {
    expect(projectBootSequenceAgents(base).map((agent) => agent.label)).toEqual([
      "Codex",
      "Claude Code",
      "Grok",
      "Apple FM",
    ])
  })

  test("maps a verifying codex lane to 'checking' and a ready lane to 'available' with its model", () => {
    const checking = projectBootSequenceAgents(
      withState({ harnessLanes: { ...base.harnessLanes, codex: { available: false, reason: CODEX_CHIP_REASON_VERIFYING } } }),
    )
    expect(checking.find((agent) => agent.id === "codex")?.status).toBe("checking")

    const ready = projectBootSequenceAgents(
      withState({
        harnessLanes: { ...base.harnessLanes, codex: { available: true, reason: null } },
        providerLaneCapabilities: [lane({ laneRef: "codex-local", models: ["gpt-5.6-sol"] })],
      }),
    )
    const codex = ready.find((agent) => agent.id === "codex")
    expect(codex?.status).toBe("available")
    expect(codex?.detail).toBe("gpt-5.6-sol")
  })

  test("an admitted Grok ACP lane is available", () => {
    const agents = projectBootSequenceAgents(
      withState({
        providerLaneCapabilities: [
          lane({ laneRef: "acp:grok-cli", provider: "grok", displayName: "Grok CLI", models: ["grok-4"] }),
        ],
      }),
    )
    expect(agents.find((agent) => agent.id === "grok")?.status).toBe("available")
  })

  test("Apple FM reflects live discovery: unprobed → checking, ready → available with its test inference", () => {
    const unprobed = projectBootSequenceAgents(base).find((agent) => agent.id === "apple-fm")
    expect(unprobed?.status).toBe("checking")

    const unavailable = projectBootSequenceAgents(
      withState({ appleFmBoot: { status: "unavailable", detail: "unsupported_hardware", testInference: null } }),
    ).find((agent) => agent.id === "apple-fm")
    expect(unavailable?.status).toBe("unavailable")
    expect(unavailable?.detail).toBe("unsupported_hardware")

    const ready = projectBootSequenceAgents(
      withState({ appleFmBoot: { status: "available", detail: "apple-fm-3b", testInference: "I am online." } }),
    ).find((agent) => agent.id === "apple-fm")
    expect(ready?.status).toBe("available")
    expect(ready?.detail).toBe("apple-fm-3b")
    expect(ready?.testInference).toBe("I am online.")
  })

  test("Grok reads 'checking' while discovery is still scanning, not a premature 'not connected'", () => {
    // Codex still verifying → the scan is active → an ACP peer not yet seen must
    // read as checking (its lane cap arrives on the same background refresh).
    const scanning = projectBootSequenceAgents(
      withState({ harnessLanes: { ...base.harnessLanes, codex: { available: false, reason: CODEX_CHIP_REASON_VERIFYING } } }),
    ).find((agent) => agent.id === "grok")
    expect(scanning?.status).toBe("checking")
    expect(scanning?.detail).toBe("checking…")

    // Scan settled (Apple FM resolved, codex/claude not verifying) and still no
    // Grok lane → honest "not connected".
    const settled = projectBootSequenceAgents(
      withState({ appleFmBoot: { status: "available", detail: "apple-fm-3b", testInference: null } }),
    ).find((agent) => agent.id === "grok")
    expect(settled?.status).toBe("unavailable")
    expect(settled?.detail).toBe("not connected")
  })

  test("a quarantined Grok lane is not counted as available", () => {
    const agents = projectBootSequenceAgents(
      withState({ providerLaneCapabilities: [lane({ laneRef: "acp:grok-cli", provider: "grok", admission: "quarantined" })] }),
    )
    expect(agents.find((agent) => agent.id === "grok")?.status).toBe("unavailable")
  })

  test("ready-count and scanning helpers reflect the projected lines", () => {
    const agents = projectBootSequenceAgents(
      withState({
        harnessLanes: {
          claude: { available: true, reason: null },
          codex: { available: false, reason: CODEX_CHIP_REASON_VERIFYING },
        },
      }),
    )
    expect(bootSequenceScanning(agents)).toBe(true)
    // Claude Code (claude) available; codex still checking; grok/apple-fm off.
    expect(bootSequenceReadyCount(agents)).toBe(1)
  })
})

describe("boot sequence sovereign identity scan (IDR-BS #9103)", () => {
  const publicNpub = "npub1az708q3kd9zy6z6f44zav5ygvdwelkzspf6mtusttx47lft2z38sghk0w7"
  const publicFingerprint = "d986ed01"

  test("undefined identity state reads as checking for both rows", () => {
    const rows = projectBootSequenceIdentity(base)
    expect(rows.map((row) => row.id)).toEqual(["identity", "wallet"])
    expect(rows.map((row) => row.label)).toEqual(["Identity", "Wallet"])
    expect(rows.every((row) => row.status === "checking")).toBe(true)
  })

  test("an available rehydrated identity shows the truncated npub + wallet ready", () => {
    const rows = projectBootSequenceIdentity(
      withState({
        identityBoot: {
          status: "available",
          npub: publicNpub,
          walletFingerprint: publicFingerprint,
          source: "rehydrated",
          profileId: "openagents.legacy_unified_nostr_spark.v1",
        },
      }),
    )
    const identity = rows.find((row) => row.id === "identity")
    const wallet = rows.find((row) => row.id === "wallet")
    expect(identity?.status).toBe("available")
    // Truncated for display, source suffixed, full npub never shown in the row.
    expect(identity?.detail).toBe("npub1az708q3…ghk0w7 · rehydrated")
    expect(identity?.detail?.includes(publicNpub)).toBe(false)
    expect(wallet?.detail).toBe(`${publicFingerprint} · ready`)
  })

  test("a freshly created identity is labelled 'new'", () => {
    const rows = projectBootSequenceIdentity(
      withState({
        identityBoot: {
          status: "available",
          npub: publicNpub,
          walletFingerprint: publicFingerprint,
          source: "created",
          profileId: "openagents.legacy_unified_nostr_spark.v1",
        },
      }),
    )
    expect(rows.find((row) => row.id === "identity")?.detail).toBe("npub1az708q3…ghk0w7 · new")
  })

  test("an unavailable identity reads as not detected", () => {
    const rows = projectBootSequenceIdentity(
      withState({
        identityBoot: { status: "unavailable", npub: null, walletFingerprint: null, source: null, profileId: null },
      }),
    )
    expect(rows.every((row) => row.status === "unavailable")).toBe(true)
    expect(rows.map((row) => row.detail)).toEqual(["not detected", "not detected"])
  })
})
