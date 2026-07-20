import { describe, expect, test } from "vite-plus/test"

import { CODEX_CHIP_REASON_VERIFYING } from "../codex-local-contract.ts"
import type { ProviderLaneComposerProjection } from "../provider-lane-capabilities.ts"
import {
  bootSequenceReadyCount,
  bootSequenceScanning,
  projectBootSequenceAgents,
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

  test("an admitted Grok ACP lane is available; Apple FM is always unavailable on desktop", () => {
    const agents = projectBootSequenceAgents(
      withState({
        providerLaneCapabilities: [
          lane({ laneRef: "acp:grok-cli", provider: "grok", displayName: "Grok CLI", models: ["grok-4"] }),
        ],
      }),
    )
    expect(agents.find((agent) => agent.id === "grok")?.status).toBe("available")
    expect(agents.find((agent) => agent.id === "apple-fm")?.status).toBe("unavailable")
    expect(agents.find((agent) => agent.id === "apple-fm")?.detail).toBe("not available on desktop")
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
          fable: { available: true, reason: null },
          codex: { available: false, reason: CODEX_CHIP_REASON_VERIFYING },
        },
      }),
    )
    expect(bootSequenceScanning(agents)).toBe(true)
    // Claude Code (fable) available; codex still checking; grok/apple-fm off.
    expect(bootSequenceReadyCount(agents)).toBe(1)
  })
})
