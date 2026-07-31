import { Schema as S } from "effect";

import { verifySignedEvent } from "./nostr-identity/crypto.ts";
import {
  SARAH_NOSTR_PRINCIPAL,
  type SarahNostrEventTemplate,
  type SarahNostrSignedEvent,
} from "./nostr-identity/types.ts";

export const SARAH_SIGNING_REQUEST_SCHEMA =
  "openagents.sarah.nostr_signing_request.v1" as const;
export const SARAH_SIGNING_RESPONSE_SCHEMA =
  "openagents.sarah.nostr_signing_response.v1" as const;
export const SARAH_PRESENCE_KIND = 30_382 as const;
export const SARAH_GROUP_TEXT_KIND = 9 as const;

const SafeSlug = S.String.check(S.isPattern(/^[a-z0-9][a-z0-9._-]{0,127}$/u));
const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Digest = S.String.check(S.isPattern(/^[a-f0-9]{64}$/u));
const Timestamp = S.Number.check(S.isInt(), S.isGreaterThan(0));
const Generation = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));

const PresenceTemplate = S.Struct({
  type: S.Literal("presence"),
  createdAt: Timestamp,
  expiresAt: Timestamp,
  groupRef: SafeSlug,
  channelRef: SafeSlug,
  presenceLeaseRef: Ref,
  roomEpochDigest: Digest,
  sessionDigest: Digest,
  generation: Generation,
  membershipRevision: Digest,
  e2eeKeyRevision: Digest,
  admissionDigest: Digest,
  authorityDigest: Digest,
});

const Kind9ProjectionTemplate = S.Struct({
  type: S.Literal("kind9_projection"),
  createdAt: Timestamp,
  groupRef: SafeSlug,
  channelRef: SafeSlug,
  messageRef: Ref,
  presenceLeaseRef: Ref,
  generation: Generation,
  content: S.String.check(S.isMinLength(1), S.isMaxLength(4_096)),
});

export const SarahSignerTemplateSchema = S.Union([
  PresenceTemplate,
  Kind9ProjectionTemplate,
]);
export type SarahSignerTemplate = typeof SarahSignerTemplateSchema.Type;

export const SarahSigningRequestSchema = S.Struct({
  schemaVersion: S.Literal(SARAH_SIGNING_REQUEST_SCHEMA),
  template: SarahSignerTemplateSchema,
});
export type SarahSigningRequest = typeof SarahSigningRequestSchema.Type;

export const SarahSigningResponseSchema = S.Struct({
  schemaVersion: S.Literal(SARAH_SIGNING_RESPONSE_SCHEMA),
  eventId: Digest,
  pubkey: Digest,
  signature: S.String.check(S.isPattern(/^[a-f0-9]{128}$/u)),
});
export type SarahSigningResponse = typeof SarahSigningResponseSchema.Type;

export const buildSarahSigningTemplate = (
  template: SarahSignerTemplate,
): SarahNostrEventTemplate => {
  if (template.type === "presence") {
    return {
      kind: SARAH_PRESENCE_KIND,
      created_at: template.createdAt,
      tags: [
        ["d", template.presenceLeaseRef],
        ["h", template.groupRef],
        ["channel", template.channelRef],
        ["principal", SARAH_NOSTR_PRINCIPAL],
        ["participant", SARAH_NOSTR_PRINCIPAL],
        ["room_epoch", template.roomEpochDigest],
        ["session", template.sessionDigest],
        ["generation", String(template.generation)],
        ["membership", template.membershipRevision],
        ["e2ee_key_revision", template.e2eeKeyRevision],
        ["capability", "community_member_v1"],
        ["admission", template.admissionDigest],
        ["processors", "sarah_openagents_openai_v1"],
        ["expires", String(template.expiresAt)],
        ["alt", "OpenAgents verified Sarah room presence"],
      ],
      content: JSON.stringify({
        schema: "openagents.sarah.livekit-room-authority.v1",
        authority: "presence_only",
        authorityDigest: template.authorityDigest,
      }),
    };
  }
  return {
    kind: SARAH_GROUP_TEXT_KIND,
    created_at: template.createdAt,
    tags: [
      ["h", template.groupRef],
      ["channel", template.channelRef],
      ["message", template.messageRef],
      ["presence", template.presenceLeaseRef],
      ["principal", SARAH_NOSTR_PRINCIPAL],
      ["generation", String(template.generation)],
      ["authority", "projection_only"],
      ["alt", "Sarah text projection; not command, audio, membership, or settlement authority"],
    ],
    content: template.content,
  };
};

export const reconstructSarahSignedEvent = (
  template: SarahSignerTemplate,
  response: SarahSigningResponse,
  expectedPubkey: string,
): SarahNostrSignedEvent => {
  if (response.pubkey !== expectedPubkey) {
    throw new Error("Sarah signer returned an unexpected public key");
  }
  const unsigned = buildSarahSigningTemplate(template);
  const event: SarahNostrSignedEvent = {
    id: response.eventId,
    pubkey: response.pubkey,
    sig: response.signature,
    ...unsigned,
  };
  if (!verifySignedEvent(event)) {
    throw new Error("Sarah signer returned an invalid event");
  }
  return event;
};
