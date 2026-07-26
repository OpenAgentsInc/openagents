import { createHash } from "node:crypto";

import type {
  ForgeMergeOutcomeReceipt,
  ForgeMergeOutcomeReceiptDraft,
} from "@openagentsinc/forge-protocol";
import { Context, Effect, Layer, Semaphore } from "effect";

import { ForgeGitDatabase } from "./database.js";
import { ForgeGitConfiguration } from "./config.js";
import { ForgeGitAdmissionError, type ForgeGitSignedRefPolicy } from "./model.js";

export type ForgeGitAdmissionEvent = Readonly<{
  readonly createdAt: string;
  readonly eventId: string;
  readonly kind: 1111 | 1617 | 1618 | 1619 | 1621 | 1630 | 1631 | 1632 | 1633 | 30617 | 30618;
  readonly objectIds: ReadonlyArray<string>;
  readonly repositoryRef: string;
  readonly tenantRef: string;
}>;

/** An admitted event is the only collaboration input. This is a projection,
 * never a second collaboration authority. */
export type ForgeGitProjectedEvent = ForgeGitAdmissionEvent &
  Readonly<{
    readonly actorBindingRef: string;
    readonly authorPubkey: string;
    readonly eventJson: string;
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

export type ForgeGitMergeReceipt = ForgeMergeOutcomeReceiptDraft &
  Readonly<{
    readonly finalizedAt: string | null;
    readonly signedState: ForgeMergeOutcomeReceipt["signedState"] | null;
    readonly state: "prepared" | "finalized" | "refused";
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
  readonly prepareMergeReceipt: (
    receipt: ForgeMergeOutcomeReceiptDraft,
  ) => Effect.Effect<void, ForgeGitAdmissionError>;
  readonly finalizeMergeReceipt: (input: {
    readonly eventId: string;
    readonly receiptRef: string;
    readonly signerPubkey: string;
    readonly signature: string;
    readonly targetRef: string;
    readonly oldObjectId: string;
    readonly newObjectId: string;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<void, ForgeGitAdmissionError>;
  readonly readMergeReceipt: (input: {
    readonly receiptRef: string;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<ForgeGitMergeReceipt | undefined, ForgeGitAdmissionError>;
  /**
   * Returns a receipt only after stock Git applied its exact signed ref state.
   * A finalized signer receipt by itself is not canonical promotion success.
   */
  readonly readAppliedMergeReceiptRef: (input: {
    readonly newObjectId: string;
    readonly repositoryRef: string;
    readonly targetRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<string | undefined, ForgeGitAdmissionError>;
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
  /** Stores an already-admitted event for the read-only collaboration projection. */
  readonly recordProjectedEvent: (
    input: ForgeGitProjectedEvent,
  ) => Effect.Effect<void, ForgeGitAdmissionError>;
  readonly listProjectedEvents: (input: {
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<ReadonlyArray<ForgeGitProjectedEvent>, ForgeGitAdmissionError>;
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

const mergeReceiptMissing = () =>
  new ForgeGitAdmissionError({
    code: "forge_git_merge_receipt_missing_or_mismatched",
    status: 403,
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
  readonly preparedMergeReceipts?: ReadonlyArray<ForgeMergeOutcomeReceiptDraft>;
  readonly projectedEvents?: ReadonlyArray<ForgeGitProjectedEvent>;
}): Layer.Layer<ForgeGitAdmission> => {
  const withReceiveLease = serialLease();
  const admitted = new Set(
    input.admittedRepositories.map((item) => `${item.tenantRef}/${item.repositoryRef}`),
  );
  const policies = (input.signedRefPolicies ?? []) as Array<
    ForgeGitSignedRefPolicy & Readonly<{ repositoryRef: string; tenantRef: string }>
  >;
  const mergeReceipts = new Map<string, ForgeGitMergeReceipt>(
    (input.preparedMergeReceipts ?? []).map((receipt) => [
      receipt.receiptRef,
      { ...receipt, finalizedAt: null, signedState: null, state: "prepared" as const },
    ]),
  );
  const projectedEvents: Array<ForgeGitProjectedEvent> = [...(input.projectedEvents ?? [])];
  const appliedStateEventIds = new Set<string>();
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
        function* (value) {
          for (const eventId of value.stateEventIds) appliedStateEventIds.add(eventId);
        },
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
      prepareMergeReceipt: Effect.fn("ForgeGitAdmission.memory.prepareMergeReceipt")(
        function* (receipt) {
          const existing = mergeReceipts.get(receipt.receiptRef);
          if (existing !== undefined) {
            if (
              existing.tenantRef !== receipt.tenantRef ||
              existing.repositoryRef !== receipt.repositoryRef ||
              existing.targetRef !== receipt.targetRef ||
              existing.oldObjectId !== receipt.oldObjectId ||
              existing.newObjectId !== receipt.newObjectId
            ) {
              return yield* mergeReceiptMissing();
            }
            return;
          }
          mergeReceipts.set(receipt.receiptRef, {
            ...receipt,
            finalizedAt: null,
            signedState: null,
            state: "prepared",
          });
        },
      ),
      finalizeMergeReceipt: Effect.fn("ForgeGitAdmission.memory.finalizeMergeReceipt")(
        function* (value) {
          const receipt = mergeReceipts.get(value.receiptRef);
          if (
            receipt === undefined ||
            receipt.state !== "prepared" ||
            receipt.tenantRef !== value.tenantRef ||
            receipt.repositoryRef !== value.repositoryRef ||
            receipt.targetRef !== value.targetRef ||
            receipt.oldObjectId !== value.oldObjectId ||
            receipt.newObjectId !== value.newObjectId
          ) {
            return yield* mergeReceiptMissing();
          }
          mergeReceipts.set(value.receiptRef, {
            ...receipt,
            finalizedAt: new Date().toISOString(),
            signedState: {
              eventId: value.eventId,
              eventKind: 30618,
              signerPubkey: value.signerPubkey,
              signature: value.signature,
            },
            state: "finalized",
          });
        },
      ),
      readMergeReceipt: Effect.fn("ForgeGitAdmission.memory.readMergeReceipt")(function* (value) {
        const receipt = mergeReceipts.get(value.receiptRef);
        return receipt !== undefined &&
          receipt.tenantRef === value.tenantRef &&
          receipt.repositoryRef === value.repositoryRef
          ? receipt
          : undefined;
      }),
      readAppliedMergeReceiptRef: Effect.fn("ForgeGitAdmission.memory.readAppliedMergeReceiptRef")(
        function* (value) {
          return [...mergeReceipts.values()].find(
            (receipt) =>
              receipt.tenantRef === value.tenantRef &&
              receipt.repositoryRef === value.repositoryRef &&
              receipt.targetRef === value.targetRef &&
              receipt.newObjectId === value.newObjectId &&
              receipt.state === "finalized" &&
              receipt.signedState !== null &&
              appliedStateEventIds.has(receipt.signedState.eventId),
          )?.receiptRef;
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
      recordProjectedEvent: Effect.fn("ForgeGitAdmission.memory.recordProjectedEvent")(
        function* (event) {
          if (!projectedEvents.some((candidate) => candidate.eventId === event.eventId)) {
            projectedEvents.push(event);
          }
        },
      ),
      listProjectedEvents: Effect.fn("ForgeGitAdmission.memory.listProjectedEvents")(
        function* (value) {
          return projectedEvents.filter(
            (event) =>
              event.tenantRef === value.tenantRef && event.repositoryRef === value.repositoryRef,
          );
        },
      ),
      listAdmittedRepositories: Effect.fn("ForgeGitAdmission.memory.listAdmittedRepositories")(
        function* () {
          return [...admitted].map((key) => {
            const [tenantRef, repositoryRef] = key.split("/", 2) as [string, string];
            return { repositoryRef, tenantRef };
          });
        },
      ),
      recordUnclaimedNostrRefs: Effect.fn("ForgeGitAdmission.memory.recordUnclaimedNostrRefs")(
        function* () {},
      ),
      claimNostrEvent: Effect.fn("ForgeGitAdmission.memory.claimNostrEvent")(function* () {}),
      dueNostrRefGc: Effect.fn("ForgeGitAdmission.memory.dueNostrRefGc")(function* () {
        return [];
      }),
      markNostrRefsDeleted: Effect.fn("ForgeGitAdmission.memory.markNostrRefsDeleted")(
        function* () {},
      ),
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
                     SET applied_at = COALESCE(applied_at, now()::text)
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
      prepareMergeReceipt: Effect.fn("ForgeGitAdmission.prepareMergeReceipt")(function* (receipt) {
        yield* Effect.tryPromise({
          try: async () => {
            const inserted = await database.sql<Readonly<{ receipt_ref: string }>[]>`
              INSERT INTO forge_git_merge_outcome_receipts (
                receipt_ref, tenant_ref, repository_ref, change_ref, maintainer_binding_ref,
                target_ref, old_object_id, new_object_id, authority_generation, policy_version,
                proposal_event_ids_json, gate_results_json, state, decided_at
              ) VALUES (
                ${receipt.receiptRef}, ${receipt.tenantRef}, ${receipt.repositoryRef}, ${receipt.changeRef},
                ${receipt.maintainerBindingRef}, ${receipt.targetRef}, ${receipt.oldObjectId},
                ${receipt.newObjectId}, ${receipt.authorityGeneration}, ${receipt.policyVersion},
                ${JSON.stringify(receipt.proposalEventIds)}, ${JSON.stringify(receipt.gateResults)},
                'prepared', ${receipt.decidedAt}
              ) ON CONFLICT (receipt_ref) DO NOTHING
              RETURNING receipt_ref`;
            if (inserted.length > 0) return;
            const existing = await database.sql<
              Readonly<{
                tenant_ref: string;
                repository_ref: string;
                target_ref: string;
                old_object_id: string;
                new_object_id: string;
              }>[]
            >`
              SELECT tenant_ref, repository_ref, target_ref, old_object_id, new_object_id
                FROM forge_git_merge_outcome_receipts
               WHERE receipt_ref = ${receipt.receiptRef}`;
            const row = existing[0];
            if (
              row === undefined ||
              row.tenant_ref !== receipt.tenantRef ||
              row.repository_ref !== receipt.repositoryRef ||
              row.target_ref !== receipt.targetRef ||
              row.old_object_id !== receipt.oldObjectId ||
              row.new_object_id !== receipt.newObjectId
            )
              throw mergeReceiptMissing();
          },
          catch: (cause) =>
            cause instanceof ForgeGitAdmissionError ? cause : admissionUnavailable(),
        });
      }),
      finalizeMergeReceipt: Effect.fn("ForgeGitAdmission.finalizeMergeReceipt")(function* (input) {
        yield* Effect.tryPromise({
          try: async () => {
            const finalized = await database.sql<Readonly<{ receipt_ref: string }>[]>`
              UPDATE forge_git_merge_outcome_receipts
                 SET state = 'finalized', state_event_id = ${input.eventId},
                     state_author_pubkey = ${input.signerPubkey}, state_signature = ${input.signature},
                     finalized_at = now()
               WHERE receipt_ref = ${input.receiptRef}
                 AND tenant_ref = ${input.tenantRef}
                 AND repository_ref = ${input.repositoryRef}
                 AND target_ref = ${input.targetRef}
                 AND old_object_id = ${input.oldObjectId}
                 AND new_object_id = ${input.newObjectId}
                 AND state = 'prepared'
              RETURNING receipt_ref`;
            if (finalized.length !== 1) throw mergeReceiptMissing();
          },
          catch: (cause) =>
            cause instanceof ForgeGitAdmissionError ? cause : admissionUnavailable(),
        });
      }),
      readMergeReceipt: Effect.fn("ForgeGitAdmission.readMergeReceipt")(function* (input) {
        const rows = yield* Effect.tryPromise({
          try: () => database.sql<
            Readonly<{
              authority_generation: number;
              change_ref: string;
              decided_at: string;
              finalized_at: string | null;
              gate_results_json: string;
              maintainer_binding_ref: string;
              new_object_id: string;
              old_object_id: string;
              policy_version: string;
              proposal_event_ids_json: string;
              receipt_ref: string;
              repository_ref: string;
              state: "prepared" | "finalized" | "refused";
              state_author_pubkey: string | null;
              state_event_id: string | null;
              state_signature: string | null;
              target_ref: string;
              tenant_ref: string;
            }>[]
          >`SELECT receipt_ref, tenant_ref, repository_ref, change_ref, maintainer_binding_ref, target_ref,
                    old_object_id, new_object_id, authority_generation, policy_version,
                    proposal_event_ids_json, gate_results_json, state, decided_at, finalized_at,
                    state_event_id, state_author_pubkey, state_signature
               FROM forge_git_merge_outcome_receipts
              WHERE receipt_ref = ${input.receiptRef} AND tenant_ref = ${input.tenantRef}
                AND repository_ref = ${input.repositoryRef}`,
          catch: admissionUnavailable,
        });
        const row = rows[0];
        if (row === undefined) return undefined;
        return {
          authorityGeneration: row.authority_generation,
          changeRef: row.change_ref,
          decidedAt: row.decided_at,
          finalizedAt: row.finalized_at,
          gateResults: JSON.parse(row.gate_results_json),
          maintainerBindingRef: row.maintainer_binding_ref,
          newObjectId: row.new_object_id,
          oldObjectId: row.old_object_id,
          policyVersion: row.policy_version,
          proposalEventIds: JSON.parse(row.proposal_event_ids_json),
          receiptRef: row.receipt_ref,
          redacted: true,
          repositoryRef: row.repository_ref,
          schema: "openagents.forge.merge.outcome.receipt.v1",
          signedState:
            row.state_event_id === null ||
            row.state_author_pubkey === null ||
            row.state_signature === null
              ? null
              : {
                  eventId: row.state_event_id,
                  eventKind: 30618,
                  signerPubkey: row.state_author_pubkey,
                  signature: row.state_signature,
                },
          state: row.state,
          targetRef: row.target_ref,
          tenantRef: row.tenant_ref,
        } as ForgeGitMergeReceipt;
      }),
      readAppliedMergeReceiptRef: Effect.fn("ForgeGitAdmission.readAppliedMergeReceiptRef")(
        function* (input) {
          const rows = yield* Effect.tryPromise({
            try: () => database.sql<Readonly<{ receipt_ref: string }>[]>`
            SELECT receipt.receipt_ref
              FROM forge_git_merge_outcome_receipts AS receipt
              JOIN forge_git_signed_ref_states AS state
                ON state.tenant_ref = receipt.tenant_ref
               AND state.repository_ref = receipt.repository_ref
               AND state.ref_name = receipt.target_ref
               AND state.event_id = receipt.state_event_id
               AND state.new_object_id = receipt.new_object_id
             WHERE receipt.tenant_ref = ${input.tenantRef}
               AND receipt.repository_ref = ${input.repositoryRef}
               AND receipt.target_ref = ${input.targetRef}
               AND receipt.new_object_id = ${input.newObjectId}
               AND receipt.state = 'finalized'
               AND state.applied_at IS NOT NULL
             ORDER BY receipt.finalized_at DESC
             LIMIT 1`,
            catch: admissionUnavailable,
          });
          return rows[0]?.receipt_ref;
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
          try: () => database.sql<
            Readonly<{
              created_at: string;
              actor_binding_ref: string;
              event_id: string;
              event_json: string;
              expires_at: string;
              kind: 30617 | 30618 | 1617 | 1618 | 1619;
              required_object_ids_json: string;
            }>[]
          >`SELECT event_id, kind, actor_binding_ref, required_object_ids_json, event_json, expires_at, created_at
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
      recordProjectedEvent: Effect.fn("ForgeGitAdmission.recordProjectedEvent")(function* (input) {
        yield* Effect.tryPromise({
          try: () => database.sql`
            INSERT INTO forge_git_projected_events (
              tenant_ref, repository_ref, event_id, kind, author_pubkey,
              actor_binding_ref, event_json, observed_at, projected_at
            ) VALUES (
              ${input.tenantRef}, ${input.repositoryRef}, ${input.eventId}, ${input.kind},
              ${input.authorPubkey}, ${input.actorBindingRef}, ${input.eventJson},
              ${input.createdAt}, now()
            ) ON CONFLICT (tenant_ref, event_id) DO NOTHING`,
          catch: admissionUnavailable,
        });
      }),
      listProjectedEvents: Effect.fn("ForgeGitAdmission.listProjectedEvents")(function* (input) {
        const rows = yield* Effect.tryPromise({
          try: () =>
            database.sql<
              Readonly<{
                actor_binding_ref: string;
                author_pubkey: string;
                event_id: string;
                event_json: string;
                kind: ForgeGitAdmissionEvent["kind"];
                observed_at: string;
              }>[]
            >`SELECT event_id, kind, author_pubkey, actor_binding_ref, event_json, observed_at
                FROM forge_git_projected_events
               WHERE tenant_ref = ${input.tenantRef}
                 AND repository_ref = ${input.repositoryRef}
               ORDER BY observed_at ASC, event_id ASC`,
          catch: admissionUnavailable,
        });
        return rows.map((row) => ({
          actorBindingRef: row.actor_binding_ref,
          authorPubkey: row.author_pubkey,
          createdAt: row.observed_at,
          eventId: row.event_id,
          eventJson: row.event_json,
          kind: row.kind,
          objectIds: [],
          repositoryRef: input.repositoryRef,
          tenantRef: input.tenantRef,
        }));
      }),
      listAdmittedRepositories: Effect.fn("ForgeGitAdmission.listAdmittedRepositories")(
        function* () {
          const rows = yield* Effect.tryPromise({
            try: () => database.sql<
              Readonly<{ repository_ref: string; tenant_ref: string }>[]
            >`SELECT tenant_ref, repository_ref
               FROM forge_git_repository_admissions
              WHERE state = 'admitted'`,
            catch: admissionUnavailable,
          });
          return rows.map((row) => ({
            repositoryRef: row.repository_ref,
            tenantRef: row.tenant_ref,
          }));
        },
      ),
      recordUnclaimedNostrRefs: Effect.fn("ForgeGitAdmission.recordUnclaimedNostrRefs")(
        function* (input) {
          yield* Effect.tryPromise({
            try: () =>
              database.sql.begin(async (sql) => {
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
        },
      ),
      claimNostrEvent: Effect.fn("ForgeGitAdmission.claimNostrEvent")(function* (input) {
        yield* Effect.tryPromise({
          try: () => database.sql`
            UPDATE forge_git_unclaimed_nostr_refs
               SET claimed_at = COALESCE(claimed_at, now()::text)
             WHERE tenant_ref = ${input.tenantRef} AND repository_ref = ${input.repositoryRef}
               AND event_id = ${input.eventId}`,
          catch: admissionUnavailable,
        });
      }),
      dueNostrRefGc: Effect.fn("ForgeGitAdmission.dueNostrRefGc")(function* (input) {
        const rows = yield* Effect.tryPromise({
          try: () => database.sql<
            Readonly<{ ref_name: string }>[]
          >`SELECT ref_name FROM forge_git_unclaimed_nostr_refs
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
