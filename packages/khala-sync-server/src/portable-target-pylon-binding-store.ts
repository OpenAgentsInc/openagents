import { createHash } from "node:crypto";

import { Schema } from "effect";

import type { PortableCommandPylonBindingResolver } from "./portable-session-command-runner.js";
import type { SqlTag, SyncSql } from "./sql.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const SHA256 = /^sha256:[0-9a-f]{64}$/u;
const DEFAULT_TTL_MS = 5 * 60 * 1_000;

export type PortableTargetPylonBindingHealth = "ready" | "draining" | "offline" | "revoked";

export type PortableTargetPylonBindingRecord = Readonly<{
  bindingRef: string;
  ownerUserId: string;
  ownerAgentUserId: string;
  sessionRef: string;
  targetRef: string;
  pylonRef: string;
  workerInstanceRef: string;
  bindingDigest: string;
  revision: number;
  state: "active" | "revoked";
  health: PortableTargetPylonBindingHealth;
  evidenceRefs: ReadonlyArray<string>;
  lastRenewedAt: string;
  expiresAt: string;
  revokedAt: string | null;
  createdAt: string;
  updatedAt: string;
}>;

export type PortableTargetPylonBindingWrite = Readonly<{
  idempotencyKeyHash: string;
  ownerUserId: string;
  ownerAgentUserId: string;
  sessionRef: string;
  targetRef: string;
  pylonRef: string;
  workerInstanceRef: string;
  bindingDigest: string;
  health: "ready" | "draining";
  evidenceRefs: ReadonlyArray<string>;
  expectedRevision?: number;
}>;

export class PortableTargetPylonBindingError extends Schema.TaggedErrorClass<PortableTargetPylonBindingError>()(
  "PortableTargetPylonBindingError",
  {
    reason: Schema.Literals([
      "invalid",
      "not_found",
      "conflict",
      "stale_revision",
      "authority_mismatch",
      "unavailable",
      "expired",
    ]),
  },
) {}

type BindingRow = {
  binding_ref: string;
  owner_user_id: string;
  owner_agent_user_id: string;
  session_ref: string;
  target_ref: string;
  pylon_ref: string;
  worker_instance_ref: string;
  binding_digest: string;
  revision: string | number;
  state: "active" | "revoked";
  health: PortableTargetPylonBindingHealth;
  evidence_refs_json: unknown;
  last_renewed_at: Date | string;
  expires_at: Date | string;
  revoked_at: Date | string | null;
  created_at: Date | string;
  updated_at: Date | string;
};

type ReplayRow = {
  binding_ref: string;
  owner_user_id: string;
  owner_agent_user_id: string;
  session_ref: string;
  target_ref: string;
  pylon_ref: string;
  worker_instance_ref: string;
  binding_digest: string;
  event_kind: string;
  health: PortableTargetPylonBindingHealth;
  evidence_refs_json: unknown;
};

const iso = (value: Date | string): string =>
  value instanceof Date ? value.toISOString() : new Date(value).toISOString();

const refs = (value: unknown): ReadonlyArray<string> => {
  let decoded = value;
  if (typeof value === "string") {
    try {
      decoded = JSON.parse(value);
    } catch {
      throw new PortableTargetPylonBindingError({ reason: "unavailable" });
    }
  }
  if (
    !Array.isArray(decoded) ||
    decoded.some((item) => typeof item !== "string" || !SAFE_REF.test(item))
  ) {
    throw new PortableTargetPylonBindingError({ reason: "unavailable" });
  }
  return decoded.filter((item): item is string => typeof item === "string");
};

const record = (row: BindingRow): PortableTargetPylonBindingRecord => ({
  bindingRef: row.binding_ref,
  ownerUserId: row.owner_user_id,
  ownerAgentUserId: row.owner_agent_user_id,
  sessionRef: row.session_ref,
  targetRef: row.target_ref,
  pylonRef: row.pylon_ref,
  workerInstanceRef: row.worker_instance_ref,
  bindingDigest: row.binding_digest,
  revision: Number(row.revision),
  state: row.state,
  health: row.health,
  evidenceRefs: refs(row.evidence_refs_json),
  lastRenewedAt: iso(row.last_renewed_at),
  expiresAt: iso(row.expires_at),
  revokedAt: row.revoked_at === null ? null : iso(row.revoked_at),
  createdAt: iso(row.created_at),
  updatedAt: iso(row.updated_at),
});

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex")}`;

const assertWrite = (input: PortableTargetPylonBindingWrite): void => {
  if (
    ![
      input.ownerUserId,
      input.ownerAgentUserId,
      input.sessionRef,
      input.targetRef,
      input.pylonRef,
      input.workerInstanceRef,
      ...input.evidenceRefs,
    ].every((value) => SAFE_REF.test(value)) ||
    !SHA256.test(input.bindingDigest) ||
    !SHA256.test(input.idempotencyKeyHash) ||
    new Set(input.evidenceRefs).size !== input.evidenceRefs.length ||
    (input.expectedRevision !== undefined &&
      (!Number.isSafeInteger(input.expectedRevision) || input.expectedRevision < 1))
  )
    throw new PortableTargetPylonBindingError({ reason: "invalid" });
};

const replayMatches = (
  replay: ReplayRow,
  input: PortableTargetPylonBindingWrite,
  eventKind: "admit" | "revoke",
): boolean =>
  (eventKind === "revoke" ? replay.event_kind === "revoked" : replay.event_kind !== "revoked") &&
  replay.owner_user_id === input.ownerUserId &&
  replay.owner_agent_user_id === input.ownerAgentUserId &&
  replay.session_ref === input.sessionRef &&
  replay.target_ref === input.targetRef &&
  replay.pylon_ref === input.pylonRef &&
  replay.worker_instance_ref === input.workerInstanceRef &&
  replay.binding_digest === input.bindingDigest &&
  (eventKind === "revoke" || replay.health === input.health) &&
  JSON.stringify(refs(replay.evidence_refs_json)) === JSON.stringify(input.evidenceRefs);

export class PostgresPortableTargetPylonBindingStore {
  constructor(
    private readonly sql: SyncSql,
    private readonly now: () => string = () => new Date().toISOString(),
    private readonly ttlMs: number = DEFAULT_TTL_MS,
  ) {
    if (!Number.isSafeInteger(ttlMs) || ttlMs < 30_000 || ttlMs > DEFAULT_TTL_MS) {
      throw new PortableTargetPylonBindingError({ reason: "invalid" });
    }
  }

  async admit(input: PortableTargetPylonBindingWrite): Promise<PortableTargetPylonBindingRecord> {
    assertWrite(input);
    const now = new Date(this.now());
    if (!Number.isFinite(now.getTime()))
      throw new PortableTargetPylonBindingError({ reason: "invalid" });
    const expiresAt = new Date(now.getTime() + this.ttlMs);
    return this.sql.begin(async (tx) => {
      const replay: ReplayRow[] = await tx`
        SELECT binding_ref,owner_user_id,owner_agent_user_id,session_ref,target_ref,pylon_ref,
          worker_instance_ref,binding_digest,event_kind,health,evidence_refs_json
        FROM khala_sync_portable_target_pylon_binding_events
        WHERE idempotency_key_hash = ${input.idempotencyKeyHash}
        LIMIT 1`;
      if (replay[0] !== undefined) {
        const current = await this.readWith(
          tx,
          input.ownerUserId,
          input.sessionRef,
          input.targetRef,
        );
        if (
          !replayMatches(replay[0], input, "admit") ||
          current === undefined ||
          current.bindingRef !== replay[0].binding_ref
        ) {
          throw new PortableTargetPylonBindingError({ reason: "conflict" });
        }
        return current;
      }

      await this.assertAuthority(tx, input, now);
      const rows: BindingRow[] = await tx`
        SELECT * FROM khala_sync_portable_target_pylon_bindings
        WHERE owner_user_id = ${input.ownerUserId}
          AND session_ref = ${input.sessionRef}
          AND target_ref = ${input.targetRef}
        FOR UPDATE`;
      const current = rows[0] === undefined ? undefined : record(rows[0]);
      const exact =
        current !== undefined &&
        current.pylonRef === input.pylonRef &&
        current.ownerAgentUserId === input.ownerAgentUserId &&
        current.workerInstanceRef === input.workerInstanceRef &&
        current.bindingDigest === input.bindingDigest;
      if (
        current !== undefined &&
        current.state === "active" &&
        new Date(current.expiresAt) > now &&
        !exact
      ) {
        throw new PortableTargetPylonBindingError({ reason: "conflict" });
      }
      if (current !== undefined && input.expectedRevision !== current.revision) {
        throw new PortableTargetPylonBindingError({ reason: "stale_revision" });
      }
      if (current === undefined && input.expectedRevision !== undefined) {
        throw new PortableTargetPylonBindingError({ reason: "stale_revision" });
      }
      const bindingRef =
        current?.bindingRef ??
        stableRef(
          "binding.portable-target-pylon",
          `${input.ownerUserId}\0${input.sessionRef}\0${input.targetRef}`,
        );
      const revision = (current?.revision ?? 0) + 1;
      const kind = current === undefined ? "admitted" : exact ? "renewed" : "rebound";
      const evidenceJson = JSON.stringify(input.evidenceRefs);
      await tx`
        INSERT INTO khala_sync_portable_target_pylon_bindings
          (binding_ref, owner_user_id, owner_agent_user_id, session_ref, target_ref,
           pylon_ref, worker_instance_ref, binding_digest, revision, state, health,
           evidence_refs_json, last_renewed_at, expires_at, revoked_at, created_at, updated_at)
        VALUES (${bindingRef}, ${input.ownerUserId}, ${input.ownerAgentUserId}, ${input.sessionRef},
          ${input.targetRef}, ${input.pylonRef}, ${input.workerInstanceRef}, ${input.bindingDigest},
          ${revision}, 'active', ${input.health}, ${evidenceJson}::jsonb, ${now}, ${expiresAt},
          NULL, ${current === undefined ? now : new Date(current.createdAt)}, ${now})
        ON CONFLICT (owner_user_id, session_ref, target_ref) DO UPDATE SET
          owner_agent_user_id = EXCLUDED.owner_agent_user_id,
          pylon_ref = EXCLUDED.pylon_ref,
          worker_instance_ref = EXCLUDED.worker_instance_ref,
          binding_digest = EXCLUDED.binding_digest,
          revision = EXCLUDED.revision,
          state = 'active', health = EXCLUDED.health,
          evidence_refs_json = EXCLUDED.evidence_refs_json,
          last_renewed_at = EXCLUDED.last_renewed_at,
          expires_at = EXCLUDED.expires_at,
          revoked_at = NULL, updated_at = EXCLUDED.updated_at`;
      await tx`
        INSERT INTO khala_sync_portable_target_pylon_binding_events
          (event_ref, idempotency_key_hash, binding_ref, revision, owner_user_id,
           owner_agent_user_id, session_ref, target_ref, pylon_ref, worker_instance_ref,
           binding_digest, event_kind, health, evidence_refs_json, created_at)
        VALUES (${stableRef("event.portable-target-pylon", input.idempotencyKeyHash)},
          ${input.idempotencyKeyHash}, ${bindingRef}, ${revision}, ${input.ownerUserId},
          ${input.ownerAgentUserId}, ${input.sessionRef}, ${input.targetRef}, ${input.pylonRef},
          ${input.workerInstanceRef}, ${input.bindingDigest}, ${kind}, ${input.health},
          ${evidenceJson}::jsonb, ${now})`;
      const stored = await this.readWith(tx, input.ownerUserId, input.sessionRef, input.targetRef);
      if (stored === undefined)
        throw new PortableTargetPylonBindingError({ reason: "unavailable" });
      return stored;
    });
  }

  async revoke(input: PortableTargetPylonBindingWrite): Promise<PortableTargetPylonBindingRecord> {
    assertWrite(input);
    if (input.expectedRevision === undefined) {
      throw new PortableTargetPylonBindingError({ reason: "stale_revision" });
    }
    const now = new Date(this.now());
    return this.sql.begin(async (tx) => {
      const replay: ReplayRow[] = await tx`
        SELECT binding_ref,owner_user_id,owner_agent_user_id,session_ref,target_ref,pylon_ref,
          worker_instance_ref,binding_digest,event_kind,health,evidence_refs_json
        FROM khala_sync_portable_target_pylon_binding_events
        WHERE idempotency_key_hash=${input.idempotencyKeyHash}
        LIMIT 1`;
      if (replay[0] !== undefined) {
        const current = await this.readWith(
          tx,
          input.ownerUserId,
          input.sessionRef,
          input.targetRef,
        );
        if (
          !replayMatches(replay[0], input, "revoke") ||
          current === undefined ||
          current.bindingRef !== replay[0].binding_ref
        )
          throw new PortableTargetPylonBindingError({ reason: "conflict" });
        return current;
      }
      const rows: BindingRow[] = await tx`
        SELECT * FROM khala_sync_portable_target_pylon_bindings
        WHERE owner_user_id=${input.ownerUserId} AND session_ref=${input.sessionRef}
          AND target_ref=${input.targetRef} FOR UPDATE`;
      const current = rows[0] === undefined ? undefined : record(rows[0]);
      if (current === undefined) throw new PortableTargetPylonBindingError({ reason: "not_found" });
      if (current.revision !== input.expectedRevision || current.pylonRef !== input.pylonRef) {
        throw new PortableTargetPylonBindingError({ reason: "stale_revision" });
      }
      if (current.state === "revoked") return current;
      const revision = current.revision + 1;
      await tx`UPDATE khala_sync_portable_target_pylon_bindings SET
        revision=${revision}, state='revoked', health='revoked', revoked_at=${now}, updated_at=${now}
        WHERE binding_ref=${current.bindingRef}`;
      await tx`INSERT INTO khala_sync_portable_target_pylon_binding_events
        (event_ref,idempotency_key_hash,binding_ref,revision,owner_user_id,owner_agent_user_id,
         session_ref,target_ref,pylon_ref,worker_instance_ref,binding_digest,event_kind,health,
         evidence_refs_json,created_at)
        VALUES (${stableRef("event.portable-target-pylon", input.idempotencyKeyHash)},
          ${input.idempotencyKeyHash},${current.bindingRef},${revision},${input.ownerUserId},
          ${input.ownerAgentUserId},${input.sessionRef},${input.targetRef},${input.pylonRef},
          ${input.workerInstanceRef},${input.bindingDigest},'revoked','revoked',
          ${JSON.stringify(input.evidenceRefs)}::jsonb,${now})`;
      const stored = await this.readWith(tx, input.ownerUserId, input.sessionRef, input.targetRef);
      if (stored === undefined)
        throw new PortableTargetPylonBindingError({ reason: "unavailable" });
      return stored;
    });
  }

  async read(ownerUserId: string, sessionRef: string, targetRef: string) {
    return this.readWith(this.sql, ownerUserId, sessionRef, targetRef);
  }

  async resolveActive(ownerUserId: string, sessionRef: string, targetRef: string) {
    const now = new Date(this.now());
    const staleBefore = new Date(now.getTime() - DEFAULT_TTL_MS);
    const rows: BindingRow[] = await this.sql`
      SELECT binding.*
      FROM khala_sync_portable_target_pylon_bindings binding
      JOIN khala_sync_portable_targets target
        ON target.target_ref=binding.target_ref AND target.owner_user_id=binding.owner_user_id
      JOIN khala_sync_portable_session_targets allowed
        ON allowed.target_ref=binding.target_ref AND allowed.session_ref=binding.session_ref
      JOIN khala_sync_portable_sessions session
        ON session.session_ref=binding.session_ref AND session.owner_user_id=binding.owner_user_id
      JOIN pylon_registrations pylon ON pylon.pylon_ref=binding.pylon_ref
      WHERE binding.owner_user_id=${ownerUserId}
        AND binding.session_ref=${sessionRef}
        AND binding.target_ref=${targetRef}
        AND binding.state='active'
        AND binding.health IN ('ready','draining')
        AND binding.expires_at>${now}
        AND target.health='ready'
        AND pylon.owner_agent_user_id=binding.owner_agent_user_id
        AND pylon.status='active' AND pylon.archived_at IS NULL
        AND lower(coalesce(pylon.latest_heartbeat_status,''))='online'
        AND pylon.latest_heartbeat_at IS NOT NULL
        AND pylon.latest_heartbeat_at::timestamptz>=${staleBefore}
        AND NOT EXISTS (SELECT 1 FROM pylon_quarantines quarantine
          WHERE quarantine.pylon_ref=pylon.pylon_ref AND quarantine.state='active'
            AND quarantine.released_at IS NULL
            AND (quarantine.expires_at IS NULL OR quarantine.expires_at::timestamptz>${now}))
      LIMIT 1`;
    return rows[0] === undefined ? undefined : record(rows[0]);
  }

  private async readWith(sql: SqlTag, ownerUserId: string, sessionRef: string, targetRef: string) {
    const rows: BindingRow[] = await sql`SELECT * FROM khala_sync_portable_target_pylon_bindings
      WHERE owner_user_id=${ownerUserId} AND session_ref=${sessionRef} AND target_ref=${targetRef}`;
    return rows[0] === undefined ? undefined : record(rows[0]);
  }

  private async assertAuthority(sql: SqlTag, input: PortableTargetPylonBindingWrite, now: Date) {
    const staleBefore = new Date(now.getTime() - DEFAULT_TTL_MS);
    const rows: Array<{ ok: boolean }> = await sql`
      SELECT true AS ok
      FROM khala_sync_portable_targets target
      JOIN khala_sync_portable_session_targets allowed
        ON allowed.target_ref=target.target_ref AND allowed.session_ref=${input.sessionRef}
      JOIN khala_sync_portable_sessions session ON session.session_ref=allowed.session_ref
      JOIN pylon_registrations pylon ON pylon.pylon_ref=${input.pylonRef}
      WHERE target.target_ref=${input.targetRef}
        AND target.owner_user_id=${input.ownerUserId}
        AND target.health='ready'
        AND session.owner_user_id=${input.ownerUserId}
        AND pylon.owner_agent_user_id=${input.ownerAgentUserId}
        AND pylon.status='active' AND pylon.archived_at IS NULL
        AND lower(coalesce(pylon.latest_heartbeat_status,''))='online'
        AND pylon.latest_heartbeat_at IS NOT NULL
        AND pylon.latest_heartbeat_at::timestamptz >= ${staleBefore}
        AND NOT EXISTS (SELECT 1 FROM pylon_quarantines quarantine
          WHERE quarantine.pylon_ref=pylon.pylon_ref AND quarantine.state='active'
            AND quarantine.released_at IS NULL
            AND (quarantine.expires_at IS NULL OR quarantine.expires_at::timestamptz>${now}))
      LIMIT 1`;
    if (rows[0] === undefined)
      throw new PortableTargetPylonBindingError({ reason: "authority_mismatch" });
  }
}

export class PostgresPortableCommandPylonBindingResolver implements PortableCommandPylonBindingResolver {
  constructor(
    private readonly store: PostgresPortableTargetPylonBindingStore,
    private readonly now: () => string = () => new Date().toISOString(),
  ) {}

  async resolve(scope: Parameters<PortableCommandPylonBindingResolver["resolve"]>[0]) {
    const binding = await this.store.resolveActive(
      scope.ownerRef,
      scope.sessionRef,
      scope.targetRef,
    );
    if (binding === undefined) throw new PortableTargetPylonBindingError({ reason: "not_found" });
    if (
      binding.state !== "active" ||
      !["ready", "draining"].includes(binding.health) ||
      new Date(binding.expiresAt) <= new Date(this.now())
    )
      throw new PortableTargetPylonBindingError({ reason: "expired" });
    return {
      commandExecutionClaimRef: scope.commandExecutionClaimRef,
      ownerRef: scope.ownerRef,
      targetRef: scope.targetRef,
      pylonRef: binding.pylonRef,
    };
  }
}
