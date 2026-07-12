import { SQL } from "bun"
import { RetentionError, type AccessReceipt, type GapManifest, type RetainedSessionReceipt, type SegmentManifest } from "./model.js"

/** Private Cloud SQL manifest adapter. Its API accepts metadata only. */
export class CloudSqlAudioRepository {
  readonly sql: SQL
  constructor(databaseUrl: string) { this.sql = new SQL({ url: databaseUrl, max: 4 }) }

  async close(): Promise<void> { await this.sql.close() }

  async saveSession(value: RetainedSessionReceipt): Promise<void> {
    await this.sql`INSERT INTO audio_retained_sessions
      (session_ref,generation,receipt_id,owner_ref,device_ref,thread_ref,policy_version,consent_version,key_epoch,accepted_at,expires_at)
      VALUES (${value.sessionRef},${value.generation},${value.receiptId},${value.ownerRef},${value.deviceRef},${value.threadRef},${value.policyVersion},${value.consentVersion},${value.keyEpoch},${value.acceptedAt},${value.expiresAt})
      ON CONFLICT (session_ref,generation) DO NOTHING`
  }

  async saveManifest(value: SegmentManifest): Promise<void> {
    const rows = await this.sql`INSERT INTO audio_segment_manifests
      (segment_id,session_ref,generation,first_sequence,last_sequence,digest_sha256,capture_started_at,capture_ended_at,server_received_at,codec,byte_length,object_ref,disposition_class,receipt_id,policy_version,consent_version,key_epoch,expires_at,deletion_state,exported_at)
      VALUES (${value.segmentId},${value.sessionRef},${value.generation},${value.firstSequence},${value.lastSequence},${value.digest},${value.captureStartedAt},${value.captureEndedAt},${value.serverReceivedAt},${value.codec},${value.byteLength},${value.objectRef},${value.dispositionClass},${value.receiptId},${value.policyVersion},${value.consentVersion},${value.keyEpoch},${value.expiresAt},${value.deletionState},${value.exportedAt ?? null})
      ON CONFLICT (segment_id) DO UPDATE SET exported_at=EXCLUDED.exported_at,deletion_state=EXCLUDED.deletion_state
      WHERE audio_segment_manifests.digest_sha256=EXCLUDED.digest_sha256
      RETURNING segment_id`
    if (rows.length !== 1) throw new RetentionError("digest_mismatch", "Cloud SQL segment identity already has a different digest")
  }

  async saveGap(value: GapManifest): Promise<void> {
    await this.sql`INSERT INTO audio_sequence_gaps
      (session_ref,generation,first_sequence,last_sequence,reason,recorded_at)
      VALUES (${value.sessionRef},${value.generation},${value.firstSequence},${value.lastSequence},${value.reason},${value.recordedAt})
      ON CONFLICT DO NOTHING`
  }

  async saveAccessReceipt(value: AccessReceipt): Promise<void> {
    await this.sql`INSERT INTO audio_access_receipts
      (receipt_id,operation,owner_ref,session_ref,occurred_at,disposition_classes,segment_ids,remaining_lawful_records)
      VALUES (${value.receiptId},${value.operation},${value.ownerRef},${value.sessionRef},${value.occurredAt},${JSON.stringify(value.dispositionClasses)},${JSON.stringify(value.segmentIds)},${JSON.stringify(value.remainingLawfulRecords)})
      ON CONFLICT (receipt_id) DO NOTHING`
  }

  async fixtureSummary(sessionRef: string): Promise<{ segments: number; gaps: number; active: number }> {
    const [row] = await this.sql`SELECT
      (SELECT count(*)::int FROM audio_segment_manifests WHERE session_ref=${sessionRef}) AS segments,
      (SELECT count(*)::int FROM audio_sequence_gaps WHERE session_ref=${sessionRef}) AS gaps,
      (SELECT count(*)::int FROM audio_segment_manifests WHERE session_ref=${sessionRef} AND deletion_state='active') AS active`
    return row as { segments: number; gaps: number; active: number }
  }
}
