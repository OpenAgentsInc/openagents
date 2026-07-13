import { createHash } from "node:crypto"

import {
  PORTABLE_CAPABILITY_BROKER_VERSION,
  type CapabilityBrokerAtomicCommit,
  type CapabilityBrokerAtomicLoad,
  type CapabilityBrokerAtomicStateStore,
  type CapabilityBrokerEvidence,
  type CapabilityBrokerPrivateDurableState,
} from "@openagentsinc/portable-session-contract"

import type { SyncSql, SyncTransactionSql } from "./sql.js"

export type PortableCapabilityMoveClaim = {
  readonly moveRef: string
  readonly commandRef: string
  readonly sourceAttachmentRef: string
  readonly sourceGeneration: number
  readonly destinationTargetRef: string
}

export type PortableCapabilityBrokerStoreScope = {
  readonly ownerRef: string
  readonly sessionRef: string
  readonly moveClaim: PortableCapabilityMoveClaim
}

export class PortableCapabilityBrokerStoreError extends Error {
  readonly _tag = "PortableCapabilityBrokerStoreError"
  override readonly name = "PortableCapabilityBrokerStoreError"

  constructor(
    readonly code:
      | "invalid"
      | "not_found"
      | "stale_revision"
      | "claim_conflict"
      | "claim_missing"
      | "unsafe_state",
    message: string,
  ) {
    super(message)
  }
}

type BrokerRow = {
  revision: string | number
  state_json: unknown
  active_move_ref: string | null
  active_move_fingerprint: string | null
  active_command_ref: string | null
  active_source_attachment_ref: string | null
  active_source_generation: string | number | null
  active_destination_target_ref: string | null
}

const safeRef = (value: string): boolean =>
  /^[a-zA-Z0-9][a-zA-Z0-9._:-]{2,255}$/.test(value)

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`
  }
  return JSON.stringify(value)
}

const claimFingerprint = (claim: PortableCapabilityMoveClaim): string =>
  `sha256:${createHash("sha256").update(canonical(claim)).digest("hex")}`

const forbiddenPrivateMaterial =
  /"(?:token|apiKey|authorization|sessionToken|refreshToken|mnemonic|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)/i

const assertScope = (scope: PortableCapabilityBrokerStoreScope): void => {
  const refs = [
    scope.ownerRef,
    scope.sessionRef,
    scope.moveClaim.moveRef,
    scope.moveClaim.commandRef,
    scope.moveClaim.sourceAttachmentRef,
    scope.moveClaim.destinationTargetRef,
  ]
  if (refs.some(ref => !safeRef(ref)) ||
      !Number.isInteger(scope.moveClaim.sourceGeneration) ||
      scope.moveClaim.sourceGeneration <= 0) {
    throw new PortableCapabilityBrokerStoreError("invalid", "portable broker scope is invalid")
  }
}

const parseJson = (value: unknown): unknown =>
  typeof value === "string" ? JSON.parse(value) : value

const decodeState = (value: unknown): CapabilityBrokerPrivateDurableState | null => {
  if (value === null || value === undefined) return null
  const parsed = parseJson(value)
  if (parsed === null || typeof parsed !== "object") {
    throw new PortableCapabilityBrokerStoreError("invalid", "portable broker state is not an object")
  }
  const state = parsed as CapabilityBrokerPrivateDurableState
  if (state.schema !== PORTABLE_CAPABILITY_BROKER_VERSION ||
      state.material !== "excluded" ||
      !Array.isArray(state.records) ||
      !Array.isArray(state.operations) ||
      !Array.isArray(state.evidence)) {
    throw new PortableCapabilityBrokerStoreError("invalid", "portable broker state schema is invalid")
  }
  assertPublicSafe(state)
  return state
}

const assertPublicSafe = (value: unknown): void => {
  if (forbiddenPrivateMaterial.test(canonical(value))) {
    throw new PortableCapabilityBrokerStoreError(
      "unsafe_state",
      "portable broker state contains forbidden private material",
    )
  }
}

const revisionOf = (value: string | number): number => {
  const revision = Number(value)
  if (!Number.isSafeInteger(revision) || revision < 0) {
    throw new PortableCapabilityBrokerStoreError("invalid", "portable broker revision is invalid")
  }
  return revision
}

const selectRow = async (
  sql: SyncTransactionSql | SyncSql,
  ownerRef: string,
  sessionRef: string,
  lock: boolean,
): Promise<BrokerRow | undefined> => {
  const rows: BrokerRow[] = lock
    ? await sql`
        SELECT revision, state_json, active_move_ref, active_move_fingerprint,
               active_command_ref, active_source_attachment_ref,
               active_source_generation, active_destination_target_ref
        FROM khala_sync_portable_capability_brokers
        WHERE owner_user_id = ${ownerRef} AND session_ref = ${sessionRef}
        FOR UPDATE
      `
    : await sql`
        SELECT revision, state_json, active_move_ref, active_move_fingerprint,
               active_command_ref, active_source_attachment_ref,
               active_source_generation, active_destination_target_ref
        FROM khala_sync_portable_capability_brokers
        WHERE owner_user_id = ${ownerRef} AND session_ref = ${sessionRef}
      `
  return rows[0]
}

export class PostgresPortableCapabilityBrokerStore
implements CapabilityBrokerAtomicStateStore {
  private readonly fingerprint: string
  private observedRevision: number | null = null

  constructor(
    private readonly sql: SyncSql,
    readonly scope: PortableCapabilityBrokerStoreScope,
  ) {
    assertScope(scope)
    this.fingerprint = claimFingerprint(scope.moveClaim)
  }

  /**
   * Read the aggregate revision before attempting claim acquisition. The
   * subsequent `acquireMoveClaim` transaction still performs the authoritative
   * CAS, so a concurrent claimant cannot be overwritten.
   */
  async readRevision(): Promise<number> {
    const row = await selectRow(this.sql, this.scope.ownerRef, this.scope.sessionRef, false)
    const revision = row === undefined ? 0 : revisionOf(row.revision)
    this.observedRevision = revision
    return revision
  }

  /** Latest revision observed by this exact store instance. */
  currentRevision(): number {
    if (this.observedRevision === null) {
      throw new PortableCapabilityBrokerStoreError(
        "invalid",
        "portable broker revision has not been observed",
      )
    }
    return this.observedRevision
  }

  async acquireMoveClaim(expectedRevision: number): Promise<{ readonly revision: number }> {
    const acquired = await this.sql.begin(async tx => {
      await tx`
        INSERT INTO khala_sync_portable_capability_brokers
          (owner_user_id, session_ref, revision)
        SELECT ${this.scope.ownerRef}, ${this.scope.sessionRef}, 0
        FROM khala_sync_portable_sessions
        WHERE session_ref = ${this.scope.sessionRef}
          AND owner_user_id = ${this.scope.ownerRef}
        ON CONFLICT (owner_user_id, session_ref) DO NOTHING
      `
      const row = await selectRow(tx, this.scope.ownerRef, this.scope.sessionRef, true)
      if (!row) {
        throw new PortableCapabilityBrokerStoreError("not_found", "portable session does not exist for owner")
      }
      const revision = revisionOf(row.revision)
      if (revision !== expectedRevision) {
        throw new PortableCapabilityBrokerStoreError("stale_revision", "portable broker claim revision is stale")
      }
      if (row.active_move_ref !== null) {
        if (this.matchesClaim(row)) {
          return { revision }
        }
        throw new PortableCapabilityBrokerStoreError("claim_conflict", "another portable move owns the broker")
      }
      const next = revision + 1
      await tx`
        UPDATE khala_sync_portable_capability_brokers
        SET revision = ${next},
            active_move_ref = ${this.scope.moveClaim.moveRef},
            active_move_fingerprint = ${this.fingerprint},
            active_command_ref = ${this.scope.moveClaim.commandRef},
            active_source_attachment_ref = ${this.scope.moveClaim.sourceAttachmentRef},
            active_source_generation = ${this.scope.moveClaim.sourceGeneration},
            active_destination_target_ref = ${this.scope.moveClaim.destinationTargetRef},
            claim_acquired_at = now(), updated_at = now()
        WHERE owner_user_id = ${this.scope.ownerRef}
          AND session_ref = ${this.scope.sessionRef}
          AND revision = ${revision}
      `
      return { revision: next }
    })
    this.observedRevision = acquired.revision
    return acquired
  }

  async load(): Promise<CapabilityBrokerAtomicLoad> {
    const row = await selectRow(this.sql, this.scope.ownerRef, this.scope.sessionRef, false)
    if (!row) return { revision: 0, state: null }
    if (!this.matchesClaim(row)) {
      throw new PortableCapabilityBrokerStoreError(
        row.active_move_ref === null ? "claim_missing" : "claim_conflict",
        "exact active portable move claim is required",
      )
    }
    const revision = revisionOf(row.revision)
    this.observedRevision = revision
    return { revision, state: decodeState(row.state_json) }
  }

  async commit(input: CapabilityBrokerAtomicCommit): Promise<{ readonly revision: number }> {
    assertPublicSafe(input.state)
    assertPublicSafe(input.evidence)
    if (input.state.schema !== PORTABLE_CAPABILITY_BROKER_VERSION ||
        input.state.material !== "excluded" ||
        input.evidence.schema !== PORTABLE_CAPABILITY_BROKER_VERSION ||
        input.evidence.ownerRef !== this.scope.ownerRef ||
        input.evidence.sessionRef !== this.scope.sessionRef) {
      throw new PortableCapabilityBrokerStoreError("invalid", "broker commit scope or schema mismatch")
    }
    const committed = await this.sql.begin(async tx => {
      const row = await selectRow(tx, this.scope.ownerRef, this.scope.sessionRef, true)
      if (!row) throw new PortableCapabilityBrokerStoreError("not_found", "portable broker aggregate is absent")
      const revision = revisionOf(row.revision)
      if (revision !== input.expectedRevision) {
        throw new PortableCapabilityBrokerStoreError("stale_revision", "portable broker commit revision is stale")
      }
      if (!this.matchesClaim(row)) {
        throw new PortableCapabilityBrokerStoreError(
          row.active_move_ref === null ? "claim_missing" : "claim_conflict",
          "portable broker commit lost its exact move claim",
        )
      }
      const next = revision + 1
      await tx`
        INSERT INTO khala_sync_portable_capability_evidence
          (owner_user_id, session_ref, evidence_ref, operation_ref,
           broker_revision, evidence_json)
        VALUES
          (${this.scope.ownerRef}, ${this.scope.sessionRef},
           ${input.evidence.evidenceRef}, ${input.evidence.operationRef},
           ${next}, ${JSON.stringify(input.evidence)}::jsonb)
      `
      await tx`
        UPDATE khala_sync_portable_capability_brokers
        SET revision = ${next}, state_json = ${JSON.stringify(input.state)}::jsonb,
            updated_at = now()
        WHERE owner_user_id = ${this.scope.ownerRef}
          AND session_ref = ${this.scope.sessionRef}
          AND revision = ${revision}
      `
      return { revision: next }
    })
    this.observedRevision = committed.revision
    return committed
  }

  async releaseMoveClaim(expectedRevision: number): Promise<{ readonly revision: number }> {
    const released = await this.sql.begin(async tx => {
      const row = await selectRow(tx, this.scope.ownerRef, this.scope.sessionRef, true)
      if (!row) throw new PortableCapabilityBrokerStoreError("not_found", "portable broker aggregate is absent")
      const revision = revisionOf(row.revision)
      if (revision !== expectedRevision) {
        throw new PortableCapabilityBrokerStoreError("stale_revision", "portable broker release revision is stale")
      }
      if (!this.matchesClaim(row)) {
        throw new PortableCapabilityBrokerStoreError(
          row.active_move_ref === null ? "claim_missing" : "claim_conflict",
          "portable broker release lost its exact move claim",
        )
      }
      const next = revision + 1
      await tx`
        UPDATE khala_sync_portable_capability_brokers
        SET revision = ${next}, active_move_ref = NULL,
            active_move_fingerprint = NULL, active_command_ref = NULL,
            active_source_attachment_ref = NULL,
            active_source_generation = NULL,
            active_destination_target_ref = NULL,
            claim_acquired_at = NULL, updated_at = now()
        WHERE owner_user_id = ${this.scope.ownerRef}
          AND session_ref = ${this.scope.sessionRef}
          AND revision = ${revision}
      `
      return { revision: next }
    })
    this.observedRevision = released.revision
    return released
  }

  private matchesClaim(row: BrokerRow): boolean {
    return row.active_move_ref === this.scope.moveClaim.moveRef &&
      row.active_move_fingerprint === this.fingerprint &&
      row.active_command_ref === this.scope.moveClaim.commandRef &&
      row.active_source_attachment_ref === this.scope.moveClaim.sourceAttachmentRef &&
      Number(row.active_source_generation) === this.scope.moveClaim.sourceGeneration &&
      row.active_destination_target_ref === this.scope.moveClaim.destinationTargetRef
  }
}

export const readPortableCapabilityEvidence = async (
  sql: SyncSql,
  ownerRef: string,
  sessionRef: string,
): Promise<ReadonlyArray<CapabilityBrokerEvidence>> => {
  const rows: ReadonlyArray<{ evidence_json: unknown }> = await sql`
    SELECT evidence_json
    FROM khala_sync_portable_capability_evidence
    WHERE owner_user_id = ${ownerRef} AND session_ref = ${sessionRef}
    ORDER BY broker_revision ASC
  `
  return rows.map(row => parseJson(row.evidence_json) as CapabilityBrokerEvidence)
}
