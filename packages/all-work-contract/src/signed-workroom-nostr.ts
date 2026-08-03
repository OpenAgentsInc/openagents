import { createHash } from "node:crypto";
import { schnorr } from "@noble/curves/secp256k1";

import type { SignedWorkroomActivity, SignedWorkroomActivityKind } from "./generated.ts";

export const SIGNED_WORKROOM_NOSTR_PROFILE = "openagents.signed-workroom.v2" as const;
export const LEGACY_SIGNED_WORKROOM_NOSTR_PROFILE = "openagents.signed-workroom.v1" as const;

const kinds: Record<SignedWorkroomActivityKind, number> = {
  membership: 32150,
  thread: 32151,
  mention: 32152,
  assignment: 32153,
  delegation: 32154,
  agent_session: 32155,
  agent_activity: 32156,
  code_change: 32157,
  review: 32158,
  decision: 32159,
  evidence: 32160,
  verification_ref: 32161,
  receipt_ref: 32162,
  revocation: 32163,
};

const hexToBytes = (value: string): Uint8Array => {
  if (!/^[a-f0-9]+$/u.test(value) || value.length % 2 !== 0) {
    throw new Error("invalid lowercase hex");
  }
  return Uint8Array.from(Buffer.from(value, "hex"));
};

type ProjectionBytes = Omit<SignedWorkroomActivity, "nostrEventId" | "signature">;

export interface SignedWorkroomNostrTemplate {
  readonly pubkey: string;
  readonly created_at: number;
  readonly kind: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content: string;
}

export const signedWorkroomNostrTemplate = (
  activity: ProjectionBytes,
): SignedWorkroomNostrTemplate => {
  const occurredAt = Date.parse(activity.occurredAt);
  if (!Number.isFinite(occurredAt)) throw new Error("invalid Workroom occurrence time");
  const tags: Array<ReadonlyArray<string>> = [
    ["d", activity.eventRef],
    ["projection", activity.projectionProfile ?? LEGACY_SIGNED_WORKROOM_NOSTR_PROFILE],
    ["actor", activity.actorRef],
    ["workroom", activity.workroomRef],
    ["audience", activity.audience],
    ["privacy", activity.privacyClass],
    ["revision", String(activity.revision)],
    ["generation", String(activity.generation)],
    ["occurred-at", activity.occurredAt],
    ["payload", activity.payloadDigest],
  ];
  const actorGrantRef = activity.actorGrantRef ?? null;
  const actorGrantGeneration = activity.actorGrantGeneration ?? null;
  if (actorGrantRef !== null && actorGrantGeneration !== null) {
    tags.push(["actor-grant", actorGrantRef]);
    tags.push(["actor-grant-generation", String(actorGrantGeneration)]);
  }
  if (activity.workRef !== null) tags.push(["work", activity.workRef]);
  for (const parentRef of activity.causalParentRefs) tags.push(["parent", parentRef]);
  for (const evidenceRef of activity.evidenceRefs) tags.push(["evidence", evidenceRef]);
  if (activity.supersedesEventRef !== null) {
    tags.push(["supersedes", activity.supersedesEventRef]);
  }
  if (activity.revokesEventRef !== null) tags.push(["revokes", activity.revokesEventRef]);
  return {
    pubkey: activity.signerPubkey,
    created_at: Math.floor(occurredAt / 1_000),
    kind: kinds[activity.kind],
    tags,
    content: "",
  };
};

export const serializeSignedWorkroomNostrEvent = (activity: ProjectionBytes): string => {
  const event = signedWorkroomNostrTemplate(activity);
  return JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content]);
};

export const signedWorkroomNostrEventId = (activity: ProjectionBytes): string =>
  createHash("sha256").update(serializeSignedWorkroomNostrEvent(activity)).digest("hex");

export const verifySignedWorkroomNostrActivity = (
  activity: SignedWorkroomActivity,
): "valid" | "event_id_mismatch" | "signature_invalid" => {
  const expectedEventId = signedWorkroomNostrEventId(activity);
  if (activity.nostrEventId !== expectedEventId) return "event_id_mismatch";
  try {
    return schnorr.verify(
      hexToBytes(activity.signature),
      hexToBytes(activity.nostrEventId),
      hexToBytes(activity.signerPubkey),
    )
      ? "valid"
      : "signature_invalid";
  } catch {
    return "signature_invalid";
  }
};
