import { Effect } from "effect"
import {
  AudioRetentionService,
  CloudSqlAudioRepository,
  dispositionClasses,
  GcsPrivateObjectStore,
  sha256Hex,
  type RetainedSessionReceipt,
} from "../src/index.js"

const required = (name: string): string => {
  const value = process.env[name]
  if (!value) throw new Error(`missing ${name}`)
  return value
}

const bucket = required("AUDIO_GCS_BUCKET")
const token = required("GOOGLE_OAUTH_ACCESS_TOKEN")
const databaseUrl = required("AUDIO_DATABASE_URL")
const runRef = `fixture-${Date.now()}-${crypto.randomUUID().slice(0, 8)}`
const now = new Date()
const expiresAt = new Date(now.getTime() + 60 * 60 * 1000)
const keyEpoch = `smoke-${runRef}`
const key = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"])
const receipt: RetainedSessionReceipt = {
  receiptId: `retained.${runRef}`,
  ownerRef: `owner.${runRef}`,
  deviceRef: `device.${runRef}`,
  threadRef: `thread.${runRef}`,
  sessionRef: `session.${runRef}`,
  generation: 1,
  policyVersion: "audio-retention.mvp.v1",
  consentVersion: "audio-consent.mvp.v1",
  keyEpoch,
  acceptedAt: now.toISOString(),
  expiresAt: expiresAt.toISOString(),
}
const repository = new CloudSqlAudioRepository(databaseUrl)
const objects = new GcsPrivateObjectStore(bucket, async () => token)
const service = new AudioRetentionService(objects, new Map([[keyEpoch, key]]), {
  maxSegmentBytes: 64 * 1024,
  maxSessionBytes: 1024 * 1024,
  maxSegmentsPerSession: 64,
}, () => now)

try {
  service.start(receipt)
  await repository.saveSession(receipt)
  const plaintexts: Uint8Array[] = []
  for (const [index, dispositionClass] of dispositionClasses.entries()) {
    const bytes = new TextEncoder().encode(`AUDIO-3 encrypted live fixture ${runRef} ${dispositionClass}`)
    plaintexts.push(bytes)
    const manifest = await Effect.runPromise(service.accept({
      ownerRef: receipt.ownerRef,
      deviceRef: receipt.deviceRef,
      threadRef: receipt.threadRef,
      sessionRef: receipt.sessionRef,
      generation: receipt.generation,
      firstSequence: index * 10,
      lastSequence: index * 10 + 9,
      captureStartedAt: now.toISOString(),
      captureEndedAt: now.toISOString(),
      serverReceivedAt: now.toISOString(),
      codec: dispositionClass.includes("audio") ? "audio/opus;rate=48000" : "application/openagents-audio-derived+json",
      dispositionClass,
      bytes,
      digest: await sha256Hex(bytes),
    }))
    await repository.saveManifest(manifest)
  }
  const reconciliation = await Effect.runPromise(service.reconcile(receipt.sessionRef))
  if (reconciliation.missingObjects.length || reconciliation.orphanObjects.length || reconciliation.uncoveredSequences.length) throw new Error("fixture reconciliation was not continuous")
  const exported = await Effect.runPromise(service.exportSession(receipt.sessionRef, receipt.ownerRef))
  if (exported.objects.some((bytes, index) => new TextDecoder().decode(bytes) !== new TextDecoder().decode(plaintexts[index]!))) throw new Error("export plaintext mismatch")
  await repository.saveAccessReceipt(exported.receipt)
  for (const manifest of service.manifests.values()) await repository.saveManifest(manifest)
  const deleted = await Effect.runPromise(service.disposeSession(receipt.sessionRef, receipt.ownerRef, "delete"))
  await repository.saveAccessReceipt(deleted)
  for (const manifest of service.manifests.values()) await repository.saveManifest(manifest)
  const summary = await repository.fixtureSummary(receipt.sessionRef)
  const remaining = await Effect.runPromise(objects.listRefs(`owners/${receipt.ownerRef}/sessions/${receipt.sessionRef}/`))
  if (summary.segments !== dispositionClasses.length || summary.gaps !== 0 || summary.active !== 0 || remaining.length !== 0) throw new Error("delete closeout did not reconcile")
  console.log(JSON.stringify({ ok: true, contract: "openagents.audio_retention.live_smoke.v1", segments: summary.segments, continuousSequences: dispositionClasses.length * 10, encryptedObjectsDeleted: dispositionClasses.length, dispositionClasses, remainingLawfulRecords: deleted.remainingLawfulRecords }))
} finally {
  await repository.close()
}
