import { createHash } from "node:crypto";

import { Context, Effect, Layer, Semaphore } from "effect";

import { ForgeGitDatabase } from "./database.js";
import { ForgeGitConfiguration } from "./config.js";
import { ForgeGitAdmissionError, type ForgeGitSignedRefPolicy } from "./model.js";

export type ForgeGitAdmissionEvent = Readonly<{
  readonly createdAt: string;
  readonly eventId: string;
  readonly kind: 30617 | 30618 | 1617 | 1618 | 1619;
  readonly objectIds: ReadonlyArray<string>;
  readonly repositoryRef: string;
  readonly tenantRef: string;
}>;

export type ForgeGitAdmittedRepository = Readonly<{
  readonly admittedBindingRef: string;
  readonly announcementAuthorPubkey: string;
  readonly announcementEventId: string;
  readonly maintainerPubkeys: ReadonlyArray<string>;
  readonly repositoryRef: string;
  readonly tenantRef: string;
}>;

export type ForgeGitAuthorizedRefState = ForgeGitSignedRefPolicy &
  Readonly<{
    readonly authorPubkey: string;
    readonly eventJson: string;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }>;

export type ForgeGitPurgatoryEvent = ForgeGitAdmissionEvent &
  Readonly<{
    readonly actorBindingRef: string;
    readonly eventJson: string;
    readonly expiresAt: string;
  }>;

export interface ForgeGitAdmissionShape {
  /** Serializes a stock receive-pack operation for one bare repository. */
  readonly withReceiveLease: <A, E, R>(
    repositoryKey: string,
    effect: Effect.Effect<A, E, R>,
  ) => Effect.Effect<A, E | ForgeGitAdmissionError, R>;
  /** Refuses all Git traffic until a verified, invited-maintainer 30617 exists. */
  readonly requireRepository: (input: {
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<void, ForgeGitAdmissionError>;
  /**
   * Returns only current 30618 facts. A receive hook consumes these exact
   * old/new/ref triples; it never accepts an inferred or stale ref move.
   */
  readonly signedRefPolicies: (input: {
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<ReadonlyArray<ForgeGitSignedRefPolicy>, ForgeGitAdmissionError>;
  /**
   * Stores a relay-visible publication job only after stock Git has accepted
   * the update. Relay publication is derived/retryable and cannot split ref
   * truth from the bare repository.
   */
  readonly recordCommittedReceive: (input: {
    readonly repositoryRef: string;
    readonly stateEventIds: ReadonlyArray<string>;
    readonly tenantRef: string;
  }) => Effect.Effect<void, ForgeGitAdmissionError>;
  /** Called only by the trusted relay/admission projector after signature and membership proof. */
  readonly admitRepository: (
    input: ForgeGitAdmittedRepository,
  ) => Effect.Effect<void, ForgeGitAdmissionError>;
  /** Stores the latest exact signed state before the corresponding Git push. */
  readonly authorizeSignedRefState: (
    input: ForgeGitAuthorizedRefState,
  ) => Effect.Effect<void, ForgeGitAdmissionError>;
  readonly holdPurgatory: (
    input: ForgeGitPurgatoryEvent,
  ) => Effect.Effect<void, ForgeGitAdmissionError>;
  readonly listPurgatory: (input: {
    readonly nowIso: string;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<ReadonlyArray<ForgeGitPurgatoryEvent>, ForgeGitAdmissionError>;
  readonly resolvePurgatory: (input: {
    readonly eventId: string;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<void, ForgeGitAdmissionError>;
  readonly expirePurgatory: (input: {
    readonly nowIso: string;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<number, ForgeGitAdmissionError>;
  readonly listAdmittedRepositories: () => Effect.Effect<
    ReadonlyArray<Readonly<{ repositoryRef: string; tenantRef: string }>>,
    ForgeGitAdmissionError
  >;
  readonly recordUnclaimedNostrRefs: (input: {
    readonly refNames: ReadonlyArray<string>;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<void, ForgeGitAdmissionError>;
  readonly claimNostrEvent: (input: {
    readonly eventId: string;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<void, ForgeGitAdmissionError>;
  readonly dueNostrRefGc: (input: {
    readonly nowIso: string;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<ReadonlyArray<string>, ForgeGitAdmissionError>;
  readonly markNostrRefsDeleted: (input: {
    readonly refNames: ReadonlyArray<string>;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<void, ForgeGitAdmissionError>;
}

export class ForgeGitAdmission extends Context.Service<ForgeGitAdmission, ForgeGitAdmissionShape>()(
  "@openagentsinc/forge-git-service/Admission",
) {}

const notAdmitted = () =>
  new ForgeGitAdmissionError({
    code: "forge_git_repository_not_admitted",
    status: 403,
  });

const admissionUnavailable = () =>
  new ForgeGitAdmissionError({
    code: "forge_git_admission_unavailable",
    status: 503,
  });

const advisoryLockKey = (repositoryKey: string): string => {
  const digest = createHash("sha256").update(repositoryKey).digest();
  return BigInt.asIntN(64, digest.readBigUInt64BE(0)).toString();
};

const serialLease = () => {
  const leases = new Map<string, Semaphore.Semaphore>();
  return <A, E, R>(repositoryKey: string, effect: Effect.Effect<A, E, R>) => {
    const semaphore = leases.get(repositoryKey) ?? Semaphore.makeUnsafe(1);
    leases.set(repositoryKey, semaphore);
    return semaphore.withPermit(effect);
  };
};

/**
 * Strict in-memory admission layer for focused tests. Callers explicitly
 * supply the repository/state facts they need; there is no permissive default.
 */
export const makeMemoryAdmissionLayer = (input: {
  readonly admittedRepositories: ReadonlyArray<
    Readonly<{ repositoryRef: string; tenantRef: string }>
  >;
  readonly signedRefPolicies?: ReadonlyArray<
    ForgeGitSignedRefPolicy & Readonly<{ repositoryRef: string; tenantRef: string }>
  >;
}): Layer.Layer<ForgeGitAdmission> => {
  const withReceiveLease = serialLease();
  const admitted = new Set(
    input.admittedRepositories.map((item) => `${item.tenantRef}/${item.repositoryRef}`),
  );
  const policies = (input.signedRefPolicies ?? []) as Array<
    ForgeGitSignedRefPolicy & Readonly<{ repositoryRef: string; tenantRef: string }>
  >;
  return Layer.succeed(
    ForgeGitAdmission,
    ForgeGitAdmission.of({
      withReceiveLease,
      requireRepository: Effect.fn("ForgeGitAdmission.memory.requireRepository")(function* (value) {
        if (!admitted.has(`${value.tenantRef}/${value.repositoryRef}`)) return yield* notAdmitted();
      }),
      signedRefPolicies: Effect.fn("ForgeGitAdmission.memory.signedRefPolicies")(function* (value) {
        return policies
          .filter(
            (policy) =>
              policy.tenantRef === value.tenantRef && policy.repositoryRef === value.repositoryRef,
          )
          .map(({ repositoryRef: _repositoryRef, tenantRef: _tenantRef, ...policy }) => policy);
      }),
      recordCommittedReceive: Effect.fn("ForgeGitAdmission.memory.recordCommittedReceive")(
        function* () {},
      ),
      admitRepository: Effect.fn("ForgeGitAdmission.memory.admitRepository")(function* (value) {
        admitted.add(`${value.tenantRef}/${value.repositoryRef}`);
      }),
      authorizeSignedRefState: Effect.fn("ForgeGitAdmission.memory.authorizeSignedRefState")(
        function* (value) {
          const index = policies.findIndex(
            (policy) =>
              policy.tenantRef === value.tenantRef &&
              policy.repositoryRef === value.repositoryRef &&
              policy.refName === value.refName,
          );
          if (index >= 0) policies.splice(index, 1);
          policies.push(value);
        },
      ),
      holdPurgatory: Effect.fn("ForgeGitAdmission.memory.holdPurgatory")(function* () {}),
      listPurgatory: Effect.fn("ForgeGitAdmission.memory.listPurgatory")(function* () {
        return [];
      }),
      resolvePurgatory: Effect.fn("ForgeGitAdmission.memory.resolvePurgatory")(function* () {}),
      expirePurgatory: Effect.fn("ForgeGitAdmission.memory.expirePurgatory")(function* () {
        return 0;
      }),
      listAdmittedRepositories: Effect.fn("ForgeGitAdmission.memory.listAdmittedRepositories")(function* () {
        return [...admitted].map((key) => {
          const [tenantRef, repositoryRef] = key.split("/", 2) as [string, string];
          return { repositoryRef, tenantRef };
        });
      }),
      recordUnclaimedNostrRefs: Effect.fn("ForgeGitAdmission.memory.recordUnclaimedNostrRefs")(function* () {}),
      claimNostrEvent: Effect.fn("ForgeGitAdmission.memory.claimNostrEvent")(function* () {}),
      dueNostrRefGc: Effect.fn("ForgeGitAdmission.memory.dueNostrRefGc")(function* () {
        return [];
      }),
      markNostrRefsDeleted: Effect.fn("ForgeGitAdmission.memory.markNostrRefsDeleted")(function* () {}),
    }),
  );
};

/** @deprecated Use makeMemoryAdmissionLayer with explicit facts in new tests. */
export const layerAdmission = makeMemoryAdmissionLayer({ admittedRepositories: [] });

/** Database-backed production admission. Tables are created by migration 0099. */
export const layerDistributedAdmission = Layer.effect(
  ForgeGitAdmission,
  Effect.gen(function* () {
    const database = yield* ForgeGitDatabase;
    const configuration = yield* ForgeGitConfiguration;
    const withLocalLease = serialLease();

    return ForgeGitAdmission.of({
      withReceiveLease: (repositoryKey, effect) => {
        const lockKey = advisoryLockKey(repositoryKey);
        return withLocalLease(
          repositoryKey,
          Effect.acquireUseRelease(
            Effect.tryPromise({
              try: async () => {
                const sql = await database.sql.reserve();
                try {
                  await sql`SELECT pg_advisory_lock(${lockKey}::bigint)`;
                  return sql;
                } catch (error) {
                  sql.release();
                  throw error;
                }
              },
              catch: admissionUnavailable,
            }),
            () => effect,
            (sql) =>
              Effect.promise(async () => {
                try {
                  await sql`SELECT pg_advisory_unlock(${lockKey}::bigint)`;
                } finally {
                  sql.release();
                }
              }),
          ),
        );
      },
      requireRepository: Effect.fn("ForgeGitAdmission.requireRepository")(function* (input) {
        const rows = yield* Effect.tryPromise({
          try: () =>
            database.sql<Readonly<{ repository_ref: string }>[]>`SELECT repository_ref
                 FROM forge_git_repository_admissions
                WHERE tenant_ref = ${input.tenantRef}
                  AND repository_ref = ${input.repositoryRef}
                  AND state = 'admitted'
                LIMIT 1`,
          catch: admissionUnavailable,
        });
        if (rows.length !== 1) return yield* notAdmitted();
      }),
      signedRefPolicies: Effect.fn("ForgeGitAdmission.signedRefPolicies")(function* (input) {
        const rows = yield* Effect.tryPromise({
          try: () =>
            database.sql<
              Readonly<{
                event_id: string;
                new_object_id: string;
                old_object_id: string;
                ref_name: string;
              }>[]
            >`SELECT event_id, new_object_id, old_object_id, ref_name
                 FROM forge_git_signed_ref_states
                WHERE tenant_ref = ${input.tenantRef}
                  AND repository_ref = ${input.repositoryRef}
                  AND state = 'authorized'
                  AND superseded_at IS NULL`,
          catch: admissionUnavailable,
        });
        return rows.map((row) => ({
          eventId: row.event_id,
          newObjectId: row.new_object_id,
          oldObjectId: row.old_object_id,
          refName: row.ref_name,
        }));
      }),
      recordCommittedReceive: Effect.fn("ForgeGitAdmission.recordCommittedReceive")(
        function* (input) {
          if (input.stateEventIds.length === 0) return;
          yield* Effect.tryPromise({
            try: () =>
              database.sql.begin(async (sql) => {
                for (const eventId of input.stateEventIds) {
                  await sql`
                  UPDATE forge_git_signed_ref_states
                     SET applied_at = COALESCE(applied_at, now())
                   WHERE tenant_ref = ${input.tenantRef}
                     AND repository_ref = ${input.repositoryRef}
                     AND event_id = ${eventId}
                     AND state = 'authorized'
                `;
                  await sql`
                  INSERT INTO forge_git_relay_outbox (
                    outbox_ref, tenant_ref, repository_ref, event_id, kind,
                    state, available_at, created_at
                  ) VALUES (
                    ${`forge-relay-outbox.${eventId}`}, ${input.tenantRef},
                    ${input.repositoryRef}, ${eventId}, 30618, 'pending', now(), now()
                  ) ON CONFLICT (tenant_ref, event_id) DO NOTHING
                `;
                }
              }),
            catch: admissionUnavailable,
          });
        },
      ),
      admitRepository: Effect.fn("ForgeGitAdmission.admitRepository")(function* (input) {
        yield* Effect.tryPromise({
          try: () =>
            database.sql.begin(async (sql) => {
              const existing = await sql<Readonly<{ count: number }>[]>`
                SELECT count(*)::int AS count
                  FROM forge_git_repository_admissions
                 WHERE tenant_ref = ${input.tenantRef}
                   AND admitted_binding_ref = ${input.admittedBindingRef}
                   AND state = 'admitted'`;
              if ((existing[0]?.count ?? 0) >= configuration.maxRepositoriesPerOwner) {
                throw new ForgeGitAdmissionError({
                  code: "forge_git_repository_owner_quota_exceeded",
                  status: 403,
                });
              }
              await sql`
                INSERT INTO forge_git_repository_admissions (
                  tenant_ref, repository_ref, announcement_event_id,
                  announcement_author_pubkey, admitted_binding_ref,
                  maintainer_pubkeys_json, state, admitted_at, revoked_at
                ) VALUES (
                  ${input.tenantRef}, ${input.repositoryRef}, ${input.announcementEventId},
                  ${input.announcementAuthorPubkey}, ${input.admittedBindingRef},
                  ${JSON.stringify([...new Set(input.maintainerPubkeys)].sort())}, 'admitted', now(), NULL
                ) ON CONFLICT (tenant_ref, repository_ref) DO NOTHING
              `;
            }),
          catch: admissionUnavailable,
        });
      }),
      authorizeSignedRefState: Effect.fn("ForgeGitAdmission.authorizeSignedRefState")(
        function* (input) {
          yield* Effect.tryPromise({
            try: () =>
              database.sql.begin(async (sql) => {
                const maintainers = await sql<
                  Readonly<{ maintainer_pubkeys_json: string }>[]
                >`SELECT maintainer_pubkeys_json
                   FROM forge_git_repository_admissions
                  WHERE tenant_ref = ${input.tenantRef}
                    AND repository_ref = ${input.repositoryRef}
                    AND state = 'admitted'
                  LIMIT 1`;
                const row = maintainers[0];
                if (row === undefined) throw notAdmitted();
                const allowed = JSON.parse(row.maintainer_pubkeys_json) as unknown;
                if (!Array.isArray(allowed) || !allowed.includes(input.authorPubkey)) {
                  throw notAdmitted();
                }
                await sql`
                UPDATE forge_git_signed_ref_states
                   SET superseded_at = now()
                 WHERE tenant_ref = ${input.tenantRef}
                   AND repository_ref = ${input.repositoryRef}
                   AND ref_name = ${input.refName}
                   AND state = 'authorized'
                   AND superseded_at IS NULL
              `;
                await sql`
                INSERT INTO forge_git_signed_ref_states (
                  tenant_ref, repository_ref, ref_name, event_id, author_pubkey,
                  old_object_id, new_object_id, event_json, state, authorized_at, applied_at, superseded_at
                ) VALUES (
                  ${input.tenantRef}, ${input.repositoryRef}, ${input.refName},
                  ${input.eventId}, ${input.authorPubkey}, ${input.oldObjectId},
                  ${input.newObjectId}, ${input.eventJson}, 'authorized', now(), NULL, NULL
                )
              `;
              }),
            catch: (cause) =>
              cause instanceof ForgeGitAdmissionError ? cause : admissionUnavailable(),
          });
        },
      ),
      holdPurgatory: Effect.fn("ForgeGitAdmission.holdPurgatory")(function* (input) {
        yield* Effect.tryPromise({
          try: () => database.sql`
            INSERT INTO forge_git_purgatory_events (
              tenant_ref, repository_ref, event_id, kind, actor_binding_ref, required_object_ids_json,
              event_json, state, expires_at, created_at, resolved_at
            ) VALUES (
              ${input.tenantRef}, ${input.repositoryRef}, ${input.eventId}, ${input.kind}, ${input.actorBindingRef},
              ${JSON.stringify([...new Set(input.objectIds)].sort())}, ${input.eventJson},
              'pending', ${input.expiresAt}, ${input.createdAt}, NULL
            ) ON CONFLICT (tenant_ref, event_id) DO NOTHING`,
          catch: admissionUnavailable,
        });
      }),
      listPurgatory: Effect.fn("ForgeGitAdmission.listPurgatory")(function* (input) {
        const rows = yield* Effect.tryPromise({
          try: () => database.sql<Readonly<{
            created_at: string;
            actor_binding_ref: string;
            event_id: string;
            event_json: string;
            expires_at: string;
            kind: 30617 | 30618 | 1617 | 1618 | 1619;
            required_object_ids_json: string;
          }>[]>
            `SELECT event_id, kind, actor_binding_ref, required_object_ids_json, event_json, expires_at, created_at
               FROM forge_git_purgatory_events
              WHERE tenant_ref = ${input.tenantRef}
                AND repository_ref = ${input.repositoryRef}
                AND state = 'pending'
                AND expires_at > ${input.nowIso}`,
          catch: admissionUnavailable,
        });
        return rows.map((row) => ({
          createdAt: row.created_at,
          actorBindingRef: row.actor_binding_ref,
          eventId: row.event_id,
          eventJson: row.event_json,
          expiresAt: row.expires_at,
          kind: row.kind,
          objectIds: JSON.parse(row.required_object_ids_json) as ReadonlyArray<string>,
          repositoryRef: input.repositoryRef,
          tenantRef: input.tenantRef,
        }));
      }),
      resolvePurgatory: Effect.fn("ForgeGitAdmission.resolvePurgatory")(function* (input) {
        yield* Effect.tryPromise({
          try: () => database.sql`
            UPDATE forge_git_purgatory_events
               SET state = 'resolved', resolved_at = now()
             WHERE tenant_ref = ${input.tenantRef} AND repository_ref = ${input.repositoryRef}
               AND event_id = ${input.eventId} AND state = 'pending'`,
          catch: admissionUnavailable,
        });
      }),
      expirePurgatory: Effect.fn("ForgeGitAdmission.expirePurgatory")(function* (input) {
        const result = yield* Effect.tryPromise({
          try: () => database.sql`
            UPDATE forge_git_purgatory_events
               SET state = 'expired'
             WHERE tenant_ref = ${input.tenantRef} AND repository_ref = ${input.repositoryRef}
               AND state = 'pending' AND expires_at <= ${input.nowIso}`,
          catch: admissionUnavailable,
        });
        return result.count;
      }),
      listAdmittedRepositories: Effect.fn("ForgeGitAdmission.listAdmittedRepositories")(function* () {
        const rows = yield* Effect.tryPromise({
          try: () => database.sql<Readonly<{ repository_ref: string; tenant_ref: string }>[]>
            `SELECT tenant_ref, repository_ref
               FROM forge_git_repository_admissions
              WHERE state = 'admitted'`,
          catch: admissionUnavailable,
        });
        return rows.map((row) => ({ repositoryRef: row.repository_ref, tenantRef: row.tenant_ref }));
      }),
      recordUnclaimedNostrRefs: Effect.fn("ForgeGitAdmission.recordUnclaimedNostrRefs")(function* (input) {
        yield* Effect.tryPromise({
          try: () => database.sql.begin(async (sql) => {
            for (const refName of input.refNames) {
              const eventId = refName.replace(/^refs\/nostr\//u, "");
              if (!/^[0-9a-f]{64}$/u.test(eventId)) continue;
              await sql`
                INSERT INTO forge_git_unclaimed_nostr_refs (
                  tenant_ref, repository_ref, event_id, ref_name, first_seen_at, gc_after, claimed_at, deleted_at
                ) VALUES (
                  ${input.tenantRef}, ${input.repositoryRef}, ${eventId}, ${refName}, now(), now() + interval '20 minutes', NULL, NULL
                ) ON CONFLICT (tenant_ref, repository_ref, ref_name) DO NOTHING`;
            }
          }),
          catch: admissionUnavailable,
        });
      }),
      claimNostrEvent: Effect.fn("ForgeGitAdmission.claimNostrEvent")(function* (input) {
        yield* Effect.tryPromise({
          try: () => database.sql`
            UPDATE forge_git_unclaimed_nostr_refs
               SET claimed_at = COALESCE(claimed_at, now())
             WHERE tenant_ref = ${input.tenantRef} AND repository_ref = ${input.repositoryRef}
               AND event_id = ${input.eventId}`,
          catch: admissionUnavailable,
        });
      }),
      dueNostrRefGc: Effect.fn("ForgeGitAdmission.dueNostrRefGc")(function* (input) {
        const rows = yield* Effect.tryPromise({
          try: () => database.sql<Readonly<{ ref_name: string }>[]>
            `SELECT ref_name FROM forge_git_unclaimed_nostr_refs
              WHERE tenant_ref = ${input.tenantRef} AND repository_ref = ${input.repositoryRef}
                AND claimed_at IS NULL AND deleted_at IS NULL AND gc_after <= ${input.nowIso}`,
          catch: admissionUnavailable,
        });
        return rows.map((row) => row.ref_name);
      }),
      markNostrRefsDeleted: Effect.fn("ForgeGitAdmission.markNostrRefsDeleted")(function* (input) {
        if (input.refNames.length === 0) return;
        yield* Effect.tryPromise({
          try: () => database.sql`
            UPDATE forge_git_unclaimed_nostr_refs
               SET deleted_at = now()
             WHERE tenant_ref = ${input.tenantRef} AND repository_ref = ${input.repositoryRef}
               AND ref_name = ANY(${input.refNames as string[]})`,
          catch: admissionUnavailable,
        });
      }),
    });
  }),
);
