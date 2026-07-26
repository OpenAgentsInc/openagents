import { Context, Effect, Layer, Schema } from "effect";
import { verifyEvent, type Event as NostrEvent } from "nostr-effect/pure";

import {
  ForgeGitAdmission,
  type ForgeGitAdmissionEvent,
} from "./admission.js";
import { ForgeGitRepository } from "./repository.js";

export class ForgeGitProjectorError extends Schema.TaggedErrorClass<ForgeGitProjectorError>()(
  "ForgeGitProjectorError",
  { code: Schema.String },
) {}

export interface ForgeGitProjectorShape {
  /**
   * Projects a signature-verified NIP-34 event from the trusted membership
   * boundary. The actor binding is never inferred from a pubkey alone.
   */
  readonly project: (input: {
    readonly actorBindingRef: string;
    readonly event: NostrEvent;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<"authorized" | "purgatory", ForgeGitProjectorError>;
  /** Resolves object-backed pending events and expires stale rows deterministically. */
  readonly reconcile: (input: {
    readonly nowIso: string;
    readonly repositoryRef: string;
    readonly tenantRef: string;
  }) => Effect.Effect<Readonly<{ expired: number; resolved: number }>, ForgeGitProjectorError>;
}

export class ForgeGitProjector extends Context.Service<ForgeGitProjector, ForgeGitProjectorShape>()(
  "@openagentsinc/forge-git-service/Projector",
) {}

const zeroObjectId = "0".repeat(40);
const oid = /^[0-9a-f]{40,64}$/u;
const safeRef = /^refs\/(?:heads|tags)\/[A-Za-z0-9][A-Za-z0-9._/-]{0,240}$/u;
/**
 * NIP-34 repository announcements and state use a `d` identifier. Contribution
 * and pull-request events point back to that coordinate with an `a` tag.
 * Treating the latter as a `d` tag silently discarded valid object-first
 * proposals, so normalize both forms at this boundary.
 */
const repoTag = (event: NostrEvent): string | undefined => {
  const identifier = event.tags.find((tag) => tag[0] === "d")?.[1];
  if (identifier !== undefined) return identifier;
  const coordinate = event.tags.find((tag) => tag[0] === "a")?.[1];
  if (coordinate === undefined) return undefined;
  const [kind, _pubkey, repositoryRef] = coordinate.split(":", 3);
  return kind === "30617" ? repositoryRef : undefined;
};
const tagValues = (event: NostrEvent, name: string): ReadonlyArray<string> =>
  event.tags.filter((tag) => tag[0] === name).flatMap((tag) => tag.slice(1));
const objectIds = (event: NostrEvent): ReadonlyArray<string> => {
  if (event.kind === 30617) {
    return event.tags
      .filter((tag) => tag[0] === "r" && tag[2] === "euc")
      .map((tag) => tag[1])
      .filter((value): value is string => value !== undefined && oid.test(value));
  }
  if (event.kind === 30618) {
    return event.tags
      .filter((tag) => tag[0] !== undefined && tag[0].startsWith("refs/"))
      .map((tag) => tag[1])
      .filter((value): value is string => value !== undefined && oid.test(value));
  }
  const commits = tagValues(event, "c").filter((value) => oid.test(value));
  const emailPatchCommit = /^From ([0-9a-f]{40,64}) /mu.exec(event.content)?.[1];
  return emailPatchCommit !== undefined && oid.test(emailPatchCommit)
    ? [...new Set([...commits, emailPatchCommit])]
    : commits;
};

const eventJson = (event: NostrEvent): string => JSON.stringify(event);
const purgeDeadline = (event: NostrEvent): string =>
  new Date((event.created_at + (event.kind === 1618 || event.kind === 1619 ? 20 * 60 : 30 * 60)) * 1000).toISOString();

const asAdmissionEvent = (
  event: NostrEvent,
  repositoryRef: string,
  tenantRef: string,
): ForgeGitAdmissionEvent => ({
  createdAt: new Date(event.created_at * 1000).toISOString(),
  eventId: event.id,
  kind: event.kind as 30617 | 30618 | 1617 | 1618 | 1619,
  objectIds: objectIds(event),
  repositoryRef,
  tenantRef,
});

const projectorError = (code: string) => new ForgeGitProjectorError({ code });

export const layerProjector = Layer.effect(
  ForgeGitProjector,
  Effect.gen(function* () {
    const admission = yield* ForgeGitAdmission;
    const repository = yield* ForgeGitRepository;

    const project = Effect.fn("ForgeGitProjector.project")(function* (
      input: Parameters<ForgeGitProjectorShape["project"]>[0],
    ) {
      const { event } = input;
      if (!verifyEvent(event)) return yield* projectorError("forge_git_nostr_signature_invalid");
      if (![30617, 30618, 1617, 1618, 1619].includes(event.kind)) {
        return yield* projectorError("forge_git_nip34_kind_unsupported");
      }
      if (repoTag(event) !== input.repositoryRef) {
        return yield* projectorError("forge_git_nip34_repository_mismatch");
      }
      const fact = asAdmissionEvent(event, input.repositoryRef, input.tenantRef);
      if (event.kind === 30617) {
        const maintainers = tagValues(event, "maintainers");
        if (!maintainers.includes(event.pubkey) || maintainers.some((value) => !/^[0-9a-f]{64}$/u.test(value))) {
          return yield* projectorError("forge_git_announcement_maintainers_invalid");
        }
        yield* admission.admitRepository({
          admittedBindingRef: input.actorBindingRef,
          announcementAuthorPubkey: event.pubkey,
          announcementEventId: event.id,
          maintainerPubkeys: maintainers,
          repositoryRef: input.repositoryRef,
          tenantRef: input.tenantRef,
        }).pipe(Effect.mapError(() => projectorError("forge_git_admission_write_failed")));
        yield* repository.provision(input).pipe(Effect.mapError(() => projectorError("forge_git_repository_provision_failed")));
      } else {
        yield* admission.requireRepository(input).pipe(Effect.mapError(() => projectorError("forge_git_repository_not_admitted")));
      }
      const present = yield* repository.hasObjects({ ...input, objectIds: fact.objectIds }).pipe(
        Effect.mapError(() => projectorError("forge_git_object_lookup_failed")),
      );
      if (present.size !== fact.objectIds.length) {
        yield* admission.holdPurgatory({
          ...fact,
          actorBindingRef: input.actorBindingRef,
          eventJson: eventJson(event),
          expiresAt: purgeDeadline(event),
        }).pipe(
          Effect.mapError(() => projectorError("forge_git_purgatory_write_failed")),
        );
        yield* admission.claimNostrEvent({
          eventId: event.id,
          repositoryRef: input.repositoryRef,
          tenantRef: input.tenantRef,
        }).pipe(
          Effect.mapError(() => projectorError("forge_git_nostr_claim_write_failed")),
        );
        return "purgatory" as const;
      }
      if (event.kind === 30618) {
        const current = yield* admission.signedRefPolicies(input).pipe(
          Effect.mapError(() => projectorError("forge_git_state_lookup_failed")),
        );
        for (const tag of event.tags.filter((candidate) => candidate[0] !== undefined && candidate[0].startsWith("refs/"))) {
          const [refName, newObjectId] = tag;
          if (refName === undefined || newObjectId === undefined || !safeRef.test(refName) || !oid.test(newObjectId)) {
            return yield* projectorError("forge_git_state_ref_invalid");
          }
          const oldObjectId = current.find((policy) => policy.refName === refName)?.newObjectId ?? zeroObjectId;
          yield* admission.authorizeSignedRefState({
            authorPubkey: event.pubkey,
            eventId: event.id,
            eventJson: eventJson(event),
            newObjectId,
            oldObjectId,
            refName,
            repositoryRef: input.repositoryRef,
            tenantRef: input.tenantRef,
          }).pipe(Effect.mapError(() => projectorError("forge_git_signed_state_refused")));
        }
      }
      yield* admission.claimNostrEvent({
        eventId: event.id,
        repositoryRef: input.repositoryRef,
        tenantRef: input.tenantRef,
      }).pipe(Effect.mapError(() => projectorError("forge_git_nostr_claim_write_failed")));
      return "authorized" as const;
    });

    return ForgeGitProjector.of({
      project,
      reconcile: Effect.fn("ForgeGitProjector.reconcile")(function* (input) {
        const expired = yield* admission.expirePurgatory(input).pipe(
          Effect.mapError(() => projectorError("forge_git_purgatory_expiry_failed")),
        );
        const pending = yield* admission.listPurgatory(input).pipe(
          Effect.mapError(() => projectorError("forge_git_purgatory_read_failed")),
        );
        let resolved = 0;
        for (const entry of pending) {
          const present = yield* repository.hasObjects({ ...input, objectIds: entry.objectIds }).pipe(
            Effect.mapError(() => projectorError("forge_git_object_lookup_failed")),
          );
          if (present.size !== entry.objectIds.length) continue;
          const event = JSON.parse(entry.eventJson) as NostrEvent;
          yield* project({
            actorBindingRef: entry.actorBindingRef,
            event,
            repositoryRef: entry.repositoryRef,
            tenantRef: entry.tenantRef,
          });
          // Resolution is the durable acknowledgement of successful projection,
          // never a precondition. A transient policy/signature failure must leave
          // the event pending for a safe retry instead of making it disappear.
          yield* admission.resolvePurgatory(entry).pipe(
            Effect.mapError(() => projectorError("forge_git_purgatory_resolve_failed")),
          );
          resolved += 1;
        }
        return { expired, resolved };
      }),
    });
  }),
);

export const layerNoopProjector = Layer.succeed(
  ForgeGitProjector,
  ForgeGitProjector.of({
    project: () => Effect.fail(projectorError("forge_git_projector_unavailable")),
    reconcile: () => Effect.succeed({ expired: 0, resolved: 0 }),
  }),
);
