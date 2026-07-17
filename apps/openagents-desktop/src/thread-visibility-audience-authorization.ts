import {
  decodeThreadDisclosureReceipt,
  ThreadDisclosureReceipt,
  type ThreadDisclosureTarget,
} from "@openagentsinc/agent-runtime-schema";
import { Schema as S } from "effect";

export const DesktopThreadVisibilityAuthorizationRequestSchemaLiteral =
  "openagents.desktop_thread_visibility_authorization_request.v1" as const;

const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);

const AuthorityFact = S.Struct({
  workspaceRef: Ref,
  role: S.Literals(["member", "administrator"]),
  groupRefs: S.Array(Ref).check(S.isMaxLength(64)),
});
type AuthorityFact = typeof AuthorityFact.Type;

const AuthorizationRequest = S.Struct({
  schema: S.Literal(DesktopThreadVisibilityAuthorizationRequestSchemaLiteral),
  actorRef: Ref,
  ownerRef: Ref,
  receipt: ThreadDisclosureReceipt,
  authorities: S.Array(AuthorityFact).check(S.isMaxLength(32)),
});
type AuthorizationRequest = typeof AuthorizationRequest.Type;

type DesktopThreadVisibilityAuthorizationBasis =
  | "owner"
  | "internet_readable"
  | "workspace_member"
  | "named_group"
  | "workspace_administrator";

export type DesktopThreadVisibilityAuthorizationDecision =
  | Readonly<{
      status: "authorized";
      basis: DesktopThreadVisibilityAuthorizationBasis;
      receiptRef: string;
      threadRef: string;
      visibilityVersion: number;
    }>
  | Readonly<{
      status: "denied";
      reason: "no_matching_authority";
      receiptRef: string;
      threadRef: string;
      visibilityVersion: number;
    }>
  | Readonly<{ status: "rejected"; reason: "invalid_request" }>;

const field = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;

const ownKeysAre = (value: unknown, allowed: ReadonlyArray<string>): boolean => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const actual = Object.keys(value).sort();
  const expected = [...allowed].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
};

const exactAudience = (value: unknown): boolean => {
  const kind = field(value, "kind");
  if (kind === "owner_only" || kind === "internet_readable") {
    return ownKeysAre(value, ["kind"]);
  }
  if (kind === "workspace_members") return ownKeysAre(value, ["kind", "workspaceRef"]);
  if (kind === "named_group") {
    return ownKeysAre(value, ["kind", "workspaceRef", "groupRef"]);
  }
  return false;
};

const exactAdministratorAccess = (value: unknown): boolean => {
  const kind = field(value, "kind");
  return kind === "none"
    ? ownKeysAre(value, ["kind"])
    : kind === "workspace_admins"
      ? ownKeysAre(value, ["kind", "workspaceRef"])
      : false;
};

const exactTarget = (value: unknown): boolean =>
  ownKeysAre(value, ["audience", "administratorAccess"]) &&
  exactAudience(field(value, "audience")) &&
  exactAdministratorAccess(field(value, "administratorAccess"));

const exactVisibilityReceipt = (value: unknown): boolean =>
  ownKeysAre(value, [
    "schema",
    "receiptRef",
    "intentRef",
    "idempotencyKey",
    "threadRef",
    "observedAt",
    "kind",
    "result",
  ]) &&
  ownKeysAre(field(value, "result"), ["status", "visibilityVersion", "target"]) &&
  exactTarget(field(field(value, "result"), "target"));

const exactAuthorityFact = (value: unknown): boolean => {
  if (!ownKeysAre(value, ["workspaceRef", "role", "groupRefs"])) return false;
  const groupRefs = field(value, "groupRefs");
  return Array.isArray(groupRefs) && new Set(groupRefs).size === groupRefs.length;
};

const decodeRequest = (input: unknown): AuthorizationRequest | null => {
  if (!ownKeysAre(input, ["schema", "actorRef", "ownerRef", "receipt", "authorities"])) {
    return null;
  }
  const rawReceipt = field(input, "receipt");
  const rawAuthorities = field(input, "authorities");
  if (
    !exactVisibilityReceipt(rawReceipt) ||
    !Array.isArray(rawAuthorities) ||
    !rawAuthorities.every(exactAuthorityFact)
  ) {
    return null;
  }
  try {
    const decoded = S.decodeUnknownSync(AuthorizationRequest)(input);
    const receipt = decodeThreadDisclosureReceipt(decoded.receipt);
    if (
      receipt.kind !== "thread.visibility.set" ||
      receipt.result.status !== "visibility_applied"
    ) {
      return null;
    }
    const workspaceRefs = decoded.authorities.map((fact) => fact.workspaceRef);
    if (new Set(workspaceRefs).size !== workspaceRefs.length) return null;
    return { ...decoded, receipt };
  } catch {
    return null;
  }
};

const matchingWorkspaceFact = (
  authorities: ReadonlyArray<AuthorityFact>,
  workspaceRef: string,
): AuthorityFact | undefined => authorities.find((fact) => fact.workspaceRef === workspaceRef);

const authorizeTarget = (
  actorRef: string,
  ownerRef: string,
  target: ThreadDisclosureTarget,
  authorities: ReadonlyArray<AuthorityFact>,
): DesktopThreadVisibilityAuthorizationBasis | null => {
  if (actorRef === ownerRef) return "owner";
  if (target.audience.kind === "internet_readable") return "internet_readable";
  if (target.audience.kind === "workspace_members") {
    if (matchingWorkspaceFact(authorities, target.audience.workspaceRef) !== undefined) {
      return "workspace_member";
    }
  }
  if (target.audience.kind === "named_group") {
    const fact = matchingWorkspaceFact(authorities, target.audience.workspaceRef);
    if (fact?.groupRefs.includes(target.audience.groupRef) === true) return "named_group";
  }
  if (target.administratorAccess.kind === "workspace_admins") {
    const fact = matchingWorkspaceFact(authorities, target.administratorAccess.workspaceRef);
    if (fact?.role === "administrator") return "workspace_administrator";
  }
  return null;
};

/**
 * Pure decision over one applied visibility receipt and caller-supplied
 * authority facts. Authorization does not fetch, transport, or publish data.
 */
export const evaluateDesktopThreadVisibilityAudience = (
  input: unknown,
): DesktopThreadVisibilityAuthorizationDecision => {
  const request = decodeRequest(input);
  if (request === null) return { status: "rejected", reason: "invalid_request" };
  if (request.receipt.result.status !== "visibility_applied") {
    return { status: "rejected", reason: "invalid_request" };
  }
  const binding = {
    receiptRef: request.receipt.receiptRef,
    threadRef: request.receipt.threadRef,
    visibilityVersion: request.receipt.result.visibilityVersion,
  } as const;
  const basis = authorizeTarget(
    request.actorRef,
    request.ownerRef,
    request.receipt.result.target,
    request.authorities,
  );
  return basis === null
    ? { status: "denied", reason: "no_matching_authority", ...binding }
    : { status: "authorized", basis, ...binding };
};
