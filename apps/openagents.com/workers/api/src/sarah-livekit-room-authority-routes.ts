import { Schema as S } from "effect";

import type { SarahLiveKitRoomAuthorityStore } from "@openagentsinc/khala-sync-server";
import { SarahLiveKitRoomAuthorityStoreError } from "@openagentsinc/khala-sync-server";

import {
  bargeInSarahLiveKitFloor,
  requestSarahLiveKitFloor,
  stopSarahLiveKitFloor,
  transferSarahLiveKitFloor,
  type SarahLiveKitFloorDecision,
  type SarahLiveKitRoomMemberAccess,
} from "./sarah-livekit-room-authority.js";

const MAX_BODY_BYTES = 16_384;
const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Digest = S.String.check(S.isPattern(/^[a-f0-9]{64}$/u));
const Nonce = S.String.check(S.isPattern(/^[A-Za-z0-9_-]{32,256}$/u));
const Revision = S.Int.check(S.isGreaterThanOrEqualTo(1));
const LeaseMs = S.Int.check(S.isGreaterThanOrEqualTo(1), S.isLessThanOrEqualTo(30_000));

const MemberRequestSchema = S.Union([
  S.Struct({
    action: S.Literal("acquire"),
    presenceLeaseRef: Ref,
    expectedRevision: Revision,
    nonce: Nonce,
    requestedLeaseMs: LeaseMs,
  }),
  S.Struct({
    action: S.Literal("barge_in"),
    presenceLeaseRef: Ref,
    expectedRevision: Revision,
    nonce: Nonce,
  }),
  S.Struct({
    action: S.Literal("transfer"),
    presenceLeaseRef: Ref,
    expectedRevision: Revision,
    nonce: Nonce,
    requestedLeaseMs: LeaseMs,
    targetUserRefDigest: Digest,
  }),
]);

const ModeratorRequestSchema = S.Struct({
  action: S.Literal("stop"),
  presenceLeaseRef: Ref,
  expectedRevision: Revision,
  nonce: Nonce,
});

type AuthenticatedSarahRoomUser = Readonly<{
  userId: string;
  userRefDigest: string;
}>;

export type SarahLiveKitRoomAuthorityRouteDependencies<Environment> = Readonly<{
  store: SarahLiveKitRoomAuthorityStore;
  authenticate: (
    request: Request,
    environment: Environment,
  ) => Promise<AuthenticatedSarahRoomUser | undefined>;
  resolveMember: (
    environment: Environment,
    input: Readonly<{
      userId: string;
      presenceLeaseRef: string;
      targetUserRefDigest?: string;
    }>,
  ) => Promise<SarahLiveKitRoomMemberAccess | undefined>;
  now: () => Date;
}>;

const json = (status: number, body: unknown): Response =>
  Response.json(body, {
    status,
    headers: {
      "cache-control": "no-store",
    },
  });

const decodeBody = async <A>(
  request: Request,
  decode: (value: unknown) => A,
): Promise<A | undefined> => {
  const declaredLength = Number(request.headers.get("content-length") ?? "0");
  if (!Number.isFinite(declaredLength) || declaredLength > MAX_BODY_BYTES) return undefined;
  const text = await request.text();
  if (new TextEncoder().encode(text).byteLength > MAX_BODY_BYTES) return undefined;
  try {
    return decode(JSON.parse(text));
  } catch {
    return undefined;
  }
};

const authenticatedMember = async <Environment>(
  request: Request,
  environment: Environment,
  dependencies: SarahLiveKitRoomAuthorityRouteDependencies<Environment>,
  presenceLeaseRef: string,
): Promise<
  | Readonly<{
      identity: AuthenticatedSarahRoomUser;
      member: SarahLiveKitRoomMemberAccess;
    }>
  | Response
> => {
  const identity = await dependencies.authenticate(request, environment);
  if (identity === undefined) return json(401, { error: "authentication_required" });
  const member = await dependencies.resolveMember(environment, {
    userId: identity.userId,
    presenceLeaseRef,
  });
  if (member === undefined || member.userRefDigest !== identity.userRefDigest) {
    return json(403, { error: "room_membership_required" });
  }
  return { identity, member };
};

const persistDecision = async <Environment>(
  dependencies: SarahLiveKitRoomAuthorityRouteDependencies<Environment>,
  presenceLeaseRef: string,
  expectedRevision: number,
  decision: SarahLiveKitFloorDecision<unknown>,
): Promise<Response> => {
  if (!decision.accepted) {
    const forbidden = [
      "member_not_authenticated",
      "member_not_allowlisted",
      "member_removed",
      "participant_mismatch",
      "membership_changed",
      "moderator_required",
      "transfer_target_invalid",
    ].includes(decision.reason);
    return json(forbidden ? 403 : 409, { error: decision.reason });
  }
  try {
    const snapshot = await dependencies.store.compareAndSwap({
      presenceLeaseRef,
      expectedRevision,
      snapshot: decision.snapshot,
      now: dependencies.now().toISOString(),
    });
    return json(200, { value: decision.value, revision: snapshot.revision });
  } catch (error) {
    if (error instanceof SarahLiveKitRoomAuthorityStoreError) {
      if (error.reason === "not_found") return json(404, { error: error.reason });
      if (error.reason === "stale_revision" || error.reason === "conflict") {
        return json(409, { error: error.reason });
      }
    }
    return json(503, { error: "authority_unavailable" });
  }
};

const readSnapshot = async (store: SarahLiveKitRoomAuthorityStore, presenceLeaseRef: string) => {
  try {
    const snapshot = await store.read(presenceLeaseRef);
    return snapshot ?? json(404, { error: "authority_not_found" });
  } catch {
    return json(503, { error: "authority_unavailable" });
  }
};

export const handleSarahLiveKitRoomMemberFloorRequest = async <Environment>(
  request: Request,
  environment: Environment,
  dependencies: SarahLiveKitRoomAuthorityRouteDependencies<Environment>,
): Promise<Response> => {
  if (request.method !== "POST") return json(405, { error: "method_not_allowed" });
  const body = await decodeBody(request, (value) =>
    S.decodeUnknownSync(MemberRequestSchema)(value, { onExcessProperty: "error" }),
  );
  if (body === undefined) return json(400, { error: "invalid_request" });
  const resolved = await authenticatedMember(
    request,
    environment,
    dependencies,
    body.presenceLeaseRef,
  );
  if (resolved instanceof Response) return resolved;
  const snapshot = await readSnapshot(dependencies.store, body.presenceLeaseRef);
  if (snapshot instanceof Response) return snapshot;
  if (snapshot.revision !== body.expectedRevision) {
    return json(409, { error: "stale_revision" });
  }
  const nowMs = dependencies.now().getTime();
  if (body.action === "acquire") {
    return persistDecision(
      dependencies,
      body.presenceLeaseRef,
      body.expectedRevision,
      requestSarahLiveKitFloor(snapshot, {
        member: resolved.member,
        nonce: body.nonce,
        requestedLeaseMs: body.requestedLeaseMs,
        nowMs,
      }),
    );
  }
  if (body.action === "barge_in") {
    return persistDecision(
      dependencies,
      body.presenceLeaseRef,
      body.expectedRevision,
      bargeInSarahLiveKitFloor(snapshot, {
        member: resolved.member,
        nonce: body.nonce,
        nowMs,
      }),
    );
  }
  const target = await dependencies.resolveMember(environment, {
    userId: resolved.identity.userId,
    presenceLeaseRef: body.presenceLeaseRef,
    targetUserRefDigest: body.targetUserRefDigest,
  });
  if (target === undefined || target.userRefDigest !== body.targetUserRefDigest) {
    return json(403, { error: "transfer_target_invalid" });
  }
  return persistDecision(
    dependencies,
    body.presenceLeaseRef,
    body.expectedRevision,
    transferSarahLiveKitFloor(snapshot, {
      actor: resolved.member,
      target,
      nonce: body.nonce,
      requestedLeaseMs: body.requestedLeaseMs,
      nowMs,
    }),
  );
};

export const handleSarahLiveKitRoomModeratorFloorRequest = async <Environment>(
  request: Request,
  environment: Environment,
  dependencies: SarahLiveKitRoomAuthorityRouteDependencies<Environment>,
): Promise<Response> => {
  if (request.method !== "POST") return json(405, { error: "method_not_allowed" });
  const body = await decodeBody(request, (value) =>
    S.decodeUnknownSync(ModeratorRequestSchema)(value, { onExcessProperty: "error" }),
  );
  if (body === undefined) return json(400, { error: "invalid_request" });
  const resolved = await authenticatedMember(
    request,
    environment,
    dependencies,
    body.presenceLeaseRef,
  );
  if (resolved instanceof Response) return resolved;
  const snapshot = await readSnapshot(dependencies.store, body.presenceLeaseRef);
  if (snapshot instanceof Response) return snapshot;
  if (snapshot.revision !== body.expectedRevision) {
    return json(409, { error: "stale_revision" });
  }
  return persistDecision(
    dependencies,
    body.presenceLeaseRef,
    body.expectedRevision,
    stopSarahLiveKitFloor(snapshot, {
      moderator: resolved.member,
      nonce: body.nonce,
      nowMs: dependencies.now().getTime(),
    }),
  );
};
