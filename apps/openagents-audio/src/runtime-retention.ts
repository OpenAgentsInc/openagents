import type { VoiceIdentity } from "@openagentsinc/audio-contract"
import { Effect } from "effect"
import { CloudSqlAudioRepository } from "./cloud-sql.js"
import { sha256Hex } from "./crypto.js"
import type { RetainedSessionReceipt } from "./model.js"
import type { AudioRetentionRuntime } from "./server.js"
import { AudioRetentionService } from "./service.js"
import { GcsPrivateObjectStore } from "./storage.js"

const POLICY = "audio-retention.mvp.v1"
const MAX_RETENTION_MS = 30 * 24 * 60 * 60 * 1_000

const googleAccessToken = async (): Promise<string> => {
  const response = await fetch("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token", {
    headers: { "metadata-flavor": "Google" },
  })
  if (!response.ok) throw new Error("google_adc_token_unavailable")
  const body = await response.json() as { access_token?: unknown }
  if (typeof body.access_token !== "string" || body.access_token.length < 16) throw new Error("google_adc_token_invalid")
  return body.access_token
}

const importEncryptionKey = async (encoded: string): Promise<CryptoKey> => {
  const bytes = Buffer.from(encoded, "base64")
  if (bytes.byteLength !== 32) throw new Error("audio_encryption_key_must_be_32_bytes")
  return crypto.subtle.importKey("raw", bytes, { name: "AES-GCM" }, false, ["encrypt", "decrypt"])
}

export const createProductionRetentionRuntime = async (input: Readonly<{
  bucket: string
  databaseUrl: string
  encryptionKeyBase64: string
  keyEpoch: string
  now?: () => Date
}>): Promise<AudioRetentionRuntime> => {
  const now = input.now ?? (() => new Date())
  const key = await importEncryptionKey(input.encryptionKeyBase64)
  const repository = new CloudSqlAudioRepository(input.databaseUrl)
  const service = new AudioRetentionService(
    new GcsPrivateObjectStore(input.bucket, googleAccessToken),
    new Map([[input.keyEpoch, key]]),
    { maxSegmentBytes: 24_000, maxSessionBytes: 1_843_200_000, maxSegmentsPerSession: 172_800 },
    now,
  )
  const receiptFor = (identity: VoiceIdentity): RetainedSessionReceipt => {
    const acceptedAt = now()
    return {
      receiptId: `retained.${identity.sessionRef}.${identity.generation}`,
      ownerRef: identity.ownerRef,
      deviceRef: identity.deviceRef,
      threadRef: identity.threadRef,
      sessionRef: identity.sessionRef,
      generation: identity.generation,
      policyVersion: POLICY,
      consentVersion: POLICY,
      keyEpoch: input.keyEpoch,
      acceptedAt: acceptedAt.toISOString(),
      expiresAt: new Date(acceptedAt.getTime() + MAX_RETENTION_MS).toISOString(),
    }
  }
  const saveNewGaps = async (before: number): Promise<void> => {
    for (const gap of service.gaps.slice(before)) await repository.saveGap(gap)
  }
  return {
    admit: async (identity) => {
      const receipt = receiptFor(identity)
      service.start(receipt)
      await repository.saveSession(receipt)
      return {
        receipt: { receiptRef: receipt.receiptId, expiresAtMs: Date.parse(receipt.expiresAt) },
        accept: async (frame) => {
          const gapCount = service.gaps.length
          try {
            const receivedAt = now().toISOString()
            const manifest = await Effect.runPromise(service.accept({
              ownerRef: identity.ownerRef, deviceRef: identity.deviceRef, threadRef: identity.threadRef,
              sessionRef: identity.sessionRef, generation: identity.generation,
              firstSequence: frame.sequence, lastSequence: frame.sequence,
              captureStartedAt: receivedAt, captureEndedAt: receivedAt, serverReceivedAt: receivedAt,
              codec: `${frame.codec};rate=${frame.sampleRateHz}`, dispositionClass: "raw_audio",
              bytes: frame.payload, digest: frame.sha256,
            }))
            await repository.saveManifest(manifest)
          } finally { await saveNewGaps(gapCount) }
        },
        gap: async (firstSequence, lastSequence) => {
          if (lastSequence < firstSequence) return
          const before = service.gaps.length
          service.recordGap({ sessionRef: identity.sessionRef, generation: identity.generation, firstSequence, lastSequence, reason: "transport_gap" })
          await saveNewGaps(before)
        },
        stop: async () => { service.stop(identity.sessionRef, identity.generation) },
      }
    },
    reconcile: async (identity) => Effect.runPromise(service.reconcile(identity.sessionRef)),
    exportSession: async (identity) => {
      const exported = await Effect.runPromise(service.exportSession(identity.sessionRef, identity.ownerRef))
      await repository.saveAccessReceipt(exported.receipt)
      for (const manifest of service.manifests.values()) if (manifest.sessionRef === identity.sessionRef) await repository.saveManifest(manifest)
      return { receipt: exported.receipt, objectCount: exported.objects.length }
    },
    deleteSession: async (identity) => {
      const receipt = await Effect.runPromise(service.disposeSession(identity.sessionRef, identity.ownerRef, "delete"))
      await repository.saveAccessReceipt(receipt)
      for (const manifest of service.manifests.values()) if (manifest.sessionRef === identity.sessionRef) await repository.saveManifest(manifest)
      return { receipt }
    },
    close: () => repository.close(),
  }
}
