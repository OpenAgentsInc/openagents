import { Effect } from "effect"
import { decryptEnvelope, encryptEnvelope, sha256Hex } from "./crypto.js"
import {
  dispositionClasses,
  RetentionError,
  type AccessReceipt,
  type DispositionClass,
  type GapManifest,
  type RetainedSessionReceipt,
  type SegmentInput,
  type SegmentManifest,
} from "./model.js"
import type { PrivateObjectStore } from "./storage.js"

export interface RetentionLimits {
  readonly maxSegmentBytes: number
  readonly maxSessionBytes: number
  readonly maxSegmentsPerSession: number
}

const keyOf = (sessionRef: string, generation: number) => `${sessionRef}:${generation}`
const rangesOverlap = (a: SegmentManifest, b: SegmentInput) => a.generation === b.generation && a.firstSequence <= b.lastSequence && b.firstSequence <= a.lastSequence

export class AudioRetentionService {
  readonly sessions = new Map<string, RetainedSessionReceipt>()
  readonly manifests = new Map<string, SegmentManifest>()
  readonly gaps: GapManifest[] = []
  readonly receipts: AccessReceipt[] = []
  readonly stopped = new Set<string>()
  readonly legalHolds = new Set<string>()

  constructor(
    readonly objects: PrivateObjectStore,
    readonly encryptionKeys: ReadonlyMap<string, CryptoKey>,
    readonly limits: RetentionLimits,
    readonly now: () => Date = () => new Date(),
  ) {}

  start(receipt: RetainedSessionReceipt): void {
    this.sessions.set(keyOf(receipt.sessionRef, receipt.generation), receipt)
  }

  stop(sessionRef: string, generation: number): void {
    this.stopped.add(keyOf(sessionRef, generation))
  }

  hold(sessionRef: string): void { this.legalHolds.add(sessionRef) }
  releaseHold(sessionRef: string): void { this.legalHolds.delete(sessionRef) }

  recordGap(gap: Omit<GapManifest, "recordedAt">): void {
    this.gaps.push({ ...gap, recordedAt: this.now().toISOString() })
  }

  accept(input: SegmentInput): Effect.Effect<SegmentManifest, RetentionError> {
    return Effect.tryPromise({
      try: async () => {
        const sessionKey = keyOf(input.sessionRef, input.generation)
        const receipt = this.sessions.get(sessionKey)
        if (!receipt || this.stopped.has(sessionKey) || this.now() >= new Date(receipt.expiresAt)) {
          this.recordGap({ sessionRef: input.sessionRef, generation: input.generation, firstSequence: input.firstSequence, lastSequence: input.lastSequence, reason: "policy_refused" })
          throw new RetentionError("retention_not_active", "explicit retained-session receipt is not active")
        }
        if (receipt.ownerRef !== input.ownerRef || receipt.deviceRef !== input.deviceRef || receipt.threadRef !== input.threadRef) {
          throw new RetentionError("identity_mismatch", "segment identity does not match retained-session receipt")
        }
        const actualDigest = await sha256Hex(input.bytes)
        if (actualDigest !== input.digest) throw new RetentionError("digest_mismatch", "segment digest mismatch")
        const segmentId = `${input.sessionRef}.${input.generation}.${input.firstSequence}-${input.lastSequence}`
        const existing = this.manifests.get(segmentId)
        if (existing) {
          if (existing.digest !== input.digest) throw new RetentionError("digest_mismatch", "duplicate segment has a different digest")
          return existing
        }
        const sessionManifests = [...this.manifests.values()].filter((manifest) => manifest.sessionRef === input.sessionRef && manifest.generation === input.generation && manifest.deletionState === "active")
        if (sessionManifests.some((manifest) => rangesOverlap(manifest, input))) throw new RetentionError("sequence_conflict", "accepted sequence range overlaps another segment")
        const totalBytes = sessionManifests.reduce((sum, manifest) => sum + manifest.byteLength, 0)
        if (input.bytes.byteLength > this.limits.maxSegmentBytes || totalBytes + input.bytes.byteLength > this.limits.maxSessionBytes || sessionManifests.length >= this.limits.maxSegmentsPerSession) {
          this.recordGap({ sessionRef: input.sessionRef, generation: input.generation, firstSequence: input.firstSequence, lastSequence: input.lastSequence, reason: "quota_refused" })
          throw new RetentionError("quota_exceeded", "retention quota refused segment before capture could become unbounded")
        }
        const key = this.encryptionKeys.get(receipt.keyEpoch)
        if (!key) throw new RetentionError("retention_not_active", "encryption key epoch unavailable")
        const objectRef = `owners/${receipt.ownerRef}/sessions/${input.sessionRef}/generations/${input.generation}/${input.dispositionClass}/${segmentId}.enc`
        const encrypted = await encryptEnvelope({ plaintext: input.bytes, key, keyEpoch: receipt.keyEpoch, objectRef })
        try {
          await Effect.runPromise(this.objects.putIfAbsent(objectRef, encrypted.bytes))
        } catch (error) {
          this.recordGap({ sessionRef: input.sessionRef, generation: input.generation, firstSequence: input.firstSequence, lastSequence: input.lastSequence, reason: "storage_outage" })
          throw error
        }
        const { bytes: _mediaBytes, ...manifestInput } = input
        const manifest: SegmentManifest = {
          ...manifestInput,
          segmentId,
          byteLength: input.bytes.byteLength,
          objectRef,
          receiptId: receipt.receiptId,
          policyVersion: receipt.policyVersion,
          consentVersion: receipt.consentVersion,
          keyEpoch: receipt.keyEpoch,
          expiresAt: receipt.expiresAt,
          deletionState: "active",
        }
        this.manifests.set(segmentId, manifest)
        return manifest
      },
      catch: (error) => error instanceof RetentionError ? error : new RetentionError("storage_unavailable", "segment acceptance failed closed"),
    })
  }

  read(segmentId: string, ownerRef: string): Effect.Effect<Uint8Array, RetentionError> {
    return Effect.tryPromise({
      try: async () => {
        const manifest = this.manifests.get(segmentId)
        if (!manifest || manifest.ownerRef !== ownerRef || manifest.deletionState !== "active") throw new RetentionError("not_found", "active segment not found")
        const key = this.encryptionKeys.get(manifest.keyEpoch)
        if (!key) throw new RetentionError("retention_not_active", "encryption key epoch unavailable")
        const result = await decryptEnvelope({ encrypted: await Effect.runPromise(this.objects.get(manifest.objectRef)), key, objectRef: manifest.objectRef })
        this.receipts.push(this.accessReceipt("read", ownerRef, manifest.sessionRef, [manifest]))
        return result
      },
      catch: (error) => error instanceof RetentionError ? error : new RetentionError("storage_unavailable", "private object read failed"),
    })
  }

  exportSession(sessionRef: string, ownerRef: string): Effect.Effect<{ receipt: AccessReceipt; objects: readonly Uint8Array[] }, RetentionError> {
    return Effect.tryPromise({
      try: async () => {
        const manifests = this.activeManifests(sessionRef, ownerRef)
        const objects = await Promise.all(manifests.map((manifest) => Effect.runPromise(this.read(manifest.segmentId, ownerRef))))
        const receipt = this.accessReceipt("export", ownerRef, sessionRef, manifests)
        this.receipts.push(receipt)
        for (const manifest of manifests) this.manifests.set(manifest.segmentId, { ...manifest, exportedAt: receipt.occurredAt })
        return { receipt, objects }
      },
      catch: (error) => error instanceof RetentionError ? error : new RetentionError("storage_unavailable", "export failed")
    })
  }

  disposeSession(sessionRef: string, ownerRef: string, operation: "delete" | "expire"): Effect.Effect<AccessReceipt, RetentionError> {
    return Effect.tryPromise({
      try: async () => {
        if (this.legalHolds.has(sessionRef)) throw new RetentionError("legal_hold", "lawful hold prohibits media deletion")
        const manifests = this.activeManifests(sessionRef, ownerRef)
        for (const manifest of manifests) {
          await Effect.runPromise(this.objects.delete(manifest.objectRef))
          this.manifests.set(manifest.segmentId, { ...manifest, deletionState: operation === "delete" ? "deleted" : "expired" })
        }
        const receipt = this.accessReceipt(operation, ownerRef, sessionRef, manifests)
        this.receipts.push(receipt)
        return receipt
      },
      catch: (error) => error instanceof RetentionError ? error : new RetentionError("storage_unavailable", "disposition failed")
    })
  }

  reconcile(sessionRef: string): Effect.Effect<{ missingObjects: readonly string[]; orphanObjects: readonly string[]; uncoveredSequences: readonly number[] }, RetentionError> {
    const service = this
    return Effect.gen(function* () {
      const manifests = [...service.manifests.values()].filter((manifest) => manifest.sessionRef === sessionRef && manifest.deletionState === "active")
      const refs = yield* service.objects.listRefs(`owners/${manifests[0]?.ownerRef ?? ""}/sessions/${sessionRef}/`)
      const expected = new Set(manifests.map(({ objectRef }) => objectRef))
      const actual = new Set(refs)
      const missingObjects = [...expected].filter((ref) => !actual.has(ref)).sort()
      const orphanObjects = [...actual].filter((ref) => !expected.has(ref)).sort()
      const covered = new Set<number>()
      for (const item of [...manifests, ...service.gaps.filter((gap) => gap.sessionRef === sessionRef)]) for (let sequence = item.firstSequence; sequence <= item.lastSequence; sequence++) covered.add(sequence)
      const first = Math.min(...covered)
      const last = Math.max(-1, ...covered)
      const uncoveredSequences = covered.size === 0 ? [] : Array.from({ length: last - first + 1 }, (_, offset) => first + offset).filter((sequence) => !covered.has(sequence))
      return { missingObjects, orphanObjects, uncoveredSequences }
    })
  }

  private activeManifests(sessionRef: string, ownerRef: string): SegmentManifest[] {
    return [...this.manifests.values()].filter((manifest) => manifest.sessionRef === sessionRef && manifest.ownerRef === ownerRef && manifest.deletionState === "active")
  }
  private accessReceipt(operation: AccessReceipt["operation"], ownerRef: string, sessionRef: string, manifests: readonly SegmentManifest[]): AccessReceipt {
    const present = new Set(manifests.map(({ dispositionClass }) => dispositionClass))
    return {
      receiptId: crypto.randomUUID(), operation, ownerRef, sessionRef, occurredAt: this.now().toISOString(),
      dispositionClasses: dispositionClasses.filter((kind) => present.has(kind)),
      segmentIds: manifests.map(({ segmentId }) => segmentId).sort(),
      remainingLawfulRecords: ["retained_session_receipt", "access_receipt"],
    }
  }
}
