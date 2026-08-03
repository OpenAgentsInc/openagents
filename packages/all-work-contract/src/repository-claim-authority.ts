import { createHash, randomBytes } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

import { Context, Effect, Layer, Ref, Schema as S } from "effect";

import {
  decodeRepositoryClaimExecuteRequest,
  decodeRepositoryClaimAudit,
  decodeRepositoryClaimLedger,
  decodeRepositoryClaimReadRequest,
  decodeRepositoryWorkClaim,
  decodeWorkPacket,
  type RepositoryClaimAudit,
  type RepositoryClaimExecuteRequest,
  type RepositoryClaimExecuteResult,
  RepositoryClaimExecuteResultSchema,
  type RepositoryClaimLedger,
  RepositoryClaimLedgerSchema,
  type RepositoryClaimReadRequest,
  type RepositoryClaimReadResult,
  RepositoryClaimReadResultSchema,
  type RepositoryClaimReceipt,
  RepositoryClaimReceiptSchema,
  type RepositoryWorkClaim,
  type WorkPacket,
} from "./generated.ts";
import { encodeAllWorkCanonicalJson } from "./semantic.ts";

export const REPOSITORY_CLAIM_AUTHORITY_STATE_SCHEMA =
  "openagents.repository_claim_authority_state.v1" as const;
export const REPOSITORY_CLAIM_WRITE_CAPABILITY = "capability:repository-claim:write" as const;
export const REPOSITORY_CLAIM_STALE_AFTER_MS = 90 * 60 * 1_000;

export const RepositoryClaimStateSchema = S.Struct({
  schema: S.Literal(REPOSITORY_CLAIM_AUTHORITY_STATE_SCHEMA),
  ledger: RepositoryClaimLedgerSchema,
  receipts: S.Array(RepositoryClaimReceiptSchema),
});
export interface RepositoryClaimAuthorityState extends S.Schema.Type<
  typeof RepositoryClaimStateSchema
> {}

export class RepositoryClaimAuthorityError extends S.TaggedErrorClass<RepositoryClaimAuthorityError>()(
  "RepositoryClaimAuthority.Error",
  {
    reason: S.Literals([
      "invalid_state",
      "invalid_request",
      "storage_unavailable",
      "revision_conflict",
      "idempotency_conflict",
      "forbidden",
      "packet_exists",
      "packet_not_found",
      "packet_not_ready",
      "claim_exists",
      "claim_not_found",
      "claim_collision",
      "claim_not_active",
      "stale_generation",
    ]),
    detail: S.String,
  },
) {}

export interface RepositoryClaimStateStoreShape {
  readonly load: Effect.Effect<RepositoryClaimAuthorityState | null, RepositoryClaimAuthorityError>;
  readonly save: (
    expectedRevision: number,
    state: RepositoryClaimAuthorityState,
  ) => Effect.Effect<void, RepositoryClaimAuthorityError>;
}

export class RepositoryClaimStateStore extends Context.Service<
  RepositoryClaimStateStore,
  RepositoryClaimStateStoreShape
>()("RepositoryClaimAuthority.StateStore") {}

const digest = (value: unknown): string =>
  createHash("sha256").update(encodeAllWorkCanonicalJson(value)).digest("hex");

const uniqueSorted = <T extends string>(values: ReadonlyArray<T>): ReadonlyArray<T> =>
  [...new Set(values)].sort((left, right) => left.localeCompare(right));

const normalizePath = (value: string): string =>
  value.replaceAll("\\", "/").replace(/^\.\//u, "").replace(/\/+$/u, "");

const pathOverlap = (left: string, right: string): boolean => {
  const a = normalizePath(left);
  const b = normalizePath(right);
  return a === b || a.startsWith(`${b}/`) || b.startsWith(`${a}/`);
};

export type RepositoryClaimCollision = Readonly<{
  claimRef: string;
  kinds: ReadonlyArray<
    | "same_work"
    | "path"
    | "hot_file"
    | "hot_contract"
    | "generated_artifact"
    | "migration"
    | "route_table"
    | "lockfile"
    | "shared_schema"
  >;
}>;

const specializedCollisionKind = (
  value: string,
): RepositoryClaimCollision["kinds"][number] | null => {
  const normalized = value.toLowerCase();
  if (normalized.includes("generated") || normalized.includes("contract")) {
    return "generated_artifact";
  }
  if (normalized.includes("migration")) return "migration";
  if (normalized.includes("route")) return "route_table";
  if (normalized.endsWith("lock") || normalized.includes("lockfile")) return "lockfile";
  if (normalized.includes("schema")) return "shared_schema";
  return null;
};

export const repositoryClaimCollision = (
  packet: WorkPacket,
  claim: RepositoryWorkClaim,
): RepositoryClaimCollision | null => {
  if (claim.repositoryRef !== packet.repositoryRef) return null;
  const kinds = new Set<RepositoryClaimCollision["kinds"][number]>();
  if (claim.workRef === packet.workRef) kinds.add("same_work");
  if (
    packet.ownedPaths.some((path) => claim.ownedPaths.some((other) => pathOverlap(path, other)))
  ) {
    kinds.add("path");
  }
  for (const value of packet.hotFiles) {
    if (claim.hotFiles.includes(value)) {
      kinds.add("hot_file");
      const specialized = specializedCollisionKind(value);
      if (specialized !== null) kinds.add(specialized);
    }
  }
  for (const value of packet.hotContracts) {
    if (claim.hotContracts.includes(value)) {
      kinds.add("hot_contract");
      const specialized = specializedCollisionKind(value);
      if (specialized !== null) kinds.add(specialized);
    }
  }
  return kinds.size === 0
    ? null
    : { claimRef: claim.claimRef, kinds: [...kinds].sort((a, b) => a.localeCompare(b)) };
};

export const emptyRepositoryClaimLedger = (observedAt: string): RepositoryClaimLedger =>
  decodeRepositoryClaimLedger({
    contractVersion: "openagents.all_work_boundary.v1",
    revision: 0,
    eventCursor: "cursor:repository-claim:0",
    packets: [],
    claims: [],
    audit: [],
    completeness: { state: "complete", cursor: "cursor:repository-claim:0", gapRefs: [] },
    freshness: { state: "fresh", observedAt },
  });

export const emptyRepositoryClaimAuthorityState = (
  observedAt: string,
): RepositoryClaimAuthorityState => ({
  schema: REPOSITORY_CLAIM_AUTHORITY_STATE_SCHEMA,
  ledger: emptyRepositoryClaimLedger(observedAt),
  receipts: [],
});

const decodeState = (input: unknown) =>
  S.decodeUnknownEffect(RepositoryClaimStateSchema)(input, { onExcessProperty: "error" }).pipe(
    Effect.mapError(
      () => new RepositoryClaimAuthorityError({ reason: "invalid_state", detail: "decode" }),
    ),
  );

const activeClaim = (claim: RepositoryWorkClaim): boolean =>
  claim.state === "claimed" || claim.state === "blocked";

const event = (
  revision: number,
  kind: RepositoryClaimAudit["kind"],
  packetRef: string,
  claimRef: string | null,
  principalRef: string,
  generation: number,
  occurredAt: string,
  evidenceRefs: ReadonlyArray<string>,
  detail: string,
): RepositoryClaimAudit =>
  decodeRepositoryClaimAudit({
    eventRef: `claim-event:${revision}`,
    kind,
    packetRef,
    claimRef,
    principalRef,
    generation,
    occurredAt,
    evidenceRefs: uniqueSorted(evidenceRefs),
    detail,
  });

const advance = (
  current: RepositoryClaimLedger,
  occurredAt: string,
  patch: Pick<RepositoryClaimLedger, "packets" | "claims"> & {
    audit: RepositoryClaimAudit;
  },
): RepositoryClaimLedger => {
  const revision = current.revision + 1;
  return decodeRepositoryClaimLedger({
    ...current,
    revision,
    eventCursor: `cursor:repository-claim:${revision}`,
    packets: [...patch.packets].sort((a, b) => a.packetRef.localeCompare(b.packetRef)),
    claims: [...patch.claims].sort((a, b) => a.claimRef.localeCompare(b.claimRef)),
    audit: [...current.audit, patch.audit],
    completeness: {
      state: "complete",
      cursor: `cursor:repository-claim:${revision}`,
      gapRefs: [],
    },
    freshness: { state: "fresh", observedAt: occurredAt },
  });
};

const requirePacket = (ledger: RepositoryClaimLedger, packetRef: string): WorkPacket => {
  const packet = ledger.packets.find((candidate) => candidate.packetRef === packetRef);
  if (packet === undefined) {
    throw new RepositoryClaimAuthorityError({ reason: "packet_not_found", detail: packetRef });
  }
  return packet;
};

const requireClaim = (ledger: RepositoryClaimLedger, claimRef: string): RepositoryWorkClaim => {
  const claim = ledger.claims.find((candidate) => candidate.claimRef === claimRef);
  if (claim === undefined) {
    throw new RepositoryClaimAuthorityError({ reason: "claim_not_found", detail: claimRef });
  }
  return claim;
};

const replacePacket = (
  packets: ReadonlyArray<WorkPacket>,
  packet: WorkPacket,
): ReadonlyArray<WorkPacket> =>
  packets.map((candidate) => (candidate.packetRef === packet.packetRef ? packet : candidate));

const replaceClaim = (
  claims: ReadonlyArray<RepositoryWorkClaim>,
  claim: RepositoryWorkClaim,
): ReadonlyArray<RepositoryWorkClaim> =>
  claims.map((candidate) => (candidate.claimRef === claim.claimRef ? claim : candidate));

type Transition = Readonly<{
  ledger: RepositoryClaimLedger;
  claimRef: string | null;
  admitted: boolean;
  refusalReason: string | null;
}>;

const applyRequest = (
  ledger: RepositoryClaimLedger,
  request: RepositoryClaimExecuteRequest,
): Transition => {
  if (request.capabilityRef !== REPOSITORY_CLAIM_WRITE_CAPABILITY) {
    throw new RepositoryClaimAuthorityError({ reason: "forbidden", detail: "capability" });
  }
  const command = request.command;
  const revision = ledger.revision + 1;
  if (command.command === "create_packet") {
    if (ledger.packets.some((packet) => packet.packetRef === command.packetRef)) {
      throw new RepositoryClaimAuthorityError({
        reason: "packet_exists",
        detail: command.packetRef,
      });
    }
    const packet = decodeWorkPacket({
      packetRef: command.packetRef,
      workRef: command.workRef,
      repositoryRef: command.repositoryRef,
      title: command.title,
      scope: command.scope,
      ownedPaths: uniqueSorted(command.ownedPaths),
      hotFiles: uniqueSorted(command.hotFiles),
      hotContracts: uniqueSorted(command.hotContracts),
      verification: command.verification,
      state: "ready",
      revision: 1,
      createdAt: request.occurredAt,
      updatedAt: request.occurredAt,
    });
    return {
      ledger: advance(ledger, request.occurredAt, {
        packets: [...ledger.packets, packet],
        claims: ledger.claims,
        audit: event(
          revision,
          "packet_created",
          packet.packetRef,
          null,
          request.effectivePrincipalRef,
          0,
          request.occurredAt,
          [],
          "Work Packet created; no claim authority inferred.",
        ),
      }),
      claimRef: null,
      admitted: true,
      refusalReason: null,
    };
  }
  if (command.command === "claim_packet") {
    const packet = requirePacket(ledger, command.packetRef);
    if (packet.state !== "ready") {
      throw new RepositoryClaimAuthorityError({
        reason: "packet_not_ready",
        detail: packet.packetRef,
      });
    }
    if (ledger.claims.some((claim) => claim.claimRef === command.claimRef)) {
      throw new RepositoryClaimAuthorityError({ reason: "claim_exists", detail: command.claimRef });
    }
    const collision = ledger.claims
      .filter(activeClaim)
      .map((claim) => repositoryClaimCollision(packet, claim))
      .find((candidate) => candidate !== null);
    if (collision !== undefined) {
      throw new RepositoryClaimAuthorityError({
        reason: "claim_collision",
        detail: `${collision.claimRef}:${collision.kinds.join(",")}`,
      });
    }
    const claim = decodeRepositoryWorkClaim({
      claimRef: command.claimRef,
      packetRef: packet.packetRef,
      workRef: packet.workRef,
      repositoryRef: packet.repositoryRef,
      holderRef: request.effectivePrincipalRef,
      scope: packet.scope,
      ownedPaths: packet.ownedPaths,
      hotFiles: packet.hotFiles,
      hotContracts: packet.hotContracts,
      claimedAt: request.occurredAt,
      lastEvidenceAt: request.occurredAt,
      evidenceRefs: [],
      state: "claimed",
      generation: 1,
      revision: 1,
      releasedAt: null,
      releaserRef: null,
      releaseEvidenceRefs: [],
    });
    const claimedPacket = {
      ...packet,
      state: "claimed" as const,
      revision: packet.revision + 1,
      updatedAt: request.occurredAt,
    };
    return {
      ledger: advance(ledger, request.occurredAt, {
        packets: replacePacket(ledger.packets, claimedPacket),
        claims: [...ledger.claims, claim],
        audit: event(
          revision,
          "claimed",
          packet.packetRef,
          claim.claimRef,
          request.effectivePrincipalRef,
          1,
          request.occurredAt,
          [],
          "Repository Work Claim admitted.",
        ),
      }),
      claimRef: claim.claimRef,
      admitted: true,
      refusalReason: null,
    };
  }
  const claim = requireClaim(ledger, command.claimRef);
  const packet = requirePacket(ledger, claim.packetRef);
  if (!activeClaim(claim)) {
    throw new RepositoryClaimAuthorityError({ reason: "claim_not_active", detail: claim.claimRef });
  }
  if (command.expectedGeneration !== claim.generation) {
    throw new RepositoryClaimAuthorityError({ reason: "stale_generation", detail: claim.claimRef });
  }
  if (command.command !== "takeover" && claim.holderRef !== request.effectivePrincipalRef) {
    throw new RepositoryClaimAuthorityError({ reason: "forbidden", detail: claim.claimRef });
  }
  if (command.command === "takeover") {
    const elapsed = Date.parse(command.auditedAt) - Date.parse(claim.lastEvidenceAt);
    const stale = elapsed >= REPOSITORY_CLAIM_STALE_AFTER_MS && !command.auditFoundActiveWork;
    if (!stale) {
      return {
        ledger: advance(ledger, request.occurredAt, {
          packets: ledger.packets,
          claims: ledger.claims,
          audit: event(
            revision,
            "stale_takeover_refused",
            packet.packetRef,
            claim.claimRef,
            request.effectivePrincipalRef,
            claim.generation,
            request.occurredAt,
            [command.auditRef],
            "Takeover refused: elapsed time and an inactive process/worktree audit are both required.",
          ),
        }),
        claimRef: claim.claimRef,
        admitted: false,
        refusalReason: "staleness_requires_90_minutes_and_inactive_audit",
      };
    }
    const next = {
      ...claim,
      holderRef: request.effectivePrincipalRef,
      claimedAt: request.occurredAt,
      lastEvidenceAt: request.occurredAt,
      evidenceRefs: uniqueSorted([...claim.evidenceRefs, command.auditRef]),
      state: "claimed" as const,
      generation: claim.generation + 1,
      revision: claim.revision + 1,
    };
    return {
      ledger: advance(ledger, request.occurredAt, {
        packets: ledger.packets,
        claims: replaceClaim(ledger.claims, next),
        audit: event(
          revision,
          "stale_takeover",
          packet.packetRef,
          claim.claimRef,
          request.effectivePrincipalRef,
          next.generation,
          request.occurredAt,
          [command.auditRef],
          "Stale claim superseded after the required inactive process/worktree audit.",
        ),
      }),
      claimRef: claim.claimRef,
      admitted: true,
      refusalReason: null,
    };
  }
  const evidenceRefs = uniqueSorted([...claim.evidenceRefs, ...command.evidenceRefs]);
  if (command.command === "release") {
    const next = {
      ...claim,
      state: "released" as const,
      lastEvidenceAt: request.occurredAt,
      evidenceRefs,
      revision: claim.revision + 1,
      releasedAt: request.occurredAt,
      releaserRef: request.effectivePrincipalRef,
      releaseEvidenceRefs: uniqueSorted(command.evidenceRefs),
    };
    const releasedPacket = {
      ...packet,
      state: "released" as const,
      revision: packet.revision + 1,
      updatedAt: request.occurredAt,
    };
    return {
      ledger: advance(ledger, request.occurredAt, {
        packets: replacePacket(ledger.packets, releasedPacket),
        claims: replaceClaim(ledger.claims, next),
        audit: event(
          revision,
          "released",
          packet.packetRef,
          claim.claimRef,
          request.effectivePrincipalRef,
          claim.generation,
          request.occurredAt,
          command.evidenceRefs,
          "Claim explicitly released; landing and verification remain separate facts.",
        ),
      }),
      claimRef: claim.claimRef,
      admitted: true,
      refusalReason: null,
    };
  }
  const blocked = command.command === "block";
  const next = {
    ...claim,
    state: blocked ? ("blocked" as const) : claim.state,
    lastEvidenceAt: request.occurredAt,
    evidenceRefs,
    revision: claim.revision + 1,
  };
  const nextPacket = blocked
    ? {
        ...packet,
        state: "blocked" as const,
        revision: packet.revision + 1,
        updatedAt: request.occurredAt,
      }
    : packet;
  const detail =
    command.command === "status" || command.command === "block"
      ? command.detail
      : "Claim heartbeat recorded; presence alone is not active-work proof.";
  return {
    ledger: advance(ledger, request.occurredAt, {
      packets: replacePacket(ledger.packets, nextPacket),
      claims: replaceClaim(ledger.claims, next),
      audit: event(
        revision,
        command.command === "heartbeat"
          ? "heartbeat"
          : command.command === "block"
            ? "blocked"
            : "status",
        packet.packetRef,
        claim.claimRef,
        request.effectivePrincipalRef,
        claim.generation,
        request.occurredAt,
        command.evidenceRefs,
        detail,
      ),
    }),
    claimRef: claim.claimRef,
    admitted: true,
    refusalReason: null,
  };
};

export type HistoricalRepositoryClaimComment = Readonly<{
  sourceRef: string;
  workRef: string;
  repositoryRef: string;
  authorRef: string;
  observedAt: string;
  body: string;
}>;

/**
 * Imports old GitHub CLAIM/CLAIM-STATUS/CLAIM-RELEASE comments as inert audit
 * history. The source-linked packet is canceled and therefore cannot be
 * claimed. Native packets and claims are never updated by this projection.
 */
export const importHistoricalRepositoryClaimComments = (
  current: RepositoryClaimLedger,
  comments: ReadonlyArray<HistoricalRepositoryClaimComment>,
  complete: boolean,
  observedAt: string,
): RepositoryClaimLedger => {
  let ledger = current;
  const importedSources = new Set(
    current.audit
      .filter((entry) => entry.kind === "historical_import")
      .flatMap((entry) => entry.evidenceRefs),
  );
  for (const comment of [...comments].sort((a, b) => a.sourceRef.localeCompare(b.sourceRef))) {
    if (importedSources.has(comment.sourceRef)) continue;
    if (!/^CLAIM(?:-STATUS|-RELEASE)?\b/u.test(comment.body.trimStart())) continue;
    const suffix = createHash("sha256").update(comment.sourceRef).digest("hex").slice(0, 16);
    const packet = decodeWorkPacket({
      packetRef: `work-packet:historical:${suffix}`,
      workRef: comment.workRef,
      repositoryRef: comment.repositoryRef,
      title: `Historical claim comment for ${comment.workRef}`,
      scope: "Historical GitHub claim comment. Read-only source projection.",
      ownedPaths: [],
      hotFiles: [],
      hotContracts: [],
      verification: "Historical projection only; no verification authority.",
      state: "canceled",
      revision: 1,
      createdAt: comment.observedAt,
      updatedAt: comment.observedAt,
    });
    const revision = ledger.revision + 1;
    ledger = advance(ledger, comment.observedAt, {
      packets: [...ledger.packets, packet],
      claims: ledger.claims,
      audit: event(
        revision,
        "historical_import",
        packet.packetRef,
        null,
        comment.authorRef,
        0,
        comment.observedAt,
        [comment.sourceRef],
        "Imported historical GitHub claim comment; no native claim authority.",
      ),
    });
    importedSources.add(comment.sourceRef);
  }
  if (complete) {
    return decodeRepositoryClaimLedger({
      ...ledger,
      completeness: { state: "complete", cursor: ledger.eventCursor, gapRefs: [] },
      freshness: { state: "fresh", observedAt },
    });
  }
  return decodeRepositoryClaimLedger({
    ...ledger,
    completeness: {
      state: "gap",
      cursor: ledger.eventCursor,
      gapRefs: ["source:github:claim-comments:page-gap"],
    },
    freshness: { state: "stale", observedAt },
  });
};

export interface RepositoryClaimAuthorityShape {
  readonly read: (
    input: unknown,
  ) => Effect.Effect<RepositoryClaimReadResult, RepositoryClaimAuthorityError>;
  readonly execute: (
    input: unknown,
  ) => Effect.Effect<RepositoryClaimExecuteResult, RepositoryClaimAuthorityError>;
}

export class RepositoryClaimAuthority extends Context.Service<
  RepositoryClaimAuthority,
  RepositoryClaimAuthorityShape
>()("RepositoryClaimAuthority.Service") {}

export const RepositoryClaimAuthorityLive = Layer.effect(
  RepositoryClaimAuthority,
  Effect.gen(function* () {
    const store = yield* RepositoryClaimStateStore;
    const load = store.load.pipe(
      Effect.flatMap((state) =>
        state === null
          ? Effect.fail(
              new RepositoryClaimAuthorityError({
                reason: "invalid_state",
                detail: "store is empty",
              }),
            )
          : Effect.succeed(state),
      ),
    );
    return RepositoryClaimAuthority.of({
      read: (input) =>
        Effect.gen(function* () {
          const request = yield* Effect.try({
            try: () => decodeRepositoryClaimReadRequest(input),
            catch: () =>
              new RepositoryClaimAuthorityError({ reason: "invalid_request", detail: "decode" }),
          });
          const state = yield* load;
          const packets = state.ledger.packets.filter(
            (packet) =>
              (request.repositoryRef == null || packet.repositoryRef === request.repositoryRef) &&
              (request.workRef == null || packet.workRef === request.workRef),
          );
          const packetRefs = new Set(packets.map((packet) => packet.packetRef));
          const ledger = decodeRepositoryClaimLedger({
            ...state.ledger,
            packets,
            claims: state.ledger.claims.filter((claim) => packetRefs.has(claim.packetRef)),
            audit: state.ledger.audit.filter((entry) => packetRefs.has(entry.packetRef)),
          });
          return yield* S.decodeUnknownEffect(RepositoryClaimReadResultSchema)({ ledger }).pipe(
            Effect.mapError(
              () => new RepositoryClaimAuthorityError({ reason: "invalid_state", detail: "read" }),
            ),
          );
        }),
      execute: (input) =>
        Effect.gen(function* () {
          const request = yield* Effect.try({
            try: () => decodeRepositoryClaimExecuteRequest(input),
            catch: () =>
              new RepositoryClaimAuthorityError({ reason: "invalid_request", detail: "decode" }),
          });
          const state = yield* load;
          const commandDigest = digest(request);
          const prior = state.receipts.find(
            (receipt) => receipt.idempotencyKey === request.idempotencyKey,
          );
          if (prior !== undefined) {
            if (prior.commandDigest !== commandDigest) {
              return yield* new RepositoryClaimAuthorityError({
                reason: "idempotency_conflict",
                detail: request.idempotencyKey,
              });
            }
            return yield* S.decodeUnknownEffect(RepositoryClaimExecuteResultSchema)({
              ledger: state.ledger,
              receipt: prior,
            }).pipe(
              Effect.mapError(
                () =>
                  new RepositoryClaimAuthorityError({ reason: "invalid_state", detail: "replay" }),
              ),
            );
          }
          if (state.ledger.revision !== request.expectedRevision) {
            return yield* new RepositoryClaimAuthorityError({
              reason: "revision_conflict",
              detail: `expected ${request.expectedRevision}, found ${state.ledger.revision}`,
            });
          }
          const transition = yield* Effect.try({
            try: () => applyRequest(state.ledger, request),
            catch: (error) =>
              error instanceof RepositoryClaimAuthorityError
                ? error
                : new RepositoryClaimAuthorityError({
                    reason: "invalid_request",
                    detail: "transition",
                  }),
          });
          const receipt = yield* S.decodeUnknownEffect(RepositoryClaimReceiptSchema)({
            requestRef: request.requestRef,
            idempotencyKey: request.idempotencyKey,
            commandDigest,
            previousRevision: state.ledger.revision,
            revision: transition.ledger.revision,
            eventCursor: transition.ledger.eventCursor,
            effectivePrincipalRef: request.effectivePrincipalRef,
            claimRef: transition.claimRef,
            acceptedAt: request.occurredAt,
            admitted: transition.admitted,
            refusalReason: transition.refusalReason,
            githubWriteCount: 0,
          }).pipe(
            Effect.mapError(
              () =>
                new RepositoryClaimAuthorityError({ reason: "invalid_state", detail: "receipt" }),
            ),
          );
          const next = yield* decodeState({
            schema: REPOSITORY_CLAIM_AUTHORITY_STATE_SCHEMA,
            ledger: transition.ledger,
            receipts: [...state.receipts, receipt],
          });
          yield* store.save(state.ledger.revision, next);
          return yield* S.decodeUnknownEffect(RepositoryClaimExecuteResultSchema)({
            ledger: next.ledger,
            receipt,
          }).pipe(
            Effect.mapError(
              () =>
                new RepositoryClaimAuthorityError({ reason: "invalid_state", detail: "result" }),
            ),
          );
        }),
    });
  }),
);

export const inMemoryRepositoryClaimStateStoreLayer = (
  initial: RepositoryClaimAuthorityState,
): Layer.Layer<RepositoryClaimStateStore> =>
  Layer.effect(
    RepositoryClaimStateStore,
    Effect.gen(function* () {
      const state = yield* Ref.make(initial);
      return RepositoryClaimStateStore.of({
        load: Ref.get(state),
        save: (expectedRevision, next) =>
          Ref.modify(state, (current) =>
            current.ledger.revision !== expectedRevision
              ? [
                  Effect.fail(
                    new RepositoryClaimAuthorityError({
                      reason: "revision_conflict",
                      detail: `expected ${expectedRevision}, found ${current.ledger.revision}`,
                    }),
                  ),
                  current,
                ]
              : [Effect.void, next],
          ).pipe(Effect.flatten),
      });
    }),
  );

export const repositoryClaimStatePath = (rootDir: string): string =>
  path.join(rootDir, "all-work", "repository-claims.v1.json");

const storageError = (detail: string) =>
  new RepositoryClaimAuthorityError({ reason: "storage_unavailable", detail });
const isNotFound = (error: unknown): boolean =>
  typeof error === "object" && error !== null && Reflect.get(error, "code") === "ENOENT";

const readFileState = (
  filePath: string,
): Effect.Effect<RepositoryClaimAuthorityState | null, RepositoryClaimAuthorityError> =>
  Effect.tryPromise({
    try: () => readFile(filePath, "utf8"),
    catch: (error) => (isNotFound(error) ? storageError("not_found") : storageError("read")),
  }).pipe(
    Effect.flatMap((contents) =>
      Effect.try({ try: () => JSON.parse(contents), catch: () => storageError("json") }),
    ),
    Effect.flatMap(decodeState),
    Effect.catch((error) =>
      error.detail === "not_found" ? Effect.succeed(null) : Effect.fail(error),
    ),
  );

const atomicWrite = (
  filePath: string,
  state: RepositoryClaimAuthorityState,
): Effect.Effect<void, RepositoryClaimAuthorityError> =>
  Effect.tryPromise({
    try: async () => {
      const directory = path.dirname(filePath);
      await mkdir(directory, { recursive: true });
      const temporary = path.join(
        directory,
        `.${path.basename(filePath)}.${randomBytes(8).toString("hex")}.tmp`,
      );
      await writeFile(temporary, `${JSON.stringify(state, null, 2)}\n`, {
        encoding: "utf8",
        mode: 0o600,
      });
      await rename(temporary, filePath);
    },
    catch: () => storageError("write"),
  });

export const fileRepositoryClaimStateStoreLayer = (
  rootDir: string,
): Layer.Layer<RepositoryClaimStateStore> => {
  const filePath = repositoryClaimStatePath(rootDir);
  const load = readFileState(filePath);
  return Layer.succeed(
    RepositoryClaimStateStore,
    RepositoryClaimStateStore.of({
      load,
      save: (expectedRevision, state) =>
        Effect.gen(function* () {
          const current = yield* load;
          if (current === null || current.ledger.revision !== expectedRevision) {
            return yield* new RepositoryClaimAuthorityError({
              reason: "revision_conflict",
              detail: `expected ${expectedRevision}, found ${current?.ledger.revision ?? "none"}`,
            });
          }
          yield* atomicWrite(filePath, state);
        }),
    }),
  );
};

export const initializeFileRepositoryClaimState = (
  rootDir: string,
  state: RepositoryClaimAuthorityState,
): Effect.Effect<void, RepositoryClaimAuthorityError> => {
  const filePath = repositoryClaimStatePath(rootDir);
  return readFileState(filePath).pipe(
    Effect.flatMap((current) => (current === null ? atomicWrite(filePath, state) : Effect.void)),
  );
};
