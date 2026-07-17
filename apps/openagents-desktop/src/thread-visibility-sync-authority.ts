import {
  decodeKhalaCodeTeamMembershipEntity,
  KHALA_CODE_TEAM_MEMBERSHIP_ENTITY_TYPE,
  SyncScope,
} from "@openagentsinc/khala-sync";
import {
  decodeThreadDisclosureReceipt,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";
import type {
  ConfirmedEntity,
  KhalaSyncLocalStore,
  KhalaSyncSession,
} from "@openagentsinc/khala-sync-client";
import { Effect, Schema as S } from "effect";

import {
  DesktopThreadVisibilityAuthorizationRequestSchemaLiteral,
  evaluateDesktopThreadVisibilityAudience,
  type DesktopThreadVisibilityAuthorizationDecision,
} from "./thread-visibility-audience-authorization.ts";

export const DesktopThreadVisibilitySyncAuthorizationRequestSchemaLiteral =
  "openagents.desktop_thread_visibility_sync_authorization_request.v1" as const;

export type DesktopThreadVisibilitySyncAuthorizationDecision =
  | DesktopThreadVisibilityAuthorizationDecision
  | Readonly<{ status: "rejected"; reason: "authority_unavailable" }>;

export type DesktopThreadVisibilitySyncAuthorityDependencies = Readonly<{
  session: Pick<KhalaSyncSession, "lastDeltaAt" | "state">;
  store: Pick<KhalaSyncLocalStore, "readEntities">;
}>;

const field = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;

const ownKeysAre = (value: unknown, allowed: ReadonlyArray<string>): boolean => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const actual = Object.keys(value).sort();
  const expected = [...allowed].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
};

const initialDecision = (
  input: unknown,
): Readonly<{
  actorRef: unknown;
  ownerRef: unknown;
  receipt: unknown;
  decision: DesktopThreadVisibilityAuthorizationDecision;
}> | null => {
  if (
    !ownKeysAre(input, ["schema", "actorRef", "ownerRef", "receipt"]) ||
    field(input, "schema") !== DesktopThreadVisibilitySyncAuthorizationRequestSchemaLiteral
  ) {
    return null;
  }
  const actorRef = field(input, "actorRef");
  const ownerRef = field(input, "ownerRef");
  const receipt = field(input, "receipt");
  return {
    actorRef,
    ownerRef,
    receipt,
    decision: evaluateDesktopThreadVisibilityAudience({
      schema: DesktopThreadVisibilityAuthorizationRequestSchemaLiteral,
      actorRef,
      ownerRef,
      receipt,
      authorities: [],
    }),
  };
};

const targetWorkspaceRef = (receipt: ThreadDisclosureReceipt): string | null => {
  if (receipt.kind !== "thread.visibility.set" || receipt.result.status !== "visibility_applied") {
    return null;
  }
  const { target } = receipt.result;
  if (target.audience.kind === "workspace_members" || target.audience.kind === "named_group") {
    return target.audience.workspaceRef;
  }
  return target.administratorAccess.kind === "workspace_admins"
    ? target.administratorAccess.workspaceRef
    : null;
};

const decodeTeamScope = (
  workspaceRef: string,
): Readonly<{ scope: SyncScope; teamId: string }> | null => {
  if (!workspaceRef.startsWith("scope.team.")) return null;
  const teamId = workspaceRef.slice("scope.team.".length);
  if (teamId.length === 0) return null;
  try {
    return { scope: S.decodeUnknownSync(SyncScope)(workspaceRef), teamId };
  } catch {
    return null;
  }
};

const liveConfirmed = (
  dependencies: DesktopThreadVisibilitySyncAuthorityDependencies,
  scope: SyncScope,
): boolean =>
  dependencies.session.state(scope).phase === "live" &&
  dependencies.session.lastDeltaAt(scope) !== null;

const decodeMemberships = (
  entities: ReadonlyArray<ConfirmedEntity>,
  teamId: string,
): ReadonlyArray<ReturnType<typeof decodeKhalaCodeTeamMembershipEntity>> | null => {
  const decoded = [];
  try {
    for (const entity of entities) {
      if (entity.entityType !== KHALA_CODE_TEAM_MEMBERSHIP_ENTITY_TYPE) return null;
      const membership = decodeKhalaCodeTeamMembershipEntity(JSON.parse(entity.postImageJson));
      if (membership.teamId !== teamId || membership.membershipId !== entity.entityId) return null;
      decoded.push(membership);
    }
    return decoded;
  } catch {
    return null;
  }
};

/**
 * Resolve FF-D1-23 authority only from a live, server-confirmed Khala Sync
 * team-membership projection. The returned decision remains ref-only.
 */
export const evaluateDesktopThreadVisibilityFromConfirmedSync = Effect.fn(
  "DesktopThreadVisibilitySyncAuthority.evaluate",
)(function* (dependencies: DesktopThreadVisibilitySyncAuthorityDependencies, input: unknown) {
  const initial = initialDecision(input);
  if (initial === null || initial.decision.status === "rejected") {
    return { status: "rejected", reason: "invalid_request" };
  }
  if (initial.decision.status === "authorized") return initial.decision;

  let receipt: ThreadDisclosureReceipt;
  try {
    receipt = decodeThreadDisclosureReceipt(initial.receipt);
  } catch {
    return { status: "rejected", reason: "invalid_request" };
  }
  const workspaceRef = targetWorkspaceRef(receipt);
  if (workspaceRef === null) return initial.decision;
  const teamScope = decodeTeamScope(workspaceRef);
  if (teamScope === null || !liveConfirmed(dependencies, teamScope.scope)) {
    return { status: "rejected", reason: "authority_unavailable" };
  }

  const entities = yield* dependencies.store
    .readEntities(teamScope.scope, KHALA_CODE_TEAM_MEMBERSHIP_ENTITY_TYPE)
    .pipe(Effect.option);
  if (entities._tag === "None" || !liveConfirmed(dependencies, teamScope.scope)) {
    return { status: "rejected", reason: "authority_unavailable" };
  }
  const memberships = decodeMemberships(entities.value, teamScope.teamId);
  if (memberships === null) return { status: "rejected", reason: "authority_unavailable" };

  const actorMemberships = memberships.filter(
    (membership) => membership.userId === initial.actorRef,
  );
  if (actorMemberships.length > 1) {
    return { status: "rejected", reason: "authority_unavailable" };
  }
  const membership = actorMemberships[0];
  const authorities =
    membership === undefined || membership.status !== "active"
      ? []
      : [
          {
            workspaceRef,
            role:
              membership.role === "owner" || membership.role === "admin"
                ? ("administrator" as const)
                : ("member" as const),
            groupRefs: [],
          },
        ];
  return evaluateDesktopThreadVisibilityAudience({
    schema: DesktopThreadVisibilityAuthorizationRequestSchemaLiteral,
    actorRef: initial.actorRef,
    ownerRef: initial.ownerRef,
    receipt: initial.receipt,
    authorities,
  });
});
