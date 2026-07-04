import { readFileSync } from "node:fs"
import { describe, expect, test } from "bun:test"
import {
  KhalaSyncEvidenceValidationError,
  validateKhalaSyncCrossDeviceEvidence,
} from "./validate-khala-sync-cross-device-evidence"

const validOwnerSignedBundle = () => ({
  schema: "openagents.khala_sync.cross_device_chat_dogfood.v1",
  status: "owner_signed",
  issueRef: "OpenAgentsInc/openagents#8354",
  epicRef: "OpenAgentsInc/openagents#8339",
  generatedAt: "2026-07-04T20:30:00.000Z",
  routeRefs: [
    "route.khala_sync.push.v0_1",
    "route.khala_sync.bootstrap.v0_1",
  ],
  blockerRefs: [],
  flows: [
    {
      flowRef: "flow.phone_to_desktop_web",
      sourceSurface: "phone",
      observedSurfaces: ["desktop", "web"],
      counts: {
        threadsCreated: 1,
        messagesAppended: 1,
        threadsObserved: 2,
        messagesObserved: 1,
      },
      latencyMs: {
        phoneToDesktop: 1280,
        phoneToWeb: 1540,
      },
      scopeRefs: [
        "scope.user.user.public.owner",
        "scope.thread.thread.public.fixture",
      ],
      receiptRefs: ["receipt.khala_sync.cross_device.fixture"],
      routeRefs: ["route.khala_sync.push.v0_1"],
    },
  ],
  safeguards: {
    containsChatContent: false,
    containsSecrets: false,
    contentFieldsRedacted: true,
    ownerSignedTransitionsOnly: true,
    promiseFlips: false,
  },
  ownerSignoff: {
    signerRef: "owner.public.christopher",
    signedAt: "2026-07-04T20:35:00.000Z",
    methodRef: "github.issue_comment",
    commentRef: "https://github.com/OpenAgentsInc/openagents/issues/8354#issuecomment-fixture",
  },
  khalaCodeEvidenceRefs: [
    "khala_code.chat.cross_device_dogfood.fixture.v1",
  ],
})

describe("validateKhalaSyncCrossDeviceEvidence", () => {
  test("accepts an owner-signed public-safe count/latency bundle", () => {
    expect(validateKhalaSyncCrossDeviceEvidence(validOwnerSignedBundle()).status)
      .toBe("owner_signed")
  })

  test("accepts the committed pending owner-run bundle", () => {
    const bundle = JSON.parse(
      readFileSync(
        "docs/khala-sync/receipts/2026-07-04-cross-device-chat-dogfood.pending.json",
        "utf8",
      ),
    ) as unknown
    expect(validateKhalaSyncCrossDeviceEvidence(bundle).status)
      .toBe("pending_owner_signoff")
  })

  test("rejects raw chat body fields", () => {
    const bundle = validOwnerSignedBundle()
    ;(bundle.flows[0] as Record<string, unknown>).body = "do not publish this"
    expect(() => validateKhalaSyncCrossDeviceEvidence(bundle))
      .toThrow(KhalaSyncEvidenceValidationError)
  })

  test("rejects secret-shaped strings", () => {
    const bundle = validOwnerSignedBundle()
    bundle.ownerSignoff.commentRef = "bearer oa_agent_secret"
    expect(() => validateKhalaSyncCrossDeviceEvidence(bundle))
      .toThrow(KhalaSyncEvidenceValidationError)
  })
})
