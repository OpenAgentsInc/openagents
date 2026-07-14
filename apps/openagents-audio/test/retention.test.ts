import { describe, expect, test } from "vite-plus/test"
import { Effect } from "effect"
import { AudioRetentionService, dispositionClasses, MemoryPrivateObjectStore, sha256Hex, type RetainedSessionReceipt, type SegmentInput } from "../src/index.js"

const at = "2026-07-12T20:00:00.000Z"
const later = "2026-07-13T20:00:00.000Z"
const bytes = new TextEncoder().encode("bounded retained audio fixture")
const key = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"])
const receipt: RetainedSessionReceipt = {
  receiptId: "retained.fixture.v1", ownerRef: "owner.fixture", deviceRef: "device.fixture",
  threadRef: "thread.fixture", sessionRef: "session.fixture", generation: 1,
  policyVersion: "policy.v1", consentVersion: "consent.v1", keyEpoch: "epoch.fixture",
  acceptedAt: at, expiresAt: later,
}
const segment = async (overrides: Partial<SegmentInput> = {}): Promise<SegmentInput> => ({
  ownerRef: receipt.ownerRef, deviceRef: receipt.deviceRef, threadRef: receipt.threadRef,
  sessionRef: receipt.sessionRef, generation: receipt.generation, firstSequence: 0, lastSequence: 49,
  captureStartedAt: at, captureEndedAt: at, serverReceivedAt: at, codec: "audio/opus;rate=48000",
  dispositionClass: "raw_audio", bytes, digest: await sha256Hex(bytes), ...overrides,
})
const setup = (limits = { maxSegmentBytes: 1024, maxSessionBytes: 8192, maxSegmentsPerSession: 16 }) => {
  const store = new MemoryPrivateObjectStore()
  const service = new AudioRetentionService(store, new Map([[receipt.keyEpoch, key]]), limits, () => new Date(at))
  service.start(receipt)
  return { store, service }
}
const failure = async (effect: Effect.Effect<unknown, unknown>) => Effect.runPromise(Effect.flip(effect)) as Promise<{ reason: string }>

describe("AUDIO-3 retained object policy", () => {
  test("duplicate segment is idempotent and digest mismatch fails", async () => {
    const { service, store } = setup()
    const input = await segment()
    const first = await Effect.runPromise(service.accept(input))
    expect(await Effect.runPromise(service.accept(input))).toEqual(first)
    expect(store.objects.size).toBe(1)
    expect((await failure(service.accept({ ...input, digest: "0".repeat(64) }))).reason).toBe("digest_mismatch")
  })

  test("partial upload/outage records an explicit gap and accepts safe retry", async () => {
    const { service, store } = setup()
    store.available = false
    expect((await failure(service.accept(await segment()))).reason).toBe("storage_unavailable")
    expect(service.gaps.at(-1)?.reason).toBe("storage_outage")
    store.available = true
    expect((await Effect.runPromise(service.accept(await segment()))).segmentId).toContain("0-49")
  })

  test("generation fences permit concurrent identical sequence numbers", async () => {
    const { service } = setup()
    service.start({ ...receipt, receiptId: "retained.fixture.g2", generation: 2 })
    const [a, b] = await Promise.all([
      Effect.runPromise(service.accept(await segment())),
      Effect.runPromise(service.accept(await segment({ generation: 2 }))),
    ])
    expect(a.objectRef).not.toBe(b.objectRef)
  })

  test("TTL expiry and stop prohibit new acceptance", async () => {
    const { store } = setup()
    const expired = new AudioRetentionService(store, new Map([[receipt.keyEpoch, key]]), { maxSegmentBytes: 1024, maxSessionBytes: 8192, maxSegmentsPerSession: 16 }, () => new Date(later))
    expired.start(receipt)
    expect((await failure(expired.accept(await segment()))).reason).toBe("retention_not_active")
    const { service } = setup(); service.stop(receipt.sessionRef, receipt.generation)
    expect((await failure(service.accept(await segment()))).reason).toBe("retention_not_active")
  })

  test("export/delete and backup policy dispose every raw/derived class while retaining lawful SQL receipts only", async () => {
    const { service, store } = setup({ maxSegmentBytes: 1024, maxSessionBytes: 16384, maxSegmentsPerSession: 16 })
    for (const [index, dispositionClass] of dispositionClasses.entries()) {
      const value = new TextEncoder().encode(`fixture-${dispositionClass}`)
      await Effect.runPromise(service.accept(await segment({ firstSequence: index * 50, lastSequence: index * 50 + 49, dispositionClass, bytes: value, digest: await sha256Hex(value) })))
    }
    const exported = await Effect.runPromise(service.exportSession(receipt.sessionRef, receipt.ownerRef))
    expect(exported.receipt.dispositionClasses).toEqual(dispositionClasses)
    expect(exported.objects).toHaveLength(dispositionClasses.length)
    const deleted = await Effect.runPromise(service.disposeSession(receipt.sessionRef, receipt.ownerRef, "delete"))
    expect(deleted.dispositionClasses).toEqual(dispositionClasses)
    expect(deleted.remainingLawfulRecords).toEqual(["retained_session_receipt", "access_receipt"])
    expect(store.objects.size).toBe(0)
  })

  test("legal hold preserves objects until released", async () => {
    const { service, store } = setup()
    await Effect.runPromise(service.accept(await segment()))
    service.hold(receipt.sessionRef)
    expect((await failure(service.disposeSession(receipt.sessionRef, receipt.ownerRef, "delete"))).reason).toBe("legal_hold")
    expect(store.objects.size).toBe(1)
    service.releaseHold(receipt.sessionRef)
    expect((await Effect.runPromise(service.disposeSession(receipt.sessionRef, receipt.ownerRef, "expire"))).operation).toBe("expire")
  })

  test("quota fails closed and records sequence disposition", async () => {
    const { service, store } = setup({ maxSegmentBytes: 4, maxSessionBytes: 4, maxSegmentsPerSession: 1 })
    expect((await failure(service.accept(await segment()))).reason).toBe("quota_exceeded")
    expect(service.gaps.at(-1)?.reason).toBe("quota_refused")
    expect(store.objects.size).toBe(0)
  })

  test("reconciliation detects missing objects, orphans, and uncovered sequences", async () => {
    const { service, store } = setup()
    const manifest = await Effect.runPromise(service.accept(await segment({ firstSequence: 0, lastSequence: 9 })))
    service.recordGap({ sessionRef: receipt.sessionRef, generation: 1, firstSequence: 11, lastSequence: 12, reason: "transport_gap" })
    store.objects.delete(manifest.objectRef)
    store.objects.set(`owners/${receipt.ownerRef}/sessions/${receipt.sessionRef}/orphan.enc`, new Uint8Array([1]))
    expect(await Effect.runPromise(service.reconcile(receipt.sessionRef))).toEqual({
      missingObjects: [manifest.objectRef],
      orphanObjects: [`owners/${receipt.ownerRef}/sessions/${receipt.sessionRef}/orphan.enc`],
      uncoveredSequences: [10],
    })
  })
})
