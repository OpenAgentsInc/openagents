import { readFileSync } from "node:fs"
import { describe, expect, test } from "bun:test"
import {
  KhalaSyncRuntimeDogfoodEvidenceValidationError,
  validateKhalaSyncRuntimeDogfoodEvidence,
} from "./validate-khala-sync-runtime-dogfood-evidence"

const validBundle = () => ({
  schema: "openagents.khala_sync.runtime_ai_sdk_shaped_dogfood.v1",
  status: "simulator_only",
  evidenceMode: "simulator_only",
  issueRef: "OpenAgentsInc/openagents#8375",
  generatedAt: "2026-07-05T00:00:00.000Z",
  roadmapIssueRefs: [
    "OpenAgentsInc/openagents#8363",
    "OpenAgentsInc/openagents#8364",
    "OpenAgentsInc/openagents#8365",
    "OpenAgentsInc/openagents#8370",
    "OpenAgentsInc/openagents#8373",
    "OpenAgentsInc/openagents#8374",
    "OpenAgentsInc/openagents#8375",
  ],
  routeRefs: [
    "route.khala_sync.push.v0_1",
    "route.khala_sync.bootstrap.v0_1",
    "route.khala_sync.connect.v0_1",
  ],
  scopeRefs: [
    "scope.user.owner_runtime_sim",
    "scope.thread.runtime_thread_sim",
  ],
  buildRefs: [
    "build.khala_mobile.expo_ios_simulator.local_debug",
    "build.khala_code_desktop.local_debug",
  ],
  safeguards: {
    containsRawPrompts: false,
    containsChatBodies: false,
    containsProviderChunks: false,
    containsLocalPaths: false,
    containsTokens: false,
    containsSecrets: false,
    contentFieldsRedacted: true,
    publicSafeProjectionOnly: true,
    simulatorOnlyLabel: true,
    promiseFlips: false,
  },
  flows: [
    {
      flowRef: "flow.khala_sync.runtime.simulator.mobile_desktop_resume.v1",
      evidenceMode: "simulator_only",
      sourceSurface: "khala-mobile-ios-simulator",
      observedSurfaces: [
        "khala-code-desktop-local",
        "khala-mobile-ios-simulator-after-restart",
      ],
      proofs: {
        mobileIntentAppearedOnDesktopWithoutRestart: true,
        desktopRuntimeEventAppearedOnMobileAfterResume: true,
        restartResumeWithoutDuplicateEvents: true,
      },
      counts: {
        threadsCreated: 1,
        userMessagesAppended: 1,
        runtimeControlIntentsAccepted: 4,
        runtimeTurnsObservedDesktop: 1,
        runtimeEventsObservedMobile: 2,
        restartResumeCycles: 1,
        duplicateEventsAfterResume: 0,
      },
      latencyBuckets: {
        mobileIntentToDesktop: "lt_5s",
        desktopEventToMobileCatchup: "lt_5s",
        restartResumeCatchup: "lt_30s",
      },
      routeRefs: [
        "route.khala_sync.push.v0_1",
        "route.khala_sync.bootstrap.v0_1",
      ],
      scopeRefs: [
        "scope.user.owner_runtime_sim",
        "scope.thread.runtime_thread_sim",
      ],
      receiptRefs: [
        "receipt.khala_sync.runtime_dogfood.simulator.2026_07_05",
      ],
      buildRefs: [
        "build.khala_mobile.expo_ios_simulator.local_debug",
        "build.khala_code_desktop.local_debug",
      ],
      issueRefs: ["OpenAgentsInc/openagents#8375"],
    },
  ],
  gaps: [
    {
      gapRef: "gap.khala_sync.runtime.web_projection_runtime_stream",
      status: "open",
      summaryRef: "summary.public_safe.web_projection_not_yet_live",
      issueRefs: ["OpenAgentsInc/openagents#8375"],
    },
  ],
})

describe("validateKhalaSyncRuntimeDogfoodEvidence", () => {
  test("accepts the committed simulator-only runtime dogfood bundle", () => {
    const bundle = JSON.parse(
      readFileSync(
        "docs/khala-sync/receipts/2026-07-05-runtime-ai-sdk-shaped-dogfood.simulator.json",
        "utf8",
      ),
    ) as unknown
    expect(validateKhalaSyncRuntimeDogfoodEvidence(bundle).status).toBe(
      "simulator_only",
    )
  })

  test("accepts a valid public-safe simulator bundle", () => {
    expect(validateKhalaSyncRuntimeDogfoodEvidence(validBundle()).status).toBe(
      "simulator_only",
    )
  })

  test("rejects raw prompts", () => {
    const bundle = validBundle()
    ;(bundle.flows[0] as Record<string, unknown>).prompt = "hidden user text"
    expect(() => validateKhalaSyncRuntimeDogfoodEvidence(bundle)).toThrow(
      KhalaSyncRuntimeDogfoodEvidenceValidationError,
    )
  })

  test("rejects chat bodies", () => {
    const bundle = validBundle()
    ;(bundle.flows[0] as Record<string, unknown>).chatBody = "hidden chat body"
    expect(() => validateKhalaSyncRuntimeDogfoodEvidence(bundle)).toThrow(
      KhalaSyncRuntimeDogfoodEvidenceValidationError,
    )
  })

  test("rejects provider chunks", () => {
    const bundle = validBundle()
    ;(bundle.flows[0] as Record<string, unknown>).providerChunk = {
      type: "text-delta",
    }
    expect(() => validateKhalaSyncRuntimeDogfoodEvidence(bundle)).toThrow(
      KhalaSyncRuntimeDogfoodEvidenceValidationError,
    )
  })

  test("rejects local paths and secret-shaped strings", () => {
    const bundle = validBundle()
    bundle.buildRefs.push("/Users/christopherdavid/work/openagents")
    expect(() => validateKhalaSyncRuntimeDogfoodEvidence(bundle)).toThrow(
      KhalaSyncRuntimeDogfoodEvidenceValidationError,
    )
  })

  test("rejects token-shaped strings", () => {
    const bundle = validBundle()
    bundle.routeRefs.push("bearer oa_agent_secret")
    expect(() => validateKhalaSyncRuntimeDogfoodEvidence(bundle)).toThrow(
      KhalaSyncRuntimeDogfoodEvidenceValidationError,
    )
  })

  test("rejects duplicate events after resume", () => {
    const bundle = validBundle()
    bundle.flows[0].counts.duplicateEventsAfterResume = 1
    expect(() => validateKhalaSyncRuntimeDogfoodEvidence(bundle)).toThrow(
      KhalaSyncRuntimeDogfoodEvidenceValidationError,
    )
  })
})
