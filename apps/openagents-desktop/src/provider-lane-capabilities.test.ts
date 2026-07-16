import { describe, expect, test } from "vite-plus/test"

import {
  decodeProviderLaneComposerProjections,
  projectProviderLaneCapabilities,
  type ProviderLaneCapabilityReport,
} from "./provider-lane-capabilities.ts"

const report = (
  extra: Partial<ProviderLaneCapabilityReport> = {},
): ProviderLaneCapabilityReport => ({
  laneRef: "codex-local",
  provider: "codex",
  models: ["gpt-5.6-sol", "gpt-5.5"],
  features: {
    skills: false,
    planOnly: false,
    reasoningEffort: true,
    images: true,
    fullAuto: true,
    interrupt: true,
    queueFollowup: true,
    steerTurn: true,
    steerChild: false,
    answerQuestion: true,
  },
  composer: {
    displayName: "Codex",
    reasoningEfforts: ["low", "medium", "high", "xhigh"],
    permissionModes: ["owner_full"],
    approvals: "host_mediated",
    extensions: [],
  },
  policy: {
    source: "native-static-declaration",
    profileRef: "native:codex-local:v1",
    evidence: "conformant",
    allowedModels: ["gpt-5.6-sol", "gpt-5.5"],
    allowedFeatures: ["reasoningEffort", "images", "fullAuto", "interrupt", "queueFollowup", "steerTurn", "answerQuestion"],
    allowedExtensions: [],
  },
  recovery: "provider_session_replay",
  ...extra,
})

describe("provider lane composer capability projection", () => {
  test("projects distinct native lane affordances without Codex residue", () => {
    const codex = projectProviderLaneCapabilities(report())
    const claude = projectProviderLaneCapabilities(report({
      laneRef: "fable-local",
      provider: "claude_agent",
      models: ["claude-fable-5", "claude-opus-4-8"],
      features: {
        skills: true,
        planOnly: true,
        reasoningEffort: false,
        images: true,
        fullAuto: false,
        interrupt: true,
        queueFollowup: true,
        steerTurn: false,
        steerChild: true,
        answerQuestion: true,
      },
      composer: {
        displayName: "Claude",
        reasoningEfforts: [],
        permissionModes: ["owner_full", "plan_only"],
        approvals: "provider_native",
        extensions: ["skills"],
      },
      policy: {
        source: "native-static-declaration",
        profileRef: "native:claude-agent:v1",
        evidence: "conformant",
        allowedModels: ["claude-fable-5", "claude-opus-4-8"],
        allowedFeatures: ["skills", "planOnly", "images", "interrupt", "queueFollowup", "steerChild", "answerQuestion"],
        allowedExtensions: ["skills"],
      },
      recovery: "interrupt_on_restart",
    }))
    expect(codex).toMatchObject({ admission: "admitted", displayName: "Codex", reasoningEfforts: ["low", "medium", "high", "xhigh"], permissionModes: ["owner_full"], fullAuto: true, steerTurn: true, skills: false })
    expect(claude).toMatchObject({ admission: "admitted", displayName: "Claude", reasoningEfforts: [], permissionModes: ["owner_full", "plan_only"], fullAuto: false, steerTurn: false, skills: true })
    expect(claude.models.every(model => model.startsWith("claude-"))).toBe(true)
  })

  test("quarantines an ACP profile capability lie instead of partially enabling it", () => {
    const projection = projectProviderLaneCapabilities(report({
      laneRef: "grok-acp",
      provider: "grok",
      models: ["grok-code-fast"],
      features: { ...report().features, fullAuto: true, reasoningEffort: false, images: false },
      composer: { displayName: "Grok", reasoningEfforts: [], permissionModes: ["owner_full"], approvals: "provider_native", extensions: ["vendor.rogue/exfiltrate"] },
      policy: {
        source: "trusted-acp-peer-profile",
        profileRef: "grok-cli",
        evidence: "experimental",
        allowedModels: ["grok-code-fast"],
        allowedFeatures: ["interrupt", "answerQuestion"],
        allowedExtensions: ["x.ai/ask_user_question"],
      },
    }))
    expect(projection.admission).toBe("quarantined")
    expect(projection.reason).toContain("feature:fullAuto")
    expect(projection.reason).toContain("extension:vendor.rogue/exfiltrate")
    expect(projection).toMatchObject({ models: [], images: false, fullAuto: false, questions: false, extensions: [] })
  })

  test("preload decoder refuses malformed projections", () => {
    const valid = projectProviderLaneCapabilities(report())
    expect(decodeProviderLaneComposerProjections([valid])).toEqual([valid])
    expect(decodeProviderLaneComposerProjections([{ ...valid, fullAuto: "yes" }])).toBeNull()
  })
})
